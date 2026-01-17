//! UpdateAuthority instruction handler

use lazorkit_interface::{VerifyInstruction, INSTRUCTION_VERIFY};
use lazorkit_state::{
    policy::PolicyHeader, IntoBytes, LazorKitWallet, Position, RoleIterator, Transmutable,
    TransmutableMut,
};
use pinocchio::{
    account_info::AccountInfo,
    instruction::{AccountMeta, Instruction},
    msg,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvars::{rent::Rent, Sysvar},
    ProgramResult,
};
use pinocchio_system::instructions::Transfer;

use crate::actions::verify_policy_registry;
use crate::error::LazorKitError;
use crate::instruction::UpdateOperation;

pub fn process_update_authority(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    acting_role_id: u32,
    target_role_id: u32,
    operation: u8,
    payload: Vec<u8>,
) -> ProgramResult {
    let mut account_info_iter = accounts.iter();
    let config_account = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let payer_account = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let _system_program = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;

    if !payer_account.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    if config_account.owner() != program_id {
        return Err(ProgramError::IllegalOwner);
    }

    let op = UpdateOperation::try_from(operation)?;

    // Only Owner can update authorities for now
    if acting_role_id != 0 {
        msg!("Only Owner can update authorities");
        return Err(LazorKitError::Unauthorized.into());
    }

    msg!(
        "UpdateAuthority: target={}, operation={:?}, payload_len={}",
        target_role_id,
        op,
        payload.len()
    );

    match op {
        UpdateOperation::ReplaceAll => {
            let registry_accounts = &accounts[3..];
            process_replace_all(
                config_account,
                payer_account,
                target_role_id,
                &payload,
                registry_accounts,
            )
        },
        UpdateOperation::AddPolicies => {
            let registry_accounts = &accounts[3..];
            process_add_policies(
                config_account,
                payer_account,
                target_role_id,
                &payload,
                registry_accounts,
            )
        },
        UpdateOperation::RemoveByType => {
            process_remove_by_type(config_account, payer_account, target_role_id, &payload)
        },
        UpdateOperation::RemoveByIndex => {
            process_remove_by_index(config_account, payer_account, target_role_id, &payload)
        },
    }
}

// Helper to extract program ID from payload
fn extract_policy_id(payload: &[u8], cursor: usize) -> Result<Pubkey, ProgramError> {
    if cursor + 32 > payload.len() {
        return Err(ProgramError::InvalidInstructionData);
    }
    let mut pk = [0u8; 32];
    pk.copy_from_slice(&payload[cursor..cursor + 32]);
    Ok(Pubkey::from(pk))
}

fn process_replace_all(
    config_account: &AccountInfo,
    payer_account: &AccountInfo,
    target_role_id: u32,
    payload: &[u8],
    registry_accounts: &[AccountInfo],
) -> ProgramResult {
    let mut cursor = 0;
    if payload.len() < 4 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let num_new_policies = u32::from_le_bytes(payload[0..4].try_into().unwrap());
    cursor += 4;

    let mut new_policies_total_size = 0;
    let mut new_policies_regions = Vec::new();

    for _ in 0..num_new_policies {
        if cursor + 34 > payload.len() {
            return Err(ProgramError::InvalidInstructionData);
        }
        let data_len =
            u16::from_le_bytes(payload[cursor + 32..cursor + 34].try_into().unwrap()) as usize;
        // Input payload includes full header (40 bytes)
        let policy_total_len = PolicyHeader::LEN + data_len;

        if cursor + policy_total_len > payload.len() {
            return Err(ProgramError::InvalidInstructionData);
        }

        let storage_len = PolicyHeader::LEN + data_len;
        new_policies_total_size += storage_len;

        new_policies_regions.push((cursor, policy_total_len));

        // Registry Verification
        let pid = extract_policy_id(payload, cursor)?;
        verify_policy_registry(config_account.owner(), &pid, registry_accounts)?;

        cursor += policy_total_len;
    }

    let mut config_data = config_account.try_borrow_mut_data()?;
    let wallet = unsafe { LazorKitWallet::load_unchecked(&config_data[..LazorKitWallet::LEN])? };

    // Find role info
    let (role_offset, mut role_pos, current_policies_size) = {
        let mut offset = LazorKitWallet::LEN;
        let mut found = None;

        for (pos, _, policies_data) in
            RoleIterator::new(&config_data, wallet.role_count, LazorKitWallet::LEN)
        {
            if pos.id == target_role_id {
                // role_offset needs to point to Role start (where Position is).
                found = Some((offset, pos, policies_data.len()));
                break;
            }
            offset = pos.boundary as usize;
        }
        found.ok_or(LazorKitError::AuthorityNotFound)?
    };

    let size_diff = new_policies_total_size as isize - current_policies_size as isize;

    drop(config_data);
    if size_diff > 0 {
        let new_len = (config_account.data_len() as isize + size_diff) as usize;
        reallocate_account(config_account, payer_account, new_len)?;
    }

    let mut config_data = config_account.try_borrow_mut_data()?;
    let policies_start_offset = role_offset + Position::LEN + role_pos.authority_length as usize;
    let shift_start_index = policies_start_offset + current_policies_size;

    if size_diff != 0 {
        if size_diff > 0 {
            let move_amt = size_diff as usize;
            let src_end = config_data.len() - move_amt;
            config_data.copy_within(shift_start_index..src_end, shift_start_index + move_amt);
        } else {
            let move_amt = (-size_diff) as usize;
            config_data.copy_within(shift_start_index.., shift_start_index - move_amt);
        }

        role_pos.num_policies = num_new_policies as u16;
        let mut dest = &mut config_data[role_offset..role_offset + Position::LEN];
        dest.copy_from_slice(role_pos.into_bytes()?);

        // Update subsequent roles
        let wallet_header =
            unsafe { LazorKitWallet::load_unchecked(&config_data[..LazorKitWallet::LEN])? };
        let mut apply_diff = false;
        let mut offset = LazorKitWallet::LEN;

        for _ in 0..wallet_header.role_count {
            if offset >= config_data.len() {
                break;
            }

            let pos_slice = &mut config_data[offset..offset + Position::LEN];
            let mut p = *unsafe { Position::load_unchecked(pos_slice)? };

            if apply_diff {
                p.boundary = (p.boundary as isize + size_diff) as u32;
                pos_slice.copy_from_slice(p.into_bytes()?);
            }

            if p.id == target_role_id {
                apply_diff = true;
            }

            if offset >= config_data.len() {
                break;
            }

            offset = p.boundary as usize;
            if offset >= config_data.len() {
                break;
            }
        }
    }

    // Write new policies
    let mut write_cursor = policies_start_offset;
    for (offset, len) in new_policies_regions {
        let item_slice = &payload[offset..offset + len];
        let program_id_bytes = &item_slice[0..32];
        let data_len_bytes = &item_slice[32..34];
        // Skip Padding(2) + Boundary(4) = 6 bytes from input
        let data_bytes = &item_slice[PolicyHeader::LEN..];

        config_data[write_cursor..write_cursor + 32].copy_from_slice(program_id_bytes);
        write_cursor += 32;

        config_data[write_cursor..write_cursor + 2].copy_from_slice(data_len_bytes);
        write_cursor += 2;

        let data_len = data_bytes.len();
        // Boundary must be relative to policies_start_offset
        // Structure: [ProgramID(32)] [Len(2)] [Padding(2)] [Boundary(4)] [Data...]
        // Total Header = 40 bytes

        let boundary = (write_cursor - policies_start_offset) as u32 + 6 + (data_len as u32);

        // Write Padding (2 bytes explicitly to 0)
        config_data[write_cursor..write_cursor + 2].fill(0);
        write_cursor += 2;

        // Write Boundary
        config_data[write_cursor..write_cursor + 4].copy_from_slice(&boundary.to_le_bytes());
        write_cursor += 4;

        config_data[write_cursor..write_cursor + data_len].copy_from_slice(data_bytes);
        write_cursor += data_len;
    }

    if size_diff < 0 {
        let new_len = (config_account.data_len() as isize + size_diff) as usize;
        reallocate_account(config_account, payer_account, new_len)?;
    }

    msg!("ReplaceAll complete");
    Ok(())
}

fn process_add_policies(
    config_account: &AccountInfo,
    payer_account: &AccountInfo,
    target_role_id: u32,
    payload: &[u8],
    registry_accounts: &[AccountInfo],
) -> ProgramResult {
    let mut cursor = 0;
    if payload.len() < 4 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let num_new_policies = u32::from_le_bytes(payload[0..4].try_into().unwrap());
    cursor += 4;

    let mut new_policies_total_size = 0;
    let mut new_policies_regions = Vec::new();

    for _ in 0..num_new_policies {
        if cursor + 34 > payload.len() {
            return Err(ProgramError::InvalidInstructionData);
        }
        let data_len =
            u16::from_le_bytes(payload[cursor + 32..cursor + 34].try_into().unwrap()) as usize;
        // Input payload includes full header (40 bytes)
        let policy_total_len = PolicyHeader::LEN + data_len;
        if cursor + policy_total_len > payload.len() {
            return Err(ProgramError::InvalidInstructionData);
        }
        let storage_len = PolicyHeader::LEN + data_len;
        new_policies_total_size += storage_len;
        new_policies_regions.push((cursor, policy_total_len));

        // Registry Verification
        let pid = extract_policy_id(payload, cursor)?;
        verify_policy_registry(config_account.owner(), &pid, registry_accounts)?;

        cursor += policy_total_len;
    }

    let mut config_data = config_account.try_borrow_mut_data()?;
    let wallet = unsafe { LazorKitWallet::load_unchecked(&config_data[..LazorKitWallet::LEN])? };

    let (role_offset, mut role_pos, current_policies_size) = {
        let mut offset = LazorKitWallet::LEN;
        let mut found = None;
        for (pos, _, policies_data) in
            RoleIterator::new(&config_data, wallet.role_count, LazorKitWallet::LEN)
        {
            if pos.id == target_role_id {
                found = Some((offset, pos, policies_data.len()));
                break;
            }
            offset = pos.boundary as usize;
        }
        found.ok_or(LazorKitError::AuthorityNotFound)?
    };

    drop(config_data);
    let new_len = config_account.data_len() + new_policies_total_size;
    reallocate_account(config_account, payer_account, new_len)?;

    let mut config_data = config_account.try_borrow_mut_data()?;
    let base_policies_offset = role_offset + Position::LEN + role_pos.authority_length as usize;
    let insert_at_offset = base_policies_offset + current_policies_size;

    let src_end = config_data.len() - new_policies_total_size;
    config_data.copy_within(
        insert_at_offset..src_end,
        insert_at_offset + new_policies_total_size,
    );

    let mut role_pos_ref = unsafe {
        Position::load_mut_unchecked(&mut config_data[role_offset..role_offset + Position::LEN])?
    };
    role_pos_ref.boundary += new_policies_total_size as u32;
    role_pos_ref.num_policies += num_new_policies as u16;

    let wallet_header =
        unsafe { LazorKitWallet::load_unchecked(&config_data[..LazorKitWallet::LEN])? };
    let mut apply_diff = false;
    let mut offset = LazorKitWallet::LEN;

    for _ in 0..wallet_header.role_count {
        if offset >= config_data.len() {
            break;
        }
        let pos_slice = &mut config_data[offset..offset + Position::LEN];
        let mut p = unsafe { Position::load_mut_unchecked(pos_slice)? };

        if apply_diff {
            p.boundary += new_policies_total_size as u32;
        }

        if p.id == target_role_id {
            apply_diff = true;
        }
        offset = p.boundary as usize;
    }

    let mut write_cursor = insert_at_offset;
    for (offset, len) in new_policies_regions {
        let item_slice = &payload[offset..offset + len];
        let program_id_bytes = &item_slice[0..32];
        let data_len_bytes = &item_slice[32..34];
        // Skip Padding(2) + Boundary(4) = 6 bytes from input
        let data_bytes = &item_slice[PolicyHeader::LEN..];

        config_data[write_cursor..write_cursor + 32].copy_from_slice(program_id_bytes);
        write_cursor += 32;
        config_data[write_cursor..write_cursor + 2].copy_from_slice(data_len_bytes);
        write_cursor += 2;

        let data_len = data_bytes.len();
        // Boundary must be relative to base_policies_offset
        // Structure: [ProgramID(32)] [Len(2)] [Padding(2)] [Boundary(4)] [Data...]
        let boundary = (write_cursor - base_policies_offset) as u32 + 6 + (data_len as u32);

        // Write Padding
        config_data[write_cursor..write_cursor + 2].fill(0);
        write_cursor += 2;

        // Write Boundary
        config_data[write_cursor..write_cursor + 4].copy_from_slice(&boundary.to_le_bytes());
        write_cursor += 4;

        config_data[write_cursor..write_cursor + data_len].copy_from_slice(data_bytes);
        write_cursor += data_len;
    }

    msg!("AddPolicies complete");
    Ok(())
}

fn process_remove_by_type(
    config_account: &AccountInfo,
    payer_account: &AccountInfo,
    target_role_id: u32,
    payload: &[u8],
) -> ProgramResult {
    if payload.len() < 32 {
        return Err(ProgramError::InvalidInstructionData);
    }
    // pubkey logic?
    // Pinocchio Pubkey construction from array?
    // Pubkey is wrapper struct around [u8; 32].
    // Assuming standard behavior or direct bytes.
    let target_policy_id_bytes = &payload[0..32];

    remove_policy(
        config_account,
        payer_account,
        target_role_id,
        |policy_id, _, _| policy_id.as_ref() == target_policy_id_bytes,
    )
}

fn process_remove_by_index(
    config_account: &AccountInfo,
    payer_account: &AccountInfo,
    target_role_id: u32,
    payload: &[u8],
) -> ProgramResult {
    if payload.len() < 4 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let target_index = u32::from_le_bytes(payload[0..4].try_into().unwrap());

    let mut current_index = 0;
    remove_policy(config_account, payer_account, target_role_id, |_, _, _| {
        let is_match = current_index == target_index;
        current_index += 1;
        is_match
    })
}

fn remove_policy<F>(
    config_account: &AccountInfo,
    payer_account: &AccountInfo,
    target_role_id: u32,
    mut match_fn: F,
) -> ProgramResult
where
    F: FnMut(&Pubkey, usize, &[u8]) -> bool,
{
    let mut config_data = config_account.try_borrow_mut_data()?;
    let wallet = unsafe { LazorKitWallet::load_unchecked(&config_data[..LazorKitWallet::LEN])? };

    let (role_offset, mut role_pos, policies_data_len) = {
        let mut offset = LazorKitWallet::LEN;
        let mut found = None;
        for (pos, _, policies_data) in
            RoleIterator::new(&config_data, wallet.role_count, LazorKitWallet::LEN)
        {
            if pos.id == target_role_id {
                found = Some((offset, pos, policies_data.len()));
                break;
            }
            offset = pos.boundary as usize;
        }
        found.ok_or(LazorKitError::AuthorityNotFound)?
    };

    let policies_start_offset = role_offset + Position::LEN + role_pos.authority_length as usize;
    let mut cursor = 0;
    let mut policy_region = None;

    while cursor < policies_data_len {
        let abs_cursor = policies_start_offset + cursor;
        if abs_cursor + 38 > config_data.len() {
            break;
        }

        // Read header without try_into unwraps if possible?
        // Pubkey from bytes
        let mut pk_arr = [0u8; 32];
        pk_arr.copy_from_slice(&config_data[abs_cursor..abs_cursor + 32]);
        let policy_id = Pubkey::from(pk_arr);

        let data_len = u16::from_le_bytes(
            config_data[abs_cursor + 32..abs_cursor + 34]
                .try_into()
                .unwrap(),
        ) as usize;
        let total_len = 32 + 2 + 2 + 4 + data_len; // 40 + data_len

        if match_fn(&policy_id, data_len, &[]) {
            policy_region = Some((cursor, total_len));
            break;
        }
        cursor += total_len;
    }

    let (remove_offset, remove_len) = policy_region.ok_or(ProgramError::InvalidArgument)?;
    let remove_start_abs = policies_start_offset + remove_offset;
    let src_start = remove_start_abs + remove_len;
    config_data.copy_within(src_start.., remove_start_abs);

    let mut role_pos_ref = unsafe {
        Position::load_mut_unchecked(&mut config_data[role_offset..role_offset + Position::LEN])?
    };
    role_pos_ref.boundary = role_pos_ref.boundary.saturating_sub(remove_len as u32);
    role_pos_ref.num_policies = role_pos_ref.num_policies.saturating_sub(1);

    let wallet_header =
        unsafe { LazorKitWallet::load_unchecked(&config_data[..LazorKitWallet::LEN])? };
    let mut apply_diff = false;
    let mut offset = LazorKitWallet::LEN;

    for _ in 0..wallet_header.role_count {
        if offset >= config_data.len() {
            break;
        }
        let pos_slice = &mut config_data[offset..offset + Position::LEN];
        let mut p = unsafe { Position::load_mut_unchecked(pos_slice)? };

        if apply_diff {
            p.boundary = p.boundary.saturating_sub(remove_len as u32);
        }

        if p.id == target_role_id {
            apply_diff = true;
        }
        offset = p.boundary as usize;
    }

    drop(config_data);
    let new_len = config_account.data_len().saturating_sub(remove_len);
    reallocate_account(config_account, payer_account, new_len)?;

    msg!("RemovePolicy complete");
    Ok(())
}

fn reallocate_account(
    account: &AccountInfo,
    payer: &AccountInfo,
    new_size: usize,
) -> ProgramResult {
    let rent = Rent::get()?;
    let new_minimum_balance = rent.minimum_balance(new_size);
    let lamports_diff = new_minimum_balance.saturating_sub(account.lamports());

    if lamports_diff > 0 {
        Transfer {
            from: payer,
            to: account,
            lamports: lamports_diff,
        }
        .invoke()?;
    }

    account.resize(new_size)?;
    Ok(())
}
