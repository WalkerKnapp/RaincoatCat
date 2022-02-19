mod role;

use serenity::builder::CreateApplicationCommands;
use serenity::model::interactions::application_command::ApplicationCommandInteraction;
use serenity::model::interactions::InteractionResponseType;
use serenity::model::prelude::InteractionApplicationCommandCallbackDataFlags;
use serenity::model::prelude::message_component::MessageComponentInteraction;
use serenity::prelude::*;

pub fn create_commands(commands: &mut CreateApplicationCommands) -> &mut CreateApplicationCommands {
    role::create_command(commands);

    commands
}

pub async fn create_command_response(ctx: Context, command: &ApplicationCommandInteraction) {
    match command.data.name.as_str() {
        "role" => {
            role::create_response(ctx, command).await;
        }
        _ => {
            if let Err(err) = command.create_interaction_response(&ctx.http, |response| {
                response.kind(InteractionResponseType::ChannelMessageWithSource)
                    .interaction_response_data(|message| message
                        .content("Unknown command")
                        .flags(InteractionApplicationCommandCallbackDataFlags::EPHEMERAL))
            }).await {
                println!("Cannot respond to slash command: {}", err);
            }
        }
    };
}

pub async fn create_component_response(ctx: Context, component: &MessageComponentInteraction) {
    match component.data.custom_id.as_str() {
        "role_select" => {
            // TODO: Give roles based on component.data.values
            if let Err(err) = component.create_interaction_response(&ctx.http, |response| {
                response.kind(InteractionResponseType::ChannelMessageWithSource)
                    .interaction_response_data(|message| message.content("Gave role XYZ!")
                        .flags(InteractionApplicationCommandCallbackDataFlags::EPHEMERAL))
            }).await {
                println!("Cannot respond to message component: {}", err);
            }
        }
        _ => {
            if let Err(err) = component.create_interaction_response(&ctx.http, |response| {
                response.kind(InteractionResponseType::ChannelMessageWithSource)
                    .interaction_response_data(|message| message.content("Unknown component"))
            }).await {
                println!("Cannot respond to message component: {}", err);
            }
        }
    }
}
