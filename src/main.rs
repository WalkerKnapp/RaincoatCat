mod config;
mod commands;
mod model;
mod error;

use std::sync::Arc;
use chrono::{Utc, Duration};
use sqlx::postgres::PgPoolOptions;
use sea_orm::{DatabaseConnection, SqlxPostgresConnector, EntityTrait};

use serenity::async_trait;
use serenity::cache::Cache;
use serenity::client::bridge::gateway::GatewayIntents;
use serenity::http::Http;
use serenity::model::channel::{Reaction, ReactionType};
use serenity::model::gateway::Ready;
use serenity::model::id::{GuildId, RoleId};
use serenity::model::interactions::{Interaction, InteractionResponseType};
use serenity::model::prelude::InteractionApplicationCommandCallbackDataFlags;
use serenity::prelude::*;

use crate::model::server;

struct RaincoatCatEventHandler {
    db: Arc<DatabaseConnection>
}

impl RaincoatCatEventHandler {
    async fn kick_listener(db: Arc<DatabaseConnection>, cache: Arc<Cache>, http: Arc<Http>) {
        let mut interval_timer = tokio::time::interval(std::time::Duration::from_secs(5));
        loop {
            interval_timer.tick().await;

            for server_id in cache.guilds().await {
                let server = cache.guild(server_id).await.unwrap();
                if let Some(server_model) = server::Entity::find_by_id(server_id.0 as i64).one(db.as_ref()).await
                    .expect("DB lookup failed") {

                    if let (Some(verification_timeout), Some(verified_role_id))
                        = (server_model.verification_timeout, server_model.verified_role_id) {
                        // Iterate through users and determine if any should be kicked due to the verification timeout
                        for (user_id, member) in &server.members {
                            if !member.roles.contains(&RoleId(verified_role_id as u64)) && !member.user.bot {
                                if let Some(joined_at) = member.joined_at {
                                    if joined_at < Utc::now() - Duration::hours(verification_timeout) {
                                        println!("Kicking user {} from server {} for failing to verify within {} hours.", user_id.0, server.name, verification_timeout);
                                        if let Err(err) = server.kick(&http, user_id).await {
                                            eprintln!("Failed to kick user {} from server {}: {}", user_id.0, server.name, err);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[async_trait]
impl EventHandler for RaincoatCatEventHandler {
    async fn cache_ready(&self, ctx: Context, servers: Vec<GuildId>) {
        let guild_id = GuildId(299658323500990464);

        let cmds = GuildId::set_application_commands(&guild_id, &ctx.http, commands::create_commands).await
            .expect("Failed to create application commands");

        // Set initial command permissions for every server we are in
        for server in &servers {
            // Get the mod role for this server, if present
            if let Some(server_model) = server::Entity::find_by_id(server.0 as i64).one(self.db.as_ref()).await
                .expect("DB lookup failed") {

                server.set_application_commands_permissions(&ctx.http, |f| {
                    commands::set_command_permissions(server_model.mod_role_id as u64, f, &cmds)
                }).await.expect(format!("Could not set application permissions for server {}", server.0).as_str());
            }
        }

        tokio::spawn(Self::kick_listener(Arc::clone(&self.db), ctx.cache, ctx.http));
    }

    async fn reaction_add(&self, ctx: Context, added_reaction: Reaction) {
        let server_id = match added_reaction.guild_id {
            Some(id) => id,
            None => return
        };
        let user_id = match added_reaction.user_id {
            Some(id) => id,
            None => return
        };

        if let Some(server) = server::Entity::find_by_id(server_id.0 as i64).one(self.db.as_ref()).await
            .expect("DB lookup failed") {
            if let (Some(verified_role_id), Some(verification_message_id), Some(verification_emoji))
                    = (server.verified_role_id, server.verification_message_id, server.verification_emoji) {
                // This server has verification set up, check if this reaction is a verify attempt
                let expected_verification_reaction: ReactionType = match verification_emoji.clone().try_into() {
                    Ok(r) => r,
                    Err(_) => {
                        eprintln!("Server {} has invalid emoji string: {}", server_id.0, verification_emoji);
                        return
                    }
                };

                if verification_message_id as u64 == added_reaction.message_id.0
                    && expected_verification_reaction == added_reaction.emoji {

                    // This is a verification attempt, give the verified role
                    if let Err(err) = ctx.http.add_member_role(server_id.0, user_id.0, verified_role_id as u64).await {
                        eprintln!("Failed to set verified role in server {}: {}", server_id.0, err);
                    }
                }
            }
        }
    }

    async fn ready(&self, _ctx: Context, ready: Ready) {
        println!("Connected to discord as {}#{}", ready.user.name, ready.user.discriminator);
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
        .intents(GatewayIntents::non_privileged().union(GatewayIntents::GUILD_MEMBERS))
        .event_handler(RaincoatCatEventHandler { db: Arc::new(db) })
        .application_id(config.discord_application_id)
        .await
        .expect("Failed to create discord client");

    if let Err(err) = client.start().await {
        println!("Failed to start discord client: {:?}", err);
    }
}
