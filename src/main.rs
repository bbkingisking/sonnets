mod config;
mod logger;
mod validate;

use crate::config::Config;
use crate::logger::init_logger;
use crate::validate::validate_anthropic_config;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let _ = init_logger()?;
    let conf = Config::load()?;
    let _ = validate_anthropic_config(&conf).await?;

    Ok(())
}
