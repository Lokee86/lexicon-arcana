use std::path::PathBuf;

use rustpython_parser::ast;

use crate::SourceSpan;

use super::source::LineIndex;

#[derive(Debug)]
pub struct Repository {
    pub root: PathBuf,
    pub name: String,
    pub directories: Vec<String>,
    pub files: Vec<SourceInput>,
}

#[derive(Debug, Clone)]
pub struct SourceInput {
    pub path: PathBuf,
    pub relative: String,
    pub module: String,
    pub size: u64,
}

#[derive(Debug)]
pub struct SourceFile {
    pub path: PathBuf,
    pub relative: String,
    pub module: String,
    pub bytes: Vec<u8>,
    pub source: String,
    pub lines: LineIndex,
    pub suite: Option<ast::Suite>,
    pub parse_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ImportInfo {
    pub module_name: String,
    pub owner_id: String,
    pub expression: String,
    pub binding: Option<String>,
    pub target_module: String,
    pub target_name: Option<String>,
    pub relative_level: u32,
    pub star: bool,
    pub is_package: bool,
    pub span: Option<SourceSpan>,
}

#[derive(Debug, Clone)]
pub struct InheritanceInfo {
    pub source_id: String,
    pub module_name: String,
    pub class_qname: String,
    pub base: ast::Expr,
    pub expression: String,
    pub span: Option<SourceSpan>,
}

#[derive(Debug, Clone)]
pub struct FunctionInfo {
    pub module_name: String,
    pub qname: String,
    pub node_id: String,
    pub class_qname: Option<String>,
    pub arguments: ast::Arguments,
    pub decorators: Vec<ast::Expr>,
    pub return_expressions: Vec<ast::Expr>,
    pub parameters: Vec<(String, Option<ast::Expr>)>,
    pub return_annotation: Option<ast::Expr>,
    pub is_lambda: bool,
    pub is_async: bool,
}

#[derive(Debug, Clone)]
pub struct ClassInfo {
    pub module_name: String,
    pub bases: Vec<ast::Expr>,
}

#[derive(Debug, Clone)]
pub struct CallInfo {
    pub module_name: String,
    pub owner_id: String,
    pub class_qname: Option<String>,
    pub scope_id: String,
    pub expression_node: ast::ExprCall,
    pub callee: ast::Expr,
    pub expression: String,
    pub span: Option<SourceSpan>,
    pub bare_expression: bool,
    pub outcome_eligible: bool,
}

#[derive(Debug, Clone)]
pub struct LocalAssignmentInfo {
    pub module_name: String,
    pub scope_id: String,
    pub class_qname: Option<String>,
    pub name: String,
    pub value: Option<ast::Expr>,
    pub annotation: Option<ast::Expr>,
    pub start: u32,
    pub end: u32,
    pub branch_dependent: bool,
    pub direct_class_field: bool,
}

#[derive(Debug, Clone)]
pub struct LoopBindingInfo {
    pub module_name: String,
    pub scope_id: String,
    pub class_qname: Option<String>,
    pub name: String,
    pub start: u32,
    pub iterable: ast::Expr,
    pub branch_dependent: bool,
    pub element_index: Option<usize>,
}
