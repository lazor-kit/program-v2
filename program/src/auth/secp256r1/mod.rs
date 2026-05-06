use crate::{error::AuthError, state::authority::AuthorityAccountHeader};
use pinocchio::{
    account_info::AccountInfo,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvars::{
        clock::Clock,
        instructions::{Instructions, INSTRUCTIONS_ID},
        Sysvar,
    },
};
use pinocchio_pubkey::pubkey;

pub mod introspection;
pub mod webauthn;

use self::introspection::verify_secp256r1_instruction_data;
use self::webauthn::{base64url_encode_no_pad, extract_top_level_string_field, AuthDataParser};

use crate::auth::traits::Authenticator;
use crate::utils::get_stack_height;

/// Maximum age (in slots) for a Secp256r1 signature to be considered valid (~60 seconds).
const MAX_SLOT_AGE: u64 = 150;

/// Authenticator implementation for Secp256r1 (WebAuthn).
pub struct Secp256r1Authenticator;

impl Authenticator for Secp256r1Authenticator {
    /// Authenticates a Secp256r1 signature (WebAuthn/Passkeys).
    ///
    /// Auth payload layout (raw clientDataJSON — the only supported mode):
    ///   [slot(8)] [counter(4)] [sysvarIxIdx(1)] [_reserved(1)]
    ///   [authDataLen(2 LE)] [authenticatorData(M)]
    ///   [cdjLen(2 LE)] [clientDataJson(N)]
    ///
    /// rpIdHash is pre-computed at authority creation and stored on the
    /// Authority account, so every Execute saves one sol_sha256 syscall and
    /// the Authority account size is fixed (145 bytes for Secp256r1).
    ///
    /// Counter is a program-controlled u32 odometer. Client must submit
    /// `on_chain_counter + 1`.
    ///
    /// Programmatic/bot signing should use Ed25519 authorities instead —
    /// Secp256r1 is passkeys-only.
    fn authenticate(
        &self,
        accounts: &[AccountInfo],
        auth_data: &mut [u8],
        auth_payload: &[u8],
        signed_payload: &[u8],
        discriminator: &[u8],
        program_id: &Pubkey,
    ) -> Result<(), ProgramError> {
        // Minimum: slot(8) + counter(4) + sysvarIxIdx(1) + reserved(1) = 14,
        // plus authDataLen(2) + cdjLen(2) = 18 before any payload bytes.
        if auth_payload.len() < 18 {
            return Err(AuthError::InvalidAuthorityPayload.into());
        }

        let slot = u64::from_le_bytes(auth_payload[0..8].try_into().unwrap());
        let submitted_counter = u32::from_le_bytes(auth_payload[8..12].try_into().unwrap());
        let sysvar_ix_index = auth_payload[12] as usize;
        // auth_payload[13] reserved (carried over from legacy mode byte).

        // Anti-CPI check: prevent cross-program authentication attacks
        if get_stack_height() > 1 {
            return Err(AuthError::PermissionDenied.into());
        }

        // Validate slot freshness using Clock sysvar
        let clock = Clock::get()?;
        let current_slot = clock.slot;
        if slot > current_slot {
            return Err(AuthError::InvalidSignatureAge.into());
        }
        if current_slot - slot >= MAX_SLOT_AGE {
            return Err(AuthError::InvalidSignatureAge.into());
        }

        let header_size = std::mem::size_of::<AuthorityAccountHeader>();
        if auth_data.len() < header_size {
            return Err(AuthError::InvalidAuthorityPayload.into());
        }

        let mut header = unsafe {
            std::ptr::read_unaligned(auth_data.as_ptr() as *const AuthorityAccountHeader)
        };

        // --- Odometer validation ---
        let expected_counter = header.counter.wrapping_add(1);
        if submitted_counter != expected_counter {
            return Err(AuthError::SignatureReused.into());
        }

        // Secp256r1 on-chain data layout (fixed 145 bytes total):
        //   [Header(48)] [credential_id_hash(32)] [Pubkey(33)] [rpIdHash(32)]
        let pubkey_offset = header_size + 32; // 80
        let rp_id_hash_offset = pubkey_offset + 33; // 113
        if auth_data.len() < rp_id_hash_offset + 32 {
            return Err(AuthError::InvalidAuthorityPayload.into());
        }
        let stored_rp_id_hash = &auth_data[rp_id_hash_offset..rp_id_hash_offset + 32];

        let payer = accounts.first().ok_or(ProgramError::NotEnoughAccountKeys)?;
        if !payer.is_signer() {
            return Err(ProgramError::MissingRequiredSignature);
        }

        // Challenge hash:
        //   SHA256(discriminator || auth_payload[..14] || signed_payload
        //          || payer || counter || program_id)
        //
        // Only the 14-byte fixed prefix of auth_payload is included because the
        // remainder contains clientDataJSON — which is produced by the
        // authenticator *after* signing the challenge, so it can't be in the
        // hash input.
        let counter_bytes = expected_counter.to_le_bytes();
        #[allow(unused_assignments)]
        let mut hasher = [0u8; 32];
        #[cfg(target_os = "solana")]
        unsafe {
            let _res = pinocchio::syscalls::sol_sha256(
                [
                    discriminator,
                    &auth_payload[..14],
                    signed_payload,
                    payer.key().as_ref(),
                    &counter_bytes,
                    program_id.as_ref(),
                ]
                .as_ptr() as *const u8,
                6,
                hasher.as_mut_ptr(),
            );
        }
        #[cfg(not(target_os = "solana"))]
        {
            let _ = (signed_payload, discriminator, counter_bytes, program_id);
            hasher = [0u8; 32];
        }

        // --- Parse Mode 1 payload: authenticatorData + clientDataJSON ---
        let auth_data_len =
            u16::from_le_bytes(auth_payload[14..16].try_into().unwrap()) as usize;
        if auth_payload.len() < 16 + auth_data_len + 2 {
            return Err(AuthError::InvalidAuthorityPayload.into());
        }
        let authenticator_data_raw = &auth_payload[16..16 + auth_data_len];

        let cdj_len_offset = 16 + auth_data_len;
        let cdj_len =
            u16::from_le_bytes(auth_payload[cdj_len_offset..cdj_len_offset + 2].try_into().unwrap())
                as usize;
        let cdj_offset = cdj_len_offset + 2;
        // L2: strict length — trailing bytes after cdj are not covered by
        // challenge hash or precompile message, so they're rejected.
        if cdj_len == 0 || auth_payload.len() != cdj_offset + cdj_len {
            return Err(AuthError::InvalidAuthorityPayload.into());
        }
        let raw_client_data_json = &auth_payload[cdj_offset..cdj_offset + cdj_len];

        // L1: We intentionally do NOT validate the `origin` field inside the
        // clientDataJSON. The binding that matters is the authenticator's
        // `rpIdHash` (checked below against the on-chain stored rpIdHash),
        // which the authenticator hardware/OS computes from the registered
        // relying party and refuses to sign cross-origin.

        // Validate "type" field is "webauthn.get"
        let type_value = extract_top_level_string_field(raw_client_data_json, b"type")?;
        if type_value != b"webauthn.get" {
            return Err(AuthError::InvalidAuthenticationKind.into());
        }

        // Validate "challenge" field matches expected base64url(challenge_hash).
        // L3: constant-time byte comparison.
        let challenge_value =
            extract_top_level_string_field(raw_client_data_json, b"challenge")?;
        let expected_challenge_b64 = base64url_encode_no_pad(&hasher);
        if !ct_eq(challenge_value, expected_challenge_b64.as_slice()) {
            return Err(AuthError::InvalidMessageHash.into());
        }

        // Hash the raw clientDataJSON
        #[allow(unused_assignments)]
        let mut client_data_hash = [0u8; 32];
        #[cfg(target_os = "solana")]
        unsafe {
            let _res = pinocchio::syscalls::sol_sha256(
                [raw_client_data_json].as_ptr() as *const u8,
                1,
                client_data_hash.as_mut_ptr(),
            );
        }
        #[cfg(not(target_os = "solana"))]
        {
            let _ = raw_client_data_json;
            client_data_hash = [0u8; 32];
        }

        // --- Shared validation (both modes) ---

        let auth_data_parser = AuthDataParser::new(authenticator_data_raw)?;
        if !auth_data_parser.is_user_present() {
            return Err(AuthError::PermissionDenied.into());
        }

        // Note: We intentionally do NOT check the WebAuthn hardware counter.
        // Synced passkeys (iCloud, Google) may return 0 or non-incrementing values.

        // Validate rpIdHash in authenticatorData matches the stored rpIdHash.
        if auth_data_parser.rp_id_hash() != stored_rp_id_hash {
            return Err(AuthError::InvalidPubkey.into());
        }

        // Extract the 33-byte COMPRESSED key from on-chain storage
        let instruction_pubkey_bytes = &auth_data[pubkey_offset..pubkey_offset + 33];
        let expected_pubkey: &[u8; 33] = instruction_pubkey_bytes.try_into().unwrap();

        // The precompile's signed message is authenticator_data ∥ client_data_hash.
        // Pass the two slices separately to avoid an intermediate Vec allocation.

        // Introspect the secp256r1 precompile instruction (must be the previous instruction)
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
            authenticator_data_raw,
            &client_data_hash,
        )?;

        // Signature verified successfully — commit the counter update
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

/// Constant-time byte slice equality. Returns `false` for different lengths;
/// otherwise XORs every byte pair into an accumulator and compares to zero,
/// ensuring the comparison takes the same time regardless of where (or if) the
/// bytes differ. Used for the Mode 1 challenge check.
#[inline(always)]
fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut acc: u8 = 0;
    for i in 0..a.len() {
        acc |= a[i] ^ b[i];
    }
    acc == 0
}

#[cfg(test)]
mod tests {
    use super::ct_eq;

    #[test]
    fn ct_eq_equal() {
        assert!(ct_eq(b"abc", b"abc"));
        assert!(ct_eq(b"", b""));
        assert!(ct_eq(&[0xFF; 43], &[0xFF; 43]));
    }

    #[test]
    fn ct_eq_different_length() {
        assert!(!ct_eq(b"abc", b"abcd"));
        assert!(!ct_eq(b"", b"a"));
    }

    #[test]
    fn ct_eq_differs_at_start() {
        assert!(!ct_eq(b"xbc", b"abc"));
    }

    #[test]
    fn ct_eq_differs_at_end() {
        assert!(!ct_eq(b"abx", b"abc"));
    }

    #[test]
    fn ct_eq_differs_at_middle() {
        assert!(!ct_eq(b"axc", b"abc"));
    }
}
