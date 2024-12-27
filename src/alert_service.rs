use crate::models::{Alert, AlertType, TradeSignal};
use anyhow::Result;
use chrono::Utc;
use futures_util::SinkExt;
use log::{error, info, warn};
use std::env;
use teloxide::{
    prelude::*,
    types::{ChatId, MessageId, ParseMode},
};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use url::Url;
use serde_json;

pub struct AlertService {
    bot: Bot,
    chat_id: i64,
    topic_id: Option<i32>,
    ws_url: Option<String>,
}

impl AlertService {
    pub fn new(bot_token: &str, chat_id: i64, topic_id: Option<i32>, ws_url: Option<String>) -> Self {
        Self {
            bot: Bot::new(bot_token),
            chat_id,
            topic_id,
            ws_url,
        }
    }

    pub async fn send_alert(&self, message: &str, alert_type: AlertType, mint: Option<String>) -> Result<()> {
        let alert_type_clone = alert_type.clone();
        let alert = Alert {
            message: message.to_string(),
            alert_type,
            timestamp: Utc::now().timestamp(),
        };

        // 发送到 Telegram
        match self.send_to_telegram(&self.format_alert_message(&alert)).await {
            Ok(_) => {
                info!("Successfully sent alert to Telegram");
                
                // 如果是价格提醒，发送信号到 WebSocket
                if alert_type_clone == AlertType::PriceAlert {
                    if let Some(mint_address) = mint {
                        let signal = TradeSignal {
                            signal: "sniper_pump1".to_string(),
                            mint: mint_address,
                            timestamp: Utc::now().timestamp(),
                        };
                        
                        if let Err(e) = self.send_to_ws(&signal).await {
                            error!("Failed to send signal to WebSocket: {}", e);
                        }
                    }
                }
                
                Ok(())
            }
            Err(e) => {
                let err = "Failed to send alert to Telegram";
                error!("{}: {:?}", err, e);
                Err(anyhow::anyhow!(err))
            }
        }
    }

    async fn send_to_ws(&self, signal: &TradeSignal) -> Result<()> {
        if let Some(ref ws_url) = self.ws_url {
            let url = Url::parse(ws_url)?;
            let (mut ws_stream, _) = connect_async(url).await?;
            let message = serde_json::to_string(signal)?;
            ws_stream.send(Message::Text(message.clone())).await?;
            info!("Signal sent to WebSocket: {}", message);
        }
        Ok(())
    }

    async fn send_to_telegram(&self, message: &str) -> Result<()> {
        let chat_id = ChatId(self.chat_id);
        
        match self.bot.send_message(chat_id, message)
            .message_thread_id(self.topic_id.unwrap_or(0))  
            .parse_mode(ParseMode::Html)
            .await {
            Ok(sent_message) => {
                info!("Successfully sent message to Telegram. Message ID: {}", sent_message.id);
                info!("Chat ID used: {}", chat_id.0);
                if let Some(topic_id) = self.topic_id {
                    info!("Topic ID used: {}", topic_id);
                }
                Ok(())
            },
            Err(e) => {
                error!("Failed to send telegram message: {}", e);
                Err(anyhow::anyhow!("Failed to send telegram message: {}", e))
            }
        }
    }

    fn format_alert_message(&self, alert: &Alert) -> String {
        format!(
            "<b>{}</b>\n{}\nTimestamp: {}",
            format!("{:?}", alert.alert_type),
            alert.message,
            alert.timestamp
        )
    }
}
