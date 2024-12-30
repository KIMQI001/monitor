use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::protocol::Message;

type Tx = futures_util::stream::SplitSink<tokio_tungstenite::WebSocketStream<TcpStream>, Message>;
type PeerMap = Arc<Mutex<HashMap<SocketAddr, Tx>>>;

async fn handle_connection(peer_map: PeerMap, raw_stream: TcpStream, addr: SocketAddr) {
    println!("Incoming TCP connection from: {}", addr);

    let ws_stream = tokio_tungstenite::accept_async(raw_stream)
        .await
        .expect("Error during the websocket handshake occurred");
    println!("WebSocket connection established: {}", addr);

    let (tx, mut rx) = ws_stream.split();
    peer_map.lock().await.insert(addr, tx);

    // 处理接收到的消息
    while let Some(msg) = rx.next().await {
        match msg {
            Ok(msg) => {
                println!("Received a message from {}: {}", addr, msg);
                let peers = peer_map.lock().await;
                
                // 广播消息给所有其他客户端
                for (peer_addr, tx) in peers.iter() {
                    if *peer_addr != addr {
                        if let Err(e) = tx.clone().send(msg.clone()).await {
                            println!("Error sending message to {}: {}", peer_addr, e);
                        }
                    }
                }
            }
            Err(e) => {
                println!("Error receiving message from {}: {}", addr, e);
                break;
            }
        }
    }

    // 客户端断开连接时，从列表中移除
    peer_map.lock().await.remove(&addr);
    println!("{} disconnected", addr);
}

#[tokio::main]
async fn main() {
    let addr = "0.0.0.0:9898";
    let listener = TcpListener::bind(&addr).await.expect("Can't listen");
    println!("Listening on: {}", addr);

    let peer_map = PeerMap::new(Mutex::new(HashMap::new()));

    while let Ok((stream, addr)) = listener.accept().await {
        let peer_map = peer_map.clone();
        tokio::spawn(handle_connection(peer_map, stream, addr));
    }
}
