mod role;
mod manage_roles;

use sea_orm::DatabaseConnection;
use serenity::builder::{CreateApplicationCommands, CreateApplicationCommandsPermissions};
use serenity::model::interactions::application_command::{ApplicationCommand, ApplicationCommandInteraction};
use serenity::model::interactions::InteractionResponseType;
use serenity::model::prelude::InteractionApplicationCommandCallbackDataFlags;
use serenity::model::prelude::message_component::MessageComponentInteraction;
use serenity::prelude::*;
use crate::error::RaincoatError;

pub fn create_commands(commands: &mut CreateApplicationCommands) -> &mut CreateApplicationCommands {
    role::create_command(commands);
    manage_roles::create_command(commands);

    commands
}

pub fn set_command_permissions<'a>(mod_role: u64, updater: &'a mut CreateApplicationCommandsPermissions, commands: &Vec<ApplicationCommand>) -> &'a mut CreateApplicationCommandsPermissions {
    for command in commands {
        match command.name.as_str() {
            "addrole" => {
                updater.create_application_command(|c| {
                    c.id(command.id.0);
                    manage_roles::create_permissions(mod_role, c)
                });
            }
            "removerole" => {
                updater.create_application_command(|c| {
                    c.id(command.id.0);
                    manage_roles::create_permissions(mod_role, c)
                });
            }
            _ => {}
        }
    }
    updater
}

pub async fn create_command_response(db: &DatabaseConnection, ctx: &Context, command: &ApplicationCommandInteraction) -> Result<(), RaincoatError> {
    match command.data.name.as_str() {
        "role" => {
            role::create_response(db, ctx, command).await
        }
        "addrole" => {
            manage_roles::create_add_response(db, ctx, command).await
        }
        "removerole" => {
            manage_roles::create_remove_response(db, ctx, command).await
        }
        _ => {
            command.create_interaction_response(&ctx.http, |response| {
                response.kind(InteractionResponseType::ChannelMessageWithSource)
                    .interaction_response_data(|message| message
                        .content("Unknown command")
                        .flags(InteractionApplicationCommandCallbackDataFlags::EPHEMERAL))
            }).await.map_err(|err| RaincoatError { cause: format!("Failed to respond to command: {}", err) })
        }
    }
}

pub async fn create_component_response(db: &DatabaseConnection, ctx: &Context, component: &MessageComponentInteraction) -> Result<(), RaincoatError> {
    match component.data.custom_id.as_str() {
        "role_select" => {
            role::create_component_response(db, ctx, component).await
        }
        _ => {
            component.create_interaction_response(&ctx.http, |response| {
                response.kind(InteractionResponseType::ChannelMessageWithSource)
                    .interaction_response_data(|message| message.content("Unknown component"))
            }).await.map_err(|err| RaincoatError { cause: format!("Failed to respond to component: {}", err) })
        }
    }
}
