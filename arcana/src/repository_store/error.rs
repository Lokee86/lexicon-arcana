use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StoreFormatError {
    TooManyStrings,
    StringTooLong,
    MissingString,
    AbsentString,
    InvalidStringId(u32),
    InvalidUtf8,
    NonCanonicalStrings,
    MalformedStringTable,
    InvalidExternalIdentity,
    InvalidNodeKind(u16),
    InvalidRelation(u16),
    InvalidUnresolvedReason(u16),
    MissingUnknownReason,
    UnexpectedUnknownReason,
    InvalidFlags(u16),
    InvalidOccurrenceCount,
    SizeOverflow,
}

impl fmt::Display for StoreFormatError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid repository store data: {self:?}")
    }
}

impl std::error::Error for StoreFormatError {}
