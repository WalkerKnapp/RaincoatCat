use sea_orm::{DatabaseConnection, EntityTrait, QueryFilter, ColumnTrait};

use serenity::builder::{CreateActionRow, CreateApplicationCommands, CreateSelectMenu, CreateSelectMenuOption};
use serenity::model::id::{GuildId, RoleId, UserId};
use serenity::model::interactions::application_command::ApplicationCommandInteraction;
use serenity::model::interactions::InteractionResponseType;
use serenity::model::prelude::InteractionApplicationCommandCallbackDataFlags;
use serenity::model::prelude::message_component::MessageComponentInteraction;
use serenity::prelude::*;

use crate::error::RaincoatError;
use crate::model::optional_role;

pub fn create_command(commands: &mut CreateApplicationCommands) {
    commands.create_application_command(|command| {
        command.name("role")
            .description("Assign yourself optional roles")
    });
}

pub async fn create_response(db: &DatabaseConnection, ctx: &Context, command: &ApplicationCommandInteraction) -> Result<(), RaincoatError> {
    let action_row = role_action_row(db, ctx, command
        .guild_id.ok_or(RaincoatError { cause: "This command can only be run in servers.".to_string()})?, command.user.id).await?;

    command.create_interaction_response(&ctx.http, |response| {
        response.kind(InteractionResponseType::ChannelMessageWithSource)
            .interaction_response_data(|message| {
                message.components(|c| c.add_action_row(action_row))
                    .flags(InteractionApplicationCommandCallbackDataFlags::EPHEMERAL)
            })
    }).await.map_err(|err| RaincoatError { cause: format!("Couldn't respond to command: {}", err)})
}

async fn role_action_row(db: &DatabaseConnection, ctx: &Context, server_id: GuildId, user_id: UserId) -> Result<CreateActionRow, RaincoatError> {
    let mut row = CreateActionRow::default();
    row.add_select_menu(role_select_menu(db, ctx, server_id, user_id).await?);
    Ok(row)
}

async fn role_select_menu(db: &DatabaseConnection, ctx: &Context, server_id: GuildId, user_id: UserId) -> Result<CreateSelectMenu, RaincoatError> {
    let mut menu = CreateSelectMenu::default();

    let optional_roles: Vec<optional_role::Model> = optional_role::Entity::find()
        .filter(<optional_role::Entity as EntityTrait>::Column::ServerId.eq(server_id.0))
        .all(db).await
        .map_err(|err| RaincoatError { cause: format!("DB Error: {}", err)})?;

    if optional_roles.len() == 0 {
        return Err(RaincoatError { cause: format!("No optional roles set for this server.") })
    }


    let user = ctx.http.get_member(server_id.0, user_id.0).await
        .map_err(|err| RaincoatError { cause: format!("Failed to collect existing user information: {}", err) })?;

    let mut options = Vec::with_capacity(optional_roles.len());

    for role in &optional_roles {
        let cached_role = ctx.cache.role(server_id, RoleId(role.role_id as u64)).await
            .ok_or(RaincoatError { cause: format!("Role {} no longer exists.", role.role_id)})?;

        let mut option = CreateSelectMenuOption::default();
        option.label(cached_role.name.clone());
        option.value(role.role_id);
        if let Some(emoji) = &role.emoji {
            option.emoji(emoji.clone().try_into()
                .map_err(|_err| RaincoatError { cause: format!("Invalid emoji for role {}", cached_role.name) })?);
        }
        option.default_selection(user.roles.contains(&RoleId(role.role_id as u64)));
        options.push(option);
    }

    menu.custom_id("role_select");
    menu.placeholder("Select optional roles");
    menu.max_values(optional_roles.len() as u64);
    menu.min_values(0);
    menu.options(move |f| {
        for option in options {
            f.add_option(option);
        }
        f
    });

    Ok(menu)
}

pub async fn create_component_response(db: &DatabaseConnection, ctx: &Context, command: &MessageComponentInteraction) -> Result<(), RaincoatError> {
    let server_id: GuildId = command.guild_id.ok_or(RaincoatError { cause: "This command can only be run in servers.".to_string() })?;
    let mut user = ctx.http.get_member(server_id.0, command.user.id.0).await
        .map_err(|err| RaincoatError { cause: format!("Failed to collect existing user information: {}", err) })?;

    let optional_roles: Vec<optional_role::Model> = optional_role::Entity::find()
        .filter(<optional_role::Entity as EntityTrait>::Column::ServerId.eq(server_id.0))
        .all(db).await
        .map_err(|err| RaincoatError { cause: format!("DB Error: {}", err)})?;

    let mut added_role_ids = Vec::new();
    let mut added_role_names = Vec::new();
    let mut removed_role_ids = Vec::new();
    let mut removed_role_names = Vec::new();

    for role in optional_roles {
        let id = RoleId(role.role_id as u64);
        let id_str = role.role_id.to_string();
        let name = ctx.cache.role(server_id, id).await
            .ok_or(RaincoatError { cause: format!("Failed to fetch information about role")})?
            .name;

        // Remove/add based on role being present in component.data.values
        let present = command.data.values.contains(&id_str);

        if present && !user.roles.contains(&id) {
            added_role_ids.push(id);
            added_role_names.push(name);
        } else if !present && user.roles.contains(&id) {
            removed_role_ids.push(id);
            removed_role_names.push(name);
        }
    }

    user.add_roles(&ctx.http, added_role_ids.as_slice()).await
        .map_err(|err| RaincoatError { cause: format!("Could not add roles: {}", err) })?;
    user.remove_roles(&ctx.http, removed_role_ids.as_slice()).await
        .map_err(|err| RaincoatError { cause: format!("Could not remove roles: {}", err) })?;

    let mut output_buffer = Vec::with_capacity(2);

    if !added_role_names.is_empty() {
        output_buffer.push(format!("Added roles: {}", added_role_names.join(", ")));
    }
    if !removed_role_names.is_empty() {
        output_buffer.push(format!("Removed roles: {}", removed_role_names.join(", ")));
    }

    command.create_interaction_response(&ctx.http, |response| {
        response.kind(InteractionResponseType::ChannelMessageWithSource)
            .interaction_response_data(|message| {
                if !output_buffer.is_empty() {
                    message.content(output_buffer.join("\n"));
                } else {
                    message.content("Didn't make any changes!");
                }
                message.flags(InteractionApplicationCommandCallbackDataFlags::EPHEMERAL)
            })
    }).await.map_err(|err| RaincoatError { cause: format!("Failed to respond to component: {}", err) })
}
