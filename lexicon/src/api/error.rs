use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexiconError(String);

impl LexiconError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for LexiconError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for LexiconError {}

impl From<crate::ScanExecutionError> for LexiconError {
    fn from(value: crate::ScanExecutionError) -> Self {
        Self(value.to_string())
    }
}

impl From<crate::RepositoryError> for LexiconError {
    fn from(value: crate::RepositoryError) -> Self {
        Self(value.to_string())
    }
}

impl From<crate::StorageError> for LexiconError {
    fn from(value: crate::StorageError) -> Self {
        Self(value.to_string())
    }
}

impl From<std::io::Error> for LexiconError {
    fn from(value: std::io::Error) -> Self {
        Self(value.to_string())
    }
}
