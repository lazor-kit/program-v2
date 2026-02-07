use assertions::{check_zero_data, sol_assert_bytes_eq};
use no_padding::NoPadding;
use pinocchio::{
    account_info::AccountInfo,
    instruction::Seed,
    program_error::ProgramError,
    pubkey::{find_program_address, Pubkey},
    sysvars::rent::Rent,
    ProgramResult,
};

use crate::{
    auth::{
        ed25519::Ed25519Authenticator, secp256r1::Secp256r1Authenticator, traits::Authenticator,
    },
    error::AuthError,
    state::{authority::AuthorityAccountHeader, AccountDiscriminator},
};

/// Arguments for the `AddAuthority` instruction.
///
/// Layout:
/// - `authority_type`: 0 for Ed25519, 1 for Secp256r1.
/// - `new_role`: Role to assign (0=Owner, 1=Admin, 2=Spender).
/// - `_padding`: Reserved to align to 8-byte boundary.
#[repr(C, align(8))]
#[derive(NoPadding)]
pub struct AddAuthorityArgs {
    pub authority_type: u8,
    pub new_role: u8,
    pub _padding: [u8; 6],
}

impl AddAuthorityArgs {
    pub fn from_bytes(data: &[u8]) -> Result<(Self, &[u8]), ProgramError> {
        if data.len() < 8 {
            return Err(ProgramError::InvalidInstructionData);
        }
        let (fixed, rest) = data.split_at(8);

        // Manual deserialization for safety
        let authority_type = fixed[0];
        let new_role = fixed[1];

        let args = Self {
            authority_type,
            new_role,
            _padding: [0; 6],
        };

        Ok((args, rest))
    }
}

/// Processes the `AddAuthority` instruction.
///
/// Adds a new authority to the wallet.
///
/// # Logic:
/// 1. **Authentication**: Verifies the `admin_authority` (must be Admin or Owner).
/// 2. **Authorization**: Checks permission levels:
///    - `Owner` (0) can add any role.
///    - `Admin` (1) can only add `Spender` (2).
/// 3. **Execution**: Creates a new PDA `["authority", wallet, id_hash]` and initializes it.
///
/// # Accounts:
/// 1. `[signer, writable]` Payer.
/// 2. `[]` Wallet PDA.
/// 3. `[signer]` Admin Authority: Existing authority authorizing this action.
/// 4. `[writable]` New Authority: The PDA to create.
/// 5. `[]` System Program.
pub fn process_add_authority(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let (args, rest) = AddAuthorityArgs::from_bytes(instruction_data)?;

    let (id_seed, full_auth_data) = match args.authority_type {
        0 => {
            if rest.len() < 32 {
                return Err(ProgramError::InvalidInstructionData);
            }
            let (pubkey, _) = rest.split_at(32);
            (pubkey, pubkey)
        },
        1 => {
            if rest.len() < 32 {
                return Err(ProgramError::InvalidInstructionData);
            }
            let (hash, rest_after_hash) = rest.split_at(32);
            // Expecting 33-byte COMPRESSED pubkey for storage (efficient state)
            if rest_after_hash.len() < 33 {
                return Err(ProgramError::InvalidInstructionData);
            }
            let full_data = &rest[..32 + 33]; // hash + pubkey
            (hash, full_data)
        },
        _ => return Err(AuthError::InvalidAuthenticationKind.into()),
    };

    // Split data_payload and authority_payload
    // data_payload = everything up to and including the new authority data
    let data_payload_len = 8 + full_auth_data.len(); // args + full_auth_data
    if instruction_data.len() < data_payload_len {
        return Err(ProgramError::InvalidInstructionData);
    }
    let (data_payload, authority_payload) = instruction_data.split_at(data_payload_len);

    let account_info_iter = &mut accounts.iter();
    let payer = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let wallet_pda = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let admin_auth_pda = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let new_auth_pda = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let system_program = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;

    if wallet_pda.owner() != program_id {
        return Err(ProgramError::IllegalOwner);
    }
    if admin_auth_pda.owner() != program_id {
        return Err(ProgramError::IllegalOwner);
    }
    // Validate Wallet Discriminator (Issue #7)
    let wallet_data = unsafe { wallet_pda.borrow_data_unchecked() };
    if wallet_data.is_empty() || wallet_data[0] != AccountDiscriminator::Wallet as u8 {
        return Err(ProgramError::InvalidAccountData);
    }

    // Validate Wallet Discriminator (Issue #7)
    let wallet_data = unsafe { wallet_pda.borrow_data_unchecked() };
    if wallet_data.is_empty() || wallet_data[0] != AccountDiscriminator::Wallet as u8 {
        return Err(ProgramError::InvalidAccountData);
    }

    // Validate system_program is the correct System Program (audit N2)
    if !sol_assert_bytes_eq(
        system_program.key().as_ref(),
        &crate::utils::SYSTEM_PROGRAM_ID,
        32,
    ) {
        return Err(ProgramError::IncorrectProgramId);
    }
    let rent_sysvar_info = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let rent = Rent::from_account_info(rent_sysvar_info)?;

    // Check removed here, moved to type-specific logic
    // if !admin_auth_pda.is_writable() {
    //    return Err(ProgramError::InvalidAccountData);
    // }

    let admin_data = unsafe { admin_auth_pda.borrow_mut_data_unchecked() };
    if admin_data.len() < std::mem::size_of::<AuthorityAccountHeader>() {
        return Err(ProgramError::InvalidAccountData);
    }

    // Safe Copy of Header
    let mut header_bytes = [0u8; std::mem::size_of::<AuthorityAccountHeader>()];
    header_bytes.copy_from_slice(&admin_data[..std::mem::size_of::<AuthorityAccountHeader>()]);
    let admin_header =
        unsafe { std::mem::transmute::<&[u8; 48], &AuthorityAccountHeader>(&header_bytes) };

    if admin_header.discriminator != AccountDiscriminator::Authority as u8 {
        return Err(ProgramError::InvalidAccountData);
    }
    if admin_header.wallet != *wallet_pda.key() {
        return Err(ProgramError::InvalidAccountData);
    }

    // Unified Authentication
    // Include payer + target in signed payload to prevent account swap attacks
    let mut ed25519_payload = Vec::with_capacity(64);
    ed25519_payload.extend_from_slice(payer.key().as_ref());
    ed25519_payload.extend_from_slice(new_auth_pda.key().as_ref());

    match admin_header.authority_type {
        0 => {
            // Ed25519: Include payer + new_auth_pda in signed payload
            Ed25519Authenticator.authenticate(accounts, admin_data, &[], &ed25519_payload, &[1])?;
        },
        1 => {
            // Secp256r1 (WebAuthn) - Must be Writable
            if !admin_auth_pda.is_writable() {
                return Err(ProgramError::InvalidAccountData);
            }
            // Secp256r1: Include payer in signed payload
            let mut extended_data_payload = Vec::with_capacity(data_payload.len() + 32);
            extended_data_payload.extend_from_slice(data_payload);
            extended_data_payload.extend_from_slice(payer.key().as_ref());

            Secp256r1Authenticator.authenticate(
                accounts,
                admin_data,
                authority_payload,
                &extended_data_payload,
                &[1],
            )?;
        },
        _ => return Err(AuthError::InvalidAuthenticationKind.into()),
    }

    // Authorization
    if admin_header.role != 0 && (admin_header.role != 1 || args.new_role != 2) {
        return Err(AuthError::PermissionDenied.into());
    }

    // Logic
    let (new_auth_key, bump) = find_program_address(
        &[b"authority", wallet_pda.key().as_ref(), id_seed],
        program_id,
    );
    if !sol_assert_bytes_eq(new_auth_pda.key().as_ref(), new_auth_key.as_ref(), 32) {
        return Err(ProgramError::InvalidSeeds);
    }
    check_zero_data(new_auth_pda, ProgramError::AccountAlreadyInitialized)?;

    let header_size = std::mem::size_of::<AuthorityAccountHeader>();
    // Secp256r1 needs extra 4 bytes for counter prefix
    let variable_size = if args.authority_type == 1 {
        4 + full_auth_data.len()
    } else {
        full_auth_data.len()
    };
    let space = header_size + variable_size;
    let rent_lamports = rent.minimum_balance(space);

    // Use secure transfer-allocate-assign pattern to prevent DoS (Issue #4)
    let bump_arr = [bump];
    let seeds = [
        Seed::from(b"authority"),
        Seed::from(wallet_pda.key().as_ref()),
        Seed::from(id_seed),
        Seed::from(&bump_arr),
    ];

    crate::utils::initialize_pda_account(
        payer,
        new_auth_pda,
        system_program,
        space,
        rent_lamports,
        program_id,
        &seeds,
    )?;

    let data = unsafe { new_auth_pda.borrow_mut_data_unchecked() };
    let header = AuthorityAccountHeader {
        discriminator: AccountDiscriminator::Authority as u8,
        authority_type: args.authority_type,
        role: args.new_role,
        bump,
        version: crate::state::CURRENT_ACCOUNT_VERSION,
        _padding: [0; 3],
        counter: 0,
        wallet: *wallet_pda.key(),
    };
    unsafe {
        *(data.as_mut_ptr() as *mut AuthorityAccountHeader) = header;
    }

    let variable_target = &mut data[header_size..];
    if args.authority_type == 1 {
        variable_target[0..4].copy_from_slice(&0u32.to_le_bytes());
        variable_target[4..].copy_from_slice(full_auth_data);
    } else {
        variable_target.copy_from_slice(full_auth_data);
    }

    Ok(())
}

/// Processes the `RemoveAuthority` instruction.
///
/// Removes an existing authority and refunds rent to the destination.
///
/// # Logic:
/// 1. **Authentication**: Verifies the `admin_authority`.
/// 2. **Authorization**:
///    - `Owner` can remove anyone (except potentially the last owner, though not explicitly enforced here).
///    - `Admin` can only remove `Spender`.
/// 3. **Execution**: Securely closes the account by zeroing data and transferring lamports.
///
/// # Accounts:
/// 1. `[signer]` Payer.
/// 2. `[]` Wallet PDA.
/// 3. `[signer]` Admin Authority.
/// 4. `[writable]` Target Authority: PDA to verify and close.
/// 5. `[writable]` Refund Destination.
pub fn process_remove_authority(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    // For RemoveAuthority, all instruction_data is authority_payload
    // Issue #13: Bind signature to specific target accounts to prevent reuse
    let authority_payload = instruction_data;

    // Build data_payload with target pubkeys (computed after parsing accounts)

    let account_info_iter = &mut accounts.iter();
    let _payer = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let wallet_pda = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let admin_auth_pda = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let target_auth_pda = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let refund_dest = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;

    if wallet_pda.owner() != program_id
        || admin_auth_pda.owner() != program_id
        || target_auth_pda.owner() != program_id
    {
        return Err(ProgramError::IllegalOwner);
    }

    if !admin_auth_pda.is_writable() {
        return Err(ProgramError::InvalidAccountData);
    }

    // Safe copy header
    let admin_data = unsafe { admin_auth_pda.borrow_mut_data_unchecked() };
    let mut header_bytes = [0u8; std::mem::size_of::<AuthorityAccountHeader>()];
    header_bytes.copy_from_slice(&admin_data[..std::mem::size_of::<AuthorityAccountHeader>()]);
    let admin_header =
        unsafe { std::mem::transmute::<&[u8; 48], &AuthorityAccountHeader>(&header_bytes) };

    if admin_header.discriminator != AccountDiscriminator::Authority as u8 {
        return Err(ProgramError::InvalidAccountData);
    }
    if admin_header.wallet != *wallet_pda.key() {
        return Err(ProgramError::InvalidAccountData);
    }

    // Issue #13: Build data_payload with target pubkeys to prevent signature reuse
    // Signature is now bound to specific target_auth_pda and refund_dest
    let mut data_payload = Vec::with_capacity(64);
    data_payload.extend_from_slice(target_auth_pda.key().as_ref());
    data_payload.extend_from_slice(refund_dest.key().as_ref());

    // Authentication
    match admin_header.authority_type {
        0 => {
            // Ed25519: Include data_payload in signature verification
            Ed25519Authenticator.authenticate(accounts, admin_data, &[], &data_payload, &[2])?;
        },
        1 => {
            Secp256r1Authenticator.authenticate(
                accounts,
                admin_data,
                authority_payload,
                &data_payload,
                &[2],
            )?;
        },
        _ => return Err(AuthError::InvalidAuthenticationKind.into()),
    }

    // Authorization - ALWAYS validate target authority
    let target_data = unsafe { target_auth_pda.borrow_data_unchecked() };
    // Safe copy target header
    let mut target_h_bytes = [0u8; std::mem::size_of::<AuthorityAccountHeader>()];
    target_h_bytes.copy_from_slice(&target_data[..std::mem::size_of::<AuthorityAccountHeader>()]);
    let target_header =
        unsafe { std::mem::transmute::<&[u8; 48], &AuthorityAccountHeader>(&target_h_bytes) };

    // ALWAYS verify discriminator
    if target_header.discriminator != AccountDiscriminator::Authority as u8 {
        return Err(ProgramError::InvalidAccountData);
    }

    // ALWAYS verify target belongs to THIS wallet (CRITICAL SECURITY CHECK)
    if target_header.wallet != *wallet_pda.key() {
        return Err(ProgramError::InvalidAccountData);
    }

    // Role-based permission check
    if admin_header.role != 0 {
        // Admin can only remove Spender
        if admin_header.role != 1 || target_header.role != 2 {
            return Err(AuthError::PermissionDenied.into());
        }
    }

    let target_lamports = unsafe { *target_auth_pda.borrow_mut_lamports_unchecked() };
    let refund_lamports = unsafe { *refund_dest.borrow_mut_lamports_unchecked() };
    unsafe {
        *refund_dest.borrow_mut_lamports_unchecked() = refund_lamports
            .checked_add(target_lamports)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        *target_auth_pda.borrow_mut_lamports_unchecked() = 0;
    }
    let target_data = unsafe { target_auth_pda.borrow_mut_data_unchecked() };
    target_data.fill(0);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_authority_args_from_bytes() {
        // [type(1)][role(1)][padding(6)]
        let mut data = Vec::new();
        data.push(0); // Ed25519
        data.push(2); // Spender
        data.extend_from_slice(&[0; 6]); // padding

        let extra_data = [1u8; 32];
        data.extend_from_slice(&extra_data);

        let (args, rest) = AddAuthorityArgs::from_bytes(&data).unwrap();
        assert_eq!(args.authority_type, 0);
        assert_eq!(args.new_role, 2);
        assert_eq!(rest, &extra_data);
    }

    #[test]
    fn test_add_authority_args_too_short() {
        let data = vec![0u8; 7]; // Need 8
        assert!(AddAuthorityArgs::from_bytes(&data).is_err());
    }
}
