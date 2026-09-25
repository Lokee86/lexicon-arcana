use std::collections::BTreeMap;

use crate::SourceSpan;

#[derive(Debug, Clone)]
pub struct SourceFile {
    pub path: String,
    pub raw_content_id: String,
    pub content: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct Repository {
    pub name: String,
    pub directories: Vec<String>,
    pub sources: Vec<SourceFile>,
}

#[derive(Debug, Clone)]
pub struct LogicalLine {
    pub span: SourceSpan,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct Declaration {
    pub class_id: Option<String>,
    pub id: String,
    pub owner_path: String,
    pub public: bool,
    pub qualified_name: String,
    pub span: SourceSpan,
    pub type_members_public: bool,
}

#[derive(Debug, Clone)]
pub struct UseEvidence {
    pub dynamic: bool,
    pub expression: String,
    pub import_id: String,
    pub keyword: String,
    pub owner_path: String,
    pub span: SourceSpan,
    pub target: String,
}

#[derive(Debug, Clone)]
pub struct CallEvidence {
    pub candidate: String,
    pub class_id: Option<String>,
    pub expression: String,
    pub owner_id: String,
    pub owner_path: String,
    pub span: SourceSpan,
}

#[derive(Debug, Clone)]
pub struct ExtendsEvidence {
    pub base: String,
    pub class_id: String,
    pub owner_path: String,
    pub span: SourceSpan,
}

#[derive(Debug, Clone)]
pub struct AccessEvidence {
    pub class_id: Option<String>,
    pub owner_id: String,
    pub owner_path: String,
    pub span: SourceSpan,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct VariableSymbol {
    pub data_type: String,
    pub id: String,
    pub public: bool,
}

#[derive(Default)]
pub struct AnalysisState {
    pub accesses: Vec<AccessEvidence>,
    pub bases_by_class: BTreeMap<String, Vec<String>>,
    pub callables_by_name: BTreeMap<String, Vec<Declaration>>,
    pub classes_by_name: BTreeMap<String, Vec<Declaration>>,
    pub extends: Vec<ExtendsEvidence>,
    pub field_symbols: BTreeMap<String, BTreeMap<String, VariableSymbol>>,
    pub imports_by_path: BTreeMap<String, Vec<String>>,
    pub methods_by_class: BTreeMap<String, BTreeMap<String, Vec<Declaration>>>,
    pub module_id_by_path: BTreeMap<String, String>,
    pub module_paths_by_name: BTreeMap<String, Vec<String>>,
    pub module_public: BTreeMap<String, bool>,
    pub modules_by_name: BTreeMap<String, Vec<String>>,
    pub module_symbols: BTreeMap<String, BTreeMap<String, VariableSymbol>>,
    pub variable_symbols: BTreeMap<String, BTreeMap<String, VariableSymbol>>,
    pub uses: Vec<UseEvidence>,
    pub calls: Vec<CallEvidence>,
}
