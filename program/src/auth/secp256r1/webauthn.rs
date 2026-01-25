#[allow(unused_imports)]
use crate::error::AuthError;
#[allow(unused_imports)]
use pinocchio::program_error::ProgramError;

/// Packed flags for clientDataJson reconstruction
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct ClientDataJsonReconstructionParams {
    pub type_and_flags: u8,
}

impl ClientDataJsonReconstructionParams {
    #[allow(dead_code)]
    const TYPE_CREATE: u8 = 0x00;
    const TYPE_GET: u8 = 0x10;
    const FLAG_CROSS_ORIGIN: u8 = 0x01;
    const FLAG_HTTP_ORIGIN: u8 = 0x02;
    const FLAG_GOOGLE_EXTRA: u8 = 0x04;

    pub fn auth_type(&self) -> AuthType {
        if (self.type_and_flags & 0xF0) == Self::TYPE_GET {
            AuthType::Get
        } else {
            AuthType::Create
        }
    }

    pub fn is_cross_origin(&self) -> bool {
        self.type_and_flags & Self::FLAG_CROSS_ORIGIN != 0
    }

    pub fn is_http(&self) -> bool {
        self.type_and_flags & Self::FLAG_HTTP_ORIGIN != 0
    }

    pub fn has_google_extra(&self) -> bool {
        self.type_and_flags & Self::FLAG_GOOGLE_EXTRA != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub enum AuthType {
    Create,
    Get,
}

/// Simple Base64URL encoder without padding
pub fn base64url_encode_no_pad(data: &[u8]) -> Vec<u8> {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut result = Vec::with_capacity(data.len().div_ceil(3) * 4);

    for chunk in data.chunks(3) {
        let b = match chunk.len() {
            3 => (chunk[0] as u32) << 16 | (chunk[1] as u32) << 8 | (chunk[2] as u32),
            2 => (chunk[0] as u32) << 16 | (chunk[1] as u32) << 8,
            1 => (chunk[0] as u32) << 16,
            _ => unreachable!(),
        };

        result.push(ALPHABET[((b >> 18) & 0x3f) as usize]);
        result.push(ALPHABET[((b >> 12) & 0x3f) as usize]);
        if chunk.len() > 1 {
            result.push(ALPHABET[((b >> 6) & 0x3f) as usize]);
        }
        if chunk.len() > 2 {
            result.push(ALPHABET[(b & 0x3f) as usize]);
        }
    }
    result
}

/// Reconstructs the clientDataJson
pub fn reconstruct_client_data_json(
    params: &ClientDataJsonReconstructionParams,
    rp_id: &[u8],
    challenge: &[u8],
) -> Vec<u8> {
    let challenge_b64url = base64url_encode_no_pad(challenge);
    let type_str: &[u8] = match params.auth_type() {
        AuthType::Create => b"webauthn.create",
        AuthType::Get => b"webauthn.get",
    };

    let prefix: &[u8] = if params.is_http() {
        b"http://"
    } else {
        b"https://"
    };
    let cross_origin: &[u8] = if params.is_cross_origin() {
        b"true"
    } else {
        b"false"
    };

    let mut json = Vec::with_capacity(256);
    json.extend_from_slice(b"{\"type\":\"");
    json.extend_from_slice(type_str);
    json.extend_from_slice(b"\",\"challenge\":\"");
    json.extend_from_slice(&challenge_b64url);
    json.extend_from_slice(b"\",\"origin\":\"");
    json.extend_from_slice(prefix);
    json.extend_from_slice(rp_id);
    json.extend_from_slice(b"\",\"crossOrigin\":");
    json.extend_from_slice(cross_origin);

    if params.has_google_extra() {
        json.extend_from_slice(b",\"other_keys_can_be_added_here\":\"do not compare clientDataJSON against a template. See https://goo.gl/yabPex\"");
    }

    json.extend_from_slice(b"}");
    json
}

/// Parser for WebAuthn authenticator data
pub struct AuthDataParser<'a> {
    data: &'a [u8],
}

impl<'a> AuthDataParser<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data }
    }

    pub fn rp_id_hash(&self) -> &'a [u8] {
        &self.data[0..32]
    }

    pub fn is_user_present(&self) -> bool {
        self.data[32] & 0x01 != 0
    }

    pub fn is_user_verified(&self) -> bool {
        self.data[32] & 0x04 != 0
    }

    pub fn counter(&self) -> u32 {
        u32::from_be_bytes(self.data[33..37].try_into().unwrap())
    }
}
