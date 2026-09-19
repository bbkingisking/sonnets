use anyhow::Result;
use anyhow::anyhow;
use ftail::Ftail;
use log::info;

pub fn init_logger() -> Result<()> {
    match Ftail::new()
        .console_env_level()
        .init()
    {
        Ok(_) => {
            info!("Logger initialized.");
            Ok(())
        }
        Err(e) => return Err(anyhow!("Could not initialize logger: {}", e)),
    }
}
