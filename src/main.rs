mod config;
mod commands;

use serenity::async_trait;
use serenity::model::gateway::Ready;
use serenity::model::id::GuildId;
use serenity::model::interactions::Interaction;
use serenity::prelude::*;

struct RaincoatCatEventHandler;

#[async_trait]
impl EventHandler for RaincoatCatEventHandler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("Connected to discord as {}#{}", ready.user.name, ready.user.discriminator);

        let guild_id = GuildId(299658323500990464);

        let commands = GuildId::set_application_commands(&guild_id, &ctx.http, commands::create_commands).await;

        println!("Created guild slash commands: {:#?}", commands);
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        println!("Processing Interaction: {:#?}", interaction);
        match &interaction {
            Interaction::ApplicationCommand(command) => {
                commands::create_command_response(ctx, command).await;
            }
            Interaction::MessageComponent(component) => {
                commands::create_component_response(ctx, component).await;
            }
            _ => {
                eprintln!("Unexpected interaction type: {:?}", &interaction.kind());
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let config = config::load_config();

    let mut client = Client::builder(config.discord_bot_token.as_str())
        .event_handler(RaincoatCatEventHandler)
        .application_id(config.discord_application_id)
        .await
        .expect("Failed to create discord client");

    if let Err(err) = client.start().await {
        println!("Failed to start discord client: {:?}", err);
    }
}
