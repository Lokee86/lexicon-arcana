use super::format::{ABSENT_STRING_ID, STRING_INDEX_RECORD_LEN};
use super::record_io::{get_u32, get_u64};
use super::{RepositoryStoreReadError, StoreFormatError, StringId};

#[derive(Clone, Copy)]
pub struct StringTableView<'a> {
    bytes: &'a [u8],
    count: usize,
    index_len: usize,
}

impl<'a> StringTableView<'a> {
    pub fn new(bytes: &'a [u8], count: u64) -> Result<Self, RepositoryStoreReadError> {
        let count = usize::try_from(count).map_err(|_| StoreFormatError::TooManyStrings)?;
        if count > ABSENT_STRING_ID as usize {
            return Err(StoreFormatError::TooManyStrings.into());
        }
        let index_len = count
            .checked_mul(STRING_INDEX_RECORD_LEN as usize)
            .ok_or(StoreFormatError::SizeOverflow)?;
        if index_len > bytes.len() {
            return Err(StoreFormatError::MalformedStringTable.into());
        }
        Ok(Self {
            bytes,
            count,
            index_len,
        })
    }

    pub const fn len(self) -> usize {
        self.count
    }

    pub const fn is_empty(self) -> bool {
        self.count == 0
    }

    pub fn get(self, id: StringId) -> Result<&'a str, RepositoryStoreReadError> {
        if id == StringId::ABSENT {
            return Err(StoreFormatError::AbsentString.into());
        }
        let index = id.0 as usize;
        if index >= self.count {
            return Err(StoreFormatError::InvalidStringId(id.0).into());
        }
        let base = index * STRING_INDEX_RECORD_LEN as usize;
        let offset = usize::try_from(get_u64(self.bytes, base))
            .map_err(|_| StoreFormatError::MalformedStringTable)?;
        if get_u32(self.bytes, base + 12) != 0 {
            return Err(StoreFormatError::MalformedStringTable.into());
        }
        let len = get_u32(self.bytes, base + 8) as usize;
        let start = self
            .index_len
            .checked_add(offset)
            .ok_or(StoreFormatError::SizeOverflow)?;
        let end = start
            .checked_add(len)
            .filter(|end| *end <= self.bytes.len())
            .ok_or(StoreFormatError::MalformedStringTable)?;
        std::str::from_utf8(&self.bytes[start..end])
            .map_err(|_| StoreFormatError::InvalidUtf8.into())
    }

    pub fn optional(self, id: StringId) -> Result<Option<&'a str>, RepositoryStoreReadError> {
        id.present().map(|id| self.get(id)).transpose()
    }
}
