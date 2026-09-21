use super::LexiconSnapshotError;
use super::binary_v2::MAGIC;
use super::object::SpanRecord;

const MAX_STRINGS: u64 = 4_000_000;
const MAX_STRING_SIZE: u64 = 32 * 1024 * 1024;

pub(super) struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Reader<'a> {
    pub(super) const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    pub(super) fn expect_magic(&mut self) -> Result<(), LexiconSnapshotError> {
        if !self.bytes.starts_with(MAGIC) {
            return Err(binary_error("invalid v2 object magic"));
        }
        self.position = MAGIC.len();
        Ok(())
    }

    pub(super) fn uvarint(&mut self, field: &str) -> Result<u64, LexiconSnapshotError> {
        let mut value = 0_u64;
        for shift in (0..70).step_by(7) {
            let byte = self.byte(field)?;
            if shift == 63 && byte > 1 {
                return Err(binary_error(format!("invalid {field} varint")));
            }
            value |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
        Err(binary_error(format!("invalid {field} varint")))
    }

    pub(super) fn count(
        &mut self,
        field: &str,
        maximum: u64,
    ) -> Result<usize, LexiconSnapshotError> {
        let value = self.uvarint(field)?;
        if value > maximum {
            return Err(binary_error(format!("{field} count exceeds limit")));
        }
        usize::try_from(value).map_err(|_| binary_error(format!("{field} count overflows")))
    }

    pub(super) fn byte(&mut self, field: &str) -> Result<u8, LexiconSnapshotError> {
        let value = self
            .bytes
            .get(self.position)
            .copied()
            .ok_or_else(|| binary_error(format!("truncated before {field}")))?;
        self.position += 1;
        Ok(value)
    }

    pub(super) fn take(
        &mut self,
        length: usize,
        field: &str,
    ) -> Result<&'a [u8], LexiconSnapshotError> {
        let end = self
            .position
            .checked_add(length)
            .ok_or_else(|| binary_error(format!("{field} length overflows")))?;
        let value = self
            .bytes
            .get(self.position..end)
            .ok_or_else(|| binary_error(format!("truncated in {field}")))?;
        self.position = end;
        Ok(value)
    }

    pub(super) fn bytes(
        &mut self,
        field: &str,
        maximum: u64,
    ) -> Result<&'a [u8], LexiconSnapshotError> {
        let length = self.uvarint(&format!("{field} length"))?;
        if length > maximum {
            return Err(binary_error(format!("{field} length exceeds limit")));
        }
        let length = usize::try_from(length)
            .map_err(|_| binary_error(format!("{field} length overflows")))?;
        self.take(length, field)
    }

    pub(super) fn string_table(&mut self) -> Result<Vec<String>, LexiconSnapshotError> {
        let count = self.count("string table", MAX_STRINGS)?;
        if count == 0 {
            return Err(binary_error("missing empty string sentinel"));
        }
        let mut strings = Vec::with_capacity(count);
        let mut previous = Vec::<u8>::new();
        for index in 0..count {
            let prefix = self.uvarint("string prefix length")?;
            let prefix = usize::try_from(prefix)
                .map_err(|_| binary_error("string prefix length overflows"))?;
            if prefix > previous.len() {
                return Err(binary_error(format!(
                    "string {index} prefix exceeds previous string length"
                )));
            }
            let suffix = self.bytes("string suffix", MAX_STRING_SIZE)?;
            let length = prefix
                .checked_add(suffix.len())
                .ok_or_else(|| binary_error("reconstructed string length overflows"))?;
            if length > MAX_STRING_SIZE as usize {
                return Err(binary_error("reconstructed string length exceeds limit"));
            }
            let mut value = Vec::with_capacity(length);
            value.extend_from_slice(&previous[..prefix]);
            value.extend_from_slice(suffix);
            let decoded = std::str::from_utf8(&value)
                .map_err(|_| binary_error("string table contains invalid UTF-8"))?
                .to_owned();
            strings.push(decoded);
            previous = value;
        }
        if !strings[0].is_empty() {
            return Err(binary_error("invalid empty string sentinel"));
        }
        Ok(strings)
    }

    pub(super) fn string_ref<'b>(
        &mut self,
        strings: &'b [String],
        field: &str,
    ) -> Result<&'b str, LexiconSnapshotError> {
        let index = self.uvarint(field)?;
        let index = usize::try_from(index)
            .map_err(|_| binary_error(format!("{field} string index overflows")))?;
        strings
            .get(index)
            .map(String::as_str)
            .ok_or_else(|| binary_error(format!("{field} string index is out of range")))
    }

    pub(super) fn identity(
        &mut self,
        strings: &[String],
        field: &str,
    ) -> Result<String, LexiconSnapshotError> {
        match self.byte(&format!("{field} tag"))? {
            0 => Ok(self.string_ref(strings, field)?.to_owned()),
            1 => {
                let digest = self.take(32, field)?;
                let mut value = String::with_capacity(71);
                value.push_str("sha256:");
                const HEX: &[u8; 16] = b"0123456789abcdef";
                for byte in digest {
                    value.push(HEX[(byte >> 4) as usize] as char);
                    value.push(HEX[(byte & 0x0f) as usize] as char);
                }
                Ok(value)
            }
            tag => Err(binary_error(format!("invalid {field} identity tag {tag}"))),
        }
    }

    pub(super) fn node_ref(
        &mut self,
        node_ids: &[String],
        external: &[String],
        field: &str,
    ) -> Result<String, LexiconSnapshotError> {
        let tag = self.byte(&format!("{field} tag"))?;
        let index = self.uvarint(&format!("{field} index"))?;
        if index == 0 {
            return Err(binary_error(format!("{field} index is out of range")));
        }
        let index = usize::try_from(index - 1)
            .map_err(|_| binary_error(format!("{field} index overflows")))?;
        match tag {
            0 => node_ids
                .get(index)
                .cloned()
                .ok_or_else(|| binary_error(format!("{field} ordinal is out of range"))),
            1 => external
                .get(index)
                .cloned()
                .ok_or_else(|| binary_error(format!("{field} external index is out of range"))),
            _ => Err(binary_error(format!("invalid {field} reference tag {tag}"))),
        }
    }

    pub(super) fn code_or_string(
        &mut self,
        strings: &[String],
        values: &[&str],
        field: &str,
    ) -> Result<String, LexiconSnapshotError> {
        let code = self.uvarint(&format!("{field} code"))?;
        if code == 0 {
            return Ok(self.string_ref(strings, field)?.to_owned());
        }
        let index = usize::try_from(code - 1)
            .map_err(|_| binary_error(format!("{field} code overflows")))?;
        values
            .get(index)
            .map(|value| (*value).to_owned())
            .ok_or_else(|| binary_error(format!("{field} code is out of range")))
    }

    pub(super) fn factored(
        &mut self,
        strings: &[String],
        object_owner: &str,
        field: &str,
    ) -> Result<String, LexiconSnapshotError> {
        match self.uvarint(&format!("{field} factor"))? {
            0 => Ok(String::new()),
            1 => Ok(object_owner.to_owned()),
            2 => Ok(self.string_ref(strings, field)?.to_owned()),
            factor => Err(binary_error(format!("invalid {field} factor {factor}"))),
        }
    }

    pub(super) fn qualified_name(
        &mut self,
        strings: &[String],
        name: &str,
        path: &str,
        object_owner: &str,
        field: &str,
    ) -> Result<String, LexiconSnapshotError> {
        match self.uvarint(&format!("{field} factor"))? {
            0 => Ok(self.string_ref(strings, field)?.to_owned()),
            1 => Ok(name.to_owned()),
            2 => Ok(path.to_owned()),
            3 => Ok(object_owner.to_owned()),
            factor => Err(binary_error(format!("invalid {field} factor {factor}"))),
        }
    }

    pub(super) fn attributes(&mut self) -> Result<Option<Vec<u8>>, LexiconSnapshotError> {
        let value = self.bytes("record attributes", MAX_STRING_SIZE)?;
        Ok((!value.is_empty()).then(|| value.to_vec()))
    }

    pub(super) fn span(
        &mut self,
        strings: &[String],
    ) -> Result<Option<SpanRecord>, LexiconSnapshotError> {
        match self.byte("span flag")? {
            0 => Ok(None),
            1 => Ok(Some(SpanRecord {
                path: self.string_ref(strings, "span path")?.to_owned(),
                start_line: self.uvarint("span start line")?,
                start_column: self.uvarint("span start column")?,
                end_line: self.uvarint("span end line")?,
                end_column: self.uvarint("span end column")?,
            })),
            _ => Err(binary_error("invalid span flag")),
        }
    }

    pub(super) fn finish(&self, field: &str) -> Result<(), LexiconSnapshotError> {
        if self.position == self.bytes.len() {
            Ok(())
        } else {
            Err(binary_error(format!("{field} has trailing bytes")))
        }
    }
}

fn binary_error(message: impl Into<String>) -> LexiconSnapshotError {
    LexiconSnapshotError::Binary(message.into())
}
