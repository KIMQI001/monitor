use anyhow::Result;
use chrono::Local;
use env_logger::{Builder, Target};
use log::{info, LevelFilter};
use std::fs::OpenOptions;
use std::io::Write;
use tokio;

use crate::models::AlertType;
mod wallet_monitor;
mod alert_service;
mod models;

#[tokio::main]
async fn main() -> Result<()> {
    // 加载 .env 文件
    dotenv::dotenv().ok();

    // 设置日志输出到文件
    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("monitor.log")?;
        
    Builder::new()
        .target(Target::Pipe(Box::new(log_file)))
        .format(|buf, record| {
            writeln!(
                buf,
                "{} [{}] {}",
                Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.level(),
                record.args()
            )
        })
        .filter(None, LevelFilter::Info)
        .init();

    info!("Starting PUMP program monitor...");

    let bot_token = std::env::var("TELEGRAM_BOT_TOKEN")
        .expect("TELEGRAM_BOT_TOKEN must be set");
    let chat_id = std::env::var("TELEGRAM_CHAT_ID")
        .expect("TELEGRAM_CHAT_ID must be set")
        .parse::<i64>()
        .expect("TELEGRAM_CHAT_ID must be a valid integer");
    let topic_id = std::env::var("TELEGRAM_TOPIC_ID")
        .ok()
        .and_then(|id| id.parse::<i32>().ok());
    let ws_url = std::env::var("WS_ALERT_URL").ok();

    let alert_service = alert_service::AlertService::new(
        &bot_token,
        chat_id,
        topic_id,
        ws_url
    );
    
    let mut monitor = wallet_monitor::WalletMonitor::new(alert_service)?;
    monitor.start_monitoring().await?;

    Ok(())
}
