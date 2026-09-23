use super::reader::{Reader, binary_error, empty_sentinel};
use super::{MAX_STRING_SIZE, MAX_STRINGS};
use crate::StorageError;

pub(crate) fn string_table(reader: &mut Reader<'_>) -> Result<Vec<String>, StorageError> {
    let count = reader.count("string table", MAX_STRINGS)?;
    if count == 0 {
        return Err(binary_error("missing empty string sentinel"));
    }
    let mut strings = Vec::with_capacity(count);
    let mut previous = Vec::<u8>::new();
    for index in 0..count {
        let prefix = usize::try_from(reader.uvarint("string prefix length")?)
            .map_err(|_| binary_error("string prefix length overflows"))?;
        if prefix > previous.len() {
            return Err(binary_error(format!(
                "string {index} prefix exceeds previous string length"
            )));
        }
        let suffix = reader.bytes("string suffix", MAX_STRING_SIZE)?;
        let length = prefix
            .checked_add(suffix.len())
            .ok_or_else(|| binary_error("reconstructed string length overflows"))?;
        if length > MAX_STRING_SIZE as usize {
            return Err(binary_error("reconstructed string length exceeds limit"));
        }
        let mut value = Vec::with_capacity(length);
        value.extend_from_slice(&previous[..prefix]);
        value.extend_from_slice(suffix);
        strings.push(
            std::str::from_utf8(&value)
                .map_err(|_| binary_error("string table contains invalid UTF-8"))?
                .to_owned(),
        );
        previous = value;
    }
    empty_sentinel(strings)
}

pub(crate) fn identity(
    reader: &mut Reader<'_>,
    strings: &[String],
    field: &str,
) -> Result<String, StorageError> {
    match reader.byte(&format!("{field} tag"))? {
        0 => Ok(reader.string_ref(strings, field)?.to_owned()),
        1 => {
            let digest = reader.take(32, field)?;
            let mut value = String::with_capacity(71);
            value.push_str("sha256:");
            for byte in digest {
                value.push_str(&format!("{byte:02x}"));
            }
            Ok(value)
        }
        tag => Err(binary_error(format!("invalid {field} identity tag {tag}"))),
    }
}
