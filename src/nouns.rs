use crate::config::Config;
use anyhow::Result;
use log::{info, warn};
use std::fs::read_to_string;

pub fn load_nouns(conf: &Config) -> Result<Option<String>> {
    let nouns: Option<String> = match read_to_string(&conf.nouns_path) {
        Ok(n) => {
            info!("Nouns loaded.");
            Some(n)
        }
        Err(e) => {
            warn!(
                "Could not read nouns.txt from {:?}, : {}, proceeding without it…",
                &conf.nouns_path, e
            );
            None
        }
    };

    Ok(nouns)
}
