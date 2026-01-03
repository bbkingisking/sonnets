use crate::config::Config;
use anyhow::{Result, anyhow};
use log::{info, warn};
use std::fs::read_to_string;
use rand::prelude::IndexedRandom;

pub fn load_noun(conf: &Config) -> Result<Option<String>> {
    let nouns: Option<String> = match read_to_string(&conf.nouns_path) {
        Ok(n) => {
            let nouns_list: Vec<&str> = n.lines().collect();
            if let Some(&random_noun) = nouns_list.choose(&mut rand::rng()) {
                let noun = random_noun.to_owned();
                info!("Noun for the day is: {}.", random_noun);
                Some(noun)
            } else {
                // We throw here because the user tried to provide nouns.txt but it can't be parsed
                return Err(anyhow!("Could not select a random noun from the list. Are they formatted correctly?"))
            }
        }
        Err(e) => {
            warn!("Could not read nouns.txt from {:?}, : {}, proceeding without it…", &conf.nouns_path, e);
            None
        }
    };

    Ok(nouns)
}
