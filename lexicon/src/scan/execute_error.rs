use std::fmt;

use crate::{AdapterError, RepositoryError, StorageError};

#[derive(Debug)]
pub struct ScanExecutionError(pub(crate) String);

impl ScanExecutionError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for ScanExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ScanExecutionError {}

impl From<std::io::Error> for ScanExecutionError {
    fn from(value: std::io::Error) -> Self {
        Self(value.to_string())
    }
}

impl From<StorageError> for ScanExecutionError {
    fn from(value: StorageError) -> Self {
        Self(value.to_string())
    }
}

impl From<AdapterError> for ScanExecutionError {
    fn from(value: AdapterError) -> Self {
        Self(value.to_string())
    }
}

impl From<RepositoryError> for ScanExecutionError {
    fn from(value: RepositoryError) -> Self {
        Self(value.to_string())
    }
}
