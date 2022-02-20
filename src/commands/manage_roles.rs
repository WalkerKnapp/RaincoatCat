use sea_orm::ActiveValue::Set;
use sea_orm::{DatabaseConnection, ActiveModelTrait, EntityTrait};
use serenity::builder::{CreateApplicationCommandPermissions, CreateApplicationCommands};
use serenity::model::interactions::application_command::{ApplicationCommandOptionType, ApplicationCommandPermissionType};
use serenity::model::interactions::application_command::ApplicationCommandInteractionDataOptionValue;
use serenity::model::prelude::application_command::{ApplicationCommandInteraction};
use serenity::model::prelude::InteractionResponseType;
use serenity::prelude::Context;
use crate::error::RaincoatError;
use crate::model::optional_role;

pub fn create_command(commands: &mut CreateApplicationCommands) {
    commands.create_application_command(|command| {
        command.name("addrole")
            .description("Set up a role as an optional role, assignable with /role")
            .default_permission(false)
            .create_option(|option| {
                option.name("role")
                    .kind(ApplicationCommandOptionType::Role)
                    .description("The role to set up as an optional role.")
                    .required(true)
            })
            .create_option(|option| {
                option.name("emoji")
                    .kind(ApplicationCommandOptionType::String)
                    .description("An emoji to represent this role.")
                    .required(false)
            })
            .create_option(|option| {
                option.name("description")
                    .kind(ApplicationCommandOptionType::String)
                    .description("A description for this role.")
                    .required(false)
            })
    });

    commands.create_application_command(|command| {
        command.name("removerole")
            .description("Remove a role as an optional role")
            .default_permission(false)
            .create_option(|option| {
                option.name("role")
                    .kind(ApplicationCommandOptionType::Role)
                    .description("The role to remove as an optional role.")
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

pub async fn create_add_response(db: &DatabaseConnection, ctx: &Context, command: &ApplicationCommandInteraction) -> Result<(), RaincoatError> {
    let server_id = command.guild_id.ok_or(RaincoatError { cause: "This command can only be run in servers.".to_string() })?;

    let mut role_id_opt: Option<u64> = None;
    let mut role_name_opt: Option<String> = None;
    let mut emoji_opt: Option<String> = None;
    let mut description_opt: Option<String> = None;

    for option in &command.data.options {
        match option.name.as_str() {
            "role" => {
                if let ApplicationCommandInteractionDataOptionValue::Role(role) = &option.resolved.as_ref()
                    .ok_or(RaincoatError { cause: "Couldn't resolve 'role' param".to_string() })? {
                    role_id_opt = Some(role.id.0);
                    role_name_opt = Some(role.name.clone());
                } else {
                    return Err(RaincoatError { cause: "Unexpected type for 'role' param".to_string() })
                }
            }
            "emoji" => {
                if let ApplicationCommandInteractionDataOptionValue::String(emoji) = &option.resolved.as_ref()
                    .ok_or(RaincoatError { cause: "Couldn't resolve 'emoji' param".to_string() })? {
                    emoji_opt = Some(emoji.clone());
                } else {
                    return Err(RaincoatError { cause: "Unexpected type for 'emoji' param".to_string() })
                }
            }
            "description" => {
                if let ApplicationCommandInteractionDataOptionValue::String(description) = &option.resolved.as_ref()
                    .ok_or(RaincoatError { cause: "Couldn't resolve 'description' param".to_string() })? {
                    description_opt = Some(description.clone());
                } else {
                    return Err(RaincoatError { cause: "Unexpected type for 'description' param".to_string() })
                }
            }
            unknown => {
                return Err(RaincoatError { cause: format!("Unknown param: {}", unknown) });
            }
        }
    }

    let role_id = role_id_opt.ok_or(RaincoatError { cause: "Requires 'role' param".to_string() })?;
    let role_name = role_name_opt.ok_or(RaincoatError { cause: "Requires 'role' param".to_string() })?;

    let new_role = optional_role::ActiveModel {
        role_id: Set(role_id as i64),
        server_id: Set(server_id.0 as i64),
        emoji: Set(emoji_opt),
        description: Set(description_opt)
    };
    if optional_role::Entity::find_by_id(role_id as i64).one(db).await
        .map_err(|err| RaincoatError { cause: format!("{}", err)})?.is_some() {
        // The role is already present, update it
        new_role.update(db).await.map_err(|err| RaincoatError { cause: format!("{}", err) })?;
    } else {
        // The role is not present, insert it
        new_role.insert(db).await.map_err(|err| RaincoatError { cause: format!("{}", err) })?;
    }

    command.create_interaction_response(&ctx.http, |response| {
        response.kind(InteractionResponseType::ChannelMessageWithSource)
            .interaction_response_data(|message| {
                message.content(format!("Successfully configured `{}` as an optional role.", role_name))
                    .allowed_mentions(|f| f.empty_parse())
            })
    }).await.map_err(|err| RaincoatError { cause: format!("Failed to send interaction response: {}", err) })
}

pub async fn create_remove_response(db: &DatabaseConnection, ctx: &Context, command: &ApplicationCommandInteraction) -> Result<(), RaincoatError> {
    let mut role_id_opt: Option<u64> = None;
    let mut role_name_opt: Option<String> = None;

    for option in &command.data.options {
        match option.name.as_str() {
            "role" => {
                if let ApplicationCommandInteractionDataOptionValue::Role(role) = &option.resolved.as_ref()
                    .ok_or(RaincoatError { cause: "Couldn't resolve 'role' param".to_string() })? {
                    role_id_opt = Some(role.id.0);
                    role_name_opt = Some(role.name.clone());
                } else {
                    return Err(RaincoatError { cause: "Unexpected type for 'role' param".to_string() })
                }
            }
            unknown => {
                return Err(RaincoatError { cause: format!("Unknown param: {}", unknown) });
            }
        }
    }

    let role_id = role_id_opt.ok_or(RaincoatError { cause: "Requires 'role' param".to_string() })?;
    let role_name = role_name_opt.ok_or(RaincoatError { cause: "Requires 'role' param".to_string() })?;

    let old_role = optional_role::ActiveModel {
        role_id: Set(role_id as i64),
        ..Default::default()
    };
    optional_role::Entity::delete(old_role).exec(db).await.map_err(|err| RaincoatError { cause: format!("{}", err) })?;

    command.create_interaction_response(&ctx.http, |response| {
        response.kind(InteractionResponseType::ChannelMessageWithSource)
            .interaction_response_data(|message| {
                message.content(format!("Successfully removed `{}` as an optional role.", role_name))
                    .allowed_mentions(|f| f.empty_parse())
            })
    }).await.map_err(|err| RaincoatError { cause: format!("Failed to send interaction response: {}", err) })
}
