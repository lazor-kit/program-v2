use crate::common::{
    add_authority_with_role_permission, create_lazorkit_wallet, setup_test_context,
};
use lazorkit_v2_state::role_permission::RolePermission;
use solana_sdk::message::VersionedMessage;
use solana_sdk::native_token::LAMPORTS_PER_SOL;
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk::system_instruction;
use solana_sdk::transaction::VersionedTransaction;

mod common;

#[test_log::test]
fn test_multi_authority_basic() -> anyhow::Result<()> {
    let mut context = setup_test_context()?;
    let wallet_id = rand::random::<[u8; 32]>();
    let (wallet_account, wallet_vault, root_authority_keypair) =
        create_lazorkit_wallet(&mut context, wallet_id)?;

    // Airdrop to vault
    context
        .svm
        .airdrop(&wallet_vault, 10 * LAMPORTS_PER_SOL)
        .map_err(|e| anyhow::anyhow!("Failed to airdrop: {:?}", e))?;

    // Add Authority A
    let authority_a = Keypair::new();
    add_authority_with_role_permission(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &authority_a,
        0,
        &root_authority_keypair,
        RolePermission::All,
    )?;

    // Add Authority B
    let authority_b = Keypair::new();
    add_authority_with_role_permission(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &authority_b,
        0,
        &root_authority_keypair,
        RolePermission::All,
    )?;

    // Test Authority A execution
    let recipient = Keypair::new();
    let transfer_amount = LAMPORTS_PER_SOL;
    let inner_ix =
        system_instruction::transfer(&wallet_vault, &recipient.pubkey(), transfer_amount);

    // Create Sign instruction for Authority A (assumed ID 1)
    use crate::common::create_sign_instruction_ed25519;
    let sign_ix = create_sign_instruction_ed25519(
        &wallet_account,
        &wallet_vault,
        &authority_a,
        1, // Authority A ID
        inner_ix,
    )?;

    let message = solana_sdk::message::v0::Message::try_compile(
        &context.default_payer.pubkey(),
        &[sign_ix],
        &[],
        context.svm.latest_blockhash(),
    )?;

    context
        .svm
        .send_transaction(VersionedTransaction::try_new(
            VersionedMessage::V0(message),
            &[context.default_payer.insecure_clone(), authority_a],
        )?)
        .map_err(|e| anyhow::anyhow!("Failed to send transaction: {:?}", e))?;

    Ok(())
}
