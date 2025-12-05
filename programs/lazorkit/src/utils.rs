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

// Constants for Secp256r1 signature verification
const SECP_HEADER_SIZE: u16 = 14;
const SECP_DATA_START: u16 = 2 + SECP_HEADER_SIZE;
const SECP_PUBKEY_SIZE: u16 = 33;
const SECP_SIGNATURE_SIZE: u16 = 64;
const SECP_HEADER_TOTAL: usize = 16;

#[derive(Clone, Debug)]
pub struct PdaSigner {
    pub seeds: Vec<Vec<u8>>,
    pub bump: u8,
}

impl PdaSigner {
    pub fn get_pda(&self) -> Pubkey {
        let mut seed_slices: Vec<&[u8]> = self.seeds.iter().map(|s| s.as_slice()).collect();
        let bump = &[self.bump];
        seed_slices.push(bump);
        Pubkey::create_program_address(&seed_slices, &ID).unwrap()
    }
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
    signer: &PdaSigner,
) -> Result<Vec<u8>> {
    let ix = create_cpi_instruction_optimized(accounts, data, program, &signer);

    let mut seed_slices: Vec<&[u8]> = signer.seeds.iter().map(|s| s.as_slice()).collect();
    let bump_slice = &[signer.bump];
    seed_slices.push(bump_slice);
    let signer_addr = Pubkey::create_program_address(&seed_slices, &ID).unwrap();

    require!(
        accounts.iter().any(|acc| *acc.key == signer_addr),
        LazorKitError::InvalidInstruction
    );

    invoke_signed(&ix, accounts, &[&seed_slices])?;

    if let Some((_program_id, return_data)) = get_return_data() {
        Ok(return_data)
    } else {
        Ok(Vec::new())
    }
}

fn create_cpi_instruction_optimized(
    accounts: &[AccountInfo],
    data: &[u8],
    program: &AccountInfo,
    pda_signer: &PdaSigner,
) -> Instruction {
    create_cpi_instruction_multiple_signers(accounts, data, program, &[pda_signer.clone()])
}

fn create_cpi_instruction_multiple_signers(
    accounts: &[AccountInfo],
    data: &[u8],
    program: &AccountInfo,
    pda_signers: &[PdaSigner],
) -> Instruction {
    let mut pda_pubkeys = Vec::new();
    for signer in pda_signers {
        let pda_pubkey = signer.get_pda();
        pda_pubkeys.push(pda_pubkey);
    }

    Instruction {
        program_id: program.key(),
        accounts: accounts
            .iter()
            .map(|acc| {
                let is_pda_signer = pda_pubkeys.contains(acc.key);
                AccountMeta {
                    pubkey: *acc.key,
                    is_signer: is_pda_signer,
                    is_writable: acc.is_writable,
                }
            })
            .collect(),
        data: data.to_vec(),
    }
}

pub fn verify_secp256r1_instruction(
    ix: &Instruction,
    pubkey: [u8; PASSKEY_PUBLIC_KEY_SIZE],
    msg: Vec<u8>,
    sig: [u8; 64],
) -> Result<()> {
    let expected_len =
        (SECP_DATA_START + SECP_PUBKEY_SIZE + SECP_SIGNATURE_SIZE) as usize + msg.len();

    if ix.program_id != SECP256R1_PROGRAM_ID
        || !ix.accounts.is_empty()
        || ix.data.len() != expected_len
    {
        return Err(LazorKitError::Secp256r1InvalidLength.into());
    }

    verify_secp256r1_data(&ix.data, pubkey, msg, sig)
}

fn verify_secp256r1_data(
    data: &[u8],
    public_key: [u8; PASSKEY_PUBLIC_KEY_SIZE],
    message: Vec<u8>,
    signature: [u8; 64],
) -> Result<()> {
    let msg_len = message.len() as u16;
    let offsets = calculate_secp_offsets(msg_len);

    if !verify_secp_header(data, &offsets) {
        return Err(LazorKitError::Secp256r1HeaderMismatch.into());
    }

    if !verify_secp_data(data, &public_key, &signature, &message) {
        return Err(LazorKitError::Secp256r1DataMismatch.into());
    }

    Ok(())
}

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

pub fn sighash(namespace: &str, name: &str) -> [u8; 8] {
    let preimage = format!("{}:{}", namespace, name);
    let mut out = [0u8; 8];
    out.copy_from_slice(
        &anchor_lang::solana_program::hash::hash(preimage.as_bytes()).to_bytes()[..8],
    );
    out
}

pub fn create_wallet_device_hash(smart_wallet: Pubkey, credential_hash: [u8; 32]) -> [u8; 32] {
    let mut buf = [0u8; 64];
    buf[..32].copy_from_slice(&smart_wallet.to_bytes());
    buf[32..].copy_from_slice(&credential_hash);
    hash(&buf).to_bytes()
}

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

    let secp_ix = load_instruction_at_checked(verify_instruction_index as usize, ix_sysvar)?;

    let client_hash = hash(client_data_json_raw);
    let mut message = Vec::with_capacity(authenticator_data_raw.len() + client_hash.as_ref().len());
    message.extend_from_slice(authenticator_data_raw);
    message.extend_from_slice(client_hash.as_ref());

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
    SimpleMessage::verify_hash(challenge_bytes, expected_hash)?;
    Ok(())
}

pub fn compute_instruction_hash(
    instruction_data: &[u8],
    instruction_accounts: &[AccountInfo],
    program_id: Pubkey,
) -> Result<[u8; 32]> {
    use anchor_lang::solana_program::hash::{hash, Hasher};

    let data_hash = hash(instruction_data);

    let mut rh = Hasher::default();
    rh.hash(program_id.as_ref());
    for account in instruction_accounts.iter() {
        rh.hash(account.key().as_ref());
        rh.hash(&[account.is_signer as u8]);
        rh.hash(&[account.is_writable as u8]);
    }
    let accounts_hash = rh.result();

    let mut combined = [0u8; 64];
    combined[..32].copy_from_slice(data_hash.as_ref());
    combined[32..].copy_from_slice(accounts_hash.as_ref());

    Ok(hash(&combined).to_bytes())
}

fn compute_message_hash(
    nonce: u64,
    timestamp: i64,
    hash1: [u8; 32],
    hash2: Option<[u8; 32]>,
) -> Result<[u8; 32]> {
    use anchor_lang::solana_program::hash::hash;

    let mut data = Vec::new();
    data.extend_from_slice(&nonce.to_le_bytes());
    data.extend_from_slice(&timestamp.to_le_bytes());
    data.extend_from_slice(&hash1);

    if let Some(h2) = hash2 {
        data.extend_from_slice(&h2);
    }

    Ok(hash(&data).to_bytes())
}

pub fn compute_execute_message_hash(
    nonce: u64,
    timestamp: i64,
    policy_hash: [u8; 32],
    cpi_hash: [u8; 32],
) -> Result<[u8; 32]> {
    compute_message_hash(nonce, timestamp, policy_hash, Some(cpi_hash))
}

pub fn compute_create_chunk_message_hash(
    nonce: u64,
    timestamp: i64,
    policy_hash: [u8; 32],
    cpi_hash: [u8; 32],
) -> Result<[u8; 32]> {
    compute_message_hash(nonce, timestamp, policy_hash, Some(cpi_hash))
}

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

pub fn calculate_account_ranges(
    accounts: &[AccountInfo],
    split_indices: &[u8],
) -> Result<Vec<(usize, usize)>> {
    let mut account_ranges = Vec::new();
    let mut start = 0usize;

    for &split_point in split_indices.iter() {
        let end = split_point as usize;
        require!(
            end > start && end <= accounts.len(),
            crate::error::LazorKitError::AccountSliceOutOfBounds
        );
        account_ranges.push((start, end));
        start = end;
    }

    require!(
        start < accounts.len(),
        crate::error::LazorKitError::AccountSliceOutOfBounds
    );
    account_ranges.push((start, accounts.len()));

    Ok(account_ranges)
}

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

        let program_account = &instruction_accounts[0];

        require!(
            program_account.executable,
            crate::error::LazorKitError::ProgramNotExecutable
        );

        require!(
            program_account.key() != crate::ID,
            crate::error::LazorKitError::ReentrancyDetected
        );
    }

    Ok(())
}

pub fn transfer_sol_util<'a>(
    smart_wallet: &AccountInfo<'a>,
    wallet_id: u64,
    bump: u8,
    recipient: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    fee: u64,
) -> Result<()> {
    let signer = PdaSigner {
        seeds: vec![SMART_WALLET_SEED.to_vec(), wallet_id.to_le_bytes().to_vec()],
        bump,
    };

    let transfer_ins = transfer(smart_wallet.key, recipient.key, fee);

    execute_cpi(
        &[smart_wallet.to_account_info(), recipient.to_account_info()],
        &transfer_ins.data,
        system_program,
        &signer,
    )?;

    Ok(())
}
