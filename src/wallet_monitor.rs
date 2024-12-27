use anyhow::{Result, anyhow};
use futures_util::{SinkExt, StreamExt};
use log::{error, info, debug};
use solana_sdk::{pubkey::Pubkey};
use std::{env, str::FromStr, collections::{HashMap, HashSet}, time::Duration, fmt, fmt::Write};
use tokio::{sync::Mutex, time::interval};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use url::Url;
use base64::{Engine as _, engine::general_purpose};
use serde_json::Value;
use bs58;
use std::sync::Arc;
use chrono::Local;
use crate::{alert_service::AlertService, models::AlertType};

const PUMP_PROGRAM_ID: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P"; // PUMP 程序
const MIN_HOLDING_AMOUNT: u64 = 10000; // 最小持仓数量
const SOL_DECIMALS: u32 = 9;  // SOL 的小数位数
const TOKEN_DECIMALS: u32 = 6; // SPL 代币的小数位数（大多数是6位）

// ANSI 转义序列
const CLEAR_SCREEN: &str = "\x1B[2J\x1B[1;1H";  // 清屏并移动光标到顶部
const BOLD: &str = "\x1B[1m";
const RESET: &str = "\x1B[0m";
const GREEN: &str = "\x1B[32m";
const RED: &str = "\x1B[31m";
const YELLOW: &str = "\x1B[33m";
const BLUE: &str = "\x1B[34m";
const CYAN: &str = "\x1B[36m";

fn format_f64(value: f64) -> String {
    if value < 0.000001 {
        format!("{:.9}", value)
    } else if value < 0.001 {
        format!("{:.6}", value)
    } else {
        format!("{:.3}", value)
    }
}

fn format_token_amount(raw_amount: u64) -> String {
    // 将原始数量转换为实际数量（考虑小数位）
    let actual_amount = (raw_amount as f64) / 10f64.powi(TOKEN_DECIMALS as i32);
    
    // 格式化数字，添加千位分隔符
    let amount_str = format!("{:.1}", actual_amount);
    let parts: Vec<&str> = amount_str.split('.').collect();
    
    let mut int_part = String::new();
    let mut count = 0;
    for c in parts[0].chars().rev() {
        if count > 0 && count % 3 == 0 {
            int_part.insert(0, ',');
        }
        int_part.insert(0, c);
        count += 1;
    }
    
    if parts.len() > 1 {
        format!("{}.{}", int_part, parts[1])
    } else {
        int_part
    }
}

fn format_price_change(change: i32) -> String {
    if change > 0 {
        format!("{}+{}%{}", GREEN, change, RESET)
    } else if change < 0 {
        format!("{}{}%{}", RED, change, RESET)
    } else {
        format!("{}%", change)
    }
}

fn truncate_address(address: &str, length: usize) -> String {
    if address.len() <= length {
        address.to_string()
    } else {
        format!("{}...", &address[..length - 3])
    }
}

fn format_number_with_commas(num: f64) -> String {
    let int_part = num.trunc() as i64;
    let frac_part = (num.fract() * 10.0).round() / 10.0;
    
    let mut result = String::new();
    let int_str = int_part.to_string();
    let len = int_str.len();
    
    for (i, c) in int_str.chars().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    
    if frac_part > 0.0 {
        write!(result, "{:.1}", frac_part).unwrap_or_default();
        // 移除前导0
        if result.ends_with(".0") {
            result.truncate(result.len() - 2);
        }
    }
    
    result
}

#[derive(Debug)]
struct TokenHolding {
    amount: u64,
    mint: String,
    total_cost: f64,    // 总花费的 SOL
    current_price: f64,  // 当前价格
}

impl TokenHolding {
    fn new(mint: String, amount: u64, price: f64) -> Self {
        let actual_amount = (amount as f64) / 10f64.powi(TOKEN_DECIMALS as i32);
        Self {
            amount,
            mint,
            total_cost: actual_amount * price,
            current_price: price,
        }
    }

    fn avg_price(&self) -> f64 {
        if self.amount == 0 {
            0.0
        } else {
            let actual_amount = (self.amount as f64) / 10f64.powi(TOKEN_DECIMALS as i32);
            self.total_cost / actual_amount
        }
    }

    fn price_change_percentage(&self) -> i32 {
        if self.avg_price() == 0.0 {
            0
        } else {
            ((self.current_price - self.avg_price()) / self.avg_price() * 100.0) as i32
        }
    }

    fn total_value(&self) -> f64 {
        let actual_amount = (self.amount as f64) / 10f64.powi(TOKEN_DECIMALS as i32);
        actual_amount * self.current_price
    }
}

impl fmt::Display for TokenHolding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TokenHolding {{ amount: {}, mint: {}, total_cost: {}, current_price: {} }}",
               self.amount, self.mint, self.total_cost, self.current_price)
    }
}

pub struct WalletMonitor {
    target_wallet: Pubkey,
    holdings: Arc<Mutex<HashMap<String, TokenHolding>>>,
    alerted_mints: Arc<Mutex<HashSet<String>>>,  // 记录已发送通知的代币
    alert_service: AlertService,
}

impl WalletMonitor {
    pub fn new(alert_service: AlertService) -> Result<Self> {
        let wallet_address = env::var("MONITOR_WALLET")?;
        
        info!("Attempting to parse target wallet address: {}", wallet_address);
        
        let wallet_pubkey = match Pubkey::from_str(&wallet_address) {
            Ok(pubkey) => {
                info!("Successfully parsed target wallet address");
                pubkey
            },
            Err(e) => {
                error!("Failed to parse wallet address '{}': {}", wallet_address, e);
                return Err(anyhow!("Invalid wallet address: {}", e));
            }
        };
        
        Ok(Self {
            target_wallet: wallet_pubkey,
            holdings: Arc::new(Mutex::new(HashMap::new())),
            alerted_mints: Arc::new(Mutex::new(HashSet::new())),
            alert_service,
        })
    }

    fn decode_program_data(&self, data_str: &str) -> Option<(String, String, bool, u64, u64)> {
        if let Ok(decoded_data) = general_purpose::STANDARD.decode(data_str) {
            if decoded_data.len() < 129 {
                return None;
            }

            // 跳过前8个字节的事件标识符
            let event_type = &decoded_data[..8];
            debug!("Event Type: {:02X?}", event_type);

            // 从第8个字节开始是mint地址 (32 bytes)
            let mint_bytes = &decoded_data[8..40];
            let mint = bs58::encode(mint_bytes).into_string();
            debug!("Mint: {}", mint);

            let mut pos = 40;

            // 读取 sol_amount (8 bytes)
            let sol_amount = {
                let mut bytes = [0u8; 8];
                bytes.copy_from_slice(&decoded_data[pos..pos + 8]);
                u64::from_le_bytes(bytes)
            };
            pos += 8;

            // 读取 token_amount (8 bytes)
            let token_amount = {
                let mut bytes = [0u8; 8];
                bytes.copy_from_slice(&decoded_data[pos..pos + 8]);
                u64::from_le_bytes(bytes)
            };
            pos += 8;

            // 读取 is_buy (1 byte)
            let is_buy = decoded_data[pos] != 0;
            pos += 1;

            // 读取 user 地址 (32 bytes)
            let user_bytes = &decoded_data[pos..pos + 32];
            let user = bs58::encode(user_bytes).into_string();
            debug!("User: {}", user);

            return Some((mint, user, is_buy, sol_amount, token_amount));
        }
        None
    }

    fn calculate_price(sol_amount: u64, token_amount: u64) -> f64 {
        if token_amount == 0 {
            return 0.0;
        }
        
        // 将 SOL 从 lamports 转换为实际的 SOL 数量
        let sol = (sol_amount as f64) / 10f64.powi(SOL_DECIMALS as i32);
        // 将代币数量转换为实际数量
        let tokens = (token_amount as f64) / 10f64.powi(TOKEN_DECIMALS as i32);
        
        // 计算每个代币的价格（SOL）
        sol / tokens
    }

    async fn check_and_send_alert(&self, mint: &str, holding: &TokenHolding, alerted_mints: &mut HashSet<String>) -> Result<()> {
        let price_change = holding.price_change_percentage();
        info!("Checking alert for {}: price change {}%", mint, price_change);
        
        if price_change > 100 {

            if !alerted_mints.contains(mint) {
                info!("Sending alert for {}: price change {}%", mint, price_change);
                
                // 构造通知消息
                let message = format!(
                    "🚀 Token Pump Alert!\n\n\
                    Token: <a href=\"https://gmgn.ai/sol/token/{}\">{}</a>\n\
                    Current Price: {:.9} SOL\n\
                    Avg Buy Price: {:.9} SOL",
                    mint, mint,
                    holding.current_price,
                    holding.avg_price()
                );

                // 发送通知
                match self.alert_service.send_alert(&message, AlertType::PriceAlert, Some(mint.to_string())).await {
                    Ok(_) => {
                        info!("Successfully sent alert for {}", mint);
                        // 记录已发送通知
                        alerted_mints.insert(mint.to_string());
                    },
                    Err(e) => {
                        error!("Failed to send alert for {}: {:?}", mint, e);
                        return Err(anyhow::anyhow!("Failed to send alert: {}", e));
                    }
                }
            } else {
                debug!("Alert already sent for {}", mint);
            }
        }
        Ok(())
    }

    async fn update_holdings(&self, mint: String, is_buy: bool, token_amount: u64, price: f64) {
        // 获取所有需要的锁
        let mut holdings = self.holdings.lock().await;
        let mut alerted_mints = self.alerted_mints.lock().await;
        
        if is_buy {
            // 买入，增加持仓
            let holding = holdings.entry(mint.clone()).or_insert_with(|| TokenHolding::new(mint.clone(), 0, price));
            
            // 更新总成本和数量
            let actual_amount = (token_amount as f64) / 10f64.powi(TOKEN_DECIMALS as i32);
            holding.total_cost += actual_amount * price;
            holding.amount = holding.amount.saturating_add(token_amount);
            holding.current_price = price;
            
            // 检查是否需要发送通知
            if let Err(e) = self.check_and_send_alert(&mint, holding, &mut alerted_mints).await {
                error!("Failed to send alert: {:?}", e);
            }
            
            info!("\n=== 🛍️  Buy Transaction ===");
            info!("{}", holding);
            info!("====================");
        } else {
            // 卖出，减少持仓
            if let Some(holding) = holdings.get_mut(&mint) {
                // 按比例减少总成本
                let sell_ratio = token_amount as f64 / holding.amount as f64;
                holding.total_cost *= (1.0 - sell_ratio);
                holding.amount = holding.amount.saturating_sub(token_amount);
                holding.current_price = price;
                
                // 检查是否需要发送通知
                if let Err(e) = self.check_and_send_alert(&mint, holding, &mut alerted_mints).await {
                    error!("Failed to send alert: {:?}", e);
                }
                
                info!("\n=== 💰 Sell Transaction ===");
                info!("{}", holding);
                info!("====================");
                
                // 检查是否清仓
                if holding.amount < MIN_HOLDING_AMOUNT {
                    info!("\n🔔 Position Closed 🔔");
                    info!("{}", holding);
                    info!("====================");
                    holdings.remove(&mint);
                    alerted_mints.remove(&mint);
                }
            }
        }
    }

    async fn update_price(&self, mint: &str, price: f64) {
        // 获取所有需要的锁
        let mut holdings = self.holdings.lock().await;
        let mut alerted_mints = self.alerted_mints.lock().await;
        
        // 如果价格为 0，跳过更新
        if price == 0.0 {
            debug!("Skipping price update with zero price for {}", mint);
            return;
        }
        
        // 如果持仓数量为 0，直接移除
        if let Some(holding) = holdings.get(mint) {
            let real_amount = holding.amount as f64 / 1e6;
            if real_amount < MIN_HOLDING_AMOUNT as f64 {
                info!("Removing token {} from holdings during price update (real_amount: {})", mint, format_number_with_commas(real_amount));
                holdings.remove(mint);
                alerted_mints.remove(mint);
                return;
            }
        }
        
        if let Some(holding) = holdings.get_mut(mint) {
            // 先克隆需要的数据
            let real_amount = holding.amount as f64 / 1e6;
            let holding_info = holding.to_string();
            
            holding.current_price = price;
            
            // 检查是否需要发送通知
            if let Err(e) = self.check_and_send_alert(mint, holding, &mut alerted_mints).await {
                error!("Failed to send alert: {:?}", e);
            }
            
            // 如果数量小于最小持仓量，从列表中移除
            if real_amount < MIN_HOLDING_AMOUNT as f64 {
                holdings.remove(mint);
                alerted_mints.remove(mint);
                
                info!("\n🔔 Position Closed (Price Update) 🔔");
                info!("{}", holding_info);
                info!("====================");
            } else {
                debug!("\n=== 📊 Price Update ===");
                debug!("{}", holding_info);
                debug!("====================");
            }
        }
    }

    async fn print_holdings(&self) {
        let mut holdings = self.holdings.lock().await;
        let mut alerted_mints = self.alerted_mints.lock().await;
        
        // 打印所有持仓的详细信息
        info!("\n=== Current Holdings Debug ===");
        for (mint, holding) in holdings.iter() {
            // SPL代币是6位小数
            let real_amount = holding.amount as f64 / 1e6;
            info!("Token {}: real_amount = {}, min_amount = {}", 
                  mint, format_number_with_commas(real_amount), MIN_HOLDING_AMOUNT);
        }
        info!("============================\n");
        
        // 清理数量为 0 的持仓，考虑小数位
        let to_remove: Vec<_> = holdings.iter()
            .filter(|(_, holding)| {
                let real_amount = holding.amount as f64 / 1e6;
                real_amount < MIN_HOLDING_AMOUNT as f64
            })
            .map(|(mint, holding)| {
                let real_amount = holding.amount as f64 / 1e6;
                info!("Will remove token {} from holdings (real_amount: {})", 
                     mint, format_number_with_commas(real_amount));
                mint.clone()
            })
            .collect();
        
        // 移除代币和对应的通知记录
        for mint in to_remove {
            info!("Actually removing mint: {}", mint);
            holdings.remove(&mint);
            alerted_mints.remove(&mint);
        }
        
        if !holdings.is_empty() {
            print!("{}", CLEAR_SCREEN);  // 清屏
            
            // 打印标题和时间
            let now = Local::now();
            println!("\n{}📊 Sol Pump Monitor Holdings{}", BOLD, RESET);
            println!("{}Last Update: {}{}\n", CYAN, now.format("%Y-%m-%d %H:%M:%S"), RESET);
            
            // 打印表头
            println!("╔══════════════════╦════════════════╦════════════════╦════════════════╦════════════╗");
            println!("║ {}{:^16}║ {:^14}║ {:^14}║ {:^14}║ {:^10}║{}",
                    BOLD, "Token", "Amount", "Avg Price", "Price", "Change", RESET);
            println!("╠══════════════════╬════════════════╬════════════════╬════════════════╬════════════╣");
            
            // 打印每个代币的信息
            for holding in holdings.values() {
                let price_change = holding.price_change_percentage();
                println!("║ {:16}║ {:>14}║ {:>14}║ {:>14}║ {:>10}║",
                    format!("{}{:16}{}", YELLOW, truncate_address(&holding.mint, 16), RESET),
                    format_token_amount(holding.amount),
                    format!("{} SOL", format_f64(holding.avg_price())),
                    format!("{} SOL", format_f64(holding.current_price)),
                    format_price_change(price_change)
                );
            }
            println!("╚══════════════════╩════════════════╩════════════════╩════════════════╩════════════╝");
            
            // 打印总计
            let total_value: f64 = holdings.values().map(|h| h.total_value()).sum();
            let total_cost: f64 = holdings.values().map(|h| h.total_cost).sum();
            let total_pnl = total_value - total_cost;
            let total_pnl_percentage = if total_cost > 0.0 { (total_pnl / total_cost * 100.0) as i32 } else { 0 };
            
            println!("\n{}Portfolio Summary:{}", BOLD, RESET);
            println!("Total Value: {} SOL", format_f64(total_value));
            println!("Total Cost:  {} SOL", format_f64(total_cost));
            println!("Total PnL:   {} SOL ({})", 
                    format_f64(total_pnl),
                    format_price_change(total_pnl_percentage));
        }
    }

    pub async fn start_monitoring(&mut self) -> Result<()> {
        // 启动持仓打印任务
        let holdings_clone = self.holdings.clone();
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(1));
            loop {
                interval.tick().await;
                let monitor = WalletMonitor {
                    target_wallet: Pubkey::from_str("ZDLFG5UNPzeNsEkacw9TdKHT1fBZCACfAQymjWnpcvg").unwrap(),
                    holdings: holdings_clone.clone(),
                    alerted_mints: Arc::new(Mutex::new(HashSet::new())),
                    alert_service: AlertService::new(
                        &env::var("TELEGRAM_BOT_TOKEN").expect("TELEGRAM_BOT_TOKEN must be set"),
                        env::var("TELEGRAM_CHAT_ID")
                            .expect("TELEGRAM_CHAT_ID must be set")
                            .parse()
                            .expect("TELEGRAM_CHAT_ID must be a valid integer"),
                        env::var("TELEGRAM_TOPIC_ID")
                            .ok()
                            .and_then(|id| id.parse::<i32>().ok()),
                        env::var("WS_ALERT_URL").ok()
                    ),
                };
                monitor.print_holdings().await;
            }
        });

        // 连接 Helius WebSocket
        let ws_url = format!(
            "wss://mainnet.helius-rpc.com/?api-key={}",
            env::var("HELIUS_API_KEY")?
        );
        let url = Url::parse(&ws_url)?;
        let (ws_stream, _) = connect_async(url).await?;
        let (mut write, mut read) = ws_stream.split();

        // 订阅 pump 程序的日志
        let subscribe_msg = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "logsSubscribe",
            "params": [
                {
                    "mentions": [PUMP_PROGRAM_ID]
                },
                {
                    "commitment": "confirmed",
                    "encoding": "jsonParsed"
                }
            ]
        });
        write.send(Message::Text(subscribe_msg.to_string())).await?;

        info!("Started monitoring PUMP program");

        // 处理 WebSocket 消息
        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    debug!("Received message: {}", text);
                    
                    if let Ok(json) = serde_json::from_str::<Value>(&text) {
                        // 跳过订阅确认消息
                        if json.get("id").is_some() {
                            debug!("Received subscription confirmation");
                            continue;
                        }

                        // 解析交易详情
                        if let Some(value) = json.get("params")
                            .and_then(|p| p.get("result"))
                            .and_then(|r| r.get("value")) 
                        {
                            // 获取交易签名
                            let signature = value.get("signature")
                                .and_then(|s| s.as_str())
                                .unwrap_or("unknown");
                            
                            let mut instruction_type = "Unknown";
                            let mut found_target_wallet = false;
                            let mut mint_address = String::new();
                            let mut price = 0.0;
                            let mut is_buy = false;
                            let mut token_amount = 0;
                            
                            // 检查日志
                            if let Some(logs) = value.get("logs").and_then(|l| l.as_array()) {
                                for log in logs {
                                    if let Some(log_str) = log.as_str() {
                                        debug!("Log: {}", log_str);
                                        
                                        // 检查指令类型
                                        if log_str.contains("Instruction: ") {
                                            instruction_type = log_str.split("Instruction: ").nth(1).unwrap_or("Unknown");
                                        }
                                        
                                        // 解析 Program data
                                        if log_str.contains("Program data: ") {
                                            if let Some(data_str) = log_str.split("Program data: ").nth(1) {
                                                if let Some((mint, user, trade_is_buy, sol_amount, trade_token_amount)) = self.decode_program_data(data_str) {
                                                    debug!("Decoded user: {}, is_buy: {}", user, trade_is_buy);
                                                    
                                                    // 计算价格
                                                    let trade_price = Self::calculate_price(sol_amount, trade_token_amount);
                                                    
                                                    // 如果是目标钱包的交易
                                                    if user == self.target_wallet.to_string() {
                                                        found_target_wallet = true;
                                                        mint_address = mint;
                                                        is_buy = trade_is_buy;
                                                        token_amount = trade_token_amount;
                                                        price = trade_price;
                                                    } else {
                                                        // 如果不是目标钱包的交易，检查是否需要更新价格
                                                        let holdings = self.holdings.lock().await;
                                                        if holdings.contains_key(&mint) {
                                                            drop(holdings); // 释放锁
                                                            self.update_price(&mint, trade_price).await;
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            
                            // 只有在找到目标钱包时才更新和打印信息
                            if found_target_wallet {
                                // 更新持仓信息
                                self.update_holdings(mint_address.clone(), is_buy, token_amount, price).await;

                                debug!("Found interaction with target wallet!");
                                debug!("Transaction: https://solscan.io/tx/{}", signature);
                                debug!("Instruction Type: {}", instruction_type);
                                debug!("Mint: {}", mint_address);
                                debug!("Action: {}", if is_buy { "Buy" } else { "Sell" });
                                debug!("Amount: {} tokens", token_amount);
                                debug!("Price: {} SOL/token", price);
                                debug!("-----------------------------------");
                            }
                        }
                    }
                }
                Ok(Message::Binary(data)) => {
                    debug!("Received binary message of {} bytes", data.len());
                }
                Ok(Message::Ping(_)) => {
                    debug!("Received ping");
                    write.send(Message::Pong(vec![])).await?;
                }
                Ok(Message::Pong(_)) => {
                    debug!("Received pong");
                }
                Ok(Message::Close(_)) => {
                    info!("WebSocket closed");
                    break;
                }
                Ok(Message::Frame(_)) => {
                    debug!("Received frame message");
                }
                Err(e) => {
                    error!("WebSocket error: {:?}", e);
                    break;
                }
            }
        }

        Ok(())
    }
}
