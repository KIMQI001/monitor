use anyhow::Result;
use chrono::Utc;
use futures_util::{sink::SinkExt, StreamExt};
use log::{error, info, warn};
use std::env;
use std::sync::Arc;
use teloxide::{
    prelude::*,
    types::{ChatId, MessageId, ParseMode},
};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use url::Url;
use crate::models::{Alert, AlertType};

pub struct AlertService {
    bot: Bot,
    chat_id: i64,
    topic_id: Option<i32>,
    ws_url: Option<String>,
    ws_sender: Option<Arc<tokio::sync::Mutex<futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>
        >,
        Message
    >>>>,
}

impl AlertService {
    pub async fn new(bot_token: &str, chat_id: i64, topic_id: Option<i32>, ws_url: Option<String>) -> Result<Self> {
        let mut service = Self {
            bot: Bot::new(bot_token),
            chat_id,
            topic_id,
            ws_url,
            ws_sender: None,
        };

        // 如果提供了 WebSocket URL，则初始化连接
        if let Some(url) = &service.ws_url {
            service.init_ws().await?;
        }

        Ok(service)
    }

    async fn init_ws(&mut self) -> Result<()> {
        if let Some(ws_url) = &self.ws_url {
            let url = Url::parse(ws_url)?;
            let (ws_stream, _) = connect_async(url).await?;
            let (sender, _) = ws_stream.split();
            self.ws_sender = Some(Arc::new(tokio::sync::Mutex::new(sender)));
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

#[derive(Clone)]
pub struct AlertService {
    pub bot: Bot,
    pub chat_id: i64,
    pub topic_id: i32,
    ws_url: Option<String>,
}

impl AlertService {
    pub fn new(bot_token: &str, chat_id: i64) -> Self {
        let topic_id = env::var("TELEGRAM_TOPIC_ID")
            .expect("TELEGRAM_TOPIC_ID must be set")
            .parse::<i32>()
            .expect("TELEGRAM_TOPIC_ID must be a valid integer");

        Self {
            bot: Bot::new(bot_token),
            chat_id,
            topic_id,
            ws_url: match env::var("WS_ALERT_URL") {
                Ok(url) => {
                    info!("Found WebSocket URL: {}", url);
                    Some(url)
                }
                Err(e) => {
                    warn!("WebSocket URL not found: {:?}", e);
                    None
                }
            },
        }
    }

    pub async fn send_alert(&self, message: &str, alert_type: AlertType) -> Result<()> {
        let alert = Alert {
            message: message.to_string(),
            alert_type,
            timestamp: Utc::now().timestamp(),
        };

        // 发送到 Telegram
        match self.send_to_telegram(&self.format_alert_message(&alert)).await {
            Ok(_) => {
                info!("Successfully sent alert to Telegram");
                Ok(())
            }
            Err(e) => {
                let err = "Failed to send alert to Telegram";
                error!("{}: {:?}", err, e);
                Err(anyhow::anyhow!(err))
            }
        }
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

    async fn send_to_telegram(&self, message: &str) -> Result<()> {
        let chat_id = ChatId(self.chat_id);
        
        match self.bot.send_message(chat_id, message)
            .message_thread_id(self.topic_id)  
            .parse_mode(ParseMode::Html)
            .await {
            Ok(sent_message) => {
                info!("Successfully sent message to Telegram. Message ID: {}", sent_message.id);
                info!("Chat ID used: {}", chat_id.0);
                info!("Topic ID used: {}", self.topic_id);
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
