use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = url::Url::parse("ws://3.87.175.186:7000/ws?type=signal")?;
    
    println!("Connecting to {}", url);
    
    let (ws_stream, _) = connect_async(url).await?;
    println!("WebSocket handshake has been successfully completed");
    
    let (mut _write, mut read) = ws_stream.split();
    
    while let Some(message) = read.next().await {
        match message {
            Ok(msg) => {
                if msg.is_text() || msg.is_binary() {
                    println!("Received: {}", msg);
                }
            }
            Err(e) => println!("Error receiving message: {}", e),
        }
    }
    
    Ok(())
}
