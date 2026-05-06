use assertions::{check_zero_data, sol_assert_bytes_eq};
use pinocchio::{
    account_info::AccountInfo,
    instruction::Seed,
    program_error::ProgramError,
    pubkey::{find_program_address, Pubkey},
    sysvars::rent::Rent,
    ProgramResult,
};

use crate::{
    // Unified authentication helpers.
    auth::{
        ed25519::Ed25519Authenticator, secp256r1::Secp256r1Authenticator, traits::Authenticator,
    },
    error::AuthError,
    state::{authority::AuthorityAccountHeader, AccountDiscriminator},
};

/// Processes the `TransferOwnership` instruction.
///
/// atomically transfers the "Owner" role from the current authority to a new one.
/// The old owner is closed/removed, and the new one is created with `Role::Owner`.
///
/// # Logic:
/// 1. **Authentication**: Verifies the `current_owner` matches the request logic.
/// 2. **Authorization**: strictly enforced to only work if `current_owner` has `Role::Owner` (0).
/// 3. **Atomic Swap**:
///    - Creates the `new_owner` account.
///    - Closes the `current_owner` account and refunds rent to `refund_dest`.
///
/// # Accounts:
/// 1. `[signer, writable]` Payer.
/// 2. `[]` Wallet PDA.
/// 3. `[signer, writable]` Current Owner Authority.
/// 4. `[writable]` New Owner Authority.
/// 5. `[writable]` Refund Destination (receives closed current_owner rent).
/// 6. `[]` System Program.
/// 7. `[]` Rent Sysvar.
///
/// Arguments for the `TransferOwnership` instruction.
///
/// Layout:
/// - `new_type`: Authority Type (0=Ed25519, 1=Secp256r1).
/// - `pubkey`/`hash`: The identifier for the new authority.
#[derive(Debug)]
pub struct TransferOwnershipArgs {
    pub auth_type: u8,
}

impl TransferOwnershipArgs {
    pub fn from_bytes(data: &[u8]) -> Result<(Self, &[u8]), ProgramError> {
        if data.is_empty() {
            return Err(ProgramError::InvalidInstructionData);
        }
        let auth_type = data[0];
        let rest = &data[1..];

        Ok((Self { auth_type }, rest))
    }
}

pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let (args, rest) = TransferOwnershipArgs::from_bytes(instruction_data)?;

    let (id_seed, full_auth_data) = match args.auth_type {
        0 => {
            if rest.len() < 32 {
                return Err(ProgramError::InvalidInstructionData);
            }
            let (pubkey, _) = rest.split_at(32);
            (pubkey, pubkey)
        },
        1 => {
            // [credential_id_hash(32)] [pubkey(33)] [rpIdLen(1)] [rpId(N)]
            if rest.len() < 66 {
                return Err(ProgramError::InvalidInstructionData);
            }
            let (hash, rest_after_hash) = rest.split_at(32);
            let rp_id_len = rest_after_hash[33] as usize;
            if rp_id_len == 0 || rp_id_len > 253 {
                return Err(ProgramError::InvalidInstructionData);
            }
            let total_auth_data = 32 + 33 + 1 + rp_id_len;
            if rest.len() < total_auth_data {
                return Err(ProgramError::InvalidInstructionData);
            }
            let full_data = &rest[..total_auth_data];
            (hash, full_data)
        },
        _ => return Err(AuthError::InvalidAuthenticationKind.into()),
    };

    // Issue #15: Prevent transferring ownership to zero address / SystemProgram
    if id_seed.iter().all(|&x| x == 0) {
        return Err(ProgramError::InvalidAccountData);
    }

    // Split data_payload and authority_payload
    let data_payload_len = 1 + full_auth_data.len(); // auth_type + full_auth_data
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
    let current_owner = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let new_owner = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let refund_dest = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let system_program = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let rent_sysvar = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let rent_obj = Rent::from_account_info(rent_sysvar)?;

    if wallet_pda.owner() != program_id || current_owner.owner() != program_id {
        return Err(ProgramError::IllegalOwner);
    }

    // Guard: closing current_owner to itself would burn lamports.
    if current_owner.key() == refund_dest.key() {
        return Err(ProgramError::InvalidAccountData);
    }
    // Validate Wallet Discriminator (Issue #7)
    let wallet_data = unsafe { wallet_pda.borrow_data_unchecked() };
    if wallet_data.is_empty() || wallet_data[0] != AccountDiscriminator::Wallet as u8 {
        return Err(ProgramError::InvalidAccountData);
    }

    if !current_owner.is_writable() {
        return Err(ProgramError::InvalidAccountData);
    }

    // Scope to borrow current owner data
    {
        let data = unsafe { current_owner.borrow_mut_data_unchecked() };
        if (data.as_ptr() as usize) % 8 != 0 {
            return Err(ProgramError::InvalidAccountData);
        }
        // SAFETY: Use read_unaligned for safety
        let auth =
            unsafe { std::ptr::read_unaligned(data.as_ptr() as *const AuthorityAccountHeader) };
        if auth.discriminator != AccountDiscriminator::Authority as u8 {
            return Err(ProgramError::InvalidAccountData);
        }
        if auth.wallet != *wallet_pda.key() {
            return Err(ProgramError::InvalidAccountData);
        }
        if auth.role != 0 {
            return Err(AuthError::PermissionDenied.into());
        }

        // Authenticate Current Owner.
        // Sign over payer + new_owner + refund_dest to prevent substitution attacks.
        let mut ed25519_payload = Vec::with_capacity(96);
        ed25519_payload.extend_from_slice(payer.key().as_ref());
        ed25519_payload.extend_from_slice(new_owner.key().as_ref());
        ed25519_payload.extend_from_slice(refund_dest.key().as_ref());

        match auth.authority_type {
            0 => {
                // Ed25519: sign over payer + new_owner + refund_dest
                Ed25519Authenticator.authenticate(accounts, data, &[], &ed25519_payload, &[3], program_id)?;
            },
            1 => {
                // Secp256r1 (WebAuthn) - Must be Writable
                if !current_owner.is_writable() {
                    return Err(ProgramError::InvalidAccountData);
                }
                // Sign over data_payload + payer + refund_dest
                let mut extended_data_payload = Vec::with_capacity(data_payload.len() + 64);
                extended_data_payload.extend_from_slice(data_payload);
                extended_data_payload.extend_from_slice(payer.key().as_ref());
                extended_data_payload.extend_from_slice(refund_dest.key().as_ref());

                Secp256r1Authenticator.authenticate(
                    accounts,
                    data,
                    authority_payload,
                    &extended_data_payload,
                    &[3],
                    program_id,
                )?;
            },
            _ => return Err(AuthError::InvalidAuthenticationKind.into()),
        }
    }

    let (new_key, bump) = find_program_address(
        &[b"authority", wallet_pda.key().as_ref(), id_seed],
        program_id,
    );
    if !sol_assert_bytes_eq(new_owner.key().as_ref(), new_key.as_ref(), 32) {
        return Err(ProgramError::InvalidSeeds);
    }
    check_zero_data(new_owner, ProgramError::AccountAlreadyInitialized)?;

    // Fixed sizes per auth type (see wallet/create.rs for layout).
    let header_size = std::mem::size_of::<AuthorityAccountHeader>();
    let space = match args.auth_type {
        0 => header_size + 32,                // Ed25519: pubkey
        1 => header_size + 32 + 33 + 32,      // Secp256r1: cred ∥ pubkey ∥ rpIdHash
        _ => return Err(AuthError::InvalidAuthenticationKind.into()),
    };
    let rent = rent_obj.minimum_balance(space);

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
        new_owner,
        system_program,
        space,
        rent,
        program_id,
        &seeds,
    )?;

    let data = unsafe { new_owner.borrow_mut_data_unchecked() };
    if data.len() < std::mem::size_of::<AuthorityAccountHeader>() {
        return Err(ProgramError::InvalidAccountData);
    }
    let header = AuthorityAccountHeader {
        discriminator: AccountDiscriminator::Authority as u8,
        authority_type: args.auth_type,
        role: 0,
        bump,
        version: crate::state::CURRENT_ACCOUNT_VERSION,
        _padding1: [0; 3],
        counter: 0,
        _padding2: [0; 4],
        wallet: *wallet_pda.key(),
    };
    unsafe {
        std::ptr::write_unaligned(data.as_mut_ptr() as *mut AuthorityAccountHeader, header);
    }

    // Write variable data. For Secp256r1 hash rpId once here so every Execute
    // saves a sol_sha256 syscall.
    match args.auth_type {
        0 => {
            data[header_size..header_size + 32].copy_from_slice(&full_auth_data[..32]);
        }
        1 => {
            data[header_size..header_size + 32].copy_from_slice(&full_auth_data[..32]);
            data[header_size + 32..header_size + 32 + 33]
                .copy_from_slice(&full_auth_data[32..32 + 33]);
            let rp_id_len = full_auth_data[32 + 33] as usize;
            let rp_id = &full_auth_data[32 + 33 + 1..32 + 33 + 1 + rp_id_len];
            let rp_id_hash_offset = header_size + 32 + 33;
            #[cfg(target_os = "solana")]
            unsafe {
                let _ = pinocchio::syscalls::sol_sha256(
                    [rp_id].as_ptr() as *const u8,
                    1,
                    data[rp_id_hash_offset..rp_id_hash_offset + 32].as_mut_ptr(),
                );
            }
            #[cfg(not(target_os = "solana"))]
            {
                let _ = rp_id;
                data[rp_id_hash_offset..rp_id_hash_offset + 32].fill(0);
            }
        }
        _ => unreachable!(),
    }

    let current_lamports = unsafe { *current_owner.borrow_mut_lamports_unchecked() };
    let refund_lamports = unsafe { *refund_dest.borrow_mut_lamports_unchecked() };
    unsafe {
        *refund_dest.borrow_mut_lamports_unchecked() = refund_lamports
            .checked_add(current_lamports)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        *current_owner.borrow_mut_lamports_unchecked() = 0;
    }
    let current_data = unsafe { current_owner.borrow_mut_data_unchecked() };
    current_data.fill(0);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transfer_ownership_args_from_bytes() {
        // [type(1)][rest...]
        let mut data = Vec::new();
        data.push(1); // Secp256r1
        let payload = [5u8; 65];
        data.extend_from_slice(&payload);

        let (args, rest) = TransferOwnershipArgs::from_bytes(&data).unwrap();
        assert_eq!(args.auth_type, 1);
        assert_eq!(rest, &payload);
    }

    #[test]
    fn test_transfer_ownership_args_too_short() {
        let data = vec![];
        assert!(TransferOwnershipArgs::from_bytes(&data).is_err());
    }
}
