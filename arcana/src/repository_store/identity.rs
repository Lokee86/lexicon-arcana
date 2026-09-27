use super::StoreFormatError;

const PREFIX: &str = "sha256:";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Sha256Identity(pub [u8; 32]);

impl Sha256Identity {
    pub fn parse(value: &str) -> Result<Self, StoreFormatError> {
        let digest = value
            .strip_prefix(PREFIX)
            .ok_or(StoreFormatError::InvalidExternalIdentity)?;
        if digest.len() != 64 {
            return Err(StoreFormatError::InvalidExternalIdentity);
        }
        let mut bytes = [0_u8; 32];
        for (index, pair) in digest.as_bytes().chunks_exact(2).enumerate() {
            bytes[index] = (hex_nibble(pair[0])? << 4) | hex_nibble(pair[1])?;
        }
        Ok(Self(bytes))
    }

    pub fn canonical_string(self) -> String {
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

fn hex_nibble(byte: u8) -> Result<u8, StoreFormatError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(StoreFormatError::InvalidExternalIdentity),
    }
}
