use anyhow::Result;
use chrono::Utc;
use futures_util::SinkExt;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use serde::{Serialize, Deserialize};
use url::Url;

#[derive(Debug, Serialize, Deserialize)]
struct TradeSignal {
    signal: String,
    mint: String,
    timestamp: i64,
}

#[tokio::main]
async fn main() -> Result<()> {
    let url = Url::parse("ws://3.87.175.186:7000/ws?type=signal")?;
    println!("Connecting to {}", url);

    let (mut ws_stream, response) = connect_async(url).await?;
    println!("WebSocket handshake completed. Status: {}", response.status());
    println!("Headers: {:?}", response.headers());

    let signal = TradeSignal {
        signal: "sniper_pump1".to_string(),
        mint: "66BEASEApHs5LMFoQV8LTEZUavBKNSbgBy3TRpD9pump".to_string(),
        timestamp: Utc::now().timestamp(),
    };

    let message = serde_json::to_string(&signal)?;
    println!("Sending message: {}", message);
    
    ws_stream.send(Message::Text(message)).await?;
    println!("Message sent successfully");

    // 等待一会儿确保消息发送完成
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    println!("Test completed");

    Ok(())
}
