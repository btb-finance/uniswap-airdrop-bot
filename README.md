# 🚀 Uniswap V3 Airdrop Bot

A Rust-based bot that automatically monitors Uniswap V3 liquidity provision events on Arbitrum and sends airdrop tokens to liquidity providers.

## 🌟 Features

- 🔍 Monitors Uniswap V3 `IncreaseLiquidity` events in real-time
- 💧 Automatically sends airdrop tokens to liquidity providers
- 🔒 Prevents duplicate airdrops through persistent state management
- ⛽ Dynamic gas price adjustment for reliable transactions
- 🔗 Built specifically for Arbitrum network

## 🛠️ Setup

### Prerequisites

- Rust and Cargo installed
- An Alchemy API key for Arbitrum
- A wallet with ETH for gas fees and tokens for airdrops

### Environment Variables

Create a `.env` file in the project root with the following variables:

```env
ALCHEMY_API_KEY=your_alchemy_websocket_url
UNISWAP_NFT_POSITION_MANAGER=0xC36442b4a4522E871399CD717aBDD847Ab11FE88
AIRDROP_TOKEN_ADDRESS=your_airdrop_token_address
PRIVATE_KEY=your_wallet_private_key
```

### Installation

1. Clone the repository:
```bash
git clone https://github.com/your-username/uniswap-airdrop-bot.git
cd uniswap-airdrop-bot
```

2. Install dependencies:
```bash
cargo build
```

3. Run the bot:
```bash
cargo run
```

## 🏗️ Architecture

The bot consists of several key components:

- Event monitoring system for Uniswap V3 liquidity events
- Airdrop distribution mechanism with gas optimization
- Persistent state management using JSON storage
- Error handling and logging system

## 📝 State Management

The bot maintains a `airdrop_state.json` file that tracks:
- Recipient addresses
- Airdrop amounts
- Transaction hashes
- Timestamps

This prevents duplicate airdrops and maintains a record of all distributions.

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## 🔗 Links

- Twitter: [@BTB_finance](https://twitter.com/BTB_finance)
- Telegram: [@BTBFinance](https://t.me/BTBFinance)
- Team: BTB Finance

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
