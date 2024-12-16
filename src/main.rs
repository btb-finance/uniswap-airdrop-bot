use dotenv::dotenv;
use ethers::{
    contract::abigen,
    middleware::SignerMiddleware,
    prelude::*,
    providers::{Provider, Ws},
    signers::{LocalWallet, Signer},
    types::U256,
};
use eyre::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::env;
use chrono::{DateTime, Utc};

// Generate type-safe bindings for the Uniswap NFT Position Manager contract
abigen!(
    INonfungiblePositionManager,
    r#"[
        event IncreaseLiquidity(uint256 indexed tokenId, uint128 liquidity, uint256 amount0, uint256 amount1)
        function ownerOf(uint256 tokenId) external view returns (address)
        function positions(uint256 tokenId) external view returns (uint96 nonce, address operator, address token0, address token1, uint24 fee, int24 tickLower, int24 tickUpper, uint128 liquidity, uint256 feeGrowthInside0LastX128, uint256 feeGrowthInside1LastX128, uint128 tokensOwed0, uint128 tokensOwed1)
    ]"#
);

// Generate type-safe bindings for ERC20 token
abigen!(
    IERC20,
    r#"[
        function transfer(address recipient, uint256 amount) external returns (bool)
        function balanceOf(address account) external view returns (uint256)
    ]"#
);

#[derive(Debug, Serialize, Deserialize)]
struct AirdropRecord {
    address: String,
    timestamp: DateTime<Utc>,
    amount: String,
    tx_hash: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct AirdropState {
    recipients: HashMap<String, AirdropRecord>,
}

impl AirdropState {
    fn load() -> Self {
        let path = Path::new("airdrop_state.json");
        if path.exists() {
            let data = fs::read_to_string(path).expect("Failed to read state file");
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    fn save(&self) -> Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        fs::write("airdrop_state.json", json)?;
        Ok(())
    }

    fn has_received_airdrop(&self, address: &str) -> bool {
        self.recipients.contains_key(address)
    }

    fn record_airdrop(&mut self, address: String, amount: String, tx_hash: String) {
        let record = AirdropRecord {
            address: address.clone(),
            timestamp: Utc::now(),
            amount,
            tx_hash,
        };
        self.recipients.insert(address, record);
        if let Err(e) = self.save() {
            println!("⚠️ Failed to save airdrop state: {:?}", e);
        }
    }
}

async fn is_gas_price_acceptable(provider: &Provider<Ws>) -> Result<bool> {
    let gas_price = provider.get_gas_price().await?;
    // Convert to Gwei for easier comparison (1 Gwei = 10^9 wei)
    let gas_price_gwei = gas_price / U256::exp10(9);
    println!("Current gas price: {} gwei", gas_price_gwei);
    
    // Only proceed if gas price is below 0.1 gwei (0.1 * 10^9 wei)
    let max_gas_price = U256::from(100_000_000); // 0.1 gwei in wei
    Ok(gas_price <= max_gas_price)
}

async fn send_airdrop(
    token: &IERC20<SignerMiddleware<Provider<Ws>, LocalWallet>>,
    recipient: Address,
    amount: U256,
    gas_price: U256,
) -> Result<H256> {
    // Build the transaction
    let tx_call = token.transfer(recipient, amount);
    let tx = tx_call
        .gas(U256::from(500000))
        .gas_price(gas_price);

    // Send the transaction
    let pending_tx = tx.send().await?;
    Ok(pending_tx.tx_hash())
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();

    // Load airdrop state
    let mut airdrop_state = AirdropState::load();
    println!("📝 Loaded airdrop state with {} previous recipients", airdrop_state.recipients.len());

    // Connect to Arbitrum network
    let ws_url = env::var("ALCHEMY_API_KEY").expect("ALCHEMY_API_KEY must be set");
    let provider = Provider::<Ws>::connect(ws_url).await?;
    
    // Set up wallet
    let private_key = env::var("PRIVATE_KEY").expect("PRIVATE_KEY must be set");
    let wallet = private_key.parse::<LocalWallet>()?.with_chain_id(42161u64);
    let client = SignerMiddleware::new(provider.clone(), wallet.clone());
    let client = Arc::new(client);

    // Contract addresses
    let nft_manager_address: Address = env::var("UNISWAP_NFT_POSITION_MANAGER")
        .expect("UNISWAP_NFT_POSITION_MANAGER must be set")
        .parse()?;
    let airdrop_token_address: Address = env::var("AIRDROP_TOKEN_ADDRESS")
        .expect("AIRDROP_TOKEN_ADDRESS must be set")
        .parse()?;

    // Create contract instances
    let nft_manager = INonfungiblePositionManager::new(nft_manager_address, client.clone());
    let airdrop_token = IERC20::new(airdrop_token_address, client.clone());

    // Listen for IncreaseLiquidity events
    let event = nft_manager.event::<IncreaseLiquidityFilter>();
    let mut stream = event.stream().await?;

    println!("🎯 Monitoring for new liquidity provisions...");
    println!("⛽ Will only send transactions when gas price is below 0.1 gwei");

    while let Some(Ok(event)) = stream.next().await {
        println!("🔥 New liquidity added!");
        println!("Token ID: {}", event.token_id);
        println!("Liquidity Amount: {}", event.liquidity);

        // Get the owner of the NFT position
        match nft_manager.owner_of(event.token_id).call().await {
            Ok(owner) => {
                let owner_str = format!("{:?}", owner);
                println!("Position Owner: {}", owner_str);

                // Check if this address has already received an airdrop
                if airdrop_state.has_received_airdrop(&owner_str) {
                    println!("⏭️ Address {} has already received an airdrop, skipping...", owner_str);
                    continue;
                }
                
                // Send airdrop (100 tokens with 18 decimals)
                let amount = U256::from(100_000_000_000_000_000_000u128);

                // Check gas price and wait if it's too high
                let mut attempts = 0;
                while attempts < 10 {
                    if is_gas_price_acceptable(&provider).await? {
                        let gas_price = provider.get_gas_price().await?;
                        
                        // Send the airdrop
                        match send_airdrop(&airdrop_token, owner, amount, gas_price).await {
                            Ok(tx_hash) => {
                                println!("✅ Airdrop sent to {}! Transaction: {:?}", owner_str, tx_hash);
                                
                                // Record the airdrop
                                airdrop_state.record_airdrop(
                                    owner_str,
                                    amount.to_string(),
                                    format!("{:?}", tx_hash),
                                );
                                break;
                            }
                            Err(e) => {
                                println!("❌ Failed to send airdrop: {:?}", e);
                                println!("💡 Make sure you have enough ETH in your wallet for gas fees!");
                            }
                        }
                    } else {
                        println!("⏳ Gas price too high, waiting 5 minutes before retrying...");
                        tokio::time::sleep(tokio::time::Duration::from_secs(300)).await;
                        attempts += 1;
                    }
                }
                
                if attempts >= 10 {
                    println!("⚠️ Could not find acceptable gas price after 10 attempts, skipping this airdrop");
                }
            }
            Err(e) => {
                println!("❌ Failed to get position owner: {:?}", e);
            }
        }
    }

    Ok(())
}
