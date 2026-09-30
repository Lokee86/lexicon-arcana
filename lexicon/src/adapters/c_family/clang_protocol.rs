use serde::{Deserialize, Serialize};

use crate::{SourceSpan, adapters::frontend::ProtocolResponse};

pub(crate) const PROTOCOL_VERSION: u32 = 2;
pub(crate) const HELPER_VERSION: &str = include_str!("../../../adapters/c-family-clang/VERSION");

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CapabilitiesRequest {
    pub protocol_version: u32,
    pub operation: &'static str,
    pub repository_root: String,
}

#[cfg(test)]
impl CapabilitiesRequest {
    pub(crate) fn new(repository_root: String) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            operation: "capabilities",
            repository_root,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StructuralRequest {
    pub protocol_version: u32,
    pub operation: &'static str,
    pub repository_root: String,
    pub owned_files: Vec<String>,
    pub context_files: Vec<String>,
    pub workers: usize,
    pub shards: usize,
    pub merge_fan_in: usize,
}

impl StructuralRequest {
    pub(crate) fn new(
        repository_root: String,
        mut owned_files: Vec<String>,
        mut context_files: Vec<String>,
        workers: usize,
        shards: usize,
        merge_fan_in: usize,
    ) -> Self {
        owned_files.sort();
        owned_files.dedup();
        context_files.sort();
        context_files.dedup();
        Self {
            protocol_version: PROTOCOL_VERSION,
            operation: "structural",
            repository_root,
            owned_files,
            context_files,
            workers: workers.max(1),
            shards: shards.max(1),
            merge_fan_in: merge_fan_in.max(2),
        }
    }
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CapabilitiesResponse {
    pub protocol_version: u32,
    pub helper_version: String,
    pub clang_version: String,
    pub capabilities: Vec<String>,
    pub compilation_database: bool,
    #[serde(default)]
    pub compilation_database_error: Option<String>,
}

#[cfg(test)]
impl ProtocolResponse for CapabilitiesResponse {
    fn protocol_version(&self) -> u32 {
        self.protocol_version
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StructuralMetadataFrame {
    pub protocol_version: u32,
    pub helper_version: String,
    pub clang_version: String,
    pub compilation_database: bool,
    #[serde(default)]
    pub translation_units: Vec<TranslationUnitObservation>,
    #[serde(default)]
    pub context_identities: Vec<ContextIdentityObservation>,
    #[serde(default)]
    pub diagnostics: Vec<DiagnosticObservation>,
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StructuralResponse {
    pub protocol_version: u32,
    pub helper_version: String,
    pub clang_version: String,
    pub compilation_database: bool,
    #[serde(default)]
    pub translation_units: Vec<TranslationUnitObservation>,
    #[serde(default)]
    pub files: Vec<FileObservation>,
    #[serde(default)]
    pub context_identities: Vec<ContextIdentityObservation>,
    #[serde(default)]
    pub diagnostics: Vec<DiagnosticObservation>,
}

#[cfg(test)]
impl ProtocolResponse for StructuralResponse {
    fn protocol_version(&self) -> u32 {
        self.protocol_version
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TranslationUnitObservation {
    pub path: String,
    pub language: String,
    pub directory: String,
    pub arguments: Vec<String>,
    pub synthesized: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ContextIdentityObservation {
    pub compiler_id: String,
    pub path: String,
    pub kind: String,
    pub qualified_name: String,
    #[serde(default)]
    pub signature: String,
    pub definition: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FileObservation {
    pub path: String,
    #[serde(default)]
    pub languages: Vec<String>,
    #[serde(default)]
    pub translation_units: Vec<String>,
    #[serde(default)]
    pub declarations: Vec<DeclarationObservation>,
    #[serde(default)]
    pub includes: Vec<IncludeObservation>,
    #[serde(default)]
    pub macros: Vec<MacroObservation>,
    #[serde(default)]
    pub relationships: Vec<SemanticRelationshipObservation>,
    #[serde(default)]
    pub calls: Vec<SemanticCallObservation>,
    #[serde(default)]
    pub pointer_bindings: Vec<SemanticPointerBindingObservation>,
    #[serde(default)]
    pub accesses: Vec<SemanticAccessObservation>,
    #[serde(default)]
    pub diagnostics: Vec<DiagnosticObservation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DeclarationObservation {
    pub compiler_id: String,
    pub kind: String,
    pub name: String,
    pub qualified_name: String,
    #[serde(default)]
    pub signature: String,
    #[serde(default)]
    pub type_name: String,
    #[serde(default)]
    pub tag: String,
    #[serde(default)]
    pub container_compiler_id: String,
    #[serde(default)]
    pub parent_type_compiler_id: String,
    pub span: SourceSpan,
    pub callable: bool,
    pub definition: bool,
    pub internal: bool,
    pub template: bool,
    pub virtual_member: bool,
    pub function_pointer: bool,
    pub alias: bool,
    pub enum_member: bool,
    #[serde(default)]
    pub alias_target: String,
    #[serde(default)]
    pub parameter_index: Option<usize>,
    #[serde(default)]
    pub parameter_count: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IncludeObservation {
    pub target: String,
    #[serde(default)]
    pub resolved_path: String,
    pub expression: String,
    pub system: bool,
    pub offset: u64,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MacroObservation {
    pub compiler_id: String,
    pub name: String,
    pub replacement: String,
    pub function_like: bool,
    pub conditional: bool,
    #[serde(default)]
    pub parameters: Vec<String>,
    pub offset: u64,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SymbolReferenceObservation {
    pub compiler_id: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub qualified_name: String,
    #[serde(default)]
    pub kind: String,
    pub external: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SemanticRelationshipObservation {
    pub kind: String,
    pub source_compiler_id: String,
    pub target: SymbolReferenceObservation,
    #[serde(default)]
    pub expression: String,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SemanticArgumentObservation {
    pub expression: String,
    #[serde(default)]
    pub value: Option<SymbolReferenceObservation>,
    #[serde(default)]
    pub callable: Option<SymbolReferenceObservation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SemanticPointerBindingObservation {
    pub pointer: SymbolReferenceObservation,
    pub target: SymbolReferenceObservation,
    #[serde(default)]
    pub expression: String,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SemanticAccessObservation {
    pub source_compiler_id: String,
    pub target: SymbolReferenceObservation,
    pub relation: String,
    #[serde(default)]
    pub expression: String,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SemanticCallObservation {
    pub source_compiler_id: String,
    pub form: String,
    pub resolution: String,
    pub expression: String,
    #[serde(default)]
    pub target: Option<SymbolReferenceObservation>,
    #[serde(default)]
    pub candidates: Vec<SymbolReferenceObservation>,
    #[serde(default)]
    pub receiver_type: Option<SymbolReferenceObservation>,
    #[serde(default)]
    pub callee_value: Option<SymbolReferenceObservation>,
    #[serde(default)]
    pub receiver_type_name: String,
    pub virtual_dispatch: bool,
    #[serde(default)]
    pub overload_selected: bool,
    #[serde(default)]
    pub macro_expanded: bool,
    pub compiler_candidate_count: usize,
    #[serde(default)]
    pub arguments: Vec<SemanticArgumentObservation>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DiagnosticObservation {
    pub severity: String,
    pub message: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub span: Option<SourceSpan>,
}
