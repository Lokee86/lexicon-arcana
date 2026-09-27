use crate::repository::SourceSpan;

use super::{CompactStringTable, StoreFormatError, StringId, StringIdLookup};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CompactSpan {
    pub path: StringId,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

impl CompactSpan {
    pub(crate) fn from_source(
        span: &SourceSpan,
        strings: &(impl StringIdLookup + ?Sized),
    ) -> Result<Self, StoreFormatError> {
        Ok(Self {
            path: strings.id(&span.path)?,
            start_line: span.start_line,
            start_column: span.start_column,
            end_line: span.end_line,
            end_column: span.end_column,
        })
    }

    pub fn to_source(self, strings: &CompactStringTable) -> Result<SourceSpan, StoreFormatError> {
        Ok(SourceSpan {
            path: strings.get(self.path)?.to_owned(),
            start_line: self.start_line,
            start_column: self.start_column,
            end_line: self.end_line,
            end_column: self.end_column,
        })
    }
}
