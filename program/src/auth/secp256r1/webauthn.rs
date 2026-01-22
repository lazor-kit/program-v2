use crate::error::AuthError;
use pinocchio::program_error::ProgramError;

/// Constants from the secp256r1 program
const WEBAUTHN_AUTHENTICATOR_DATA_MAX_SIZE: usize = 196;

#[repr(u8)]
pub enum WebAuthnField {
    None,
    Type,
    Challenge,
    Origin,
    CrossOrigin,
}

impl TryFrom<u8> for WebAuthnField {
    type Error = AuthError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::None),
            1 => Ok(Self::Type),
            2 => Ok(Self::Challenge),
            3 => Ok(Self::Origin),
            4 => Ok(Self::CrossOrigin),
            _ => Err(AuthError::InvalidMessage),
        }
    }
}

#[repr(u16)]
pub enum R1AuthenticationKind {
    WebAuthn = 1,
}

impl TryFrom<u16> for R1AuthenticationKind {
    type Error = AuthError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::WebAuthn),
            _ => Err(AuthError::InvalidAuthenticationKind),
        }
    }
}

/// Process WebAuthn-specific message data
pub fn webauthn_message<'a>(
    auth_payload: &[u8],
    computed_hash: [u8; 32],
    message_buf: &'a mut [u8],
) -> Result<&'a [u8], ProgramError> {
    if auth_payload.len() < 6 {
        return Err(AuthError::InvalidMessage.into());
    }

    let auth_len = u16::from_le_bytes(auth_payload[2..4].try_into().unwrap()) as usize;

    if auth_len >= WEBAUTHN_AUTHENTICATOR_DATA_MAX_SIZE {
        return Err(AuthError::InvalidMessage.into());
    }

    if auth_payload.len() < 4 + auth_len + 4 {
        return Err(AuthError::InvalidMessage.into());
    }

    let auth_data = &auth_payload[4..4 + auth_len];

    let mut offset = 4 + auth_len;

    let field_order = &auth_payload[offset..offset + 4];

    offset += 4;

    let origin_len =
        u16::from_le_bytes(auth_payload[offset..offset + 2].try_into().unwrap()) as usize;

    offset += 2;

    if auth_payload.len() < offset + 2 {
        return Err(AuthError::InvalidMessage.into());
    }
    let huffman_tree_len =
        u16::from_le_bytes(auth_payload[offset..offset + 2].try_into().unwrap()) as usize;
    offset += 2;

    if auth_payload.len() < offset + 2 {
        return Err(AuthError::InvalidMessage.into());
    }
    let huffman_encoded_len =
        u16::from_le_bytes(auth_payload[offset..offset + 2].try_into().unwrap()) as usize;
    offset += 2;

    if auth_payload.len() < offset + huffman_tree_len + huffman_encoded_len {
        return Err(AuthError::InvalidMessage.into());
    }

    let huffman_tree = &auth_payload[offset..offset + huffman_tree_len];
    let huffman_encoded_origin =
        &auth_payload[offset + huffman_tree_len..offset + huffman_tree_len + huffman_encoded_len];

    let decoded_origin = decode_huffman_origin(huffman_tree, huffman_encoded_origin, origin_len)?;

    let client_data_json =
        reconstruct_client_data_json(field_order, &decoded_origin, &computed_hash)?;

    let mut client_data_hash = [0u8; 32];

    #[cfg(target_os = "solana")]
    unsafe {
        let res = pinocchio::syscalls::sol_sha256(
            [client_data_json.as_slice()].as_ptr() as *const u8,
            1,
            client_data_hash.as_mut_ptr(),
        );
        if res != 0 {
            return Err(AuthError::InvalidMessageHash.into());
        }
    }
    #[cfg(not(target_os = "solana"))]
    {
        // Mock hash
        let _ = client_data_json; // suppress unused warning
        client_data_hash = [1u8; 32]; // mutate to justify mut
    }

    message_buf[0..auth_len].copy_from_slice(auth_data);
    message_buf[auth_len..auth_len + 32].copy_from_slice(&client_data_hash);

    Ok(&message_buf[..auth_len + 32])
}

/// Decode huffman-encoded origin URL
fn decode_huffman_origin(
    tree_data: &[u8],
    encoded_data: &[u8],
    decoded_len: usize,
) -> Result<Vec<u8>, ProgramError> {
    const NODE_SIZE: usize = 3;
    const LEAF_NODE: u8 = 0;
    const BIT_MASKS: [u8; 8] = [0x80, 0x40, 0x20, 0x10, 0x08, 0x04, 0x02, 0x01];

    if tree_data.len() % NODE_SIZE != 0 || tree_data.is_empty() {
        return Err(AuthError::InvalidMessage.into());
    }

    let node_count = tree_data.len() / NODE_SIZE;
    let root_index = node_count - 1;
    let mut current_node_index = root_index;
    let mut decoded = Vec::new();

    for &byte in encoded_data.iter() {
        for bit_pos in 0..8 {
            if decoded.len() == decoded_len {
                return Ok(decoded);
            }

            let bit = (byte & BIT_MASKS[bit_pos]) != 0;

            let node_offset = current_node_index * NODE_SIZE;
            let node_type = tree_data[node_offset];

            if node_type == LEAF_NODE {
                return Err(AuthError::InvalidMessage.into());
            }

            let left_or_char = tree_data[node_offset + 1];
            let right = tree_data[node_offset + 2];
            current_node_index = if bit {
                right as usize
            } else {
                left_or_char as usize
            };

            if current_node_index >= node_count {
                return Err(AuthError::InvalidMessage.into());
            }

            let next_node_offset = current_node_index * NODE_SIZE;
            let next_node_type = tree_data[next_node_offset];

            if next_node_type == LEAF_NODE {
                let character = tree_data[next_node_offset + 1];
                decoded.push(character);
                current_node_index = root_index;
            }
        }
    }

    Ok(decoded)
}

fn reconstruct_client_data_json(
    field_order: &[u8],
    origin: &[u8],
    challenge_data: &[u8],
) -> Result<Vec<u8>, ProgramError> {
    let origin_str = core::str::from_utf8(origin).map_err(|_| AuthError::InvalidMessage)?;
    let challenge_b64 = base64url_encode_no_pad(challenge_data);
    let mut fields = Vec::with_capacity(4);

    for key in field_order {
        match WebAuthnField::try_from(*key)? {
            WebAuthnField::None => {},
            WebAuthnField::Challenge => fields.push(format!(r#""challenge":"{}""#, challenge_b64)),
            WebAuthnField::Type => fields.push(r#""type":"webauthn.get""#.to_string()),
            WebAuthnField::Origin => fields.push(format!(r#""origin":"{}""#, origin_str)),
            WebAuthnField::CrossOrigin => fields.push(r#""crossOrigin":false"#.to_string()),
        }
    }

    let client_data_json = format!("{{{}}}", fields.join(","));
    Ok(client_data_json.into_bytes())
}

fn base64url_encode_no_pad(data: &[u8]) -> String {
    const BASE64URL_CHARS: &[u8] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

    let mut result = String::new();
    let mut i = 0;

    while i + 2 < data.len() {
        let b1 = data[i];
        let b2 = data[i + 1];
        let b3 = data[i + 2];

        result.push(BASE64URL_CHARS[(b1 >> 2) as usize] as char);
        result.push(BASE64URL_CHARS[(((b1 & 0x03) << 4) | (b2 >> 4)) as usize] as char);
        result.push(BASE64URL_CHARS[(((b2 & 0x0f) << 2) | (b3 >> 6)) as usize] as char);
        result.push(BASE64URL_CHARS[(b3 & 0x3f) as usize] as char);

        i += 3;
    }

    if i < data.len() {
        let b1 = data[i];
        result.push(BASE64URL_CHARS[(b1 >> 2) as usize] as char);

        if i + 1 < data.len() {
            let b2 = data[i + 1];
            result.push(BASE64URL_CHARS[(((b1 & 0x03) << 4) | (b2 >> 4)) as usize] as char);
            result.push(BASE64URL_CHARS[((b2 & 0x0f) << 2) as usize] as char);
        } else {
            result.push(BASE64URL_CHARS[((b1 & 0x03) << 4) as usize] as char);
        }
    }

    result
}
