mod config;
mod db;
mod generate_sonnet;
mod nouns;
mod poems;
mod telegram;
mod validate;

use crate::config::Config;
use crate::db::Db;
use crate::generate_sonnet::generate_sonnet;
use crate::nouns::load_noun;
use crate::poems::load_inspiration_poems;
use crate::telegram::send_telegram_message;
use crate::validate::{validate_anthropic_config, validate_telegram_config};
use anyhow::Result;
use log::{debug, info};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize the logger
    colog::init();
    debug!("Logger setup complete; beginning configuration load.");

    // Load the configuration
    let conf = Config::load()?;
    debug!(
        "Configuration loaded: model={:?}, telegram_recipients={}, poetry_dir_configured={}",
        conf.model,
        conf.telegram_chat_ids.len(),
        conf.poetry_dir.is_some()
    );

    // Validate Anthropic API key and model
    validate_anthropic_config(&conf).await?;
    debug!("Anthropic configuration validation complete.");

    // Validate Telegram bot token
    validate_telegram_config(&conf).await?;
    debug!("Telegram configuration validation complete.");

    // Load nouns (optional)
    let noun = load_noun(&conf)?;
    debug!("Noun loading complete: selected={:?}.", noun);

    // Load DB
    let db = Db::init_db(&conf)?;
    debug!("Database initialization complete.");

    // Load a random sample of the optional inspiration corpus.
    let inspiration = match conf.poetry_dir.as_deref() {
        Some(poetry_dir) => load_inspiration_poems(poetry_dir)?,
        None => None,
    };
    debug!(
        "Inspiration loading complete: configured={}, characters={:?}.",
        conf.poetry_dir.is_some(),
        inspiration.as_ref().map(String::len)
    );

    // Generate sonnet
    let sonnet = generate_sonnet(&conf, noun, inspiration).await?;
    debug!(
        "Sonnet generation complete: author={:?}, characters={}, noun={:?}.",
        sonnet.author,
        sonnet.content.len(),
        sonnet.noun
    );

    // Write sonnet to DB
    db.write_sonnet(&sonnet)?;
    debug!("Sonnet persistence complete.");

    // Send sonnet via telegram
    send_telegram_message(&conf, &sonnet).await?;
    info!("Sonnet workflow completed successfully.");

    Ok(())
}
