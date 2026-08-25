use std::fmt;
use std::io;

/// An error while encoding, parsing, or committing a snapshot manifest.
#[derive(Debug)]
pub enum ManifestError {
    Io(io::Error),
    MissingField(&'static str),
    DuplicateField(String),
    UnknownField(String),
    InvalidFieldOrder {
        expected: &'static str,
        found: String,
    },
    MalformedField {
        field: &'static str,
        value: String,
    },
    UnsupportedVersion {
        found: u64,
    },
    InvalidPath {
        field: &'static str,
        path: String,
    },
    NonUtf8Path {
        field: &'static str,
    },
    MissingFinalNewline,
}

impl fmt::Display for ManifestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => error.fmt(formatter),
            Self::MissingField(field) => write!(formatter, "manifest field '{field}' is missing"),
            Self::DuplicateField(field) => {
                write!(formatter, "manifest field '{field}' is duplicated")
            }
            Self::UnknownField(field) => write!(formatter, "manifest field '{field}' is unknown"),
            Self::InvalidFieldOrder { expected, found } => write!(
                formatter,
                "manifest field '{found}' appears where '{expected}' is required"
            ),
            Self::MalformedField { field, value } => {
                write!(
                    formatter,
                    "manifest field '{field}' has malformed value '{value}'"
                )
            }
            Self::UnsupportedVersion { found } => {
                write!(formatter, "manifest format version {found} is unsupported")
            }
            Self::InvalidPath { field, path } => {
                write!(formatter, "manifest {field} path '{path}' is not relative")
            }
            Self::NonUtf8Path { field } => {
                write!(formatter, "manifest {field} path is not valid UTF-8")
            }
            Self::MissingFinalNewline => {
                formatter.write_str("manifest is missing its final newline")
            }
        }
    }
}

impl std::error::Error for ManifestError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for ManifestError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
