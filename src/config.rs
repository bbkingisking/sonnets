use anyhow::{Result, anyhow};
use log::info;
use serde::{Deserialize, Serialize};
use std::io::ErrorKind;
use std::process;
use std::{env, fs, path::PathBuf};

const PKG_NAME: &str = env!("CARGO_PKG_NAME");

/// The YAML config file only contains non-secret settings.
#[derive(Serialize, Deserialize, Debug)]
struct YamlConfig {
    pub model: String,
    pub system_prompt: String,
}

/// The full Config used at runtime, with secrets from env vars and paths derived from XDG.
pub struct Config {
    pub api_key: String,
    pub model: String,
    pub telegram_bot_token: String,
    pub telegram_chat_ids: Vec<i64>,
    pub db_path: PathBuf,
    pub nouns_path: PathBuf,
    pub system_prompt: String,
}

fn xdg_config_home() -> Result<PathBuf> {
    if let Ok(val) = env::var("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(val));
    }
    let home = env::home_dir().ok_or_else(|| anyhow!("Could not determine $HOME"))?;
    Ok(home.join(".config"))
}

fn xdg_data_home() -> Result<PathBuf> {
    if let Ok(val) = env::var("XDG_DATA_HOME") {
        return Ok(PathBuf::from(val));
    }
    let home = env::home_dir().ok_or_else(|| anyhow!("Could not determine $HOME"))?;
    Ok(home.join(".local").join("state"))
}

impl Config {
    fn placeholder() -> Result<String> {
        let placeholder = YamlConfig {
            model: String::from("The LLM model to use, for example: claude-opus-4-5-20251101"),
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

        serde_yaml::to_string(&placeholder)
            .map_err(|e| anyhow!("Could not generate placeholder config to show. {}", e))
    }

    pub fn load() -> Result<Self> {
        let conf_dir = xdg_config_home()?.join(PKG_NAME);
        let conf_file = conf_dir.join("config.yaml");

        // Create config dir if needed
        fs::create_dir_all(&conf_dir).map_err(|e| {
            anyhow!("Could not create config dir at {:#?}: {}", &conf_dir, e)
        })?;

        // Create config file if it doesn't exist
        match fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&conf_file)
        {
            Ok(_) => {
                if let Ok(placeholder) = Config::placeholder() {
                    println!(
                        "Created new config file at {:?}. Please populate it with the following fields:\n\n\
                         Also set these environment variables:\n  \
                         ANTHROPIC_API_KEY   - Your Anthropic API key\n  \
                         TELEGRAM_BOT_TOKEN  - Your Telegram bot token\n  \
                         TELEGRAM_CHAT_IDS   - Comma-separated Telegram chat IDs\n\n{}",
                        conf_file, placeholder,
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

        // Read and parse YAML config
        let conf_str = fs::read_to_string(&conf_file).map_err(|e| {
            anyhow!("Could not read config from {:#?}, {}", &conf_file, e)
        })?;

        let yaml: YamlConfig = serde_yaml::from_str(&conf_str)
            .map_err(|e| anyhow!("Could not parse config from YAML file: {}", e))?;

        // Read secrets from environment variables
        let api_key = env::var("ANTHROPIC_API_KEY")
            .map_err(|_| anyhow!("Missing environment variable: ANTHROPIC_API_KEY"))?;

        let telegram_bot_token = env::var("TELEGRAM_BOT_TOKEN")
            .map_err(|_| anyhow!("Missing environment variable: TELEGRAM_BOT_TOKEN"))?;

        let telegram_chat_ids_str = env::var("TELEGRAM_CHAT_IDS")
            .map_err(|_| anyhow!("Missing environment variable: TELEGRAM_CHAT_IDS"))?;

        let telegram_chat_ids: Vec<i64> = telegram_chat_ids_str
            .split(',')
            .map(|s| s.trim().parse::<i64>())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| anyhow!("Could not parse TELEGRAM_CHAT_IDS as comma-separated i64s: {}", e))?;

        // Derive state paths from XDG_DATA_HOME
        let state_dir = xdg_data_home()?.join(PKG_NAME);
        let db_path = state_dir.join("sonnets.db");
        let nouns_path = state_dir.join("nouns.txt");

        info!("Config loaded.");
        Ok(Config {
            api_key,
            model: yaml.model,
            telegram_bot_token,
            telegram_chat_ids,
            db_path,
            nouns_path,
            system_prompt: yaml.system_prompt,
        })
    }
}
