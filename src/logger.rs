use anyhow::Result;
use anyhow::anyhow;
use ftail::Ftail;
use log::LevelFilter;
use log::info;
use std::env;
use std::fs;
use std::path::PathBuf;

const PKG_NAME: &str = env!("CARGO_PKG_NAME");

fn xdg_state_home() -> Result<PathBuf> {
    if let Ok(val) = env::var("XDG_DATA_HOME") {
        return Ok(PathBuf::from(val));
    }
    let home = env::home_dir().ok_or_else(|| anyhow!("Could not determine $HOME"))?;
    Ok(home.join(".local").join("state"))
}

pub fn init_logger() -> Result<()> {
    let logs_path = xdg_state_home()?.join(PKG_NAME);

    let logs_file = logs_path.join(format!("{}.log", PKG_NAME));

    match fs::create_dir_all(&logs_path) {
        Ok(_) => (),
        Err(e) => {
            return Err(anyhow!(
                "Could not create logs dir at {:#?}: {}",
                &logs_path,
                e
            ));
        }
    }

    match Ftail::new()
        .single_file(&logs_file, true, LevelFilter::Info)
        .init()
    {
        Ok(_) => {
            info!("Logger initialized.");
            Ok(())
        }
        Err(e) => return Err(anyhow!("Could not initialize logger: {}", e)),
    }
}
