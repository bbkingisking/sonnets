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
    init_logger()?;
    let conf = Config::load()?;
    validate_anthropic_config(&conf).await?;
    validate_telegram_config(&conf).await?;
    let nouns = load_nouns(&conf)?;
    let db = Db::init_db(&conf)?;
    Ok(())
}
