use crate::common::{TestContext, ToAddress};
use anyhow::Result;
use solana_instruction::{AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_message::Message;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use solana_system_program;
use solana_sysvar;
use solana_transaction::Transaction;

pub fn run(ctx: &mut TestContext) -> Result<()> {
    println!("\n🛡️ Running DoS Attack Mitigation Scenario...");

    // Setup: Prepare wallet creation args
    let user_seed = rand::random::<[u8; 32]>();
    let owner_keypair = Keypair::new();

    // 1. Calculate PDA addresses
    let (wallet_pda, _wallet_bump) =
        Pubkey::find_program_address(&[b"wallet", &user_seed], &ctx.program_id);
    let (vault_pda, _) =
        Pubkey::find_program_address(&[b"vault", wallet_pda.as_ref()], &ctx.program_id);
    let (auth_pda, auth_bump) = Pubkey::find_program_address(
        &[
            b"authority",
            wallet_pda.as_ref(),
            Signer::pubkey(&owner_keypair).as_ref(),
        ],
        &ctx.program_id,
    );

    println!("Target Wallet PDA: {}", wallet_pda);

    // 2. DoS Attack: Pre-fund the wallet PDA
    // Using 1 lamport to trigger the create_account error in vulnerable version
    println!("🔫 Attacker pre-funds Wallet PDA with 1 lamport...");

    // Manual transfer instruction (discriminator 2)
    let amount = 1u64;
    let mut transfer_data = Vec::new();
    transfer_data.extend_from_slice(&2u32.to_le_bytes());
    transfer_data.extend_from_slice(&amount.to_le_bytes());

    let fund_ix = Instruction {
        program_id: solana_system_program::id().to_address(),
        accounts: vec![
            AccountMeta::new(Signer::pubkey(&ctx.payer).to_address(), true),
            AccountMeta::new(wallet_pda.to_address(), false),
        ],
        data: transfer_data,
    };

    let msg = Message::new(&[fund_ix], Some(&Signer::pubkey(&ctx.payer).to_address()));
    let mut fund_tx = Transaction::new_unsigned(msg);
    fund_tx.sign(&[&ctx.payer], ctx.svm.latest_blockhash());

    ctx.execute_tx(fund_tx)?;
    println!("✅ Wallet PDA pre-funded.");

    // Verify balance
    let account = ctx.svm.get_account(&wallet_pda.to_address()).unwrap();
    assert_eq!(account.lamports, 1);

    // 3. Attempt to Create Wallet (Should succeed now)
    println!("🛡️ Victim attempts to create wallet...");

    let mut data = vec![0]; // CreateWallet discriminator
    data.extend_from_slice(&user_seed);
    data.push(0); // Ed25519
    data.push(auth_bump);
    data.extend_from_slice(&[0; 6]); // Padding
    data.extend_from_slice(Signer::pubkey(&owner_keypair).as_ref());

    let create_ix = Instruction {
        program_id: ctx.program_id.to_address(),
        accounts: vec![
            AccountMeta::new(Signer::pubkey(&ctx.payer).to_address(), true),
            AccountMeta::new(wallet_pda.to_address(), false),
            AccountMeta::new(vault_pda.to_address(), false),
            AccountMeta::new(auth_pda.to_address(), false),
            AccountMeta::new_readonly(solana_system_program::id().to_address(), false),
            AccountMeta::new_readonly(solana_sysvar::rent::ID.to_address(), false),
        ],
        data,
    };

    let msg = Message::new(&[create_ix], Some(&Signer::pubkey(&ctx.payer).to_address()));
    let mut create_tx = Transaction::new_unsigned(msg);
    create_tx.sign(&[&ctx.payer], ctx.svm.latest_blockhash());

    // This should succeed with the fix (would fail without it)
    ctx.execute_tx(create_tx)?;
    println!("✅ Wallet creation SUCCESS (DoS mitigated).");

    Ok(())
}
