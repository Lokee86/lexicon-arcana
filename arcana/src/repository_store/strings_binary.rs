use super::{CompactStringTable, StoreFormatError};
use crate::repository_store::format::{ABSENT_STRING_ID, STRING_INDEX_RECORD_LEN};

impl CompactStringTable {
    pub fn encode(&self) -> Result<Vec<u8>, StoreFormatError> {
        let count = self.len();
        let (blob, offsets) = self.parts();
        let index_len = count
            .checked_mul(STRING_INDEX_RECORD_LEN as usize)
            .ok_or(StoreFormatError::SizeOverflow)?;
        let mut bytes = vec![0_u8; index_len];
        bytes.reserve(blob.len());

        for index in 0..count {
            let base = index * STRING_INDEX_RECORD_LEN as usize;
            let start = offsets[index];
            let end = offsets[index + 1];
            let len = end
                .checked_sub(start)
                .ok_or(StoreFormatError::MalformedStringTable)?;
            let len = u32::try_from(len).map_err(|_| StoreFormatError::StringTooLong)?;
            put_u64(&mut bytes, base, start);
            put_u32(&mut bytes, base + 8, len);
        }
        bytes.extend_from_slice(blob);
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
        let mut offsets = Vec::with_capacity(count + 1);
        offsets.push(0);
        let mut expected_offset = 0usize;
        let mut previous: Option<&str> = None;

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
            if previous.is_some_and(|previous| previous >= value) {
                return Err(StoreFormatError::NonCanonicalStrings);
            }
            previous = Some(value);
            expected_offset = end;
            offsets.push(u64::try_from(end).map_err(|_| StoreFormatError::SizeOverflow)?);
        }
        if expected_offset != blob.len() {
            return Err(StoreFormatError::MalformedStringTable);
        }

        Self::from_blob_parts(blob.to_vec(), offsets)
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
