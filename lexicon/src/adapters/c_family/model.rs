use super::visibility::VisibilityIndex;
use crate::SourceSpan;
use serde_json::Map;

#[derive(Debug)]
pub struct RepositoryModel {
    pub repository: String,
    pub files: Vec<SourceFile>,
    pub visibility: VisibilityIndex,
}

#[derive(Debug)]
pub struct SourceFile {
    pub path: String,
    pub language: String,
    pub parser: String,
    pub parser_language: String,
    pub content: Vec<u8>,
    pub parse_error: bool,
    pub declarations: Vec<Declaration>,
    pub includes: Vec<IncludeObservation>,
    pub semantic_relationships: Vec<SemanticRelationshipObservation>,
    pub semantic_calls: Vec<SemanticCallObservation>,
    pub semantic_pointer_bindings: Vec<SemanticPointerBindingObservation>,
    pub semantic_accesses: Vec<SemanticAccessObservation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticRelationshipKind {
    Extends,
    Overrides,
}

#[derive(Debug, Clone)]
pub struct SemanticRelationshipObservation {
    pub source_id: String,
    pub target_id: String,
    pub target_name: String,
    pub external: bool,
    pub path: String,
    pub expression: String,
    pub kind: SemanticRelationshipKind,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticCallForm {
    Direct,
    Member,
    Constructor,
    Operator,
    Destructor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticCallResolution {
    Resolved,
    Ambiguous,
    Missing,
    Dependent,
    Indirect,
}

#[derive(Debug, Clone)]
pub struct SemanticArgumentObservation {
    pub expression: String,
    pub value_id: String,
    pub callable_id: String,
}

#[derive(Debug, Clone)]
pub struct SemanticPointerBindingObservation {
    pub pointer_id: String,
    pub target_id: String,
}

#[derive(Debug, Clone)]
pub struct SemanticAccessObservation {
    pub source_id: String,
    pub target_id: String,
    pub path: String,
    pub relation: String,
    pub span: SourceSpan,
}

#[derive(Debug, Clone)]
pub struct SemanticCallObservation {
    pub source_id: String,
    pub path: String,
    pub expression: String,
    pub form: SemanticCallForm,
    pub resolution: SemanticCallResolution,
    pub dispatch: String,
    pub overload_selected: bool,
    pub macro_expanded: bool,
    pub target_id: String,
    pub target_name: String,
    pub candidate_ids: Vec<String>,
    pub compiler_candidate_count: usize,
    pub external_candidate_count: usize,
    pub receiver_type_id: String,
    pub receiver_type: String,
    pub callee_value_id: String,
    pub arguments: Vec<SemanticArgumentObservation>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone)]
pub struct IncludeObservation {
    pub id: String,
    pub module_id: String,
    pub path: String,
    pub target: String,
    pub resolved_path: String,
    pub expression: String,
    pub system: bool,
    pub span: SourceSpan,
}

#[derive(Debug, Clone)]
pub struct Declaration {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub qualified_name: String,
    pub path: String,
    pub container_id: String,
    pub span: SourceSpan,
    pub attributes: Map<String, serde_json::Value>,
    pub file_local: bool,
}
