mod config;
mod commands;
mod model;
mod error;

use sqlx::postgres::PgPoolOptions;
use sea_orm::{DatabaseConnection, SqlxPostgresConnector};

use serenity::async_trait;
use serenity::model::gateway::Ready;
use serenity::model::id::GuildId;
use serenity::model::interactions::{Interaction, InteractionResponseType};
use serenity::model::prelude::InteractionApplicationCommandCallbackDataFlags;
use serenity::prelude::*;

struct RaincoatCatEventHandler {
    db: DatabaseConnection
}

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
                if let Err(err) = commands::create_command_response(&self.db, &ctx, command).await {
                    eprintln!("Encountered error while processing application command: {}", err);
                    if let Err(err) = command.create_interaction_response(&ctx.http, |r| r
                        .kind(InteractionResponseType::ChannelMessageWithSource)
                        .interaction_response_data(|m| m.content(format!("Couldn't process request: {}", err.cause))
                            .flags(InteractionApplicationCommandCallbackDataFlags::EPHEMERAL))).await {
                        eprintln!("Encountered error while sending error message: {}", err);
                    }
                }
            }
            Interaction::MessageComponent(component) => {
                if let Err(err) = commands::create_component_response(&self.db, &ctx, component).await {
                    eprintln!("Encountered error while processing message component: {}", err);
                    if let Err(err) = component.create_interaction_response(&ctx.http, |r| r
                        .kind(InteractionResponseType::ChannelMessageWithSource)
                        .interaction_response_data(|m| m.content(format!("Couldn't process request: {}", err.cause))
                            .flags(InteractionApplicationCommandCallbackDataFlags::EPHEMERAL))).await {
                        eprintln!("Encountered error while sending error message: {}", err);
                    }
                }
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

    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_test_writer()
        .init();

    let pool = PgPoolOptions::new()
        .max_connections(32)
        .connect(format!("postgres://raincoat:{}@localhost/raincoat", config.postgres_password).as_str()).await
        .expect("Unable to open DB connection");
    sqlx::migrate!().run(&pool).await
        .expect("Unable to migrate DB");

    let db: DatabaseConnection = SqlxPostgresConnector::from_sqlx_postgres_pool(pool);

    let mut client = Client::builder(config.discord_bot_token.as_str())
        .event_handler(RaincoatCatEventHandler { db })
        .application_id(config.discord_application_id)
        .await
        .expect("Failed to create discord client");

    if let Err(err) = client.start().await {
        println!("Failed to start discord client: {:?}", err);
    }
}
