use std::fs;
use serde::Deserialize;

#[derive(Deserialize, Clone)]
pub struct Config {
    pub discord_application_id: u64,
    pub discord_bot_token: String,
    pub postgres_password: String
}

pub fn load_config() -> Config {
    let config_content = fs::read_to_string("config.toml")
        .expect("No config.toml present.");

    toml::from_str(config_content.as_str())
        .expect("Failed to deserialize config.toml")
}