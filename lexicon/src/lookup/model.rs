use std::fmt;

use crate::{EdgeRecord, NodeRecord, StorageError, UnresolvedRecord};

#[derive(Debug, Clone, PartialEq)]
pub struct LookupNode {
    pub language: String,
    pub node: NodeRecord,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LookupDirection {
    Outgoing,
    Incoming,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LookupEdge {
    pub language: String,
    pub direction: LookupDirection,
    pub edge: EdgeRecord,
    pub source: Option<LookupNode>,
    pub target: Option<LookupNode>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LookupUnresolved {
    pub language: String,
    pub record: UnresolvedRecord,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LookupReference {
    Edge(Box<LookupEdge>),
    Unresolved(Box<LookupUnresolved>),
}

#[derive(Debug)]
pub enum LookupError {
    Storage(StorageError),
    NotFound(String),
    RelationshipsNotLoaded,
    Ambiguous {
        selector: String,
        candidates: Vec<String>,
    },
}

impl fmt::Display for LookupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(error) => error.fmt(formatter),
            Self::NotFound(selector) => write!(formatter, "Lexicon node not found: {selector:?}"),
            Self::RelationshipsNotLoaded => {
                write!(
                    formatter,
                    "Lexicon lookup was loaded without relationship evidence"
                )
            }
            Self::Ambiguous {
                selector,
                candidates,
            } => write!(
                formatter,
                "Lexicon node selector {selector:?} is ambiguous: {}",
                candidates.join(", ")
            ),
        }
    }
}

impl std::error::Error for LookupError {}

impl From<StorageError> for LookupError {
    fn from(value: StorageError) -> Self {
        Self::Storage(value)
    }
}
