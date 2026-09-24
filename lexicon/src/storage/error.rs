use std::{fmt, io};

#[derive(Debug)]
pub enum StorageError {
    Binary(String),
    Json(serde_json::Error),
    Facts(crate::facts::ValidationError),
    Io(io::Error),
    InvalidId(String),
    InvalidObject(&'static str),
    Materialization(String),
    Operation(String),
    Verification(String),
    Collision(String),
    Busy,
    NoCurrentSnapshot,
    NoPendingPublication,
    UnsupportedObjectVersion(u64),
    UnsupportedSnapshotVersion(u64),
    UnsupportedPendingVersion(u64),
}

impl fmt::Display for StorageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Binary(message) => write!(formatter, "invalid Lexicon binary object: {message}"),
            Self::Json(error) => write!(formatter, "invalid Lexicon JSON: {error}"),
            Self::Facts(error) => write!(formatter, "invalid Lexicon fact record: {error}"),
            Self::Io(error) => write!(formatter, "Lexicon storage I/O error: {error}"),
            Self::InvalidId(id) => write!(formatter, "invalid Lexicon content ID {id:?}"),
            Self::InvalidObject(field) => write!(formatter, "invalid Lexicon object field {field}"),
            Self::Materialization(message) => {
                write!(formatter, "Lexicon materialization failed: {message}")
            }
            Self::Operation(message) => write!(formatter, "{message}"),
            Self::Verification(id) => write!(formatter, "Lexicon content {id} failed verification"),
            Self::Collision(path) => {
                write!(formatter, "content-addressed object collision at {path}")
            }
            Self::Busy => write!(formatter, "Lexicon repository is already being updated"),
            Self::NoCurrentSnapshot => write!(formatter, "Lexicon has no current snapshot"),
            Self::NoPendingPublication => write!(formatter, "Lexicon has no pending publication"),
            Self::UnsupportedObjectVersion(version) => {
                write!(formatter, "unsupported Lexicon object version {version}")
            }
            Self::UnsupportedSnapshotVersion(version) => {
                write!(formatter, "unsupported Lexicon snapshot version {version}")
            }
            Self::UnsupportedPendingVersion(version) => {
                write!(
                    formatter,
                    "unsupported pending Lexicon publication version {version}"
                )
            }
        }
    }
}

impl std::error::Error for StorageError {}

impl From<io::Error> for StorageError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

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
