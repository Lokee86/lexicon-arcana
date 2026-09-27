use super::{CompactStringTable, StoreFormatError};
use crate::repository_store::format::{ABSENT_STRING_ID, STRING_INDEX_RECORD_LEN};

impl CompactStringTable {
    pub fn encode(&self) -> Result<Vec<u8>, StoreFormatError> {
        let index_len = self
            .strings
            .len()
            .checked_mul(STRING_INDEX_RECORD_LEN as usize)
            .ok_or(StoreFormatError::SizeOverflow)?;
        let blob_len = self.strings.iter().try_fold(0usize, |size, value| {
            size.checked_add(value.len())
                .ok_or(StoreFormatError::SizeOverflow)
        })?;
        let mut bytes = vec![0_u8; index_len];
        bytes.reserve(blob_len);
        let mut blob_offset = 0_u64;
        for (index, value) in self.strings.iter().enumerate() {
            let base = index * STRING_INDEX_RECORD_LEN as usize;
            put_u64(&mut bytes, base, blob_offset);
            let len = u32::try_from(value.len()).map_err(|_| StoreFormatError::StringTooLong)?;
            put_u32(&mut bytes, base + 8, len);
            bytes.extend_from_slice(value.as_bytes());
            blob_offset = blob_offset
                .checked_add(u64::from(len))
                .ok_or(StoreFormatError::SizeOverflow)?;
        }
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8], count: u64) -> Result<Self, StoreFormatError> {
        let count = usize::try_from(count).map_err(|_| StoreFormatError::TooManyStrings)?;
        if count > ABSENT_STRING_ID as usize {
            return Err(StoreFormatError::TooManyStrings);
        }
        let index_len = count
            .checked_mul(STRING_INDEX_RECORD_LEN as usize)
            .ok_or(StoreFormatError::SizeOverflow)?;
        if index_len > bytes.len() {
            return Err(StoreFormatError::MalformedStringTable);
        }

        let blob = &bytes[index_len..];
        let mut strings = Vec::with_capacity(count);
        let mut expected_offset = 0usize;
        for index in 0..count {
            let base = index * STRING_INDEX_RECORD_LEN as usize;
            let offset = usize::try_from(get_u64(bytes, base))
                .map_err(|_| StoreFormatError::MalformedStringTable)?;
            let len = get_u32(bytes, base + 8) as usize;
            if get_u32(bytes, base + 12) != 0 || offset != expected_offset {
                return Err(StoreFormatError::MalformedStringTable);
            }
            let end = offset
                .checked_add(len)
                .filter(|end| *end <= blob.len())
                .ok_or(StoreFormatError::MalformedStringTable)?;
            let value = std::str::from_utf8(&blob[offset..end])
                .map_err(|_| StoreFormatError::InvalidUtf8)?;
            if strings
                .last()
                .is_some_and(|last: &String| last.as_str() >= value)
            {
                return Err(StoreFormatError::NonCanonicalStrings);
            }
            strings.push(value.to_owned());
            expected_offset = end;
        }
        if expected_offset != blob.len() {
            return Err(StoreFormatError::MalformedStringTable);
        }
        Ok(Self { strings })
    }
}

fn get_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("string index range"),
    )
}

fn get_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(
        bytes[offset..offset + 8]
            .try_into()
            .expect("string index range"),
    )
}

fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn put_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}
