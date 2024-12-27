use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TokenPosition {
    pub mint: String,
    pub amount: f64,
    pub price: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HeliusMessage {
    pub json_rpc: Option<String>,
    pub method: Option<String>,
    #[serde(rename = "params")]
    pub params: Option<HeliusParams>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HeliusParams {
    #[serde(rename = "result")]
    pub result: HeliusResult,
    pub subscription: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HeliusResult {
    pub context: Context,
    pub value: AccountValue,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Context {
    pub slot: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AccountValue {
    pub lamports: u64,
    pub data: Value,
    pub owner: String,
    pub executable: bool,
    #[serde(rename = "rentEpoch")]
    pub rent_epoch: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenTransfer {
    pub mint: String,
    pub amount: f64,
    pub decimals: u8,
    pub from_user_account: String,
    pub to_user_account: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlertType {
    PriceAlert,
    Error,
}

#[derive(Debug, Clone)]
pub struct Alert {
    pub message: String,
    pub alert_type: AlertType,
    pub timestamp: i64,
}

#[derive(Debug, Serialize)]
pub struct TradeSignal {
    pub signal: String,
    pub mint: String,
    pub timestamp: i64,
}
