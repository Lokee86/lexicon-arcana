use std::fmt;

#[derive(Debug)]
pub struct RepositoryError(pub(crate) String);

impl RepositoryError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for RepositoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for RepositoryError {}

impl From<std::io::Error> for RepositoryError {
    fn from(value: std::io::Error) -> Self {
        Self(value.to_string())
    }
}
