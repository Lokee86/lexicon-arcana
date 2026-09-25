use crate::SourceSpan;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Identifier,
    Number,
    String,
    Symbol,
    Newline,
    Unknown,
    Eof,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub text: String,
    pub start_offset: usize,
    pub end_offset: usize,
    pub start_line: u64,
    pub start_column: u64,
    pub end_line: u64,
    pub end_column: u64,
}

#[derive(Debug, Clone)]
pub struct SyntaxDiagnostic {
    pub message: String,
    pub token: Token,
}

#[derive(Debug, Clone, Default)]
pub struct TokenRange {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone)]
pub struct ImportDecl {
    pub alias: String,
    pub path: String,
    pub span: SourceSpan,
    pub wildcard: bool,
}

#[derive(Debug, Clone)]
pub struct ParameterDecl {
    pub annotations: Vec<String>,
    pub has_default: bool,
    pub modifiers: Vec<String>,
    pub mutable: bool,
    pub name: String,
    pub property: bool,
    pub span: SourceSpan,
    pub type_name: String,
}

#[derive(Debug, Clone)]
pub struct SupertypeDecl {
    pub delegate_expression: String,
    pub delegated: bool,
    pub expression: String,
    pub span: SourceSpan,
    pub target_name: String,
}

#[derive(Debug, Clone)]
pub struct Declaration {
    pub annotations: Vec<String>,
    pub body: TokenRange,
    pub children: Vec<Declaration>,
    pub delegation: TokenRange,
    pub delegated: bool,
    pub form: String,
    pub kind: String,
    pub modifiers: Vec<String>,
    pub mutable: bool,
    pub name: String,
    pub parameters: Vec<ParameterDecl>,
    pub primary: bool,
    pub receiver: String,
    pub return_type: String,
    pub span: SourceSpan,
    pub supertypes: Vec<SupertypeDecl>,
    pub type_name: String,
}

#[derive(Debug, Clone)]
pub struct ParsedFile {
    pub content: Vec<u8>,
    pub declarations: Vec<Declaration>,
    pub diagnostics: Vec<SyntaxDiagnostic>,
    pub imports: Vec<ImportDecl>,
    pub package_name: String,
    pub package_span: Option<SourceSpan>,
    pub path: String,
    pub tokens: Vec<Token>,
}

#[derive(Debug, Clone)]
pub struct SourceFile {
    pub content: Vec<u8>,
    pub path: String,
}

#[derive(Debug, Clone)]
pub struct ManifestFile {
    pub content: Vec<u8>,
    pub format: String,
    pub path: String,
}

#[derive(Debug, Clone)]
pub struct DependencyEvidence {
    pub artifact: String,
    pub configuration: String,
    pub coordinate: String,
    pub expression: String,
    pub group: String,
    pub optional: bool,
    pub resolved: bool,
    pub scope: String,
    pub span: SourceSpan,
    pub version: String,
}
