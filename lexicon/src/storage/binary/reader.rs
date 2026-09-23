use super::{MAX_STRING_SIZE, MAX_STRINGS};
use crate::{SourceSpan, StorageError};

pub(crate) struct Reader<'a> {
    bytes: &'a [u8],
    pub(crate) position: usize,
}

impl<'a> Reader<'a> {
    pub(crate) const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    pub(crate) const fn at(bytes: &'a [u8], position: usize) -> Self {
        Self { bytes, position }
    }

    pub(crate) fn uvarint(&mut self, field: &str) -> Result<u64, StorageError> {
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

    pub(crate) fn count(&mut self, field: &str, maximum: u64) -> Result<usize, StorageError> {
        let value = self.uvarint(field)?;
        if value > maximum {
            return Err(binary_error(format!("{field} count exceeds limit")));
        }
        usize::try_from(value).map_err(|_| binary_error(format!("{field} count overflows")))
    }

    pub(crate) fn byte(&mut self, field: &str) -> Result<u8, StorageError> {
        let value = self
            .bytes
            .get(self.position)
            .copied()
            .ok_or_else(|| binary_error(format!("truncated before {field}")))?;
        self.position += 1;
        Ok(value)
    }

    pub(crate) fn take(&mut self, length: usize, field: &str) -> Result<&'a [u8], StorageError> {
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

    pub(crate) fn bytes(&mut self, field: &str, maximum: u64) -> Result<&'a [u8], StorageError> {
        let length = self.uvarint(&format!("{field} length"))?;
        if length > maximum {
            return Err(binary_error(format!("{field} length exceeds limit")));
        }
        let length = usize::try_from(length)
            .map_err(|_| binary_error(format!("{field} length overflows")))?;
        self.take(length, field)
    }

    pub(crate) fn string_ref<'b>(
        &mut self,
        strings: &'b [String],
        field: &str,
    ) -> Result<&'b str, StorageError> {
        let index = usize::try_from(self.uvarint(field)?)
            .map_err(|_| binary_error(format!("{field} string index overflows")))?;
        strings
            .get(index)
            .map(String::as_str)
            .ok_or_else(|| binary_error(format!("{field} string index is out of range")))
    }

    pub(crate) fn strings_v1(&mut self) -> Result<Vec<String>, StorageError> {
        let count = self.count("string table", MAX_STRINGS)?;
        if count == 0 {
            return Err(binary_error("missing empty string sentinel"));
        }
        let mut values = Vec::with_capacity(count);
        for _ in 0..count {
            let bytes = self.bytes("string", MAX_STRING_SIZE)?;
            values.push(
                std::str::from_utf8(bytes)
                    .map_err(|_| binary_error("string table contains invalid UTF-8"))?
                    .to_owned(),
            );
        }
        empty_sentinel(values)
    }

    pub(crate) fn span(&mut self, strings: &[String]) -> Result<Option<SourceSpan>, StorageError> {
        match self.byte("span flag")? {
            0 => Ok(None),
            1 => Ok(Some(SourceSpan {
                path: self.string_ref(strings, "span path")?.to_owned(),
                start_line: self.uvarint("span start line")?,
                start_column: self.uvarint("span start column")?,
                end_line: self.uvarint("span end line")?,
                end_column: self.uvarint("span end column")?,
            })),
            _ => Err(binary_error("invalid span flag")),
        }
    }

    pub(crate) fn finish(&self, field: &str) -> Result<(), StorageError> {
        if self.position == self.bytes.len() {
            Ok(())
        } else {
            Err(binary_error(format!("{field} has trailing bytes")))
        }
    }
}

pub(crate) fn empty_sentinel(values: Vec<String>) -> Result<Vec<String>, StorageError> {
    if values.first().is_some_and(String::is_empty) {
        Ok(values)
    } else {
        Err(binary_error("invalid empty string sentinel"))
    }
}

pub(crate) fn binary_error(message: impl Into<String>) -> StorageError {
    StorageError::Binary(message.into())
}
