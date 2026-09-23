use std::fmt;

#[derive(Debug)]
pub enum StorageError {
    Binary(String),
    Json(serde_json::Error),
    Facts(crate::facts::ValidationError),
    InvalidObject(&'static str),
    UnsupportedObjectVersion(u64),
    UnsupportedSnapshotVersion(u64),
}

impl fmt::Display for StorageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Binary(message) => write!(formatter, "invalid Lexicon binary object: {message}"),
            Self::Json(error) => write!(formatter, "invalid Lexicon JSON: {error}"),
            Self::Facts(error) => write!(formatter, "invalid Lexicon fact record: {error}"),
            Self::InvalidObject(field) => write!(formatter, "invalid Lexicon object field {field}"),
            Self::UnsupportedObjectVersion(version) => {
                write!(formatter, "unsupported Lexicon object version {version}")
            }
            Self::UnsupportedSnapshotVersion(version) => {
                write!(formatter, "unsupported Lexicon snapshot version {version}")
            }
        }
    }
}

impl std::error::Error for StorageError {}

impl From<serde_json::Error> for StorageError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

impl From<crate::facts::ValidationError> for StorageError {
    fn from(value: crate::facts::ValidationError) -> Self {
        Self::Facts(value)
    }
}
