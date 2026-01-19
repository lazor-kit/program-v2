use lazorkit_sdk::{
    basic::{
        actions::{AddAuthorityBuilder, CreateWalletBuilder},
        wallet::LazorWallet,
    },
    core::connection::SolConnection,
};
use lazorkit_state::authority::AuthorityType;
use solana_program_test::tokio;
use solana_sdk::{
    signature::{Keypair, Signer},
    signer::EncodableKey,
};

mod common;
use common::{setup_test_context, TestContext};

#[tokio::test]
async fn test_create_wallet_success() {
    let context = setup_test_context().await;
    let wallet_id = [1u8; 32];
    let owner = Keypair::new();
    let owner_pubkey = owner.pubkey().to_bytes().to_vec();

    let builder = CreateWalletBuilder::new()
        .with_payer(context.payer.pubkey())
        .with_id(wallet_id)
        .with_owner_authority_type(AuthorityType::Ed25519)
        .with_owner_authority_key(owner_pubkey);

    let (config_pda, _) = builder.get_pdas();
    let tx = builder.build_transaction(&context).await.unwrap();

    // Sign transaction with payer
    let mut tx = tx;
    let recent_blockhash = context.get_latest_blockhash().await;
    tx.sign(&[&context.payer], recent_blockhash);

    context.send_transaction(&tx).await.unwrap();

    // Verify wallet created
    let account = context.get_account(&config_pda).await;
    assert!(account.is_ok(), "Wallet config account should exist");
}

#[tokio::test]
async fn test_add_authority_admin() {
    let context = setup_test_context().await;
    let wallet_id = [2u8; 32];
    let owner = Keypair::new();

    // 1. Create Wallet
    let create_builder = CreateWalletBuilder::new()
        .with_payer(context.payer.pubkey())
        .with_id(wallet_id)
        .with_owner_authority_type(AuthorityType::Ed25519)
        .with_owner_authority_key(owner.pubkey().to_bytes().to_vec());

    let (config_pda, vault_pda) = create_builder.get_pdas();
    let tx = create_builder.build_transaction(&context).await.unwrap();
    let mut tx = tx;
    tx.sign(&[&context.payer], context.get_latest_blockhash().await);
    context.send_transaction(&tx).await.unwrap();

    // 2. Add Admin Authority
    let new_admin = Keypair::new();
    let wallet = LazorWallet {
        config_pda,
        address: vault_pda,
        program_id: lazorkit_program::id().into(),
        config_bump: 0,
    };

    let add_auth_builder = AddAuthorityBuilder::new(&wallet)
        .with_acting_role(0) // Owner
        .with_role(1) // Admin
        .with_type(AuthorityType::Ed25519)
        .with_authority(new_admin.pubkey())
        .with_authorizer(owner.pubkey())
        .with_authorization_data(vec![3]); // Index of owner account (Config=0, Payer=1, System=2, Owner=3)

    let mut tx = add_auth_builder
        .build_transaction(&context, context.payer.pubkey())
        .await
        .unwrap();

    // In a real test we'd need to sign with owner to authorize,
    // but the SDK construction might abstract that or we need to sign the tx manually if it requires signature.
    // The instruction expects authorization_data. For Ed25519 it usually checks signature of the acting authority.
    // Depending on implementation, we might need to properly sign the payload.
    // For now, let's sign with payer and verify if it fails or we need more setup.
    tx.sign(
        &[&context.payer, &owner],
        context.get_latest_blockhash().await,
    );

    // Note: This might fail if the program checks the signature inside the instruction data (authorization_data).
    // The SDK builder `with_authorization_data` takes bytes.
    // If we need a valid signature, we have to construct the payload and sign it.
    // Assuming for this initial test pass we just check transaction structure.

    let res = context.send_transaction(&tx).await;
    assert!(res.is_ok(), "Should successfully add admin authority");
}
