use anyhow::{Result, anyhow};
use log::info;
use serde::{Deserialize, Serialize};
use std::io::ErrorKind;
use std::process;
use std::{env, fs, path::PathBuf};

const CONF_DIR: &str = "config";
const PKG_NAME: &str = env!("CARGO_PKG_NAME");

// The main Config struct, change the fields here and the compiler will guide you to the rest
#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub api_key: String,
    pub model: String,
    pub telegram_bot_token: String,
    pub telegram_chat_ids: Vec<i64>,
    pub db_path: PathBuf,
    pub nouns_path: PathBuf,
    pub system_prompt: String,
}

impl Config {
    // Placeholder values will be displayed to the user if the config file does not exist, so they know what to put in
    fn placeholder() -> Result<String> {
        let placeholder_config = Config {
            api_key: String::from("Your Anthropic API key here"),
            model: String::from("The LLM model to use, for example: claude-opus-4-5-20251101"),
            telegram_bot_token: String::from("The telegram bot token"),
            telegram_chat_ids: vec![31513, 535151],
            db_path: PathBuf::from("/path/to/your/db/"),
            nouns_path: PathBuf::from("/path/to/nouns.txt"),
            system_prompt: r#"
            You are Claudio di Montefiore, a classically trained poet with a sardonic edge. Compose a traditional sonnet with the following qualities:
            Structure:
            - 14 lines, iambic pentameter.
            - Use a Shakespearean or Petrarchan rhyme scheme.
            Style & Language:
            - Avoid moralizing or sentimental platitudes.
            - Favor precise, slightly elevated diction. Channel the tone of Millay, early Auden, or a weary romantic.
            Closing:
            - The final couplet or line should not resolve with a cliché or overt lesson. Leave the reader with a twist, a turn, or a sting.
            Output only the sonnet text. No preamble, no title, no explanation.
            "#.to_string()
        };

        match serde_yaml::to_string(&placeholder_config) {
            Ok(s) => Ok(s.to_string()),
            Err(e) => {
                return Err(anyhow!(
                    "Could not generate placeholder config to show. {}",
                    e
                ));
            }
        }
    }

    pub fn load() -> Result<Self> {
        // Try to get home folder
        let home_folder = match env::home_dir() {
            Some(h) => h,
            None => return Err(anyhow!("Could not determine $HOME")),
        };

        // Get final dir
        let conf_path = home_folder.join("state").join(PKG_NAME).join(CONF_DIR);

        // Get final dir + filename
        let conf_file = conf_path.join("config.yaml");

        // Try to create the final dir (this is idempotent so it's chill to run every time)
        match fs::create_dir_all(&conf_path) {
            Ok(_) => (),
            Err(e) => {
                return Err(anyhow!(
                    "Could not create config dir at {:#?}: {}",
                    &conf_path,
                    e
                ));
            }
        }

        // Try to create the final file if it doesn't exist
        match fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&conf_file)
        {
            Ok(_) => {
                // Check if the placeholder config can be serialized
                if let Ok(placeholder_config) = Config::placeholder() {
                    println!(
                        "Created new config file at {:?}. Please populate it with the following fields:\n\n{}",
                        conf_file, placeholder_config,
                    );
                    process::exit(0);
                }
            }
            Err(e) if e.kind() == ErrorKind::AlreadyExists => (),
            Err(e) => {
                return Err(anyhow!(
                    "Could not create config file {:?}: {}",
                    conf_file,
                    e
                ));
            }
        }

        // Try to read the conf file to string
        let conf_str = match fs::read_to_string(&conf_file) {
            Ok(c) => c,
            Err(e) => {
                return Err(anyhow!(
                    "Could not read config from {:#?}, {}",
                    &conf_file,
                    e
                ));
            }
        };

        // Try to parse the string to actual YAML
        let conf: Config = match serde_yaml::from_str(&conf_str) {
            Ok(v) => v,
            Err(e) => return Err(anyhow!("Could not parse config from YAML file: {}", e)),
        };

        info!("Config loaded.");
        Ok(conf)
    }
}
