use crate::models::{Alert, AlertType};
use anyhow::Result;
use chrono::Utc;
use futures_util::SinkExt;
use log::{error, info, warn};
use std::env;
use teloxide::{
    prelude::*,
    types::{ChatId, ParseMode},
};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use url::Url;

pub struct AlertService {
    bot: Option<Bot>,
    chat_id: Option<ChatId>,
    ws_url: Option<String>,
}

impl AlertService {
    pub fn new() -> Result<Self> {
        let (bot, chat_id) = match (env::var("TELEGRAM_BOT_TOKEN"), env::var("TELEGRAM_CHAT_ID")) {
            (Ok(token), Ok(chat_id_str)) => {
                info!("Found Telegram config - token: {}, chat_id: {}", token, chat_id_str);
                match chat_id_str.parse::<i64>() {
                    Ok(chat_id_int) => {
                        info!("Successfully parsed chat_id: {}", chat_id_int);
                        (Some(Bot::new(token)), Some(ChatId(chat_id_int)))
                    },
                    Err(e) => {
                        error!("Invalid TELEGRAM_CHAT_ID: {}", e);
                        (None, None)
                    }
                }
            }
            (Err(e1), Err(e2)) => {
                error!("Telegram configuration not found - token error: {:?}, chat_id error: {:?}", e1, e2);
                (None, None)
            }
            (Err(e), _) => {
                error!("TELEGRAM_BOT_TOKEN not found: {:?}", e);
                (None, None)
            }
            (_, Err(e)) => {
                error!("TELEGRAM_CHAT_ID not found: {:?}", e);
                (None, None)
            }
        };

        let ws_url = match env::var("WS_ALERT_URL") {
            Ok(url) => {
                info!("Found WebSocket URL: {}", url);
                Some(url)
            }
            Err(e) => {
                warn!("WebSocket URL not found: {:?}", e);
                None
            }
        };

        Ok(Self {
            bot,
            chat_id,
            ws_url,
        })
    }

    pub async fn send_alert(&self, message: &str, alert_type: AlertType) -> Result<()> {
        let alert = Alert {
            message: message.to_string(),
            alert_type,
            timestamp: Utc::now().timestamp(),
        };

        // 发送到 Telegram
        match (&self.bot, self.chat_id) {
            (Some(ref bot), Some(chat_id)) => {
                info!("Sending alert to Telegram (chat_id: {}): {}", chat_id.0, message);
                match self.send_to_telegram(bot, chat_id, &alert).await {
                    Ok(_) => info!("Successfully sent alert to Telegram"),
                    Err(e) => error!("Failed to send alert to Telegram: {:?}", e),
                }
            }
            (None, _) => error!("Telegram bot not configured: {:?}", self.bot),
            (_, None) => error!("Telegram chat_id not configured: {:?}", self.chat_id),
        }

        Ok(())
    }

    async fn send_to_ws(&self, alert: &Alert) -> Result<()> {
        if let Some(ref ws_url) = self.ws_url {
            let url = Url::parse(ws_url)?;
            let (mut ws_stream, _) = connect_async(url).await?;
            let message = serde_json::to_string(alert)?;
            ws_stream.send(Message::Text(message)).await?;
            info!("Alert sent to WebSocket");
        }
        Ok(())
    }

    async fn send_to_telegram(&self, bot: &Bot, chat_id: ChatId, alert: &Alert) -> Result<()> {
        let message = format!(
            "*{}*\n{}\nTimestamp: {}",
            format!("{:?}", alert.alert_type),
            alert.message,
            alert.timestamp
        );
        
        info!("Sending Telegram message: {}", message);
        
        // 设置5秒超时
        match tokio::time::timeout(
            std::time::Duration::from_secs(5),
            bot.send_message(chat_id, message)
                .parse_mode(ParseMode::Markdown)
        ).await {
            Ok(result) => {
                match result {
                    Ok(_) => {
                        info!("Successfully sent message to Telegram");
                        Ok(())
                    },
                    Err(e) => {
                        error!("Telegram API error: {:?}", e);
                        Err(anyhow::anyhow!("Telegram API error: {:?}", e))
                    }
                }
            },
            Err(_) => {
                error!("Telegram API request timed out after 5 seconds");
                Err(anyhow::anyhow!("Telegram API request timed out"))
            }
        }
    }
}
