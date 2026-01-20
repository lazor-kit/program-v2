use lazorkit_sdk::{
    basic::{
        actions::{
            AddAuthorityBuilder, CreateSessionBuilder, CreateWalletBuilder, ExecuteBuilder,
            RemoveAuthorityBuilder, TransferOwnershipBuilder, UpdateAuthorityBuilder,
        },
        wallet::LazorWallet,
    },
    core::connection::SolConnection,
};
use lazorkit_state::authority::AuthorityType;
use solana_program_test::tokio;
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk::transaction::Transaction;

mod common;
use common::setup_test_context;

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

#[tokio::test]
async fn test_remove_authority_success() {
    let context = setup_test_context().await;
    let wallet_id = [3u8; 32];
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
    context
        .send_transaction(&tx)
        .await
        .expect("Failed to create wallet");

    let wallet = LazorWallet {
        config_pda,
        address: vault_pda,
        program_id: lazorkit_program::id().into(),
        config_bump: 0,
    };

    // 1. Add an Admin authority first (so we have something to remove)
    // Add Authority 1 (Role 1)
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
    context
        .send_transaction(&tx)
        .await
        .expect("Failed to add admin");

    // Add Authority 2 (Role 2) - This is the one we will remove
    let user = Keypair::new();
    let add_auth_builder_2 = AddAuthorityBuilder::new(&wallet)
        .with_acting_role(0)
        .with_role(2)
        .with_type(AuthorityType::Ed25519)
        .with_authority(user.pubkey())
        .with_authorizer(owner.pubkey())
        .with_authorization_data(vec![3]);
    let tx2 = add_auth_builder_2
        .build_transaction(&context, context.payer.pubkey())
        .await
        .unwrap();
    let mut tx2 = tx2;
    tx2.sign(
        &[&context.payer, &owner],
        context.get_latest_blockhash().await,
    );
    context
        .send_transaction(&tx2)
        .await
        .expect("Failed to add user");

    // 2. Remove the Authority (Use Role 2 to avoid Last Admin Protection logic which protects Role 1)
    let remove_auth_builder = RemoveAuthorityBuilder::new(&wallet)
        .with_acting_role(0) // Owner
        .with_target_role(2) // User/Spender
        .with_authorizer(owner.pubkey())
        .with_authorization_data(vec![3]); // Owner index

    let tx_remove = remove_auth_builder
        .build_transaction(&context, context.payer.pubkey())
        .await
        .unwrap();

    let mut tx_remove = tx_remove;
    tx_remove.sign(
        &[&context.payer, &owner],
        context.get_latest_blockhash().await,
    );
    context
        .send_transaction(&tx_remove)
        .await
        .expect("Failed to remove admin");
}

#[tokio::test]
async fn test_update_authority() {
    let context = setup_test_context().await;
    let wallet_id = [4u8; 32];
    let owner = Keypair::new();

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

    // Add Admin (to be updated)
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

    // Update Admin Key
    let new_admin = Keypair::new();
    let update_builder = UpdateAuthorityBuilder::new(&wallet)
        .with_acting_role(0)
        .with_target_role(1)
        .with_new_authority_data(new_admin.pubkey().to_bytes().to_vec())
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
    assert!(res.is_ok(), "Should successfully update authority");
}

#[tokio::test]
async fn test_transfer_ownership() {
    let context = setup_test_context().await;
    let wallet_id = [5u8; 32];
    let owner = Keypair::new();

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

    // Fund owner just in case (though read-only signer needed)
    // Actually, usually not needed but sometimes helps with account existence checks
    let fund_ix = solana_sdk::system_instruction::transfer(
        &context.payer.pubkey(),
        &owner.pubkey(),
        1_000_000_000,
    );
    let fund_tx = Transaction::new_signed_with_payer(
        &[fund_ix],
        Some(&context.payer.pubkey()),
        &[&context.payer],
        context.get_latest_blockhash().await,
    );
    context.send_transaction(&fund_tx).await.unwrap();

    let wallet = LazorWallet {
        config_pda,
        address: vault_pda,
        program_id: lazorkit_program::id().into(),
        config_bump: 0,
    };

    let new_owner = Keypair::new();
    let transfer_builder = TransferOwnershipBuilder::new(&wallet)
        .with_current_owner(owner.pubkey())
        .with_new_owner(
            AuthorityType::Ed25519,
            new_owner.pubkey().to_bytes().to_vec(),
        )
        .with_authorization_data(vec![1]);

    let tx = transfer_builder
        .build_transaction(&context, context.payer.pubkey())
        .await
        .unwrap();
    let mut tx = tx;
    tx.sign(
        &[&context.payer, &owner],
        context.get_latest_blockhash().await,
    );

    let res = context.send_transaction(&tx).await;
    assert!(res.is_ok(), "Should successfully transfer ownership");
}

#[tokio::test]
async fn test_remove_last_admin_fails() {
    let context = setup_test_context().await;
    let wallet_id = [6u8; 32];
    let owner = Keypair::new();

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

    // Add ONE Admin
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

    // Try to remove the ONLY Admin
    let remove_auth_builder = RemoveAuthorityBuilder::new(&wallet)
        .with_acting_role(0)
        .with_target_role(1)
        .with_authorizer(owner.pubkey())
        .with_authorization_data(vec![3]); // Owner index

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
    assert!(res.is_err(), "Should FAIL to remove the last admin");
}

#[tokio::test]
async fn test_create_session_and_execute() {
    let context = setup_test_context().await;
    let wallet_id = [7u8; 32];
    let owner = Keypair::new();

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

    // SKIP Owner Session Creation (Ed25519 does not support sessions)
    // We will proceed directly to creating an Admin role with Session support.

    // Execute using Session
    // We need to upgrade authority type to Session?
    // Or does CreateSession just add a session key for an existing role?
    // Contract: `create_session` updates `session_key` and `expiration` in the Role struct.
    // The Role's auth type must be *Session (Ed25519Session or Secp256r1Session).
    // Wait, if the role was created as Ed25519, can we just add a session?
    // `create_session.rs`: checks `role.authority_type.supports_session()`.
    // Ed25519 type does NOT support session?
    // `lazorkit_state::AuthorityType::Ed25519` - let's check definition.
    // Actually, usually we need to Upgrade the role to a Session Type first or create it as Session Type.
    // Or maybe the architecture says: "Standard types... Session types...".

    // If I look at `test_create_wallet_success`, it uses `Ed25519`.
    // I likely need to `UpdateAuthority` to change type to `Ed25519Session` OR Create a new role with `Ed25519Session`.

    // Let's creating a NEW role (Admin) with Ed25519Session type.
    let session_admin = Keypair::new();
    let session_auth_data = lazorkit_sdk::basic::actions::create_ed25519_session_data(
        session_admin.pubkey().to_bytes(),
        1000, // Max age
    );
    let add_auth_builder = AddAuthorityBuilder::new(&wallet)
        .with_acting_role(0)
        .with_role(1) // Use Role 1 (Session Admin)
        .with_type(AuthorityType::Ed25519Session) // Session Type
        .with_authority_key(session_auth_data)
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
    context
        .send_transaction(&tx)
        .await
        .expect("Failed to add authority");

    // Now create session for this admin
    let session_token = [8u8; 32];
    let create_sess_builder = CreateSessionBuilder::new(&wallet)
        .with_role(1)
        .with_session_key(session_token)
        .with_duration(1000)
        .with_authorizer(session_admin.pubkey()) // Must authenticate with master key (session_admin)
        .with_authorization_data(vec![3, 4]); // Owner(3), SessionAdmin(4)? No, just SessionAdmin.
                                              // The builder just needs to point to the signer.

    // Wait, CreateSession requires Master Key authentication.
    // In `add_auth_builder`, we added `session_admin` as authority.
    // So `session_admin` keypair IS the master key.

    // We need to add `session_admin` to signers.
    // `create_sess_builder` helper `with_authorizer` appends AccountMeta.

    let tx = create_sess_builder
        .build_transaction(&context, context.payer.pubkey())
        .await
        .unwrap();
    let mut tx = tx;
    tx.sign(
        &[&context.payer, &session_admin],
        context.get_latest_blockhash().await,
    );
    // session_admin needs to sign since it's the master key of role 1.

    context
        .send_transaction(&tx)
        .await
        .expect("Failed to create session for admin");

    // Execute an action using the SESSION
    // The `execute` instruction authenticates using the session logic.
    // For `Ed25519Session`: `authenticate` checks if `slot` is within range.
    // It doesn't check a signature of a temporary key??
    // `Ed25519SessionAuthority::authenticate`:
    // "verifies that the transaction is signed equal to the `session_key` stored in the role... IF `session_key` is set."
    // Ah, `session_key` IS the temporary key.

    // So we need a keypair corresponding to `session_token`.
    // Wait, `session_token` was [8u8; 32] -> this needs to be a pubkey of a keypair if we want to sign with it.

    let temp_key = Keypair::new();
    // Re-do Create Session with temp_key pubkey
    let create_sess_builder_2 = CreateSessionBuilder::new(&wallet)
        .with_role(1)
        .with_session_key(temp_key.pubkey().to_bytes()) // 32 bytes
        .with_duration(1000)
        .with_authorizer(session_admin.pubkey())
        .with_authorization_data(vec![3, 4]); // Need to ensure correct indices

    // To simplify indices, just pass authorizer pubkey.
    // ...

    let tx = create_sess_builder_2
        .build_transaction(&context, context.payer.pubkey())
        .await
        .unwrap();
    let mut tx = tx;
    tx.sign(
        &[&context.payer, &session_admin],
        context.get_latest_blockhash().await,
    );
    context
        .send_transaction(&tx)
        .await
        .expect("Failed to create session 2");

    // Execute Transfer of 1 lamport to payer (loopback)
    // NOTE: Vault needs funds to transfer!
    let fund_ix =
        solana_sdk::system_instruction::transfer(&context.payer.pubkey(), &vault_pda, 1_000_000);
    let fund_tx = Transaction::new_signed_with_payer(
        &[fund_ix],
        Some(&context.payer.pubkey()),
        &[&context.payer],
        context.get_latest_blockhash().await,
    );
    context.send_transaction(&fund_tx).await.unwrap();

    let transfer_ix =
        solana_sdk::system_instruction::transfer(&vault_pda, &context.payer.pubkey(), 1);

    let execute_builder = ExecuteBuilder::new(&wallet)
        .with_role_id(1)
        .add_instruction(transfer_ix)
        .with_signer(temp_key.pubkey()); // Sign with SESSION Key

    let tx = execute_builder
        .build_transaction(&context, context.payer.pubkey())
        .await
        .unwrap();
    let mut tx = tx;
    tx.sign(
        &[&context.payer, &temp_key],
        context.get_latest_blockhash().await,
    );

    let res = context.send_transaction(&tx).await;
    assert!(res.is_ok(), "Should execute successfully with session key");
}
