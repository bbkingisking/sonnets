mod config;
mod db;
mod logger;
mod nouns;
mod validate;
use crate::config::Config;
use crate::db::Db;
use crate::logger::init_logger;
use crate::nouns::load_nouns;
use crate::validate::{validate_anthropic_config, validate_telegram_config};
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize the logger
    init_logger()?;

    // Load the configuration
    let conf = Config::load()?;

    // Validate Anthropic API key and model
    validate_anthropic_config(&conf).await?;

    // Validate Telegram bot token
    validate_telegram_config(&conf).await?;

    // Load nouns (optional)
    let nouns = load_nouns(&conf)?;

    // Load DB
    let db = Db::init_db(&conf)?;
    Ok(())
}
