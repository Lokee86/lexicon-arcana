use super::LexiconSnapshotError;
use crate::repository::{ContentId, NodeKey};

const PREFIX: &str = "sha256:";

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Hash)]
pub(super) struct LexiconIdentity([u8; 32]);

impl LexiconIdentity {
    pub(super) const fn from_digest(digest: [u8; 32]) -> Self {
        Self(digest)
    }

    pub(super) fn parse(value: &str) -> Result<Self, LexiconSnapshotError> {
        let Some(hex) = value.strip_prefix(PREFIX) else {
            return Err(LexiconSnapshotError::InvalidId(value.to_owned()));
        };
        if hex.len() != 64 {
            return Err(LexiconSnapshotError::InvalidId(value.to_owned()));
        }

        let mut digest = [0_u8; 32];
        for (index, pair) in hex.as_bytes().chunks_exact(2).enumerate() {
            digest[index] = (hex_nibble(pair[0], value)? << 4) | hex_nibble(pair[1], value)?;
        }
        Ok(Self(digest))
    }

    pub(super) fn node_key(self) -> NodeKey {
        NodeKey::from_sha256_digest(&self.0)
    }

    pub(super) fn content_id(self) -> ContentId {
        ContentId::from_sha256_digest(&self.0)
    }

    pub(super) fn canonical_string(self) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut output = String::with_capacity(PREFIX.len() + 64);
        output.push_str(PREFIX);
        for byte in self.0 {
            output.push(HEX[(byte >> 4) as usize] as char);
            output.push(HEX[(byte & 0x0f) as usize] as char);
        }
        output
    }
}

fn hex_nibble(byte: u8, value: &str) -> Result<u8, LexiconSnapshotError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(LexiconSnapshotError::InvalidId(value.to_owned())),
    }
}

#[cfg(test)]
mod tests {
    use super::LexiconIdentity;
    use crate::repository::{ContentId, NodeKey};

    #[test]
    fn digest_hashing_matches_canonical_text_without_allocating_the_text() {
        let text = format!("sha256:{}", "ab".repeat(32));
        let identity = LexiconIdentity::parse(&text).unwrap();

        assert_eq!(identity.node_key(), NodeKey::from_identity(text.as_bytes()));
        assert_eq!(
            identity.content_id(),
            ContentId::from_bytes(text.as_bytes())
        );
        assert_eq!(identity.canonical_string(), text);
    }
}
