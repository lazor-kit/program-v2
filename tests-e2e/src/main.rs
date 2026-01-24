mod common;
mod scenarios;

use anyhow::Result;
use common::TestContext;
use solana_sdk::signature::Signer;

#[tokio::main]
async fn main() -> Result<()> {
    println!("🚀 Starting LazorKit Mainnet Readiness Tests...");

    // 1. Initialize Context (Client, Payer, ProgramID)
    let ctx = TestContext::new()?;
    println!("Helper Context Initialized.");
    println!("RPC URL: {}", ctx.client.url());
    println!("Payer: {}", ctx.payer.pubkey());
    println!("Program ID: {}", ctx.program_id);

    // 2. Run Scenarios
    scenarios::happy_path::run(&ctx).await?;
    scenarios::failures::run(&ctx).await?;

    println!("\n🎉 All scenarios completed successfully!");
    Ok(())
}
