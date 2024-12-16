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

async fn get_minimum_gas_price(provider: &Provider<Ws>) -> Result<U256> {
    // Get the current base fee
    let block = provider.get_block(BlockNumber::Latest).await?.unwrap();
    let base_fee = block.base_fee_per_gas.unwrap_or_default();
    
    // Add 1% to base fee to ensure it passes
    // This is still extremely low but will work
    let gas_price = base_fee + (base_fee / 100);
    
    println!("📊 Current base fee: {} gwei", base_fee / U256::exp10(9));
    println!("📊 Using gas price: {} gwei", gas_price / U256::exp10(9));
    
    Ok(gas_price)
}

async fn estimate_minimum_gas(
    token: &IERC20<SignerMiddleware<Provider<Ws>, LocalWallet>>,
    recipient: Address,
    amount: U256,
) -> Result<U256> {
    // Get the exact gas estimate
    let gas_estimate = token
        .transfer(recipient, amount)
        .estimate_gas()
        .await?;
    
    // Add just 2% buffer - on Arbitrum this is usually enough
    Ok(gas_estimate + (gas_estimate / 50))
}

async fn send_airdrop(
    token: &IERC20<SignerMiddleware<Provider<Ws>, LocalWallet>>,
    recipient: Address,
    amount: U256,
    gas_price: U256,
    gas_limit: U256,
) -> Result<H256> {
    let tx_call = token.transfer(recipient, amount);
    
    // Use legacy transaction type which often uses less gas
    let tx = tx_call
        .gas(gas_limit)
        .gas_price(gas_price)
        .legacy();

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
    println!("⛽ Using absolute minimum gas (0.001 gwei) for Arbitrum");

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

                // Get the minimum viable gas price and limit
                let gas_price = get_minimum_gas_price(&provider).await?;
                let gas_limit = estimate_minimum_gas(&airdrop_token, owner, amount).await?;
                
                println!("💡 Using minimum gas price: {} gwei", gas_price / U256::exp10(9));
                println!("💡 Estimated gas limit: {}", gas_limit);

                // Send the airdrop with minimum viable gas
                match send_airdrop(&airdrop_token, owner, amount, gas_price, gas_limit).await {
                    Ok(tx_hash) => {
                        println!("✅ Airdrop sent to {}! Transaction: {:?}", owner_str, tx_hash);
                        println!("💰 Used gas price: {} gwei", gas_price / U256::exp10(9));
                        println!("⚡ Used gas limit: {}", gas_limit);
                        
                        // Record the airdrop
                        airdrop_state.record_airdrop(
                            owner_str,
                            amount.to_string(),
                            format!("{:?}", tx_hash),
                        );
                    }
                    Err(e) => {
                        println!("❌ Failed to send airdrop: {:?}", e);
                        println!("💡 Make sure you have enough ETH in your wallet for gas fees!");
                    }
                }
            }
            Err(e) => {
                println!("❌ Failed to get position owner: {:?}", e);
            }
        }
    }

    Ok(())
}
