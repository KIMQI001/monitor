use anyhow::Result;
use chrono::Local;
use env_logger::{Builder, Target};
use log::{info, LevelFilter};
use std::fs::OpenOptions;
use std::io::Write;
use tokio;

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

    let mut monitor = wallet_monitor::WalletMonitor::new()?;
    monitor.start_monitoring().await?;

    Ok(())
}
