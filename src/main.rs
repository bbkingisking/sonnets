mod config;
mod logger;

use crate::config::Config;
use crate::logger::init_logger;
use anyhow::Result;

fn main() -> Result<()> {
    let _ = init_logger()?;
    let _config = Config::load()?;

    Ok(())
}
