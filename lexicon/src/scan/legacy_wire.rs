use serde::Deserialize;
use serde_json::Value;

use crate::SourceSpan;

#[derive(Deserialize, Default)]
pub(super) struct LegacyHeader {
    #[serde(default)]
    pub(super) adapter_version: String,
    pub(super) changed_files: Option<Vec<String>>,
    #[serde(default)]
    pub(super) language: String,
    #[serde(default)]
    pub(super) mode: String,
    #[serde(default)]
    pub(super) record: String,
    pub(super) removed_files: Option<Vec<String>>,
    #[serde(default)]
    pub(super) repository: String,
    #[serde(default)]
    pub(super) schema_version: u32,
    pub(super) shared_complete: Option<bool>,
}

#[derive(Deserialize, Default)]
pub(super) struct LegacyRecord {
    pub(super) attributes: Option<Value>,
    #[serde(default)]
    pub(super) candidate_name: String,
    #[serde(default)]
    pub(super) candidate_namespace: String,
    #[serde(default)]
    pub(super) content_id: String,
    #[serde(default)]
    pub(super) expression: String,
    #[serde(default)]
    pub(super) id: String,
    #[serde(default)]
    pub(super) kind: String,
    #[serde(default)]
    pub(super) name: String,
    #[serde(default)]
    pub(super) owner: String,
    #[serde(default)]
    pub(super) path: String,
    #[serde(default)]
    pub(super) qualified_name: String,
    #[serde(default)]
    pub(super) reason: String,
    #[serde(default)]
    pub(super) record: String,
    #[serde(default)]
    pub(super) relation: String,
    #[serde(default)]
    pub(super) source: String,
    pub(super) span: Option<LegacySpan>,
    #[serde(default)]
    pub(super) target: String,
}

#[derive(Deserialize, Default)]
pub(super) struct LegacySpan {
    #[serde(default)]
    end_column: u64,
    #[serde(default)]
    end_line: u64,
    #[serde(default)]
    path: String,
    #[serde(default)]
    start_column: u64,
    #[serde(default)]
    start_line: u64,
}

impl From<LegacySpan> for SourceSpan {
    fn from(value: LegacySpan) -> Self {
        Self {
            end_column: value.end_column,
            end_line: value.end_line,
            path: value.path,
            start_column: value.start_column,
            start_line: value.start_line,
        }
    }
}
