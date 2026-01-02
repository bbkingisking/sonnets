use crate::config::Config;
use anyhow::{Result, anyhow};
use log::info;
use reqwest::header::{self, HeaderValue};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct AnthropicResponse {
    pub data: Vec<AnthropicResponseData>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AnthropicResponseData {
    pub id: String,
    pub created_at: String,
    pub display_name: String,
    #[serde(rename = "type")]
    pub _type: String,
}

pub async fn validate_anthropic_config(conf: &Config) -> Result<()> {
    let mut headers = header::HeaderMap::new();

    headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));
    // Consider marking security-sensitive headers with `set_sensitive`.
    let mut auth_value = header::HeaderValue::from_str(&conf.api_key)?;
    auth_value.set_sensitive(true);
    headers.insert("X-Api-Key", auth_value);

    // get a client builder
    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()?;

    let res = match client
        .get("https://api.anthropic.com/v1/models")
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return Err(anyhow!(
                "Failed to get a response from Anthropic's /v1/models endpoint: {}",
                e
            ));
        }
    };

    let valid_res = match res.error_for_status() {
        Ok(r) => {
            info!("Anthropic API key is valid.");
            r
        }
        Err(e) => {
            return Err(anyhow!(
                "Received an error when checking Anthropic credentials: {}",
                e
            ));
        }
    };

    let text_res = match valid_res.text().await {
        Ok(t) => t,
        Err(e) => return Err(anyhow!("Could not get text from Anthropic response: {}", e)),
    };

    let value_res: AnthropicResponse = match serde_json::from_str(&text_res) {
        Ok(v) => v,
        Err(e) => return Err(anyhow!("Could not deserialize Anthropic response: {}", e)),
    };

    match value_res.data.iter().any(|item| item.id == conf.model) {
        true => info!("Anthropic model is valid."),
        false => return Err(anyhow!("Specified Anthropic model in conf is invalid.")),
    }

    Ok(())
}

pub async fn validate_telegram_config(conf: &Config) -> Result<()> {
    let url = format!(
        "https://api.telegram.org/bot{}/getMe",
        &conf.telegram_bot_token
    );
    let client = reqwest::Client::new();

    let res = match client.post(url).send().await {
        Ok(r) => r,
        Err(e) => {
            return Err(anyhow!(
                "Failed to get a response from Telegram's /getMe endpoint: {}",
                e
            ));
        }
    };

    match res.error_for_status() {
        Ok(_) => info!("Telegram bot token is valid."),
        Err(e) => {
            return Err(anyhow!(
                "Received an error when checking Telegram bot token: {}",
                e
            ));
        }
    };

    Ok(())
}
