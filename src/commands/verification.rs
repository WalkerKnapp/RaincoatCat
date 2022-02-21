use sea_orm::ActiveValue::Set;
use sea_orm::{DatabaseConnection, ActiveModelTrait};
use serenity::builder::{CreateApplicationCommandPermissions, CreateApplicationCommands};
use serenity::model::interactions::application_command::{ApplicationCommandInteractionDataOptionValue, ApplicationCommandOptionType};
use serenity::model::interactions::InteractionResponseType;
use serenity::model::prelude::application_command::{ApplicationCommandInteraction, ApplicationCommandPermissionType};
use serenity::prelude::Context;
use crate::error::RaincoatError;
use crate::model::server;

pub fn create_command(commands: &mut CreateApplicationCommands) {
    commands.create_application_command(|command| {
        command.name("verification")
            .description("Configure verification for this server.")
            .default_permission(false)
            .create_option(|option| {
                option.name("enable")
                    .description("Enable/configure verification on this server")
                    .kind(ApplicationCommandOptionType::SubCommand)
                    .create_sub_option(|suboption| {
                        suboption.name("role")
                            .description("The role to give users who verify")
                            .kind(ApplicationCommandOptionType::Role)
                            .required(true)
                    })
                    .create_sub_option(|suboption| {
                        suboption.name("message")
                            .description("The message ID users should react to")
                            .kind(ApplicationCommandOptionType::String)
                            .required(true)
                    })
                    .create_sub_option(|suboption| {
                        suboption.name("emoji")
                            .description("The emoji users should react with to verify")
                            .kind(ApplicationCommandOptionType::String)
                            .required(true)
                    })
                    .create_sub_option(|suboption| {
                        suboption.name("timeout")
                            .description("The hours to wait before kicking users who do not verify")
                            .kind(ApplicationCommandOptionType::Integer)
                            .required(false)
                    })
            })
            .create_option(|option| {
                option.name("disable")
                    .description("Disable verification on this server")
                    .kind(ApplicationCommandOptionType::SubCommand)
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

pub async fn create_response(db: &DatabaseConnection, ctx: &Context, command: &ApplicationCommandInteraction) -> Result<(), RaincoatError> {
    let server_id = command.guild_id.ok_or(RaincoatError { cause: "This command can only be run in servers.".to_string() })?;

    let subcommand = command.data.options.get(0).ok_or(RaincoatError { cause: "Command target is required.".to_string() })?;

    match subcommand.name.as_str() {
        "enable" => {
            let mut role_id_opt: Option<u64> = None;
            let mut message_id_opt: Option<u64> = None;
            let mut emoji_opt: Option<String> = None;
            let mut timeout_opt: Option<i64> = None;

            for option in &subcommand.options {
                match option.name.as_str() {
                    "role" => {
                        if let ApplicationCommandInteractionDataOptionValue::Role(role) = &option.resolved.as_ref()
                            .ok_or(RaincoatError { cause: "Couldn't resolve 'role' param".to_string() })? {
                            role_id_opt = Some(role.id.0);
                        } else {
                            return Err(RaincoatError { cause: "Unexpected type for 'role' param".to_string() });
                        }
                    }
                    "message" => {
                        if let ApplicationCommandInteractionDataOptionValue::String(message_id_str) = &option.resolved.as_ref()
                            .ok_or(RaincoatError { cause: "Couldn't resolve 'message' param".to_string() })? {
                            message_id_opt = Some(message_id_str.parse()
                                .map_err(|_err| RaincoatError { cause: format!("Couldn't parse {} as message id", message_id_str) })?);
                        } else {
                            return Err(RaincoatError { cause: "Unexpected type for 'message' param".to_string() });
                        }
                    }
                    "emoji" => {
                        if let ApplicationCommandInteractionDataOptionValue::String(emoji) = &option.resolved.as_ref()
                            .ok_or(RaincoatError { cause: "Couldn't resolve 'emoji' param".to_string() })? {
                            emoji_opt = Some(emoji.clone());
                        } else {
                            return Err(RaincoatError { cause: "Unexpected type for 'emoji' param".to_string() });
                        }
                    }
                    "timeout" => {
                        if let ApplicationCommandInteractionDataOptionValue::Integer(timeout) = &option.resolved.as_ref()
                            .ok_or(RaincoatError { cause: "Couldn't resolve 'timeout' param".to_string() })? {
                            timeout_opt = Some(*timeout);
                        } else {
                            return Err(RaincoatError { cause: "Unexpected type for 'timeout' param".to_string() });
                        }
                    }
                    unknown => return Err(RaincoatError { cause: format!("Unknown parameter: {}", unknown) })
                }
            }

            let role_id = role_id_opt.ok_or(RaincoatError { cause: "Requires 'role' param".to_string() })?;
            let message_id = message_id_opt.ok_or(RaincoatError { cause: "Requires 'message' param".to_string() })?;
            let emoji = emoji_opt.ok_or( RaincoatError { cause: "Requires 'emoji' param".to_string() })?;

            let new_server = server::ActiveModel {
                id: Set(server_id.0 as i64),
                verified_role_id: Set(Some(role_id as i64)),
                verification_message_id: Set(Some(message_id as i64)),
                verification_emoji: Set(Some(emoji)),
                verification_timeout: Set(timeout_opt),
                ..Default::default()
            };
            new_server.update(db).await.map_err(|err| RaincoatError { cause: format!("{}", err) })?;

            command.create_interaction_response(&ctx.http, |response| {
                response.kind(InteractionResponseType::ChannelMessageWithSource)
                    .interaction_response_data(|message| {
                        message.content(format!("Successfully configured verification."))
                    })
            }).await.map_err(|err| RaincoatError { cause: format!("Failed to send interaction response: {}", err) })
        }
        "disable" => {
            let new_server = server::ActiveModel {
                id: Set(server_id.0 as i64),
                verified_role_id: Set(None),
                verification_message_id: Set(None),
                verification_emoji: Set(None),
                verification_timeout: Set(None),
                ..Default::default()
            };
            new_server.update(db).await.map_err(|err| RaincoatError { cause: format!("{}", err) })?;

            command.create_interaction_response(&ctx.http, |response| {
                response.kind(InteractionResponseType::ChannelMessageWithSource)
                    .interaction_response_data(|message|  {
                        message.content(format!("Successfully disabled verification."))
                    })
            }).await.map_err(|err| RaincoatError { cause: format!("Failed to send interaction response: {}", err) })
        }
        unknown => return Err(RaincoatError { cause: format!("Unknown subcommand: {}", unknown) })
    }
}
