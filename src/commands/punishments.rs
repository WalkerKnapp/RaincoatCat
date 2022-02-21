use chrono::{Duration, Utc};
use sea_orm::ActiveValue::Set;
use sea_orm::{DatabaseConnection, EntityTrait, ActiveModelTrait, QueryFilter, ColumnTrait, ModelTrait};
use serenity::builder::{CreateApplicationCommand, CreateApplicationCommandPermissions, CreateApplicationCommands};
use serenity::model::interactions::application_command::{ApplicationCommandInteraction, ApplicationCommandInteractionDataOption, ApplicationCommandInteractionDataOptionValue, ApplicationCommandOptionType, ApplicationCommandPermissionType};
use serenity::model::interactions::InteractionResponseType;
use serenity::prelude::{Context, Mentionable};
use crate::error::RaincoatError;
use crate::model::punishment;
use crate::model::punishment::PunishmentType;
use crate::model::punishment_removed_role;
use crate::model::server;

fn parse_integer_option(name: &str, option: &ApplicationCommandInteractionDataOption) -> Result<i64, RaincoatError> {
    if let ApplicationCommandInteractionDataOptionValue::Integer(value) = &option.resolved.as_ref()
        .ok_or(RaincoatError { cause: format!("Couldn't resolve '{}' param", name) })? {
        Ok(*value)
    } else {
        return Err(RaincoatError { cause: format!("Unexpected type for '{}' param", name) })
    }
}

fn duration_add_options(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    command.create_option(|option| {
        option.name("years")
            .description("Years (cumulative)")
            .kind(ApplicationCommandOptionType::Integer)
    })
        .create_option(|option| {
            option.name("months")
                .description("Months (cumulative)")
                .kind(ApplicationCommandOptionType::Integer)
        })
        .create_option(|option| {
            option.name("weeks")
                .description("Weeks (cumulative)")
                .kind(ApplicationCommandOptionType::Integer)
        })
        .create_option(|option| {
            option.name("days")
                .description("Days (cumulative)")
                .kind(ApplicationCommandOptionType::Integer)
        })
        .create_option(|option| {
            option.name("hours")
                .description("Hours (cumulative)")
                .kind(ApplicationCommandOptionType::Integer)
        })
        .create_option(|option| {
            option.name("minutes")
                .description("Minutes (cumulative)")
                .kind(ApplicationCommandOptionType::Integer)
        })
}

fn duration_parse(duration_accumulator: &mut Duration, name: &str, option: &ApplicationCommandInteractionDataOption) -> Result<(), RaincoatError> {
    match name {
        "years" => {
            *duration_accumulator = *duration_accumulator + Duration::weeks(4 * 12 * parse_integer_option(name, option)?);
        }
        "months" => {
            *duration_accumulator = *duration_accumulator + Duration::weeks(4 * parse_integer_option(name, option)?);
        }
        "weeks" => {
            *duration_accumulator = *duration_accumulator + Duration::weeks(parse_integer_option(name, option)?);
        }
        "days" => {
            *duration_accumulator = *duration_accumulator + Duration::days(parse_integer_option(name, option)?);
        }
        "hours" => {
            *duration_accumulator = *duration_accumulator + Duration::hours(parse_integer_option(name, option)?);
        }
        "minutes" => {
            *duration_accumulator = *duration_accumulator + Duration::minutes(parse_integer_option(name, option)?);
        }
        unknown => {
            return Err(RaincoatError { cause: format!("Unknown param: {}", unknown) });
        }
    };

    Ok(())
}

pub fn create_command(commands: &mut CreateApplicationCommands) {
    commands.create_application_command(|command| {
        command.name("dunce")
            .description("Dunce a user for some amount of time (or indefinitely)")
            .default_permission(false)
            .create_option(|option| {
                option.name("user")
                    .kind(ApplicationCommandOptionType::User)
                    .description("The user to dunce")
                    .required(true)
            });
        duration_add_options(command)
    });

    commands.create_application_command(|command| {
        command.name("undunce")
            .description("Undunces a user")
            .default_permission(false)
            .create_option(|option| {
                option.name("user")
                    .kind(ApplicationCommandOptionType::User)
                    .description("The user to undunce")
                    .required(true)
            })
    });

    commands.create_application_command(|command| {
        command.name("ban")
            .description("Ban a user for some amount of time (or indefinitely)")
            .default_permission(false)
            .create_option(|option| {
                option.name("user")
                    .kind(ApplicationCommandOptionType::User)
                    .description("The user to ban")
                    .required(true)
            });
        duration_add_options(command)
    });

    commands.create_application_command(|command| {
        command.name("unban")
            .description("Unbans a user")
            .default_permission(false)
            .create_option(|option| {
                option.name("user")
                    .kind(ApplicationCommandOptionType::User)
                    .description("The user to unban")
                    .required(true)
            })
    });
}

pub fn create_permissions(mod_role: u64, updater: &mut CreateApplicationCommandPermissions) -> &mut CreateApplicationCommandPermissions {
    updater.create_permissions(|permissions| {
        permissions.kind(ApplicationCommandPermissionType::Role)
            .id(mod_role)
            .permission(true)
    })
}

pub async fn create_dunce_response(db: &DatabaseConnection, ctx: &Context, command: &ApplicationCommandInteraction) -> Result<(), RaincoatError> {
    let server_id = command.guild_id.ok_or(RaincoatError { cause: "This command can only be run in servers.".to_string() })?;
    let server_model: server::Model = server::Entity::find_by_id(server_id.0 as i64).one(db).await
        .map_err(|err| RaincoatError { cause: format!("{}", err) })?
        .ok_or(RaincoatError { cause: "This command can only be run in servers.".to_string() })?;
    let dunce_role_id = server_model.dunce_role_id
        .ok_or(RaincoatError { cause: "No dunce role has been configured for this server.".to_string() })? as u64;

    let mut user_id_opt: Option<u64> = None;
    let mut time_accumulator: Duration = Duration::zero();

    for option in &command.data.options {
        match option.name.as_str() {
            "user" => {
                if let ApplicationCommandInteractionDataOptionValue::User(user, _member) = &option.resolved.as_ref()
                    .ok_or(RaincoatError { cause: "Couldn't resolve 'user' param".to_string() })? {
                    user_id_opt = Some(user.id.0);
                } else {
                    return Err(RaincoatError { cause: "Unexpected type for 'user' param".to_string() })
                }
            }
            other => duration_parse(&mut time_accumulator, other, option)?
        }
    };

    let user_id = user_id_opt.ok_or(RaincoatError { cause: "Requires 'user' param".to_string() })?;
    let punishment_expires = if time_accumulator == Duration::zero() {
        None
    } else {
        Some((Utc::now() + time_accumulator).naive_utc())
    };

    let new_punishment = punishment::ActiveModel {
        user_id: Set(user_id as i64),
        server_id: Set(server_id.0 as i64),
        punishment_type: Set(PunishmentType::Dunce),
        expires: Set(punishment_expires),
        ..Default::default()
    };
    let punishment_model: punishment::Model = new_punishment.insert(db)
        .await.map_err(|err| RaincoatError { cause: format!("{}", err) })?;

    let mut member = ctx.cache.member(server_id, user_id).await
        .ok_or(RaincoatError { cause: format!("Unable to fetch information about user") })?;
    let roles = member.roles.clone();

    for role_id in &roles {
        let new_punishment_removed_role = punishment_removed_role::ActiveModel {
            punishment_id: Set(punishment_model.id),
            role_id: Set(role_id.0 as i64),
            ..Default::default()
        };
        new_punishment_removed_role.insert(db)
            .await.map_err(|err| RaincoatError { cause: format!("{}", err) })?;
    }

    member.remove_roles(&ctx.http, roles.as_slice()).await
        .map_err(|err| RaincoatError { cause: format!("Couldn't remove roles: {}", err) })?;
    member.add_role(&ctx.http, dunce_role_id).await
        .map_err(|err| RaincoatError { cause: format!("Couldn't add dunce role: {}", err)})?;

    command.create_interaction_response(&ctx.http, |response| {
        response.kind(InteractionResponseType::ChannelMessageWithSource)
            .interaction_response_data(|message| {
                match punishment_expires {
                    Some(expires) => {
                        message.content(format!("Dunced {} until <t:{}>", member.mention().to_string(), expires.timestamp()))
                            .allowed_mentions(|f| f.empty_parse())
                    }
                    None => {
                        message.content(format!("Dunced {} indefinitely", member.mention().to_string()))
                            .allowed_mentions(|f| f.empty_parse())
                    }
                }
            })
    }).await.map_err(|err| RaincoatError { cause: format!("Failed to send interaction response: {}", err) })
}

pub async fn create_undunce_response(db: &DatabaseConnection, ctx: &Context, command: &ApplicationCommandInteraction) -> Result<(), RaincoatError> {
    let server_id = command.guild_id.ok_or(RaincoatError { cause: "This command can only be run in servers.".to_string() })?;
    let server_model: server::Model = server::Entity::find_by_id(server_id.0 as i64).one(db).await
        .map_err(|err| RaincoatError { cause: format!("{}", err) })?
        .ok_or(RaincoatError { cause: "This command can only be run in servers.".to_string() })?;
    let dunce_role_id = server_model.dunce_role_id
        .ok_or(RaincoatError { cause: "No dunce role has been configured for this server.".to_string() })? as u64;

    let mut user_id_opt: Option<u64> = None;

    for option in &command.data.options {
        match option.name.as_str() {
            "user" => {
                if let ApplicationCommandInteractionDataOptionValue::User(user, _member) = &option.resolved.as_ref()
                    .ok_or(RaincoatError { cause: "Couldn't resolve 'user' param".to_string() })? {
                    user_id_opt = Some(user.id.0);
                } else {
                    return Err(RaincoatError { cause: "Unexpected type for 'user' param".to_string() })
                }
            }
            unknown => {
                return Err(RaincoatError { cause: format!("Unknown param: {}", unknown)})
            }
        }
    }

    let user_id = user_id_opt.ok_or(RaincoatError { cause: "Requires 'user' param".to_string() })?;

    let user_dunces: Vec<(punishment::Model, Vec<punishment_removed_role::Model>)> = punishment::Entity::find()
        .filter(punishment::Column::PunishmentType.eq(PunishmentType::Dunce))
        .filter(punishment::Column::UserId.eq(user_id as i64))
        .filter(punishment::Column::ServerId.eq(server_id.0 as i64))
        .find_with_related(punishment_removed_role::Entity)
        .all(db).await.map_err(|err| RaincoatError { cause: format!("{}", err) })?;

    if user_dunces.len() > 0 {
        for (dunce, roles) in user_dunces {
            dunce.delete(db).await
                .map_err(|err| RaincoatError { cause: format!("{}", err) })?;

            for role in roles {
                ctx.http.add_member_role(server_id.0, user_id, role.role_id as u64).await
                    .map_err(|err| RaincoatError { cause: format!("Unable to return user role: {}", err) })?;
                role.delete(db).await
                    .map_err(|err| RaincoatError { cause: format!("{}", err) })?;
            }
        };

        ctx.http.remove_member_role(server_id.0, user_id, dunce_role_id).await
            .map_err(|err| RaincoatError { cause: format!("Unable to remove dunce role: {}", err) })?;

        command.create_interaction_response(&ctx.http, |response| {
            response.kind(InteractionResponseType::ChannelMessageWithSource)
                .interaction_response_data(|message| {
                    message.content(format!("Undunced <@{}>", user_id))
                        .allowed_mentions(|f| f.empty_parse())
                })
        }).await.map_err(|err| RaincoatError { cause: format!("Failed to send interaction response: {}", err) })
    } else {
        command.create_interaction_response(&ctx.http, |response| {
            response.kind(InteractionResponseType::ChannelMessageWithSource)
                .interaction_response_data(|message| {
                    message.content(format!("User <@{}> is not dunced on this server", user_id))
                        .allowed_mentions(|f| f.empty_parse())
                })
        }).await.map_err(|err| RaincoatError { cause: format!("Failed to send interaction response: {}", err) })
    }
}

pub async fn create_ban_response(db: &DatabaseConnection, ctx: &Context, command: &ApplicationCommandInteraction) -> Result<(), RaincoatError> {
    let server_id = command.guild_id.ok_or(RaincoatError { cause: "This command can only be run in servers.".to_string() })?;

    let mut user_id_opt: Option<u64> = None;
    let mut time_accumulator: Duration = Duration::zero();

    for option in &command.data.options {
        match option.name.as_str() {
            "user" => {
                if let ApplicationCommandInteractionDataOptionValue::User(user, _member) = &option.resolved.as_ref()
                    .ok_or(RaincoatError { cause: "Couldn't resolve 'user' param".to_string() })? {
                    user_id_opt = Some(user.id.0);
                } else {
                    return Err(RaincoatError { cause: "Unexpected type for 'user' param".to_string() })
                }
            }
            other => duration_parse(&mut time_accumulator, other, option)?
        }
    };

    let user_id = user_id_opt.ok_or(RaincoatError { cause: "Requires 'user' param".to_string() })?;
    let punishment_expires = if time_accumulator == Duration::zero() {
        None
    } else {
        Some((Utc::now() + time_accumulator).naive_utc())
    };

    let new_punishment = punishment::ActiveModel {
        user_id: Set(user_id as i64),
        server_id: Set(server_id.0 as i64),
        punishment_type: Set(PunishmentType::Ban),
        expires: Set(punishment_expires),
        ..Default::default()
    };
    let punishment_model: punishment::Model = new_punishment.insert(db)
        .await.map_err(|err| RaincoatError { cause: format!("{}", err) })?;

    let member = ctx.cache.member(server_id, user_id).await
        .ok_or(RaincoatError { cause: format!("Unable to fetch information about user") })?;
    let roles = &member.roles;

    for role_id in roles {
        let new_punishment_removed_role = punishment_removed_role::ActiveModel {
            punishment_id: Set(punishment_model.id),
            role_id: Set(role_id.0 as i64),
            ..Default::default()
        };
        new_punishment_removed_role.insert(db)
            .await.map_err(|err| RaincoatError { cause: format!("{}", err) })?;
    }

    member.ban(&ctx.http, 0).await
        .map_err(|err| RaincoatError { cause: format!("Unable to ban user: {}", err) })?;

    command.create_interaction_response(&ctx.http, |response| {
        response.kind(InteractionResponseType::ChannelMessageWithSource)
            .interaction_response_data(|message| {
                match punishment_expires {
                    Some(expires) => {
                        message.content(format!("Banned {} until <t:{}>", member.mention().to_string(), expires.timestamp()))
                            .allowed_mentions(|f| f.empty_parse())
                    }
                    None => {
                        message.content(format!("Banned {} indefinitely", member.mention().to_string()))
                            .allowed_mentions(|f| f.empty_parse())
                    }
                }
            })
    }).await.map_err(|err| RaincoatError { cause: format!("Failed to send interaction response: {}", err) })
}

pub async fn create_unban_response(db: &DatabaseConnection, ctx: &Context, command: &ApplicationCommandInteraction) -> Result<(), RaincoatError> {
    let server_id = command.guild_id.ok_or(RaincoatError { cause: "This command can only be run in servers.".to_string() })?;

    let mut user_id_opt: Option<u64> = None;

    for option in &command.data.options {
        match option.name.as_str() {
            "user" => {
                if let ApplicationCommandInteractionDataOptionValue::User(user, _member) = &option.resolved.as_ref()
                    .ok_or(RaincoatError { cause: "Couldn't resolve 'user' param".to_string() })? {
                    user_id_opt = Some(user.id.0);
                } else {
                    return Err(RaincoatError { cause: "Unexpected type for 'user' param".to_string() })
                }
            }
            unknown => {
                return Err(RaincoatError { cause: format!("Unknown param: {}", unknown)})
            }
        }
    }

    let user_id = user_id_opt.ok_or(RaincoatError { cause: "Requires 'user' param".to_string() })?;

    let user_bans: Vec<(punishment::Model, Vec<punishment_removed_role::Model>)> = punishment::Entity::find()
        .filter(punishment::Column::PunishmentType.eq(PunishmentType::Ban))
        .filter(punishment::Column::UserId.eq(user_id as i64))
        .filter(punishment::Column::ServerId.eq(server_id.0 as i64))
        .find_with_related(punishment_removed_role::Entity)
        .all(db).await.map_err(|err| RaincoatError { cause: format!("{}", err) })?;

    if user_bans.len() > 0 {
        for (ban, roles) in user_bans {
            ban.delete(db).await
                .map_err(|err| RaincoatError { cause: format!("{}", err) })?;

            for role in roles {
                // TODO: figure out some way to automatically readd roles?
                role.delete(db).await
                    .map_err(|err| RaincoatError { cause: format!("{}", err) })?;
            }
        };

        ctx.http.remove_ban(server_id.0, user_id).await
            .map_err(|err| RaincoatError { cause: format!("Unable to unban user: {}", err) })?;

        command.create_interaction_response(&ctx.http, |response| {
            response.kind(InteractionResponseType::ChannelMessageWithSource)
                .interaction_response_data(|message| {
                    message.content(format!("Unbanned <@{}>", user_id))
                        .allowed_mentions(|f| f.empty_parse())
                })
        }).await.map_err(|err| RaincoatError { cause: format!("Failed to send interaction response: {}", err) })
    } else {
        command.create_interaction_response(&ctx.http, |response| {
            response.kind(InteractionResponseType::ChannelMessageWithSource)
                .interaction_response_data(|message| {
                    message.content(format!("User <@{}> is not banned on this server", user_id))
                        .allowed_mentions(|f| f.empty_parse())
                })
        }).await.map_err(|err| RaincoatError { cause: format!("Failed to send interaction response: {}", err) })
    }
}
