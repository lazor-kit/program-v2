use crate::{error::AuthError, state::authority::AuthorityAccountHeader};
use pinocchio::{
    account_info::AccountInfo,
    program_error::ProgramError,
    sysvars::instructions::{Instructions, INSTRUCTIONS_ID},
};
use pinocchio_pubkey::pubkey;

pub mod introspection;
pub mod nonce;
pub mod slothashes;
pub mod webauthn;

use self::introspection::verify_secp256r1_instruction_data;
use self::nonce::{validate_nonce, TruncatedSlot};
use self::webauthn::{
    reconstruct_client_data_json, AuthDataParser, ClientDataJsonReconstructionParams,
};

use crate::auth::traits::Authenticator;

/// Authenticator implementation for Secp256r1 (WebAuthn).
pub struct Secp256r1Authenticator;

impl Authenticator for Secp256r1Authenticator {
    /// Authenticates a Secp256r1 signature (WebAuthn/Passkeys).
    ///
    /// # Arguments
    /// * `accounts`: Slice of accounts, expecting Sysvar Lookups if needed.
    /// * `auth_data`: Mutable reference to the Authority account data (to update counter).
    /// * `auth_payload`: Auxiliary data (e.g., signature, authenticator data, client JSON parts).
    /// * `signed_payload`: The actual message/data that was signed (e.g. instruction args).
    fn authenticate(
        &self,
        accounts: &[AccountInfo],
        auth_data: &mut [u8],
        auth_payload: &[u8],
        signed_payload: &[u8], // The message payload (e.g. compact instructions or args) that is signed
        discriminator: &[u8],
    ) -> Result<(), ProgramError> {
        if auth_payload.len() < 12 {
            return Err(AuthError::InvalidAuthorityPayload.into());
        }

        let slot = u64::from_le_bytes(auth_payload[0..8].try_into().unwrap());
        let sysvar_ix_index = auth_payload[8] as usize;
        let sysvar_slothashes_index = auth_payload[9] as usize;

        let reconstruction_params = ClientDataJsonReconstructionParams {
            type_and_flags: auth_payload[10],
        };
        let rp_id_len = auth_payload[11] as usize;
        if auth_payload.len() < 12 + rp_id_len {
            return Err(AuthError::InvalidAuthorityPayload.into());
        }
        let rp_id = &auth_payload[12..12 + rp_id_len];
        let authenticator_data_raw = &auth_payload[12 + rp_id_len..];

        // Validate Nonce (SlotHashes)
        let slothashes_account = accounts
            .get(sysvar_slothashes_index)
            .ok_or(AuthError::InvalidAuthorityPayload)?;
        let truncated_slot = TruncatedSlot::new(slot);
        let _slot_hash = validate_nonce(slothashes_account, &truncated_slot)?;

        let header_size = std::mem::size_of::<AuthorityAccountHeader>();
        if (auth_data.as_ptr() as usize) % 8 != 0 {
            return Err(AuthError::InvalidAuthorityPayload.into());
        }
        // SAFETY: Pointer alignment checked above. size_of correct.
        let header = unsafe { &mut *(auth_data.as_mut_ptr() as *mut AuthorityAccountHeader) };

        // Compute hash of user-provided RP ID and verify against stored hash (audit N3)
        let stored_rp_id_hash = &auth_data[header_size..header_size + 32];
        #[allow(unused_assignments)]
        let mut computed_rp_id_hash = [0u8; 32];
        #[cfg(target_os = "solana")]
        unsafe {
            let _res = pinocchio::syscalls::sol_sha256(
                [rp_id].as_ptr() as *const u8,
                1,
                computed_rp_id_hash.as_mut_ptr(),
            );
        }
        #[cfg(not(target_os = "solana"))]
        {
            computed_rp_id_hash = [0u8; 32];
        }
        if computed_rp_id_hash != stored_rp_id_hash {
            return Err(AuthError::InvalidPubkey.into());
        }

        #[allow(unused_assignments)]
        let mut hasher = [0u8; 32];
        #[cfg(target_os = "solana")]
        unsafe {
            let _res = pinocchio::syscalls::sol_sha256(
                [discriminator, signed_payload, &slot.to_le_bytes()].as_ptr() as *const u8,
                3,
                hasher.as_mut_ptr(),
            );
        }
        #[cfg(not(target_os = "solana"))]
        {
            let _ = signed_payload; // suppress unused warning for non-solana
            hasher = [0u8; 32];
        }

        let client_data_json = reconstruct_client_data_json(&reconstruction_params, rp_id, &hasher);
        #[allow(unused_assignments)]
        let mut client_data_hash = [0u8; 32];
        #[cfg(target_os = "solana")]
        unsafe {
            let _res = pinocchio::syscalls::sol_sha256(
                [client_data_json.as_slice()].as_ptr() as *const u8,
                1,
                client_data_hash.as_mut_ptr(),
            );
        }
        #[cfg(not(target_os = "solana"))]
        {
            let _ = client_data_json;
            client_data_hash = [0u8; 32];
        }

        let auth_data_parser = AuthDataParser::new(authenticator_data_raw);
        if !auth_data_parser.is_user_present() {
            return Err(AuthError::PermissionDenied.into());
        }

        let authenticator_counter = auth_data_parser.counter() as u64;
        if authenticator_counter > 0 && authenticator_counter <= header.counter {
            return Err(AuthError::SignatureReused.into());
        }
        header.counter = authenticator_counter;

        let stored_rp_id_hash = &auth_data[header_size..header_size + 32];
        if auth_data_parser.rp_id_hash() != stored_rp_id_hash {
            return Err(AuthError::InvalidPubkey.into());
        }

        let expected_pubkey = &auth_data[header_size + 32..header_size + 32 + 33];
        let expected_pubkey: &[u8; 33] = expected_pubkey.try_into().unwrap();

        let mut signed_message = Vec::with_capacity(authenticator_data_raw.len() + 32);
        signed_message.extend_from_slice(authenticator_data_raw);
        signed_message.extend_from_slice(&client_data_hash);

        let sysvar_instructions = accounts
            .get(sysvar_ix_index)
            .ok_or(AuthError::InvalidAuthorityPayload)?;
        if sysvar_instructions.key().as_ref() != INSTRUCTIONS_ID.as_ref() {
            return Err(AuthError::InvalidInstruction.into());
        }

        let sysvar_data = unsafe { sysvar_instructions.borrow_data_unchecked() };
        let ixs = unsafe { Instructions::new_unchecked(sysvar_data) };
        let current_index = ixs.load_current_index() as usize;
        if current_index == 0 {
            return Err(AuthError::InvalidInstruction.into());
        }

        let secp_ix = unsafe { ixs.deserialize_instruction_unchecked(current_index - 1) };
        if secp_ix.get_program_id() != &pubkey!("Secp256r1SigVerify1111111111111111111111111") {
            return Err(AuthError::InvalidInstruction.into());
        }

        verify_secp256r1_instruction_data(
            secp_ix.get_instruction_data(),
            expected_pubkey,
            &signed_message,
        )?;

        Ok(())
    }
}
