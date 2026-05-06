use crate::error::AuthError;
use pinocchio::program_error::ProgramError;

/// Simple Base64URL encoder without padding.
/// Used to compare an on-chain-computed challenge against the base64url value
/// the browser authenticator placed inside clientDataJSON.
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

/// Minimum authenticator data length: rpIdHash(32) + flags(1) + counter(4) = 37
pub const AUTH_DATA_MIN_LEN: usize = 37;

/// Parser for WebAuthn authenticator data
pub struct AuthDataParser<'a> {
    data: &'a [u8],
}

impl<'a> AuthDataParser<'a> {
    pub fn new(data: &'a [u8]) -> Result<Self, ProgramError> {
        if data.len() < AUTH_DATA_MIN_LEN {
            return Err(AuthError::InvalidAuthorityPayload.into());
        }
        Ok(Self { data })
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

/// Extracts a top-level string value for a given key from a JSON object.
///
/// Walks `{"key":"value", ...}` looking for the specified key at depth 1.
/// Returns the value bytes (without quotes). Rejects escaped strings
/// (backslash inside key or value) to prevent challenge injection.
pub fn extract_top_level_string_field<'a>(
    json: &'a [u8],
    field_name: &[u8],
) -> Result<&'a [u8], ProgramError> {
    // Skip leading whitespace
    let mut i = 0;
    while i < json.len() && json[i].is_ascii_whitespace() {
        i += 1;
    }
    if i >= json.len() || json[i] != b'{' {
        return Err(AuthError::InvalidMessage.into());
    }

    let mut depth: usize = 0;
    let mut cursor = i;

    while cursor < json.len() {
        let byte = json[cursor];

        if byte == b'{' {
            depth += 1;
            cursor += 1;
            continue;
        }
        if byte == b'}' {
            if depth == 0 {
                return Err(AuthError::InvalidMessage.into());
            }
            depth -= 1;
            cursor += 1;
            continue;
        }

        // Only parse keys at the top level (depth == 1)
        if depth == 1 && byte == b'"' {
            // Parse key
            let key_start = cursor + 1;
            let mut key_end = key_start;
            while key_end < json.len() {
                let b = json[key_end];
                if b == b'"' {
                    break;
                }
                if b == b'\\' {
                    return Err(AuthError::InvalidMessage.into());
                }
                key_end += 1;
            }
            if key_end >= json.len() {
                return Err(AuthError::InvalidMessage.into());
            }

            // Skip past closing quote
            cursor = key_end + 1;

            // Skip whitespace before colon
            while cursor < json.len() && json[cursor].is_ascii_whitespace() {
                cursor += 1;
            }
            if cursor >= json.len() || json[cursor] != b':' {
                return Err(AuthError::InvalidMessage.into());
            }
            cursor += 1;

            // Skip whitespace after colon
            while cursor < json.len() && json[cursor].is_ascii_whitespace() {
                cursor += 1;
            }
            if cursor >= json.len() {
                return Err(AuthError::InvalidMessage.into());
            }

            // Check if this is the field we want
            if &json[key_start..key_end] == field_name {
                // Value must be a string
                if json[cursor] != b'"' {
                    return Err(AuthError::InvalidMessage.into());
                }
                let value_start = cursor + 1;
                let mut value_end = value_start;
                while value_end < json.len() {
                    let b = json[value_end];
                    if b == b'"' {
                        return Ok(&json[value_start..value_end]);
                    }
                    if b == b'\\' {
                        return Err(AuthError::InvalidMessage.into());
                    }
                    value_end += 1;
                }
                return Err(AuthError::InvalidMessage.into());
            }

            // Not our field — skip the value
            if json[cursor] == b'"' {
                // String value — skip to closing quote
                cursor += 1;
                while cursor < json.len() {
                    let b = json[cursor];
                    if b == b'"' {
                        cursor += 1;
                        break;
                    }
                    if b == b'\\' {
                        return Err(AuthError::InvalidMessage.into());
                    }
                    cursor += 1;
                }
            } else {
                // Non-string value (number, bool, null, object, array) — skip
                // until the next comma or closing brace at the same depth.
                // Track nested braces and brackets, and when we encounter a
                // string, consume it entirely so that `{`, `}`, `[`, `]`, or
                // `,` inside the string body don't corrupt depth tracking.
                //
                // Without the inner string-skip, a payload like
                //   {"tokenBinding":{"id":"x}y"},"challenge":"real"}
                // would have the `}` inside "x}y" mistakenly close the nested
                // object and the parser would then mis-locate the top-level
                // "challenge" entry.
                let mut nest: usize = 0;
                while cursor < json.len() {
                    match json[cursor] {
                        b'"' => {
                            // Consume the string; inner quotes/braces/commas
                            // must not affect outer nesting state.
                            cursor += 1;
                            while cursor < json.len() {
                                let b = json[cursor];
                                if b == b'"' {
                                    cursor += 1;
                                    break;
                                }
                                if b == b'\\' {
                                    return Err(AuthError::InvalidMessage.into());
                                }
                                cursor += 1;
                            }
                            continue;
                        }
                        b'{' | b'[' => nest += 1,
                        b'}' | b']' => {
                            if nest == 0 {
                                break;
                            }
                            nest -= 1;
                        }
                        b',' if nest == 0 => break,
                        _ => {}
                    }
                    cursor += 1;
                }
            }
            continue;
        }

        cursor += 1;
    }

    Err(AuthError::InvalidMessage.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── extract_top_level_string_field tests ───────────────────────────

    #[test]
    fn test_extract_field_basic() {
        let json = br#"{"type":"webauthn.get","challenge":"abc123","origin":"https://example.com"}"#;
        assert_eq!(
            extract_top_level_string_field(json, b"type").unwrap(),
            b"webauthn.get"
        );
        assert_eq!(
            extract_top_level_string_field(json, b"challenge").unwrap(),
            b"abc123"
        );
        assert_eq!(
            extract_top_level_string_field(json, b"origin").unwrap(),
            b"https://example.com"
        );
    }

    #[test]
    fn test_extract_field_with_bool_value() {
        let json =
            br#"{"type":"webauthn.get","challenge":"abc","crossOrigin":false,"origin":"https://x.com"}"#;
        assert_eq!(
            extract_top_level_string_field(json, b"challenge").unwrap(),
            b"abc"
        );
        assert_eq!(
            extract_top_level_string_field(json, b"origin").unwrap(),
            b"https://x.com"
        );
    }

    #[test]
    fn test_extract_field_with_nested_object() {
        // Real Android clientDataJSON has extra fields like androidPackageName
        let json =
            br#"{"type":"webauthn.get","challenge":"xyz","origin":"https://a.com","androidPackageName":"com.example.app"}"#;
        assert_eq!(
            extract_top_level_string_field(json, b"type").unwrap(),
            b"webauthn.get"
        );
        assert_eq!(
            extract_top_level_string_field(json, b"challenge").unwrap(),
            b"xyz"
        );
    }

    #[test]
    fn test_extract_field_with_nested_json_object() {
        // Nested object should be skipped when looking for top-level keys
        let json = br#"{"nested":{"challenge":"fake"},"type":"webauthn.get","challenge":"real"}"#;
        assert_eq!(
            extract_top_level_string_field(json, b"challenge").unwrap(),
            b"real"
        );
        assert_eq!(
            extract_top_level_string_field(json, b"type").unwrap(),
            b"webauthn.get"
        );
    }

    #[test]
    fn test_extract_field_missing_key() {
        let json = br#"{"type":"webauthn.get"}"#;
        assert!(extract_top_level_string_field(json, b"challenge").is_err());
    }

    #[test]
    fn test_extract_field_rejects_escaped_key() {
        // Backslash in key → reject (prevents injection)
        let json = br#"{"ty\"pe":"webauthn.get"}"#;
        assert!(extract_top_level_string_field(json, b"type").is_err());
    }

    #[test]
    fn test_extract_field_rejects_escaped_value() {
        // Backslash in value → reject
        let json = br#"{"challenge":"abc\"def"}"#;
        assert!(extract_top_level_string_field(json, b"challenge").is_err());
    }

    #[test]
    fn test_extract_field_rejects_non_string_value() {
        // challenge is a number, not a string → reject
        let json = br#"{"challenge":12345}"#;
        assert!(extract_top_level_string_field(json, b"challenge").is_err());
    }

    #[test]
    fn test_extract_field_rejects_empty_input() {
        assert!(extract_top_level_string_field(b"", b"type").is_err());
    }

    #[test]
    fn test_extract_field_rejects_not_object() {
        assert!(extract_top_level_string_field(b"[1,2,3]", b"type").is_err());
    }

    #[test]
    fn test_extract_field_nested_challenge_not_found_at_top() {
        // challenge only exists inside a nested object — should not be found
        let json = br#"{"type":"webauthn.get","nested":{"challenge":"sneaky"}}"#;
        assert!(extract_top_level_string_field(json, b"challenge").is_err());
    }

    #[test]
    fn test_extract_field_with_whitespace() {
        let json = br#"{ "type" : "webauthn.get" , "challenge" : "abc" }"#;
        assert_eq!(
            extract_top_level_string_field(json, b"type").unwrap(),
            b"webauthn.get"
        );
        assert_eq!(
            extract_top_level_string_field(json, b"challenge").unwrap(),
            b"abc"
        );
    }

    #[test]
    fn test_extract_field_google_extra() {
        // Google Chrome adds this extra field
        let json = br#"{"type":"webauthn.get","challenge":"abc","origin":"https://x.com","crossOrigin":false,"other_keys_can_be_added_here":"do not compare clientDataJSON against a template. See https://goo.gl/yabPex"}"#;
        assert_eq!(
            extract_top_level_string_field(json, b"challenge").unwrap(),
            b"abc"
        );
        assert_eq!(
            extract_top_level_string_field(json, b"type").unwrap(),
            b"webauthn.get"
        );
    }

    #[test]
    fn test_extract_field_with_array_value() {
        // Array value should be skipped properly
        let json = br#"{"arr":[1,2,3],"challenge":"abc"}"#;
        assert_eq!(
            extract_top_level_string_field(json, b"challenge").unwrap(),
            b"abc"
        );
    }

    #[test]
    fn test_extract_field_with_nested_array_of_objects() {
        let json = br#"{"arr":[{"challenge":"fake"},{"x":1}],"challenge":"real"}"#;
        assert_eq!(
            extract_top_level_string_field(json, b"challenge").unwrap(),
            b"real"
        );
    }

    // ─── M1 regression: string content inside nested value must not
    //     corrupt depth tracking ──────────────────────────────────────

    #[test]
    fn test_extract_skips_string_containing_close_brace_in_nested_object() {
        // Pre-fix, the `}` inside "x}y" would mistakenly close the nested
        // object, and the parser would then mis-locate the top-level
        // "challenge" entry. Post-fix, the inner string is consumed as a
        // whole so nested depth stays at 1 until the real `}` at end of
        // the tokenBinding value.
        let json = br#"{"tokenBinding":{"id":"x}y"},"challenge":"real"}"#;
        assert_eq!(
            extract_top_level_string_field(json, b"challenge").unwrap(),
            b"real"
        );
    }

    #[test]
    fn test_extract_skips_string_containing_close_bracket() {
        let json = br#"{"arr":[{"id":"x]y"}],"challenge":"real"}"#;
        assert_eq!(
            extract_top_level_string_field(json, b"challenge").unwrap(),
            b"real"
        );
    }

    #[test]
    fn test_extract_skips_string_containing_comma_in_nested_object() {
        // Comma inside a string at non-zero depth — should not affect anything,
        // but once we `continue` to the top of the skip loop we could be fooled
        // into thinking the comma terminates the value.
        let json = br#"{"obj":{"k":"a,b,c"},"challenge":"real"}"#;
        assert_eq!(
            extract_top_level_string_field(json, b"challenge").unwrap(),
            b"real"
        );
    }

    #[test]
    fn test_extract_skips_string_containing_open_brace_in_array() {
        let json = br#"{"arr":[{"id":"x{y"}],"challenge":"real"}"#;
        assert_eq!(
            extract_top_level_string_field(json, b"challenge").unwrap(),
            b"real"
        );
    }

    #[test]
    fn test_extract_challenge_before_tokenbinding_still_works() {
        // Safety: ensure the fix doesn't break the common happy path.
        let json = br#"{"type":"webauthn.get","challenge":"real","tokenBinding":{"id":"x}y"}}"#;
        assert_eq!(
            extract_top_level_string_field(json, b"challenge").unwrap(),
            b"real"
        );
    }

    #[test]
    fn test_extract_rejects_backslash_in_skipped_string_inside_nested() {
        // Backslashes are rejected in string values everywhere, including
        // strings inside a skipped nested object. This prevents escape
        // injection reaching the parser via nested fields.
        let json = br#"{"obj":{"id":"a\"b"},"challenge":"real"}"#;
        assert!(extract_top_level_string_field(json, b"challenge").is_err());
    }

    // ─── base64url_encode_no_pad tests ──────────────────────────────────

    #[test]
    fn test_base64url_encode_empty() {
        assert_eq!(base64url_encode_no_pad(&[]), b"");
    }

    #[test]
    fn test_base64url_encode_known_vectors() {
        // "f" → "Zg"
        assert_eq!(base64url_encode_no_pad(b"f"), b"Zg");
        // "fo" → "Zm8"
        assert_eq!(base64url_encode_no_pad(b"fo"), b"Zm8");
        // "foo" → "Zm9v"
        assert_eq!(base64url_encode_no_pad(b"foo"), b"Zm9v");
    }

    #[test]
    fn test_base64url_encode_32_bytes() {
        // SHA256 output (32 bytes) → 43 base64url chars (no padding)
        let data = [0x11u8; 32];
        let encoded = base64url_encode_no_pad(&data);
        assert_eq!(encoded.len(), 43);
        // Verify no padding characters
        assert!(!encoded.contains(&b'='));
        // Verify URL-safe: no + or /
        assert!(!encoded.contains(&b'+'));
        assert!(!encoded.contains(&b'/'));
    }

    #[test]
    fn test_base64url_uses_url_safe_chars() {
        // 0xFB, 0xFF → should produce '-' and '_' instead of '+' and '/'
        let data = [0xFB, 0xFF, 0xFE];
        let encoded = base64url_encode_no_pad(&data);
        let encoded_str = std::str::from_utf8(&encoded).unwrap();
        assert!(
            !encoded_str.contains('+') && !encoded_str.contains('/'),
            "Must use URL-safe alphabet"
        );
    }

    // ─── AuthDataParser tests ───────────────────────────────────────────

    #[test]
    fn test_auth_data_parser_basic() {
        let mut data = [0u8; 37];
        // rpIdHash = first 32 bytes (zeros)
        data[32] = 0x05; // flags: user present (0x01) + user verified (0x04)
        data[33..37].copy_from_slice(&[0, 0, 0, 42]); // counter = 42 (big-endian)

        let parser = AuthDataParser::new(&data).unwrap();
        assert!(parser.is_user_present());
        assert!(parser.is_user_verified());
        assert_eq!(parser.counter(), 42);
        assert_eq!(parser.rp_id_hash(), &[0u8; 32]);
    }

    #[test]
    fn test_auth_data_parser_no_flags() {
        let data = [0u8; 37];
        let parser = AuthDataParser::new(&data).unwrap();
        assert!(!parser.is_user_present());
        assert!(!parser.is_user_verified());
    }

    #[test]
    fn test_auth_data_parser_too_short() {
        let data = [0u8; 36]; // Less than 37
        assert!(AuthDataParser::new(&data).is_err());
    }

    #[test]
    fn test_auth_data_parser_with_extensions() {
        // Real authenticators may return > 37 bytes (with extensions)
        let mut data = [0u8; 100];
        data[32] = 0x41; // user present + attested credential data
        let parser = AuthDataParser::new(&data).unwrap();
        assert!(parser.is_user_present());
    }

    // ─── Real-world clientDataJSON samples ──────────────────────────────

    #[test]
    fn test_extract_from_chrome_sample() {
        let json = br#"{"type":"webauthn.get","challenge":"dGVzdC1jaGFsbGVuZ2U","origin":"https://lazorkit.app","crossOrigin":false}"#;
        assert_eq!(
            extract_top_level_string_field(json, b"type").unwrap(),
            b"webauthn.get"
        );
        assert_eq!(
            extract_top_level_string_field(json, b"challenge").unwrap(),
            b"dGVzdC1jaGFsbGVuZ2U"
        );
        assert_eq!(
            extract_top_level_string_field(json, b"origin").unwrap(),
            b"https://lazorkit.app"
        );
    }

    #[test]
    fn test_extract_from_android_sample() {
        // Android may include androidPackageName and topOrigin
        let json = br#"{"type":"webauthn.get","challenge":"abc123","origin":"https://example.com","androidPackageName":"com.example.app","topOrigin":"https://example.com"}"#;
        assert_eq!(
            extract_top_level_string_field(json, b"type").unwrap(),
            b"webauthn.get"
        );
        assert_eq!(
            extract_top_level_string_field(json, b"challenge").unwrap(),
            b"abc123"
        );
        assert_eq!(
            extract_top_level_string_field(json, b"androidPackageName").unwrap(),
            b"com.example.app"
        );
    }

    #[test]
    fn test_extract_from_safari_no_crossorigin() {
        // Safari may omit crossOrigin entirely
        let json =
            br#"{"type":"webauthn.get","challenge":"xyz","origin":"https://lazorkit.app"}"#;
        assert_eq!(
            extract_top_level_string_field(json, b"type").unwrap(),
            b"webauthn.get"
        );
        assert_eq!(
            extract_top_level_string_field(json, b"challenge").unwrap(),
            b"xyz"
        );
        // crossOrigin field doesn't exist → error
        assert!(extract_top_level_string_field(json, b"crossOrigin").is_err());
    }

    #[test]
    fn test_extract_rejects_webauthn_create_type() {
        let json = br#"{"type":"webauthn.create","challenge":"abc"}"#;
        let type_val = extract_top_level_string_field(json, b"type").unwrap();
        assert_ne!(type_val, b"webauthn.get");
        assert_eq!(type_val, b"webauthn.create");
    }
}
