use anyhow::{Result, anyhow};
use log::info;
use teloxide::{prelude::*, types::ChatId};

use crate::{config::Config, generate_sonnet::Sonnet};

pub async fn send_telegram_message(conf: &Config, sonnet: &Sonnet) -> Result<()> {
    let bot = Bot::new(&conf.telegram_bot_token);

    let mut errors = Vec::new();

    for &chat_id in &conf.telegram_chat_ids {
        match bot.send_message(ChatId(chat_id), &sonnet.content).await {
            Ok(_) => info!("Sent the sonnet to {}", &chat_id),
            Err(e) => errors.push((chat_id, e))
        }
    }

    if !errors.is_empty() {
        return Err(anyhow!("Failed to send to {} recipient(s): {:?}", errors.len(), errors));
    }

    Ok(())
}
