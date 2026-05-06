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
    error::AuthError,
    state::{authority::AuthorityAccountHeader, wallet::WalletAccount, AccountDiscriminator},
};

/// Arguments for the `CreateWallet` instruction.
///
/// Layout:
/// - `user_seed`: 32-byte seed for deterministic wallet derivation.
/// - `authority_type`: 0 for Ed25519, 1 for Secp256r1.
/// - `auth_bump`: Bump seed for the authority PDA (optional/informational).
/// - `_padding`: Reserved for alignment (ensure total size is multiple of 8).
#[repr(C, align(8))]
#[derive(NoPadding)]
pub struct CreateWalletArgs {
    pub user_seed: [u8; 32],
    pub authority_type: u8,
    pub auth_bump: u8,
    pub _padding: [u8; 6], // 32+1+1+6 = 40 bytes
}

impl CreateWalletArgs {
    pub fn from_bytes(data: &[u8]) -> Result<(Self, &[u8]), ProgramError> {
        if data.len() < 40 {
            return Err(ProgramError::InvalidInstructionData);
        }
        let (fixed, rest) = data.split_at(40);

        // Safe copy to ensure alignment
        let mut user_seed = [0u8; 32];
        user_seed.copy_from_slice(&fixed[0..32]);

        let authority_type = fixed[32];
        let auth_bump = fixed[33];
        // skip 6 padding bytes

        let args = Self {
            user_seed,
            authority_type,
            auth_bump,
            _padding: [0; 6],
        };

        Ok((args, rest))
    }
}

/// Processes the `CreateWallet` instruction.
///
/// This instruction initializes:
/// 1. A `Wallet` PDA: The central identity.
/// 2. A `Vault` PDA: To hold assets (signer).
/// 3. An `Authority` PDA: The initial owner (Admin/Owner role).
///
/// # Accounts:
/// 1. `[signer, writable]` Payer: Pays for account creation.
/// 2. `[writable]` Wallet PDA: Derived from `["wallet", user_seed]`.
/// 3. `[writable]` Vault PDA: Derived from `["vault", wallet_pubkey]`.
/// 4. `[writable]` Authority PDA: Derived from `["authority", wallet_pubkey, id_seed]`.
/// 5. `[]` System Program.
/// 6. `[]` Rent Sysvar.
pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let (args, rest) = CreateWalletArgs::from_bytes(instruction_data)?;

    let (id_seed, full_auth_data) = match args.authority_type {
        0 => {
            // Use minimum-length check (consistent with AddAuthority / TransferOwnership).
            // Exact-length check would reject clients that append trailing context bytes.
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
            let (credential_id_hash, rest_after_cred) = rest.split_at(32);
            let rp_id_len = rest_after_cred[33] as usize;
            // Enforce a sane upper bound: max valid domain name is 253 chars.
            // Without this an attacker-controlled payer could create a 369-byte
            // authority account with 255 bytes of arbitrary rpId data.
            if rp_id_len == 0 || rp_id_len > 253 {
                return Err(ProgramError::InvalidInstructionData);
            }
            let total_auth_data = 32 + 33 + 1 + rp_id_len;
            if rest.len() < total_auth_data {
                return Err(ProgramError::InvalidInstructionData);
            }
            let full_auth_data = &rest[..total_auth_data];
            (credential_id_hash, full_auth_data)
        },
        _ => return Err(AuthError::InvalidAuthenticationKind.into()),
    };

    let account_info_iter = &mut accounts.iter();
    let payer = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let wallet_pda = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let vault_pda = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let auth_pda = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let system_program = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let rent_sysvar = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;

    // Get rent from sysvar (fixes audit issue #5 - hardcoded rent calculations)
    let rent = Rent::from_account_info(rent_sysvar)?;

    let (wallet_key, wallet_bump) = find_program_address(&[b"wallet", &args.user_seed], program_id);
    if !sol_assert_bytes_eq(wallet_pda.key().as_ref(), wallet_key.as_ref(), 32) {
        return Err(ProgramError::InvalidSeeds);
    }
    check_zero_data(wallet_pda, ProgramError::AccountAlreadyInitialized)?;

    let (vault_key, _vault_bump) =
        find_program_address(&[b"vault", wallet_key.as_ref()], program_id);
    if !sol_assert_bytes_eq(vault_pda.key().as_ref(), vault_key.as_ref(), 32) {
        return Err(ProgramError::InvalidSeeds);
    }

    // Derive canonical authority PDA and verify user-provided bump matches (audit N1)
    // Must use find_program_address to ensure canonical bump - user-supplied bump
    // could create a valid but non-canonical PDA
    let (auth_key, auth_bump) =
        find_program_address(&[b"authority", wallet_key.as_ref(), id_seed], program_id);
    if !sol_assert_bytes_eq(auth_pda.key().as_ref(), auth_key.as_ref(), 32) {
        return Err(ProgramError::InvalidSeeds);
    }
    check_zero_data(auth_pda, ProgramError::AccountAlreadyInitialized)?;

    // --- 1. Initialize Wallet Account ---
    // Calculate rent-exempt balance for fixed 8-byte wallet account layout.
    let wallet_space = 8;
    let wallet_rent = rent.minimum_balance(wallet_space);

    // Use secure transfer-allocate-assign pattern to prevent DoS (Issue #4)
    let wallet_bump_arr = [wallet_bump];
    let wallet_seeds = [
        Seed::from(b"wallet"),
        Seed::from(&args.user_seed),
        Seed::from(&wallet_bump_arr),
    ];

    crate::utils::initialize_pda_account(
        payer,
        wallet_pda,
        system_program,
        wallet_space,
        wallet_rent,
        program_id,
        &wallet_seeds,
    )?;

    // Write Wallet Data
    let wallet_data = unsafe { wallet_pda.borrow_mut_data_unchecked() };
    if (wallet_data.as_ptr() as usize) % 8 != 0 {
        return Err(ProgramError::InvalidAccountData);
    }
    let wallet_account = WalletAccount {
        discriminator: AccountDiscriminator::Wallet as u8,
        bump: wallet_bump,
        version: crate::state::CURRENT_ACCOUNT_VERSION,
        _padding: [0; 5],
    };
    unsafe {
        std::ptr::write_unaligned(
            wallet_data.as_mut_ptr() as *mut WalletAccount,
            wallet_account,
        );
    }

    // --- 2. Initialize Authority Account ---
    // Fixed sizes per auth type:
    //   Ed25519   = header(48) + pubkey(32) = 80 bytes
    //   Secp256r1 = header(48) + cred_hash(32) + pubkey(33) + rpIdHash(32) = 145 bytes
    //
    // For Secp256r1 we hash rpId once at creation and store the digest, so
    // every subsequent Execute saves one sol_sha256 syscall.
    let header_size = std::mem::size_of::<AuthorityAccountHeader>();
    let auth_space = match args.authority_type {
        0 => header_size + 32,              // Ed25519
        1 => header_size + 32 + 33 + 32,    // Secp256r1 fixed
        _ => return Err(AuthError::InvalidAuthenticationKind.into()),
    };
    let auth_rent = rent.minimum_balance(auth_space);

    // Use secure transfer-allocate-assign pattern to prevent DoS (Issue #4)
    let auth_bump_arr = [auth_bump];
    let auth_seeds = [
        Seed::from(b"authority"),
        Seed::from(wallet_key.as_ref()),
        Seed::from(id_seed),
        Seed::from(&auth_bump_arr),
    ];

    crate::utils::initialize_pda_account(
        payer,
        auth_pda,
        system_program,
        auth_space,
        auth_rent,
        program_id,
        &auth_seeds,
    )?;

    // Write Authority Data
    let auth_account_data = unsafe { auth_pda.borrow_mut_data_unchecked() };
    let header = AuthorityAccountHeader {
        discriminator: AccountDiscriminator::Authority as u8,
        authority_type: args.authority_type,
        role: 0,
        bump: auth_bump,
        version: crate::state::CURRENT_ACCOUNT_VERSION,
        _padding1: [0; 3],
        counter: 0,
        _padding2: [0; 4],
        wallet: *wallet_pda.key(),
    };

    // safe write of header
    let header_bytes = unsafe {
        std::slice::from_raw_parts(
            &header as *const AuthorityAccountHeader as *const u8,
            header_size,
        )
    };
    auth_account_data[0..header_size].copy_from_slice(header_bytes);

    // Write variable data
    match args.authority_type {
        0 => {
            // Ed25519: pubkey(32) — full_auth_data is exactly 32 bytes
            auth_account_data[header_size..header_size + 32]
                .copy_from_slice(&full_auth_data[..32]);
        }
        1 => {
            // Secp256r1: cred_hash(32) ∥ pubkey(33) ∥ rpIdHash(32).
            // full_auth_data layout as parsed above:
            //   [cred_hash(32)] [pubkey(33)] [rpIdLen(1)] [rpId(N)]
            auth_account_data[header_size..header_size + 32]
                .copy_from_slice(&full_auth_data[..32]);
            auth_account_data[header_size + 32..header_size + 32 + 33]
                .copy_from_slice(&full_auth_data[32..32 + 33]);
            // Compute rpIdHash from rpId
            let rp_id_len = full_auth_data[32 + 33] as usize;
            let rp_id = &full_auth_data[32 + 33 + 1..32 + 33 + 1 + rp_id_len];
            let rp_id_hash_offset = header_size + 32 + 33;
            #[cfg(target_os = "solana")]
            unsafe {
                let _ = pinocchio::syscalls::sol_sha256(
                    [rp_id].as_ptr() as *const u8,
                    1,
                    auth_account_data[rp_id_hash_offset..rp_id_hash_offset + 32].as_mut_ptr(),
                );
            }
            #[cfg(not(target_os = "solana"))]
            {
                let _ = rp_id;
                auth_account_data[rp_id_hash_offset..rp_id_hash_offset + 32].fill(0);
            }
        }
        _ => unreachable!(),
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_wallet_args_from_bytes_ed25519() {
        let mut data = Vec::new();
        // Args: user_seed(32) + type(1) + bump(1) + padding(6) = 40
        let user_seed = [1u8; 32];
        data.extend_from_slice(&user_seed);
        data.push(0); // Ed25519
        data.push(255); // bump
        data.extend_from_slice(&[0; 6]); // padding

        // Payload for Ed25519: pubkey(32)
        let pubkey = [2u8; 32];
        data.extend_from_slice(&pubkey);

        let (args, rest) = CreateWalletArgs::from_bytes(&data).unwrap();
        assert_eq!(args.user_seed, user_seed);
        assert_eq!(args.authority_type, 0);
        assert_eq!(args.auth_bump, 255);
        assert_eq!(rest, &pubkey);
    }

    #[test]
    fn test_create_wallet_args_from_bytes_secp256r1() {
        let mut data = Vec::new();
        let user_seed = [3u8; 32];
        data.extend_from_slice(&user_seed);
        data.push(1); // authority_type = Secp256r1
        data.push(123); // bump
        data.extend_from_slice(&[0; 6]); // padding

        // Payload for Secp256r1: credential_id_hash(32) + pubkey(33)
        let cred_id_hash = [4u8; 32];
        let pubkey = [6u8; 33];
        data.extend_from_slice(&cred_id_hash);
        data.extend_from_slice(&pubkey);

        let (args, rest) = CreateWalletArgs::from_bytes(&data).unwrap();
        assert_eq!(args.user_seed, user_seed);
        assert_eq!(args.authority_type, 1);
        assert_eq!(args.auth_bump, 123);
        assert_eq!(rest.len(), 65); // from_bytes returns the raw remaining data
        assert_eq!(&rest[0..32], &cred_id_hash);
        assert_eq!(&rest[32..65], &pubkey);
    }

    #[test]
    fn test_create_wallet_args_too_short() {
        let data = vec![0u8; 39]; // Need 40
        assert!(CreateWalletArgs::from_bytes(&data).is_err());
    }
}
