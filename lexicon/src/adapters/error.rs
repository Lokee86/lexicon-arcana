use std::fmt;

#[derive(Debug)]
pub struct AdapterError(pub(crate) String);

impl AdapterError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for AdapterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for AdapterError {}

impl From<std::io::Error> for AdapterError {
    fn from(value: std::io::Error) -> Self {
        Self(value.to_string())
    }
}
