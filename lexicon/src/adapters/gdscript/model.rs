use std::collections::BTreeMap;

use crate::SourceSpan;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Identifier,
    String,
    Number,
    Symbol,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub text: String,
    pub line: u64,
    pub column: u64,
    pub end_line: u64,
    pub end_column: u64,
}

#[derive(Debug, Clone)]
pub struct Statement {
    pub tokens: Vec<Token>,
    pub indent: usize,
    pub start: Token,
    pub end: Token,
}

#[derive(Debug, Clone)]
pub struct Declaration {
    pub keyword: String,
    pub kind: String,
    pub name: String,
    pub name_index: usize,
    pub indent: usize,
    pub span: SourceSpan,
    pub extends: String,
    pub attributes: BTreeMap<String, serde_json::Value>,
    pub parameters: Vec<String>,
    pub parameter_names: Vec<String>,
    pub parameter_types: BTreeMap<String, String>,
    pub parameter_defaults: BTreeMap<String, Vec<Token>>,
    pub return_type: String,
    pub type_name: String,
    pub initializer: Vec<Token>,
    pub preload_path: String,
    pub is_static: bool,
    pub is_async: bool,
    pub node_id: String,
    pub key: String,
    pub owner_id: String,
    pub owner_class_id: String,
    pub owner_function: String,
}

impl Declaration {
    pub fn new(keyword: String, kind: String, indent: usize, span: SourceSpan) -> Self {
        Self {
            keyword,
            kind,
            name: String::new(),
            name_index: 0,
            indent,
            span,
            extends: String::new(),
            attributes: BTreeMap::new(),
            parameters: Vec::new(),
            parameter_names: Vec::new(),
            parameter_types: BTreeMap::new(),
            parameter_defaults: BTreeMap::new(),
            return_type: String::new(),
            type_name: String::new(),
            initializer: Vec::new(),
            preload_path: String::new(),
            is_static: false,
            is_async: false,
            node_id: String::new(),
            key: String::new(),
            owner_id: String::new(),
            owner_class_id: String::new(),
            owner_function: String::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParsedFile {
    pub path: String,
    pub project_root: String,
    pub content: Vec<u8>,
    pub statements: Vec<Statement>,
    pub declarations: Vec<Declaration>,
    pub module_id: String,
    pub class_id: String,
    pub script_owner_id: String,
}

#[derive(Debug, Clone)]
pub struct Scope {
    pub indent: usize,
    pub id: String,
    pub key: String,
    pub class_id: String,
    pub function: String,
}

#[derive(Debug, Clone)]
pub struct ImportReference {
    pub loader: String,
    pub expression: String,
    pub path: String,
    pub is_static: bool,
    pub span: SourceSpan,
}

#[derive(Debug, Clone)]
pub struct CallReference {
    pub callee: String,
    pub name: String,
    pub receiver: Vec<Token>,
    pub args: Vec<Vec<Token>>,
    pub expression: String,
    pub span: SourceSpan,
}

#[derive(Debug, Clone)]
pub struct AnalysisContext {
    pub file_index: usize,
    pub function_id: String,
    pub owner_id: String,
}
