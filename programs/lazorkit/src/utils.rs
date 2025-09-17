use crate::constants::{PASSKEY_PUBLIC_KEY_SIZE, SECP256R1_PROGRAM_ID};
use crate::state::{ExecuteMessage, CallPolicyMessage, ChangePolicyMessage};
use crate::{error::LazorKitError, ID};
use anchor_lang::solana_program::{instruction::Instruction, program::invoke_signed};
use anchor_lang::{prelude::*, solana_program::hash::hash};

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

/// Helper to check if a slice matches a pattern
#[inline]
pub fn slice_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| x == y)
}

/// Execute a Cross-Program Invocation (CPI).
///
/// * `accounts` – slice of `AccountInfo` that will be forwarded to the target program.
/// * `data` – raw instruction data **as a slice**; passing a slice removes the need for the
///   caller to allocate a new `Vec<u8>` every time a CPI is performed.  A single allocation is
///   still required internally when constructing the `Instruction`, but this change avoids an
///   additional clone at every call-site.
/// * `program` – account info of the program to invoke.
/// * `signer` – optional PDA signer information.  When provided, the seeds are appended with the
///   bump and the CPI is invoked with `invoke_signed`.
pub fn execute_cpi(
    accounts: &[AccountInfo],
    data: &[u8],
    program: &AccountInfo,
    signer: PdaSigner,
) -> Result<()> {
    // Create the CPI instruction with proper account metadata
    // We need to clone the data here because the SDK expects owned data
    let ix = create_cpi_instruction(accounts, data.to_vec(), program, &signer);

    // Build the seed slice once to avoid repeated heap allocations
    // Convert Vec<Vec<u8>> to Vec<&[u8]> for invoke_signed
    let mut seed_slices: Vec<&[u8]> = signer.seeds.iter().map(|s| s.as_slice()).collect();
    let bump_slice = [signer.bump];
    seed_slices.push(&bump_slice);
    
    // Execute the CPI with PDA signing
    invoke_signed(&ix, accounts, &[&seed_slices]).map_err(Into::into)
}

/// Create a CPI instruction with proper account meta configuration
fn create_cpi_instruction(
    accounts: &[AccountInfo],
    data: Vec<u8>,
    program: &AccountInfo,
    pda_signer: &PdaSigner,
) -> Instruction {
    // Derive the PDA address from the seeds to determine which account should be a signer
    let seed_slices: Vec<&[u8]> = pda_signer.seeds.iter().map(|s| s.as_slice()).collect();
    let pda_pubkey = Pubkey::find_program_address(&seed_slices, &ID).0;

    Instruction {
        program_id: program.key(),
        accounts: accounts
            .iter()
            .map(|acc| {
                // Mark the PDA account as a signer if it matches our derived address
                let is_pda_signer = *acc.key == pda_pubkey;
                AccountMeta {
                    pubkey: *acc.key,
                    is_signer: is_pda_signer,
                    is_writable: acc.is_writable,
                }
            })
            .collect(),
        data,
    }
}

/// Verify a Secp256r1 signature instruction
pub fn verify_secp256r1_instruction(
    ix: &Instruction,
    pubkey: [u8; SECP_PUBKEY_SIZE as usize],
    msg: Vec<u8>,
    sig: Vec<u8>,
) -> Result<()> {
    // Calculate expected instruction data length based on Secp256r1 format
    let expected_len =
        (SECP_DATA_START + SECP_PUBKEY_SIZE + SECP_SIGNATURE_SIZE) as usize + msg.len();
    
    // Validate the instruction format matches Secp256r1 requirements
    if ix.program_id != SECP256R1_PROGRAM_ID || !ix.accounts.is_empty() || ix.data.len() != expected_len {
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
    signature: Vec<u8>,
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

#[inline]
fn verify_secp_header(data: &[u8], offsets: &SecpOffsets) -> bool {
    data[0] == 1
        && u16::from_le_bytes(data[2..=3].try_into().unwrap()) == offsets.sig_offset
        && u16::from_le_bytes(data[4..=5].try_into().unwrap()) == 0xFFFF
        && u16::from_le_bytes(data[6..=7].try_into().unwrap()) == offsets.pubkey_offset
        && u16::from_le_bytes(data[8..=9].try_into().unwrap()) == 0xFFFF
        && u16::from_le_bytes(data[10..=11].try_into().unwrap()) == offsets.msg_offset
        && u16::from_le_bytes(data[12..=13].try_into().unwrap()) == offsets.msg_len
        && u16::from_le_bytes(data[14..=15].try_into().unwrap()) == 0xFFFF
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

/// Extension trait for passkey operations
pub trait PasskeyExt {
    fn to_hashed_bytes(&self, wallet: Pubkey) -> [u8; 32];
}

impl PasskeyExt for [u8; PASSKEY_PUBLIC_KEY_SIZE] {
    #[inline]
    fn to_hashed_bytes(&self, wallet: Pubkey) -> [u8; 32] {
        // Combine passkey public key with wallet address for unique hashing
        let mut buf = [0u8; 65];
        buf[..SECP_PUBKEY_SIZE as usize].copy_from_slice(self);
        buf[SECP_PUBKEY_SIZE as usize..].copy_from_slice(&wallet.to_bytes());
        // Hash the combined data to create a unique identifier
        hash(&buf).to_bytes()
    }
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

/// Helper: Create a PDA signer struct
pub fn get_wallet_device_signer(
    passkey: &[u8; PASSKEY_PUBLIC_KEY_SIZE],
    wallet: Pubkey,
    bump: u8,
) -> PdaSigner {
    PdaSigner {
        seeds: vec![
            crate::state::WalletDevice::PREFIX_SEED.to_vec(),
            wallet.to_bytes().to_vec(),
            passkey.to_hashed_bytes(wallet).to_vec(),
        ],
        bump,
    }
}

/// Helper: Check if a program is in the whitelist
pub fn check_whitelist(
    registry: &crate::state::PolicyProgramRegistry,
    program: &Pubkey,
) -> Result<()> {
    require!(
        registry.registered_programs.contains(program),
        crate::error::LazorKitError::PolicyProgramNotRegistered
    );
    Ok(())
}

/// Same as `verify_authorization` but deserializes the challenge payload into the
/// caller-provided type `T`.
pub fn verify_authorization<M: crate::state::Message + AnchorDeserialize>(
    ix_sysvar: &AccountInfo,
    device: &crate::state::WalletDevice,
    smart_wallet_key: Pubkey,
    passkey_public_key: [u8; PASSKEY_PUBLIC_KEY_SIZE],
    signature: Vec<u8>,
    client_data_json_raw: &[u8],
    authenticator_data_raw: &[u8],
    verify_instruction_index: u8,
    last_nonce: u64,
) -> Result<M> {
    use anchor_lang::solana_program::sysvar::instructions::load_instruction_at_checked;
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};

    // 1) passkey & wallet checks
    require!(
        device.passkey_public_key == passkey_public_key,
        crate::error::LazorKitError::PasskeyMismatch
    );
    require!(
        device.smart_wallet_address == smart_wallet_key,
        crate::error::LazorKitError::SmartWalletDataMismatch
    );

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

    verify_secp256r1_instruction(&secp_ix, device.passkey_public_key, message, signature)?;
    // Verify header and return the typed message
    M::verify(challenge_bytes.clone(), last_nonce)?;
    let t: M = AnchorDeserialize::deserialize(&mut &challenge_bytes[..])
        .map_err(|_| crate::error::LazorKitError::ChallengeDeserializationError)?;
    Ok(t)
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy)]
pub struct HeaderView {
    pub nonce: u64,
    pub current_timestamp: i64,
}

pub trait HasHeader {
    fn header(&self) -> HeaderView;
}

impl HasHeader for ExecuteMessage {
    fn header(&self) -> HeaderView {
        HeaderView {
            nonce: self.nonce,
            current_timestamp: self.current_timestamp,
        }
    }
}
impl HasHeader for CallPolicyMessage {
    fn header(&self) -> HeaderView {
        HeaderView {
            nonce: self.nonce,
            current_timestamp: self.current_timestamp,
        }
    }
}
impl HasHeader for ChangePolicyMessage {
    fn header(&self) -> HeaderView {
        HeaderView {
            nonce: self.nonce,
            current_timestamp: self.current_timestamp,
        }
    }
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

/// Distribute fees to payer, referral, and lazorkit vault (empty PDA)
pub fn distribute_fees<'info>(
    config: &crate::state::Config,
    smart_wallet: &AccountInfo<'info>,
    payer: &AccountInfo<'info>,
    referral: &AccountInfo<'info>,
    lazorkit_vault: &AccountInfo<'info>,
    system_program: &Program<'info, System>,
    wallet_signer: PdaSigner,
) -> Result<()> {
    use anchor_lang::solana_program::system_instruction;

    // 1. Fee to payer (reimburse transaction fees)
    if config.fee_payer_fee > 0 {
        let transfer_to_payer =
            system_instruction::transfer(&smart_wallet.key(), &payer.key(), config.fee_payer_fee);

        execute_cpi(
            &[smart_wallet.clone(), payer.clone()],
            &transfer_to_payer.data,
            system_program,
            wallet_signer.clone(),
        )?;

    }

    // 2. Fee to referral
    if config.referral_fee > 0 {
        let transfer_to_referral =
            system_instruction::transfer(&smart_wallet.key(), &referral.key(), config.referral_fee);

        execute_cpi(
            &[smart_wallet.clone(), referral.clone()],
            &transfer_to_referral.data,
            system_program,
            wallet_signer.clone(),
        )?;

    }

    // 3. Fee to lazorkit vault (empty PDA)
    if config.lazorkit_fee > 0 {
        let transfer_to_vault = system_instruction::transfer(
            &smart_wallet.key(),
            &lazorkit_vault.key(),
            config.lazorkit_fee,
        );

        execute_cpi(
            &[smart_wallet.clone(), lazorkit_vault.clone()],
            &transfer_to_vault.data,
            system_program,
            wallet_signer.clone(),
        )?;

    }

    Ok(())
}
