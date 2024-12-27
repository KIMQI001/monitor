use anyhow::Result;
use dotenv::dotenv;
use log::info;
use std::env;
use teloxide::{prelude::*, types::{ChatId, ParseMode}};

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志和环境变量
    env_logger::init();
    dotenv()?;

    // 获取配置
    let bot_token = env::var("TELEGRAM_BOT_TOKEN")
        .expect("TELEGRAM_BOT_TOKEN must be set");
    let chat_id = env::var("TELEGRAM_CHAT_ID")
        .expect("TELEGRAM_CHAT_ID must be set")
        .parse::<i64>()
        .expect("TELEGRAM_CHAT_ID must be a valid integer");
    let topic_id = env::var("TELEGRAM_TOPIC_ID")
        .expect("TELEGRAM_TOPIC_ID must be set")
        .parse::<i32>()
        .expect("TELEGRAM_TOPIC_ID must be a valid integer");

    info!("Starting Telegram test with token: {}, chat_id: {}, topic_id: {}", bot_token, chat_id, topic_id);

    // 创建bot
    let bot = Bot::new(bot_token);
    
    // 发送测试消息
    let message = format!(
        "<b>Test Message</b>\n\
        This is a test message sent at {}\n\
        With some <i>formatted</i> text",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    );

    info!("Sending message: {}", message);

    // 发送消息
    match bot.send_message(ChatId(chat_id), message)
        .message_thread_id(topic_id)
        .parse_mode(ParseMode::Html)
        .await {
        Ok(message) => {
            info!("Successfully sent message. Message ID: {}", message.id);
            info!("Chat ID used: {}", chat_id);
            info!("Topic ID used: {}", topic_id);
        }
        Err(e) => {
            info!("Failed to send message: {}", e);
        }
    }

    Ok(())
}
