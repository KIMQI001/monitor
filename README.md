# Solana Raydium V4 Wallet Monitor

A Rust application that monitors a specified wallet's interactions with the Raydium V4 program on the Solana blockchain. The application tracks token holdings, calculates price changes, and sends alerts via Telegram when significant price movements occur.

## Features

- Real-time monitoring of wallet interactions with Raydium V4
- Automatic tracking of token holdings and price changes
- Telegram alerts for significant price movements (> 5%)
- Detailed logging with both console and file output
- Support for both buy and sell transactions
- Automatic removal of small holdings

## Configuration

The application requires the following environment variables:

```env
RPC_URL=<Helius RPC URL>
MONITOR_WALLET=<Wallet address to monitor>
HELIUS_API_KEY=<Helius API key>
TELEGRAM_BOT_TOKEN=<Telegram bot token>
TELEGRAM_CHAT_ID=<Telegram chat ID>
```

## Running the Application

1. Install Rust and Cargo
2. Clone the repository
3. Create a `.env` file with the required configuration
4. Run the application:

```bash
cargo run
```

## Logging

The application logs all activities to both the console and a `monitor.log` file. The log includes:
- Token holdings and their values
- Price changes and alerts
- Transaction details
- Error messages and debugging information

## Dependencies

- Solana SDK
- Tokio for async runtime
- Teloxide for Telegram integration
- env_logger for logging
- dotenv for environment variable management

## License

MIT License
