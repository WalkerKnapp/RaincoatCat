mod role;

use sea_orm::DatabaseConnection;
use serenity::builder::CreateApplicationCommands;
use serenity::model::interactions::application_command::ApplicationCommandInteraction;
use serenity::model::interactions::InteractionResponseType;
use serenity::model::prelude::InteractionApplicationCommandCallbackDataFlags;
use serenity::model::prelude::message_component::MessageComponentInteraction;
use serenity::prelude::*;
use crate::error::RaincoatError;

pub fn create_commands(commands: &mut CreateApplicationCommands) -> &mut CreateApplicationCommands {
    role::create_command(commands);

    commands
}

pub async fn create_command_response(db: &DatabaseConnection, ctx: &Context, command: &ApplicationCommandInteraction) -> Result<(), RaincoatError> {
    match command.data.name.as_str() {
        "role" => {
            role::create_response(db, ctx, command).await
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
