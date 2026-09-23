use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FactHeader {
    pub adapter_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub changed_files: Option<Vec<String>>,
    pub language: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    pub record: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub removed_files: Option<Vec<String>>,
    pub repository: String,
    pub schema_version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shared_complete: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceSpan {
    pub end_column: u64,
    pub end_line: u64,
    pub path: String,
    pub start_column: u64,
    pub start_line: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeRecord {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attributes: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_id: Option<String>,
    pub id: String,
    pub kind: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    pub path: String,
    pub qualified_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<SourceSpan>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EdgeRecord {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attributes: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    pub relation: String,
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<SourceSpan>,
    pub target: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnresolvedRecord {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attributes: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_namespace: Option<String>,
    pub expression: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    pub reason: String,
    pub relation: String,
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<SourceSpan>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FactRecord {
    Node(NodeRecord),
    Edge(EdgeRecord),
    Unresolved(UnresolvedRecord),
}

impl FactRecord {
    pub const fn record_name(&self) -> &'static str {
        match self {
            Self::Node(_) => "node",
            Self::Edge(_) => "edge",
            Self::Unresolved(_) => "unresolved",
        }
    }

    pub(crate) fn owner(&self) -> Option<&str> {
        match self {
            Self::Node(value) => value.owner.as_deref(),
            Self::Edge(value) => value.owner.as_deref(),
            Self::Unresolved(value) => value.owner.as_deref(),
        }
    }

    pub(crate) fn span(&self) -> Option<&SourceSpan> {
        match self {
            Self::Node(value) => value.span.as_ref(),
            Self::Edge(value) => value.span.as_ref(),
            Self::Unresolved(value) => value.span.as_ref(),
        }
    }
}
