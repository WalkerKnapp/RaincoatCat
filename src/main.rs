mod config;

use serenity::async_trait;
use serenity::prelude::*;

struct RaincoatCatEventHandler;

#[async_trait]
impl EventHandler for RaincoatCatEventHandler {

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
