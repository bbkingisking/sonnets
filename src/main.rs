mod config;
mod db;
mod generate_sonnet;
mod logger;
mod nouns;
mod poems;
mod telegram;
mod validate;

use crate::config::Config;
use crate::db::Db;
use crate::generate_sonnet::generate_sonnet;
use crate::logger::init_logger;
use crate::nouns::load_noun;
use crate::poems::load_inspiration_poems;
use crate::telegram::send_telegram_message;
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
    let noun = load_noun(&conf)?;

    // Load DB
    let db = Db::init_db(&conf)?;

    // Load a random sample of the optional inspiration corpus.
    let inspiration = match conf.poetry_dir.as_deref() {
        Some(poetry_dir) => load_inspiration_poems(poetry_dir)?,
        None => None,
    };

    // Generate sonnet
    let sonnet = generate_sonnet(&conf, noun, inspiration).await?;

    // Write sonnet to DB
    db.write_sonnet(&sonnet)?;

    // Send sonnet via telegram
    send_telegram_message(&conf, &sonnet).await?;

    Ok(())
}
