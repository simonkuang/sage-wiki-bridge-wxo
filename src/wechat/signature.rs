use constant_time_eq::constant_time_eq;
use sha1::{Digest, Sha1};

pub fn calculate_signature(token: &str, timestamp: &str, nonce: &str) -> String {
    let mut parts = [token, timestamp, nonce];
    parts.sort_unstable();

    let mut hasher = Sha1::new();
    for part in parts {
        hasher.update(part.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

pub fn calculate_encrypted_signature(
    token: &str,
    timestamp: &str,
    nonce: &str,
    encrypted_payload: &str,
) -> String {
    let mut parts = [token, timestamp, nonce, encrypted_payload];
    parts.sort_unstable();

    let mut hasher = Sha1::new();
    for part in parts {
        hasher.update(part.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

pub fn verify_signature(token: &str, timestamp: &str, nonce: &str, signature: &str) -> bool {
    let expected = calculate_signature(token, timestamp, nonce);
    constant_time_eq(expected.as_bytes(), signature.as_bytes())
}

pub fn verify_encrypted_signature(
    token: &str,
    timestamp: &str,
    nonce: &str,
    encrypted_payload: &str,
    signature: &str,
) -> bool {
    let expected = calculate_encrypted_signature(token, timestamp, nonce, encrypted_payload);
    constant_time_eq(expected.as_bytes(), signature.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifies_valid_signature() {
        let token = "bridge-token";
        let timestamp = "1780000000";
        let nonce = "abc123";
        let signature = calculate_signature(token, timestamp, nonce);

        assert!(verify_signature(token, timestamp, nonce, &signature));
    }

    #[test]
    fn rejects_invalid_signature() {
        assert!(!verify_signature(
            "bridge-token",
            "1780000000",
            "abc123",
            "not-a-valid-signature"
        ));
    }

    #[test]
    fn verifies_encrypted_signature() {
        let signature =
            calculate_encrypted_signature("bridge-token", "1780000000", "abc123", "encrypted");

        assert!(verify_encrypted_signature(
            "bridge-token",
            "1780000000",
            "abc123",
            "encrypted",
            &signature
        ));
        assert!(!verify_encrypted_signature(
            "bridge-token",
            "1780000000",
            "abc123",
            "changed",
            &signature
        ));
    }
}
