use assertions::{check_zero_data, sol_assert_bytes_eq};
use pinocchio::{
    account_info::AccountInfo,
    instruction::Seed,
    program_error::ProgramError,
    pubkey::{find_program_address, Pubkey},
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
///    - Closes the `current_owner` account and refunds rent to payer.
///
/// # Accounts:
/// 1. `[signer, writable]` Payer.
/// 2. `[]` Wallet PDA.
/// 3. `[signer, writable]` Current Owner Authority.
/// 4. `[writable]` New Owner Authority.
/// 5. `[]` System Program.
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
            if rest.len() < 65 {
                // 32 (hash) + 33 (pubkey) minimum
                return Err(ProgramError::InvalidInstructionData);
            }
            let (hash, _rest_after_hash) = rest.split_at(32);
            let full_data = &rest[..65]; // hash + pubkey (33 bytes)
            (hash, full_data)
        },
        _ => return Err(AuthError::InvalidAuthenticationKind.into()),
    };

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
    let system_program = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;

    if wallet_pda.owner() != program_id || current_owner.owner() != program_id {
        return Err(ProgramError::IllegalOwner);
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

    if !current_owner.is_writable() {
        return Err(ProgramError::InvalidAccountData);
    }

    // Scope to borrow current owner data
    {
        let data = unsafe { current_owner.borrow_mut_data_unchecked() };
        if (data.as_ptr() as usize) % 8 != 0 {
            return Err(ProgramError::InvalidAccountData);
        }
        // SAFETY: Alignment checked.
        let auth = unsafe { &*(data.as_ptr() as *const AuthorityAccountHeader) };
        if auth.discriminator != AccountDiscriminator::Authority as u8 {
            return Err(ProgramError::InvalidAccountData);
        }
        if auth.wallet != *wallet_pda.key() {
            return Err(ProgramError::InvalidAccountData);
        }
        if auth.role != 0 {
            return Err(AuthError::PermissionDenied.into());
        }

        // Authenticate Current Owner
        // Issue: Include payer + new_owner to prevent rent theft via payer swap
        let mut ed25519_payload = Vec::with_capacity(64);
        ed25519_payload.extend_from_slice(payer.key().as_ref());
        ed25519_payload.extend_from_slice(new_owner.key().as_ref());

        match auth.authority_type {
            0 => {
                // Ed25519: Include payer + new_owner in signed payload
                Ed25519Authenticator.authenticate(accounts, data, &[], &ed25519_payload, &[3])?;
            },
            1 => {
                // Secp256r1 (WebAuthn) - Must be Writable
                if !current_owner.is_writable() {
                    return Err(ProgramError::InvalidAccountData);
                }
                // Secp256r1: Include payer in signed payload to prevent rent theft
                let mut extended_data_payload = Vec::with_capacity(data_payload.len() + 32);
                extended_data_payload.extend_from_slice(data_payload);
                extended_data_payload.extend_from_slice(payer.key().as_ref());

                Secp256r1Authenticator.authenticate(
                    accounts,
                    data,
                    authority_payload,
                    &extended_data_payload,
                    &[3],
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

    let header_size = std::mem::size_of::<AuthorityAccountHeader>();
    let variable_size = if args.auth_type == 1 {
        4 + full_auth_data.len()
    } else {
        full_auth_data.len()
    };
    let space = header_size + variable_size;
    let rent = (space as u64)
        .checked_mul(6960)
        .and_then(|val| val.checked_add(897840))
        .ok_or(ProgramError::ArithmeticOverflow)?;

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
    if (data.as_ptr() as usize) % 8 != 0 {
        return Err(ProgramError::InvalidAccountData);
    }
    let header = AuthorityAccountHeader {
        discriminator: AccountDiscriminator::Authority as u8,
        authority_type: args.auth_type,
        role: 0,
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
    if args.auth_type == 1 {
        variable_target[0..4].copy_from_slice(&0u32.to_le_bytes());
        variable_target[4..].copy_from_slice(full_auth_data);
    } else {
        variable_target.copy_from_slice(full_auth_data);
    }

    let current_lamports = unsafe { *current_owner.borrow_mut_lamports_unchecked() };
    let payer_lamports = unsafe { *payer.borrow_mut_lamports_unchecked() };
    unsafe {
        *payer.borrow_mut_lamports_unchecked() = payer_lamports
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
