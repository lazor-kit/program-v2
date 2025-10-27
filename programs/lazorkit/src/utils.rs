use crate::constants::{PASSKEY_PUBLIC_KEY_SIZE, SECP256R1_PROGRAM_ID, SMART_WALLET_SEED};
use crate::state::message::{Message, SimpleMessage};
use crate::state::WalletDevice;
use crate::{error::LazorKitError, ID};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    hash::hash,
    instruction::Instruction,
    program::{get_return_data, invoke_signed},
    system_instruction::transfer,
};

/// Utility functions for LazorKit smart wallet operations
///
/// This module provides helper functions for WebAuthn signature verification,
/// Cross-Program Invocation (CPI) execution, and message validation.

// Constants for Secp256r1 signature verification
const SECP_HEADER_SIZE: u16 = 14;
const SECP_DATA_START: u16 = 2 + SECP_HEADER_SIZE;
const SECP_PUBKEY_SIZE: u16 = PASSKEY_PUBLIC_KEY_SIZE as u16;
const SECP_SIGNATURE_SIZE: u16 = 64;
const SECP_HEADER_TOTAL: usize = 16;

/// Convenience wrapper to pass PDA seeds & bump into [`execute_cpi`].
///
/// Anchor expects PDA seeds as `&[&[u8]]` when calling `invoke_signed`.  Generating that slice of
/// byte-slices at every call-site is error-prone, so we hide the details behind this struct.  The
/// helper converts the `Vec<Vec<u8>>` into the required `&[&[u8]]` on the stack just before the
/// CPI.
#[derive(Clone, Debug)]
pub struct PdaSigner {
    /// PDA derivation seeds **without** the trailing bump.
    pub seeds: Vec<Vec<u8>>,
    /// The bump associated with the PDA.
    pub bump: u8,
}

pub fn get_policy_signer(
    smart_wallet: Pubkey,
    policy_signer: Pubkey,
    credential_hash: [u8; 32],
) -> Result<PdaSigner> {
    let seeds: &[&[u8]] = &[
        WalletDevice::PREFIX_SEED,
        &create_wallet_device_hash(smart_wallet, credential_hash),
    ];
    let (expected_policy_signer, bump) = Pubkey::find_program_address(seeds, &ID);

    require!(
        policy_signer == expected_policy_signer,
        LazorKitError::PasskeyMismatch
    );

    Ok(PdaSigner {
        seeds: seeds.to_vec().iter().map(|s| s.to_vec()).collect(),
        bump,
    })
}

pub fn execute_cpi(
    accounts: &[AccountInfo],
    data: &[u8],
    program: &AccountInfo,
    signer: PdaSigner,
) -> Result<Vec<u8>> {
    // Create the CPI instruction with proper account metadata
    // Optimize: avoid unnecessary clone by using slice directly where possible
    let ix = create_cpi_instruction_optimized(accounts, data, program, &signer);

    // Build the seed slice once to avoid repeated heap allocations
    // Convert Vec<Vec<u8>> to Vec<&[u8]> for invoke_signed
    let mut seed_slices: Vec<&[u8]> = signer.seeds.iter().map(|s| s.as_slice()).collect();
    let bump_slice = [signer.bump];
    seed_slices.push(&bump_slice);

    // Execute the CPI with PDA signing
    invoke_signed(&ix, accounts, &[&seed_slices])?;

    // Get the return data from the invoked program
    if let Some((_program_id, return_data)) = get_return_data() {
        Ok(return_data)
    } else {
        // If no return data was set, return empty vector
        Ok(Vec::new())
    }
}

/// Optimized CPI instruction creation that avoids unnecessary allocations
fn create_cpi_instruction_optimized(
    accounts: &[AccountInfo],
    data: &[u8],
    program: &AccountInfo,
    pda_signer: &PdaSigner,
) -> Instruction {
    create_cpi_instruction_multiple_signers(accounts, data, program, &[pda_signer.clone()])
}

/// Create CPI instruction with multiple PDA signers
fn create_cpi_instruction_multiple_signers(
    accounts: &[AccountInfo],
    data: &[u8],
    program: &AccountInfo,
    pda_signers: &[PdaSigner],
) -> Instruction {
    // Derive all PDA addresses to determine which accounts should be signers
    let mut pda_pubkeys = Vec::new();
    for signer in pda_signers {
        let seed_slices: Vec<&[u8]> = signer.seeds.iter().map(|s| s.as_slice()).collect();
        let pda_pubkey = Pubkey::find_program_address(&seed_slices, &ID).0;
        pda_pubkeys.push(pda_pubkey);
    }

    Instruction {
        program_id: program.key(),
        accounts: accounts
            .iter()
            .map(|acc| {
                // Mark the account as a signer if it matches any of our derived PDA addresses
                let is_pda_signer = pda_pubkeys.contains(acc.key);
                AccountMeta {
                    pubkey: *acc.key,
                    is_signer: is_pda_signer,
                    is_writable: acc.is_writable,
                }
            })
            .collect(),
        data: data.to_vec(), // Only allocate here when absolutely necessary
    }
}

/// Verify a Secp256r1 signature instruction
pub fn verify_secp256r1_instruction(
    ix: &Instruction,
    pubkey: [u8; SECP_PUBKEY_SIZE as usize],
    msg: Vec<u8>,
    sig: [u8; 64],
) -> Result<()> {
    // Calculate expected instruction data length based on Secp256r1 format
    let expected_len =
        (SECP_DATA_START + SECP_PUBKEY_SIZE + SECP_SIGNATURE_SIZE) as usize + msg.len();

    // Validate the instruction format matches Secp256r1 requirements
    if ix.program_id != SECP256R1_PROGRAM_ID
        || !ix.accounts.is_empty()
        || ix.data.len() != expected_len
    {
        return Err(LazorKitError::Secp256r1InvalidLength.into());
    }

    // Verify the actual signature data
    verify_secp256r1_data(&ix.data, pubkey, msg, sig)
}

/// Verify the data portion of a Secp256r1 signature
fn verify_secp256r1_data(
    data: &[u8],
    public_key: [u8; SECP_PUBKEY_SIZE as usize],
    message: Vec<u8>,
    signature: [u8; 64],
) -> Result<()> {
    // Calculate the byte offsets for each component in the Secp256r1 instruction data
    let msg_len = message.len() as u16;
    let offsets = calculate_secp_offsets(msg_len);

    // Verify the instruction header matches the expected Secp256r1 format
    if !verify_secp_header(data, &offsets) {
        return Err(LazorKitError::Secp256r1HeaderMismatch.into());
    }

    // Verify the actual signature data (public key, signature, message) matches
    if !verify_secp_data(data, &public_key, &signature, &message) {
        return Err(LazorKitError::Secp256r1DataMismatch.into());
    }

    Ok(())
}

/// Calculate offsets for Secp256r1 signature verification
#[derive(Debug)]
struct SecpOffsets {
    pubkey_offset: u16,
    sig_offset: u16,
    msg_offset: u16,
    msg_len: u16,
}

#[inline]
fn calculate_secp_offsets(msg_len: u16) -> SecpOffsets {
    SecpOffsets {
        pubkey_offset: SECP_DATA_START,
        sig_offset: SECP_DATA_START + SECP_PUBKEY_SIZE,
        msg_offset: SECP_DATA_START + SECP_PUBKEY_SIZE + SECP_SIGNATURE_SIZE,
        msg_len,
    }
}

/// Helper function to safely convert slice to u16
#[inline]
fn slice_to_u16(data: &[u8], start: usize) -> Option<u16> {
    if start + 1 < data.len() {
        Some(u16::from_le_bytes([data[start], data[start + 1]]))
    } else {
        None
    }
}

#[inline]
fn verify_secp_header(data: &[u8], offsets: &SecpOffsets) -> bool {
    data[0] == 1
        && slice_to_u16(data, 2).map_or(false, |v| v == offsets.sig_offset)
        && slice_to_u16(data, 4).map_or(false, |v| v == 0xFFFF)
        && slice_to_u16(data, 6).map_or(false, |v| v == offsets.pubkey_offset)
        && slice_to_u16(data, 8).map_or(false, |v| v == 0xFFFF)
        && slice_to_u16(data, 10).map_or(false, |v| v == offsets.msg_offset)
        && slice_to_u16(data, 12).map_or(false, |v| v == offsets.msg_len)
        && slice_to_u16(data, 14).map_or(false, |v| v == 0xFFFF)
}

#[inline]
fn verify_secp_data(data: &[u8], public_key: &[u8], signature: &[u8], message: &[u8]) -> bool {
    let pubkey_range = SECP_HEADER_TOTAL..SECP_HEADER_TOTAL + SECP_PUBKEY_SIZE as usize;
    let sig_range = pubkey_range.end..pubkey_range.end + SECP_SIGNATURE_SIZE as usize;
    let msg_range = sig_range.end..;

    data[pubkey_range] == public_key[..]
        && data[sig_range] == signature[..]
        && data[msg_range] == message[..]
}

/// Helper to get sighash for anchor instructions
pub fn sighash(namespace: &str, name: &str) -> [u8; 8] {
    let preimage = format!("{}:{}", namespace, name);
    let mut out = [0u8; 8];
    out.copy_from_slice(
        &anchor_lang::solana_program::hash::hash(preimage.as_bytes()).to_bytes()[..8],
    );
    out
}

pub fn compute_device_hash(
    passkey_public_key: [u8; PASSKEY_PUBLIC_KEY_SIZE],
    credential_hash: [u8; 32],
) -> [u8; 32] {
    let mut buf = [0u8; 65];
    buf[..33 as usize].copy_from_slice(&passkey_public_key);
    buf[33 as usize..].copy_from_slice(&credential_hash);
    hash(&buf).to_bytes()
}

pub fn create_wallet_device_hash(smart_wallet: Pubkey, credential_hash: [u8; 32]) -> [u8; 32] {
    // Combine passkey public key with wallet address for unique hashing
    let mut buf = [0u8; 64];
    buf[..32 as usize].copy_from_slice(&smart_wallet.to_bytes());
    buf[32 as usize..].copy_from_slice(&credential_hash);
    // Hash the combined data to create a unique identifier
    hash(&buf).to_bytes()
}

/// Helper: Get a slice of accounts from remaining_accounts
pub fn get_account_slice<'a>(
    accounts: &'a [AccountInfo<'a>],
    start: u8,
    len: u8,
) -> Result<&'a [AccountInfo<'a>]> {
    accounts
        .get(start as usize..(start as usize + len as usize))
        .ok_or(crate::error::LazorKitError::AccountSliceOutOfBounds.into())
}

/// Helper: Create a custom PDA signer with arbitrary seeds
pub fn create_custom_pda_signer(seeds: Vec<Vec<u8>>, bump: u8) -> PdaSigner {
    PdaSigner { seeds, bump }
}

/// Verify authorization using hash comparison instead of deserializing message data
pub fn verify_authorization_hash(
    ix_sysvar: &AccountInfo,
    passkey_public_key: [u8; PASSKEY_PUBLIC_KEY_SIZE],
    signature: [u8; 64],
    client_data_json_raw: &[u8],
    authenticator_data_raw: &[u8],
    verify_instruction_index: u8,
    expected_hash: [u8; 32],
) -> Result<()> {
    use anchor_lang::solana_program::sysvar::instructions::load_instruction_at_checked;
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};

    // 2) locate the secp256r1 verify instruction
    let secp_ix = load_instruction_at_checked(verify_instruction_index as usize, ix_sysvar)?;

    // 3) reconstruct signed message (wallet_device authenticatorData || SHA256(clientDataJSON))
    let client_hash = hash(client_data_json_raw);
    let mut message = Vec::with_capacity(authenticator_data_raw.len() + client_hash.as_ref().len());
    message.extend_from_slice(authenticator_data_raw);
    message.extend_from_slice(client_hash.as_ref());

    // 4) parse the challenge from clientDataJSON
    let json_str = core::str::from_utf8(client_data_json_raw)
        .map_err(|_| crate::error::LazorKitError::ClientDataInvalidUtf8)?;
    let parsed: serde_json::Value = serde_json::from_str(json_str)
        .map_err(|_| crate::error::LazorKitError::ClientDataJsonParseError)?;
    let challenge = parsed["challenge"]
        .as_str()
        .ok_or(crate::error::LazorKitError::ChallengeMissing)?;

    let challenge_clean = challenge.trim_matches(|c| c == '"' || c == '\'' || c == '/' || c == ' ');
    let challenge_bytes = URL_SAFE_NO_PAD
        .decode(challenge_clean)
        .map_err(|_| crate::error::LazorKitError::ChallengeBase64DecodeError)?;

    verify_secp256r1_instruction(&secp_ix, passkey_public_key, message, signature)?;
    // Verify hash instead of deserializing message data
    SimpleMessage::verify_hash(challenge_bytes, expected_hash)?;
    Ok(())
}

// HeaderView and HasHeader trait are no longer needed with simplified message structure

/// Hash computation functions for on-chain verification
/// These functions replicate the same hashing logic used off-chain

/// Compute hash of instruction data and accounts combined
/// This function can be used for both policy and CPI instructions
/// Optimized to reduce allocations
pub fn compute_instruction_hash(
    instruction_data: &[u8],
    instruction_accounts: &[AccountInfo],
    program_id: Pubkey,
) -> Result<[u8; 32]> {
    use anchor_lang::solana_program::hash::{hash, Hasher};

    // Hash instruction data
    let data_hash = hash(instruction_data);

    // Hash instruction accounts using Hasher (including program_id)
    let mut rh = Hasher::default();
    rh.hash(program_id.as_ref());
    for account in instruction_accounts.iter() {
        rh.hash(account.key().as_ref());
        rh.hash(&[account.is_signer as u8]);
        rh.hash(&[account.is_writable as u8]);
    }
    let accounts_hash = rh.result();

    // Combine hashes efficiently using a pre-allocated buffer
    let mut combined = [0u8; 64]; // 32 + 32 bytes
    combined[..32].copy_from_slice(data_hash.as_ref());
    combined[32..].copy_from_slice(accounts_hash.as_ref());

    Ok(hash(&combined).to_bytes())
}

/// Message types for hash computation
#[derive(Debug, Clone, Copy)]
pub enum MessageType {
    Execute,
    CallPolicyProgram,
    ChangePolicyProgram,
    AddDevice,
    RemoveDevice,
    CreateChunk,
    GrantPermission,
}

/// Generic message hash computation function
/// Replaces all the individual hash functions with a single, optimized implementation
pub fn compute_message_hash(
    message_type: MessageType,
    nonce: u64,
    timestamp: i64,
    hash1: [u8; 32],
    hash2: Option<[u8; 32]>,
    additional_data: Option<&[u8]>,
) -> Result<[u8; 32]> {
    use anchor_lang::solana_program::hash::hash;

    let mut data = Vec::new();

    // Common fields for all message types
    data.extend_from_slice(&nonce.to_le_bytes());
    data.extend_from_slice(&timestamp.to_le_bytes());
    data.extend_from_slice(&hash1);

    // Add second hash if provided
    if let Some(h2) = hash2 {
        data.extend_from_slice(&h2);
    }

    // Add additional data for specific message types
    match message_type {
        MessageType::GrantPermission => {
            if let Some(additional) = additional_data {
                data.extend_from_slice(additional);
            }
        }
        _ => {} // Other message types don't need additional data
    }

    Ok(hash(&data).to_bytes())
}

/// Compute execute message hash: hash(nonce, timestamp, policy_hash, cpi_hash)
/// Optimized to use stack allocation instead of heap
pub fn compute_execute_message_hash(
    nonce: u64,
    timestamp: i64,
    policy_hash: [u8; 32],
    cpi_hash: [u8; 32],
) -> Result<[u8; 32]> {
    compute_message_hash(
        MessageType::Execute,
        nonce,
        timestamp,
        policy_hash,
        Some(cpi_hash),
        None,
    )
}

/// Compute call policy message hash: hash(nonce, timestamp, policy_hash, empty_cpi_hash)
/// Optimized to use stack allocation
pub fn compute_call_policy_program_message_hash(
    nonce: u64,
    timestamp: i64,
    policy_hash: [u8; 32],
) -> Result<[u8; 32]> {
    compute_message_hash(
        MessageType::CallPolicyProgram,
        nonce,
        timestamp,
        policy_hash,
        None,
        None,
    )
}

/// Compute change policy message hash: hash(nonce, timestamp, old_policy_hash, new_policy_hash)
pub fn compute_change_policy_program_message_hash(
    nonce: u64,
    timestamp: i64,
    old_policy_hash: [u8; 32],
    new_policy_hash: [u8; 32],
) -> Result<[u8; 32]> {
    compute_message_hash(
        MessageType::ChangePolicyProgram,
        nonce,
        timestamp,
        old_policy_hash,
        Some(new_policy_hash),
        None,
    )
}

/// Compute add device message hash: hash(nonce, timestamp, policy_hash, new_device_hash)
pub fn compute_add_device_message_hash(
    nonce: u64,
    timestamp: i64,
    policy_hash: [u8; 32],
    new_device_hash: [u8; 32],
) -> Result<[u8; 32]> {
    compute_message_hash(
        MessageType::AddDevice,
        nonce,
        timestamp,
        policy_hash,
        Some(new_device_hash),
        None,
    )
}

/// Compute remove device message hash: hash(nonce, timestamp, policy_hash, remove_device_hash)
pub fn compute_remove_device_message_hash(
    nonce: u64,
    timestamp: i64,
    policy_hash: [u8; 32],
    remove_device_hash: [u8; 32],
) -> Result<[u8; 32]> {
    compute_message_hash(
        MessageType::RemoveDevice,
        nonce,
        timestamp,
        policy_hash,
        Some(remove_device_hash),
        None,
    )
}

/// Compute create chunk message hash: hash(nonce, timestamp, policy_hash, cpi_hash)
pub fn compute_create_chunk_message_hash(
    nonce: u64,
    timestamp: i64,
    policy_hash: [u8; 32],
    cpi_hash: [u8; 32],
) -> Result<[u8; 32]> {
    compute_message_hash(
        MessageType::CreateChunk,
        nonce,
        timestamp,
        policy_hash,
        Some(cpi_hash),
        None,
    )
}

/// Compute grant permission message hash: hash(nonce, timestamp, ephemeral_key, expires_at, combined_hash)
pub fn compute_grant_permission_message_hash(
    nonce: u64,
    timestamp: i64,
    ephemeral_key: Pubkey,
    expires_at: i64,
    combined_hash: [u8; 32],
) -> Result<[u8; 32]> {
    use anchor_lang::solana_program::hash::hash;

    // For GrantPermission, we need to hash the additional data separately
    let mut additional_data = Vec::new();
    additional_data.extend_from_slice(ephemeral_key.as_ref());
    additional_data.extend_from_slice(&expires_at.to_le_bytes());
    let additional_hash = hash(&additional_data).to_bytes();

    compute_message_hash(
        MessageType::GrantPermission,
        nonce,
        timestamp,
        combined_hash,
        Some(additional_hash),
        None,
    )
}

/// Helper: Split remaining accounts into `(policy_accounts, cpi_accounts)` using `split_index` coming from `Message`.
pub fn split_remaining_accounts<'a>(
    accounts: &'a [AccountInfo<'a>],
    split_index: u16,
) -> Result<(&'a [AccountInfo<'a>], &'a [AccountInfo<'a>])> {
    let idx = split_index as usize;
    require!(
        idx <= accounts.len(),
        crate::error::LazorKitError::AccountSliceOutOfBounds
    );
    Ok(accounts.split_at(idx))
}

/// Calculate account ranges for multiple instructions using split indices
/// Returns a vector of (start, end) tuples representing account ranges for each instruction
/// For n instructions, we need n-1 split indices to divide the accounts
pub fn calculate_account_ranges(
    accounts: &[AccountInfo],
    split_indices: &[u8],
) -> Result<Vec<(usize, usize)>> {
    let mut account_ranges = Vec::new();
    let mut start = 0usize;

    // Calculate account ranges for each instruction using split indices
    for &split_point in split_indices.iter() {
        let end = split_point as usize;
        require!(
            end > start && end <= accounts.len(),
            crate::error::LazorKitError::AccountSliceOutOfBounds
        );
        account_ranges.push((start, end));
        start = end;
    }

    // Add the last instruction range (from last split to end)
    require!(
        start < accounts.len(),
        crate::error::LazorKitError::AccountSliceOutOfBounds
    );
    account_ranges.push((start, accounts.len()));

    Ok(account_ranges)
}

/// Validate all programs in account ranges for security
/// Checks that each program is executable and prevents reentrancy attacks
pub fn validate_programs_in_ranges(
    accounts: &[AccountInfo],
    account_ranges: &[(usize, usize)],
) -> Result<()> {
    for &(range_start, range_end) in account_ranges.iter() {
        let instruction_accounts = &accounts[range_start..range_end];

        require!(
            !instruction_accounts.is_empty(),
            crate::error::LazorKitError::InsufficientCpiAccounts
        );

        // First account in each instruction slice is the program ID
        let program_account = &instruction_accounts[0];

        // Validate program is executable (not a data account)
        require!(
            program_account.executable,
            crate::error::LazorKitError::ProgramNotExecutable
        );

        // Prevent reentrancy attacks by blocking calls to this program
        require!(
            program_account.key() != crate::ID,
            crate::error::LazorKitError::ReentrancyDetected
        );
    }

    Ok(())
}

// Transfer transaction fee to payer
pub fn transfer_fee_to_payer<'a>(
    smart_wallet: &AccountInfo<'a>,
    wallet_id: u64,
    bump: u8,
    payer: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    fee: u64,
) -> Result<()> {
    let signer = PdaSigner {
        seeds: vec![SMART_WALLET_SEED.to_vec(), wallet_id.to_le_bytes().to_vec()],
        bump,
    };

    let transfer_ins = transfer(smart_wallet.key, payer.key, fee);

    execute_cpi(
        &[smart_wallet.to_account_info(), payer.to_account_info()],
        &transfer_ins.data,
        system_program,
        signer,
    )?;

    Ok(())
}
