use serenity::builder::{CreateActionRow, CreateApplicationCommands, CreateSelectMenu, CreateSelectMenuOption};
use serenity::model::interactions::application_command::ApplicationCommandInteraction;
use serenity::model::interactions::InteractionResponseType;
use serenity::model::prelude::InteractionApplicationCommandCallbackDataFlags;
use serenity::prelude::*;

pub fn create_command(commands: &mut CreateApplicationCommands) {
    commands.create_application_command(|command| {
        command.name("role")
            .description("Assign yourself optional roles")
    });
}

pub async fn create_response(ctx: Context, command: &ApplicationCommandInteraction) {
    if let Err(err) = command.create_interaction_response(&ctx.http, |response| {
        response.kind(InteractionResponseType::ChannelMessageWithSource)
            .interaction_response_data(|message| {
                message.components(|c| c.add_action_row(role_action_row()))
                    .flags(InteractionApplicationCommandCallbackDataFlags::EPHEMERAL)
            })
    }).await {
        println!("Cannot respond to slash command: {}", err);
    }
}

fn role_action_row() -> CreateActionRow {
    let mut row = CreateActionRow::default();
    row.add_select_menu(role_select_menu());
    row
}

fn role_select_menu() -> CreateSelectMenu {
    let mut menu = CreateSelectMenu::default();

    menu.custom_id("role_select");
    menu.placeholder("Select optional roles");
    menu.options(|f| {
        let mut cpp = CreateSelectMenuOption::default();

        cpp.label("C++");
        cpp.value("cpp");

        f.add_option(cpp);

        f
    });
    menu.max_values(1);

    menu
}