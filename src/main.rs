mod config;
mod commands;
mod model;
mod error;

use std::sync::Arc;
use chrono::{Utc, Duration};
use sqlx::postgres::PgPoolOptions;
use sea_orm::{DatabaseConnection, SqlxPostgresConnector, EntityTrait, QueryFilter, ColumnTrait};
use sea_orm::ActiveValue::Set;

use serenity::async_trait;
use serenity::cache::Cache;
use serenity::client::bridge::gateway::GatewayIntents;
use serenity::http::Http;
use serenity::model::channel::{Reaction, ReactionType};
use serenity::model::gateway::Ready;
use serenity::model::guild::{Guild, Member};
use serenity::model::id::{GuildId, RoleId};
use serenity::model::interactions::{Interaction, InteractionResponseType};
use serenity::model::prelude::InteractionApplicationCommandCallbackDataFlags;
use serenity::prelude::*;

use crate::error::RaincoatError;
use crate::model::server;
use crate::model::punishment;
use crate::model::punishment_removed_role;
use crate::punishment::PunishmentType;

struct RaincoatCatEventHandler {
    db: Arc<DatabaseConnection>
}

impl RaincoatCatEventHandler {
    async fn update_server(server: Guild, server_model: server::Model, db: Arc<DatabaseConnection>, http: Arc<Http>) -> Result<(), RaincoatError> {
        // Iterate through users and determine if any should be kicked due to the verification timeout
        if let (Some(verification_timeout), Some(verified_role_id)) = (server_model.verification_timeout, server_model.verified_role_id) {
            for (user_id, member) in &server.members {
                if !member.roles.contains(&RoleId(verified_role_id as u64)) && !member.user.bot {
                    if let Some(joined_at) = member.joined_at {
                        if joined_at < Utc::now() - Duration::hours(verification_timeout) {

                            // Check if the user has an active punishment.
                            let punishments = punishment::Entity::find()
                                .filter(punishment::Column::UserId.eq(user_id.0 as i64))
                                .filter(punishment::Column::ServerId.eq(server.id.0 as i64))
                                .all(db.as_ref()).await.map_err(|err| RaincoatError { cause: format!("{}", err) })?;

                            // Don't kick user if they're otherwise being punished right now
                            if punishments.len() == 0 {
                                println!("Kicking user {} from server {} for failing to verify within {} hours.", user_id.0, server.name, verification_timeout);
                                server.kick(&http, user_id).await
                                    .map_err(|err| RaincoatError { cause: format!("Failed to kick user {} from server {}: {}", user_id.0, server.name, err) })?;
                            }
                        }
                    }
                }
            }
        }

        // Undo punishments that have now expired
        let punishments: Vec<(punishment::Model, Vec<punishment_removed_role::Model>)> = punishment::Entity::find()
            .filter(punishment::Column::ServerId.eq(server.id.0 as i64))
            .filter(punishment::Column::Expires.lt(Utc::now().naive_utc()))
            .find_with_related(punishment_removed_role::Entity)
            .all(db.as_ref()).await.map_err(|err| RaincoatError { cause: format!("{}", err) })?;

        for (punishment, roles) in &punishments {
            let punishment_delete = punishment::ActiveModel {
                id: Set(punishment.id),
                ..Default::default()
            };
            punishment::Entity::delete(punishment_delete).exec(db.as_ref()).await
                .map_err(|err| RaincoatError { cause: format!("{}", err)})?;

            match punishment.punishment_type {
                PunishmentType::Dunce => {
                    for role in roles {
                        http.add_member_role(server.id.0, punishment.user_id as u64, role.role_id as u64).await
                            .map_err(|err| RaincoatError { cause: format!("Unable to return user role: {}", err) })?;
                        let role_delete = punishment_removed_role::ActiveModel {
                            id: Set(role.id),
                            ..Default::default()
                        };
                        punishment_removed_role::Entity::delete(role_delete).exec(db.as_ref()).await
                            .map_err(|err| RaincoatError { cause: format!("{}", err) })?;
                    }

                    if let Some(dunce_role_id) = server_model.dunce_role_id {
                        http.remove_member_role(server.id.0, punishment.user_id as u64, dunce_role_id as u64).await
                            .map_err(|err| RaincoatError { cause: format!("Unable to remove dunce role: {}", err) })?;
                    }
                }
                PunishmentType::Ban => {
                    for role in roles {
                        // TODO: figure out some way to automatically readd roles?
                        let role_delete = punishment_removed_role::ActiveModel {
                            id: Set(role.id),
                            ..Default::default()
                        };
                        punishment_removed_role::Entity::delete(role_delete).exec(db.as_ref()).await
                            .map_err(|err| RaincoatError { cause: format!("{}", err) })?;
                    }

                    http.remove_ban(server.id.0, punishment.user_id as u64).await
                        .map_err(|err| RaincoatError { cause: format!("Unable to unban user: {}", err) })?;
                }
            }
        }

        Ok(())
    }

    async fn kick_listener(db: Arc<DatabaseConnection>, cache: Arc<Cache>, http: Arc<Http>) {
        let mut interval_timer = tokio::time::interval(std::time::Duration::from_secs(5));
        loop {
            interval_timer.tick().await;

            for server_id in cache.guilds().await {
                let server = cache.guild(server_id).await.unwrap();
                if let Some(server_model) = server::Entity::find_by_id(server_id.0 as i64).one(db.as_ref()).await
                    .expect("DB lookup failed") {

                    if let Err(err) = Self::update_server(server, server_model, Arc::clone(&db), Arc::clone(&http)).await {
                        eprintln!("Failure during kick handler: {}", err);
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

    async fn guild_member_addition(&self, ctx: Context, server_id: GuildId, new_member: Member) {
        if let Some(server_model) = server::Entity::find_by_id(server_id.0 as i64).one(self.db.as_ref()).await
            .expect("DB lookup failed") {
            if let Some(dunce_role_id) = server_model.dunce_role_id {
                let punishments: Vec<punishment::Model> = match punishment::Entity::find()
                    .filter(punishment::Column::UserId.eq(new_member.user.id.0 as i64))
                    .filter(punishment::Column::ServerId.eq(server_id.0 as i64))
                    .all(self.db.as_ref()).await {
                    Ok(punishments) => punishments,
                    Err(err) => {
                        eprintln!("{}", err);
                        return;
                    }
                };

                // Repunish user if necessary
                for punishment in punishments {
                    match punishment.punishment_type {
                        PunishmentType::Dunce => {
                            if let Err(err) = ctx.http.add_member_role(server_id.0, new_member.user.id.0, dunce_role_id as u64).await {
                                eprintln!("Failed to redunce user: {}", err);
                            }
                        }
                        PunishmentType::Ban => {
                            if let Err(err) = new_member.ban(&ctx.http, 0).await {
                                eprintln!("Failed to reban user: {}", err);
                            }
                        }
                    }
                }
            }
        }
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
        .intents(GatewayIntents::all())
        .event_handler(RaincoatCatEventHandler { db: Arc::new(db) })
        .application_id(config.discord_application_id)
        .await
        .expect("Failed to create discord client");

    if let Err(err) = client.start().await {
        println!("Failed to start discord client: {:?}", err);
    }
}
