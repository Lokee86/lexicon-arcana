use crate::SourceSpan;
use serde_json::Map;

#[derive(Debug)]
pub struct RepositoryModel {
    pub repository: String,
    pub files: Vec<SourceFile>,
}

#[derive(Debug)]
pub struct SourceFile {
    pub path: String,
    pub language: String,
    pub parser_language: String,
    pub content: Vec<u8>,
    pub parse_error: bool,
    pub declarations: Vec<Declaration>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Declaration {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub qualified_name: String,
    pub path: String,
    pub container_id: String,
    pub container_qualified: String,
    pub parent_type_id: String,
    pub signature: String,
    pub file_language: String,
    pub span: SourceSpan,
    pub attributes: Map<String, serde_json::Value>,
    pub callable: bool,
    pub definition: bool,
    pub file_local: bool,
    pub callable_shape: Option<CallableShape>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallableShape {
    pub minimum: usize,
    pub maximum: Option<usize>,
    pub variadic: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ExtractionContext {
    pub container_id: String,
    pub container_qualified: String,
    pub type_id: String,
    pub type_name: String,
    pub callable_id: String,
    pub callable_scope: String,
    pub template: bool,
    pub conditional: bool,
}
