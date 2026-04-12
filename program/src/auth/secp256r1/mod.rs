use crate::{error::AuthError, state::authority::AuthorityAccountHeader};
use pinocchio::{
    account_info::AccountInfo,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvars::instructions::{Instructions, INSTRUCTIONS_ID},
};
use pinocchio_pubkey::pubkey;

pub mod introspection;
pub mod nonce;
pub mod slothashes;
pub mod webauthn;

use self::introspection::verify_secp256r1_instruction_data;
use self::nonce::validate_nonce;
use self::webauthn::{
    reconstruct_client_data_json, AuthDataParser, ClientDataJsonReconstructionParams,
};

use crate::auth::traits::Authenticator;

/// Authenticator implementation for Secp256r1 (WebAuthn).
pub struct Secp256r1Authenticator;

impl Authenticator for Secp256r1Authenticator {
    /// Authenticates a Secp256r1 signature (WebAuthn/Passkeys).
    ///
    /// Auth payload layout:
    ///   [slot(8)] [counter(8)] [sysvarIxIdx(1)] [sysvarSlotIdx(1)] [flags(1)] [rpIdLen(1)] [rpId(N)] [authenticatorData(M)]
    ///
    /// Counter is a program-controlled odometer (swig-style). The client must submit
    /// `on_chain_counter + 1`. The WebAuthn hardware counter is NOT used for replay protection.
    fn authenticate(
        &self,
        accounts: &[AccountInfo],
        auth_data: &mut [u8],
        auth_payload: &[u8],
        signed_payload: &[u8],
        discriminator: &[u8],
        program_id: &Pubkey,
    ) -> Result<(), ProgramError> {
        // Minimum: slot(8) + counter(8) + sysvarIxIdx(1) + sysvarSlotIdx(1) + flags(1) + rpIdLen(1) = 20
        if auth_payload.len() < 20 {
            return Err(AuthError::InvalidAuthorityPayload.into());
        }

        let slot = u64::from_le_bytes(auth_payload[0..8].try_into().unwrap());
        let submitted_counter = u64::from_le_bytes(auth_payload[8..16].try_into().unwrap());
        let sysvar_ix_index = auth_payload[16] as usize;
        let sysvar_slothashes_index = auth_payload[17] as usize;

        let reconstruction_params = ClientDataJsonReconstructionParams {
            type_and_flags: auth_payload[18],
        };
        let rp_id_len = auth_payload[19] as usize;
        if auth_payload.len() < 20 + rp_id_len {
            return Err(AuthError::InvalidAuthorityPayload.into());
        }
        let rp_id = &auth_payload[20..20 + rp_id_len];
        let authenticator_data_raw = &auth_payload[20 + rp_id_len..];

        // Validate Nonce (SlotHashes) — ensures signature freshness within 150 slots
        let slothashes_account = accounts
            .get(sysvar_slothashes_index)
            .ok_or(AuthError::InvalidAuthorityPayload)?;
        let _slot_hash = validate_nonce(slothashes_account, slot)?;

        let header_size = std::mem::size_of::<AuthorityAccountHeader>();
        if auth_data.len() < header_size {
            return Err(AuthError::InvalidAuthorityPayload.into());
        }

        let mut header = unsafe {
            std::ptr::read_unaligned(auth_data.as_ptr() as *const AuthorityAccountHeader)
        };

        // --- Odometer validation (swig-style) ---
        // The client must submit exactly `stored_counter + 1`.
        // This decouples replay protection from the WebAuthn hardware counter,
        // which is unreliable for synced passkeys (iCloud, Google).
        let expected_counter = header.counter.wrapping_add(1);
        if submitted_counter != expected_counter {
            return Err(AuthError::SignatureReused.into());
        }

        // Secp256r1 on-chain data layout:
        //   [Header(48)] [credential_id_hash(32)] [Pubkey(33)]
        let pubkey_offset = header_size + 32; // skip credential_id_hash
        if auth_data.len() < pubkey_offset + 33 {
            return Err(AuthError::InvalidAuthorityPayload.into());
        }

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

        let payer = accounts.first().ok_or(ProgramError::NotEnoughAccountKeys)?;
        if !payer.is_signer() {
            return Err(ProgramError::MissingRequiredSignature);
        }

        // Build challenge hash:
        //   SHA256(discriminator || auth_payload || signed_payload || slot || payer || counter || program_id)
        // The counter is included both in auth_payload (bytes 8-16) AND as a separate element
        // for belt-and-suspenders protection.
        let counter_bytes = expected_counter.to_le_bytes();
        #[allow(unused_assignments)]
        let mut hasher = [0u8; 32];
        #[cfg(target_os = "solana")]
        unsafe {
            let _res = pinocchio::syscalls::sol_sha256(
                [
                    discriminator,
                    auth_payload,
                    signed_payload,
                    &slot.to_le_bytes(),
                    payer.key().as_ref(),
                    &counter_bytes,
                    program_id.as_ref(),
                ]
                .as_ptr() as *const u8,
                7,
                hasher.as_mut_ptr(),
            );
        }
        #[cfg(not(target_os = "solana"))]
        {
            let _ = signed_payload;
            let _ = discriminator;
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

        let auth_data_parser = AuthDataParser::new(authenticator_data_raw)?;
        if !auth_data_parser.is_user_present() {
            return Err(AuthError::PermissionDenied.into());
        }

        // Note: We intentionally do NOT check auth_data_parser.counter() (the WebAuthn hardware
        // counter). Synced passkeys (iCloud Keychain, Google Password Manager) may return 0 or
        // non-incrementing values. The program-controlled odometer above provides replay protection.

        // Security Validation:
        // Ensure the domain (rp_id_hash) the user provided in the instruction payload actually matches
        // the rpIdHash that the authenticator (Hardware/FaceID) signed over inside authenticatorData.
        if auth_data_parser.rp_id_hash() != computed_rp_id_hash {
            return Err(AuthError::InvalidPubkey.into());
        }

        // Extract the 33-byte COMPRESSED key from on-chain storage
        let instruction_pubkey_bytes = &auth_data[pubkey_offset..pubkey_offset + 33];
        let expected_pubkey: &[u8; 33] = instruction_pubkey_bytes.try_into().unwrap();

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

        // Signature verified successfully — now commit the counter update
        header.counter = expected_counter;
        unsafe {
            std::ptr::write_unaligned(
                auth_data.as_mut_ptr() as *mut AuthorityAccountHeader,
                header,
            );
        }

        Ok(())
    }
}
