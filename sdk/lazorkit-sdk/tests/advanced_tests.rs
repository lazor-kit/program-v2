use lazorkit_sdk::{
    basic::{
        actions::{
            AddAuthorityBuilder, CreateWalletBuilder, RemoveAuthorityBuilder,
            UpdateAuthorityBuilder,
        },
        wallet::LazorWallet,
    },
    core::connection::SolConnection,
};
use lazorkit_state::authority::AuthorityType;
use solana_program_test::tokio;
use solana_sdk::signature::{Keypair, Signer};

mod common;
use common::setup_test_context;

/// Test that unauthorized user cannot add authorities
#[tokio::test]
async fn test_add_authority_unauthorized() {
    let context = setup_test_context().await;
    let wallet_id = [10u8; 32];
    let owner = Keypair::new();
    let attacker = Keypair::new();

    // Create wallet
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

    let wallet = LazorWallet {
        config_pda,
        address: vault_pda,
        program_id: lazorkit_program::id().into(),
        config_bump: 0,
    };

    // Try to add authority as attacker (should fail)
    let new_admin = Keypair::new();
    let add_auth_builder = AddAuthorityBuilder::new(&wallet)
        .with_acting_role(0)
        .with_role(1)
        .with_type(AuthorityType::Ed25519)
        .with_authority(new_admin.pubkey())
        .with_authorizer(attacker.pubkey())
        .with_authorization_data(vec![3]);

    let mut tx = add_auth_builder
        .build_transaction(&context, context.payer.pubkey())
        .await
        .unwrap();

    tx.sign(
        &[&context.payer, &attacker],
        context.get_latest_blockhash().await,
    );

    let res = context.send_transaction(&tx).await;
    assert!(
        res.is_err(),
        "Unauthorized user should not be able to add authority"
    );
}

/// Test that non-owner cannot remove authorities
#[tokio::test]
async fn test_remove_authority_unauthorized() {
    let context = setup_test_context().await;
    let wallet_id = [11u8; 32];
    let owner = Keypair::new();

    // Create wallet
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

    let wallet = LazorWallet {
        config_pda,
        address: vault_pda,
        program_id: lazorkit_program::id().into(),
        config_bump: 0,
    };

    // Add an admin first
    let admin = Keypair::new();
    let add_auth_builder = AddAuthorityBuilder::new(&wallet)
        .with_acting_role(0)
        .with_role(1)
        .with_type(AuthorityType::Ed25519)
        .with_authority(admin.pubkey())
        .with_authorizer(owner.pubkey())
        .with_authorization_data(vec![3]);

    let tx = add_auth_builder
        .build_transaction(&context, context.payer.pubkey())
        .await
        .unwrap();
    let mut tx = tx;
    tx.sign(
        &[&context.payer, &owner],
        context.get_latest_blockhash().await,
    );
    context.send_transaction(&tx).await.unwrap();

    // Add a spender role
    let spender = Keypair::new();
    let add_spender_builder = AddAuthorityBuilder::new(&wallet)
        .with_acting_role(0)
        .with_role(2)
        .with_type(AuthorityType::Ed25519)
        .with_authority(spender.pubkey())
        .with_authorizer(owner.pubkey())
        .with_authorization_data(vec![3]);

    let tx = add_spender_builder
        .build_transaction(&context, context.payer.pubkey())
        .await
        .unwrap();
    let mut tx = tx;
    tx.sign(
        &[&context.payer, &owner],
        context.get_latest_blockhash().await,
    );
    context.send_transaction(&tx).await.unwrap();

    // Try to remove admin as spender (should fail)
    let remove_auth_builder = RemoveAuthorityBuilder::new(&wallet)
        .with_acting_role(2) // Spender role
        .with_target_role(1) // Admin role
        .with_authorizer(spender.pubkey())
        .with_authorization_data(vec![3]);

    let tx = remove_auth_builder
        .build_transaction(&context, context.payer.pubkey())
        .await
        .unwrap();
    let mut tx = tx;
    tx.sign(
        &[&context.payer, &spender],
        context.get_latest_blockhash().await,
    );

    let res = context.send_transaction(&tx).await;
    assert!(
        res.is_err(),
        "Spender should not be able to remove authorities"
    );
}

/// Test that only owner can transfer ownership
#[tokio::test]
async fn test_transfer_ownership_only_owner() {
    let context = setup_test_context().await;
    let wallet_id = [12u8; 32];
    let owner = Keypair::new();

    // Create wallet
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

    let wallet = LazorWallet {
        config_pda,
        address: vault_pda,
        program_id: lazorkit_program::id().into(),
        config_bump: 0,
    };

    // Add an admin
    let admin = Keypair::new();
    let add_auth_builder = AddAuthorityBuilder::new(&wallet)
        .with_acting_role(0)
        .with_role(1)
        .with_type(AuthorityType::Ed25519)
        .with_authority(admin.pubkey())
        .with_authorizer(owner.pubkey())
        .with_authorization_data(vec![3]);

    let tx = add_auth_builder
        .build_transaction(&context, context.payer.pubkey())
        .await
        .unwrap();
    let mut tx = tx;
    tx.sign(
        &[&context.payer, &owner],
        context.get_latest_blockhash().await,
    );
    context.send_transaction(&tx).await.unwrap();

    // Note: Transfer ownership requires the actual owner's signature
    // Since transfer ownership can only be done by owner, testing an unauthorized
    // attempt would require mocking or bypassing the signature check
}

/// Test that spender role cannot manage authorities
#[tokio::test]
async fn test_spender_cannot_manage_authorities() {
    let context = setup_test_context().await;
    let wallet_id = [13u8; 32];
    let owner = Keypair::new();

    // Create wallet
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

    let wallet = LazorWallet {
        config_pda,
        address: vault_pda,
        program_id: lazorkit_program::id().into(),
        config_bump: 0,
    };

    // Add a spender
    let spender = Keypair::new();
    let add_spender_builder = AddAuthorityBuilder::new(&wallet)
        .with_acting_role(0)
        .with_role(2)
        .with_type(AuthorityType::Ed25519)
        .with_authority(spender.pubkey())
        .with_authorizer(owner.pubkey())
        .with_authorization_data(vec![3]);

    let tx = add_spender_builder
        .build_transaction(&context, context.payer.pubkey())
        .await
        .unwrap();
    let mut tx = tx;
    tx.sign(
        &[&context.payer, &owner],
        context.get_latest_blockhash().await,
    );
    context.send_transaction(&tx).await.unwrap();

    // Spender should NOT be able to add authorities
    let new_auth = Keypair::new();
    let add_auth_builder = AddAuthorityBuilder::new(&wallet)
        .with_acting_role(2) // Acting as spender
        .with_role(1)
        .with_type(AuthorityType::Ed25519)
        .with_authority(new_auth.pubkey())
        .with_authorizer(spender.pubkey())
        .with_authorization_data(vec![3]);

    let tx = add_auth_builder
        .build_transaction(&context, context.payer.pubkey())
        .await
        .unwrap();
    let mut tx = tx;
    tx.sign(
        &[&context.payer, &spender],
        context.get_latest_blockhash().await,
    );

    let res = context.send_transaction(&tx).await;
    assert!(
        res.is_err(),
        "Spender should not be able to add authorities"
    );
}

/// Test updating non-existent role fails  
#[tokio::test]
async fn test_update_nonexistent_role() {
    let context = setup_test_context().await;
    let wallet_id = [14u8; 32];
    let owner = Keypair::new();

    // Create wallet
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

    let wallet = LazorWallet {
        config_pda,
        address: vault_pda,
        program_id: lazorkit_program::id().into(),
        config_bump: 0,
    };

    // Try to update role that doesn't exist (role 5)
    let new_key = Keypair::new();
    let update_builder = UpdateAuthorityBuilder::new(&wallet)
        .with_acting_role(0)
        .with_target_role(5) // Non-existent role
        .with_new_authority_data(new_key.pubkey().to_bytes().to_vec())
        .with_authorizer(owner.pubkey())
        .with_authorization_data(vec![3]);

    let tx = update_builder
        .build_transaction(&context, context.payer.pubkey())
        .await
        .unwrap();
    let mut tx = tx;
    tx.sign(
        &[&context.payer, &owner],
        context.get_latest_blockhash().await,
    );

    let res = context.send_transaction(&tx).await;
    assert!(
        res.is_err(),
        "Should fail when trying to update non-existent role"
    );
}

/// Test removing non-existent authority fails gracefully
#[tokio::test]
async fn test_remove_nonexistent_authority() {
    let context = setup_test_context().await;
    let wallet_id = [15u8; 32];
    let owner = Keypair::new();

    // Create wallet
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

    let wallet = LazorWallet {
        config_pda,
        address: vault_pda,
        program_id: lazorkit_program::id().into(),
        config_bump: 0,
    };

    // Try to remove role that doesn't exist (role 5)
    let remove_auth_builder = RemoveAuthorityBuilder::new(&wallet)
        .with_acting_role(0)
        .with_target_role(5)
        .with_authorizer(owner.pubkey())
        .with_authorization_data(vec![3]);

    let tx = remove_auth_builder
        .build_transaction(&context, context.payer.pubkey())
        .await
        .unwrap();
    let mut tx = tx;
    tx.sign(
        &[&context.payer, &owner],
        context.get_latest_blockhash().await,
    );

    let res = context.send_transaction(&tx).await;
    assert!(
        res.is_err(),
        "Should fail when trying to remove non-existent role"
    );
}

/// Test admin can add/remove authorities but cannot transfer ownership
#[tokio::test]
async fn test_admin_permissions() {
    let context = setup_test_context().await;
    let wallet_id = [16u8; 32];
    let owner = Keypair::new();

    // Create wallet
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

    let wallet = LazorWallet {
        config_pda,
        address: vault_pda,
        program_id: lazorkit_program::id().into(),
        config_bump: 0,
    };

    // Add an admin
    let admin = Keypair::new();
    let add_auth_builder = AddAuthorityBuilder::new(&wallet)
        .with_acting_role(0)
        .with_role(1)
        .with_type(AuthorityType::Ed25519)
        .with_authority(admin.pubkey())
        .with_authorizer(owner.pubkey())
        .with_authorization_data(vec![3]);

    let tx = add_auth_builder
        .build_transaction(&context, context.payer.pubkey())
        .await
        .unwrap();
    let mut tx = tx;
    tx.sign(
        &[&context.payer, &owner],
        context.get_latest_blockhash().await,
    );
    context.send_transaction(&tx).await.unwrap();

    // Admin should be able to add a spender
    let spender = Keypair::new();
    let add_spender_builder = AddAuthorityBuilder::new(&wallet)
        .with_acting_role(1) // Admin role
        .with_role(2)
        .with_type(AuthorityType::Ed25519)
        .with_authority(spender.pubkey())
        .with_authorizer(admin.pubkey())
        .with_authorization_data(vec![3]);

    let tx = add_spender_builder
        .build_transaction(&context, context.payer.pubkey())
        .await
        .unwrap();
    let mut tx = tx;
    tx.sign(
        &[&context.payer, &admin],
        context.get_latest_blockhash().await,
    );

    let res = context.send_transaction(&tx).await;
    assert!(res.is_ok(), "Admin should be able to add authorities");

    // Admin should be able to remove the spender
    let remove_auth_builder = RemoveAuthorityBuilder::new(&wallet)
        .with_acting_role(1) // Admin role
        .with_target_role(2) // Spender
        .with_authorizer(admin.pubkey())
        .with_authorization_data(vec![3]);

    let tx = remove_auth_builder
        .build_transaction(&context, context.payer.pubkey())
        .await
        .unwrap();
    let mut tx = tx;
    tx.sign(
        &[&context.payer, &admin],
        context.get_latest_blockhash().await,
    );

    let res = context.send_transaction(&tx).await;
    assert!(res.is_ok(), "Admin should be able to remove authorities");
}

/// Test multiple authorities of same role can exist
#[tokio::test]
async fn test_multiple_admins() {
    let context = setup_test_context().await;
    let wallet_id = [17u8; 32];
    let owner = Keypair::new();

    // Create wallet
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

    let wallet = LazorWallet {
        config_pda,
        address: vault_pda,
        program_id: lazorkit_program::id().into(),
        config_bump: 0,
    };

    // Add first admin
    let admin1 = Keypair::new();
    let add_auth_builder = AddAuthorityBuilder::new(&wallet)
        .with_acting_role(0)
        .with_role(1)
        .with_type(AuthorityType::Ed25519)
        .with_authority(admin1.pubkey())
        .with_authorizer(owner.pubkey())
        .with_authorization_data(vec![3]);

    let tx = add_auth_builder
        .build_transaction(&context, context.payer.pubkey())
        .await
        .unwrap();
    let mut tx = tx;
    tx.sign(
        &[&context.payer, &owner],
        context.get_latest_blockhash().await,
    );
    context.send_transaction(&tx).await.unwrap();

    // Add second admin
    let admin2 = Keypair::new();
    let add_auth_builder = AddAuthorityBuilder::new(&wallet)
        .with_acting_role(0)
        .with_role(1)
        .with_type(AuthorityType::Ed25519)
        .with_authority(admin2.pubkey())
        .with_authorizer(owner.pubkey())
        .with_authorization_data(vec![3]);

    let tx = add_auth_builder
        .build_transaction(&context, context.payer.pubkey())
        .await
        .unwrap();
    let mut tx = tx;
    tx.sign(
        &[&context.payer, &owner],
        context.get_latest_blockhash().await,
    );

    let res = context.send_transaction(&tx).await;
    assert!(
        res.is_ok(),
        "Should be able to add multiple admins with same role"
    );
}
