use sha2::{Digest, Sha256};
use std::fmt;

const NODE_ID_PREFIX: &[u8] = b"lexicon:v1\0";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidSha256Id(pub String);

impl fmt::Display for InvalidSha256Id {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid canonical Lexicon SHA-256 identity {:?}",
            self.0
        )
    }
}

impl std::error::Error for InvalidSha256Id {}

pub fn validate_sha256_id(value: &str) -> Result<(), InvalidSha256Id> {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return Err(InvalidSha256Id(value.to_owned()));
    };
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(InvalidSha256Id(value.to_owned()));
    }
    Ok(())
}

pub fn node_id(language: &str, kind: &str, canonical_identity: &str) -> String {
    let mut hash = Sha256::new();
    hash.update(NODE_ID_PREFIX);
    hash.update(language.as_bytes());
    hash.update([0]);
    hash.update(kind.as_bytes());
    hash.update([0]);
    hash.update(canonical_identity.as_bytes());
    format!("sha256:{:x}", hash.finalize())
}

pub fn content_id(content: &[u8]) -> String {
    let digest = Sha256::digest(content);
    format!("sha256:{digest:x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_existing_adapter_identity_vectors() {
        assert_eq!(
            node_id("python", "file", "main.py"),
            "sha256:e774b7aef4d62e1f9c5f1b1a045d104fd913fb25b96924be36e27b4b51d63e1f"
        );
        assert_eq!(
            node_id("java", "type", "com.acme.Service"),
            "sha256:6eabcddb31caf8b3433318768f8dea5b823248d9b6299dcfe64865e6624bc0ce"
        );
    }

    #[test]
    fn content_identity_matches_sha256() {
        assert_eq!(
            content_id(b"abc"),
            "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn accepts_only_canonical_lowercase_sha256() {
        let valid = format!("sha256:{}", "a".repeat(64));
        assert!(validate_sha256_id(&valid).is_ok());
        assert!(validate_sha256_id(&valid.to_uppercase()).is_err());
        assert!(validate_sha256_id("sha256:abc").is_err());
        assert!(validate_sha256_id(&format!("sha256:{}", "g".repeat(64))).is_err());
    }
}
