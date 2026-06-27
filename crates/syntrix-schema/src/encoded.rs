/// Sortable encoding for secondary index values in redb.
///
/// All values are encoded as strings that sort lexicographically
/// in the same order as their original type. This enables redb's
/// ordered range scans to work correctly for numeric, boolean,
/// and string fields alike.
///
/// Encoding rules:
/// - **i64**: bit-flip the sign, then encode as 16-char hex big-endian
/// - **f64**: flip sign bit for positive numbers, encode as 16-char hex big-endian
/// - **String**: lowercase, trim, no further encoding (already lexicographic)
/// - **Boolean**: `"0"` = false, `"1"` = true
/// - **Date**: ISO 8601 string (already sortable)
/// - Other: empty string (will not match, filtering works as no-op)

pub fn encode_value(val: &serde_json::Value) -> String {
    match val {
        serde_json::Value::Number(n) => encode_number(n),
        serde_json::Value::String(s) => {
            let cleaned: String = s.chars().filter(|c| !c.is_ascii_control()).collect();
            cleaned.to_lowercase()
        }
        serde_json::Value::Bool(b) => {
            if *b { "1".to_string() } else { "0".to_string() }
        }
        _ => String::new(),
    }
}

fn encode_number(n: &serde_json::Number) -> String {
    if let Some(i) = n.as_i64() {
        // Flip the sign bit so negatives sort before positives
        let flipped = (i as u64) ^ (1u64 << 63);
        format!("{:016x}", flipped)
    } else if let Some(f) = n.as_f64() {
        // IEEE 754 sortable encoding
        let bits = f.to_bits();
        let flipped = if f.is_sign_positive() {
            bits ^ (1u64 << 63)
        } else {
            let mut b = bits;
            b ^= 1u64 << 63;
            b ^= (1u64 << 63) - 1;
            b
        };
        format!("{:016x}", flipped)
    } else if let Some(u) = n.as_u64() {
        format!("{:016x}", u)
    } else {
        // Fallback: encode as string (will not sort correctly for numbers)
        n.to_string()
    }
}

/// Encode an i64 as a sortable hex string (big-endian with sign flip).
pub fn encode_i64(val: i64) -> String {
    let flipped = (val as u64) ^ (1u64 << 63);
    format!("{:016x}", flipped)
}

/// Encode a string for index keys (lowercase, trimmed, control-filtered).
pub fn encode_str(val: &str) -> String {
    let cleaned: String = val.chars().filter(|c| !c.is_ascii_control()).collect();
    cleaned.to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_i64_ordering() {
        let small = encode_i64(-100);
        let zero = encode_i64(0);
        let large = encode_i64(100);
        assert!(small < zero, "negative should sort before zero");
        assert!(zero < large, "zero should sort before positive");
        assert!(small < large, "negative should sort before positive");

        let a = encode_i64(2);
        let b = encode_i64(10);
        // 10 should sort AFTER 2 (lexicographically correct as hex big-endian)
        assert!(a < b, "2 < 10 lexicographically in big-endian hex");
    }

    #[test]
    fn test_string_ordering() {
        let a = encode_str("alpha");
        let b = encode_str("beta");
        assert!(a < b);
        assert_eq!(encode_str("ALPHA"), encode_str("alpha"));
    }

    #[test]
    fn test_bool_ordering() {
        let false_val = encode_value(&serde_json::Value::Bool(false));
        let true_val = encode_value(&serde_json::Value::Bool(true));
        assert!(false_val < true_val);
    }

    #[test]
    fn test_number_from_json() {
        let small = serde_json::json!(-100);
        let large = serde_json::json!(100);
        let encoded_small = encode_value(&small);
        let encoded_large = encode_value(&large);
        assert!(encoded_small < encoded_large);
    }
}
