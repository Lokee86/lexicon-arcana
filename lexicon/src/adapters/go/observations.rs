use std::collections::BTreeMap;

use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "observation", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum Observation {
    Declaration {
        semantic_key: String,
        kind: DeclarationKind,
        name: String,
        owner: String,
        span: Span,
        #[serde(default)]
        metadata: BTreeMap<String, String>,
    },
    Relationship {
        source_key: String,
        target_key: String,
        kind: RelationshipKind,
        owner: String,
        #[serde(default)]
        span: Option<Span>,
    },
    Symbol {
        semantic_key: String,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        namespace: Option<String>,
        #[serde(default)]
        container_key: Option<String>,
        #[serde(default)]
        owner: Option<String>,
        #[serde(default)]
        span: Option<Span>,
        #[serde(default)]
        generated: bool,
    },
    Callsite {
        source_key: String,
        form: CallForm,
        resolution: CallResolution,
        #[serde(default)]
        expression: Option<String>,
        #[serde(default)]
        candidate_namespace: Option<String>,
        #[serde(default)]
        candidate_name: Option<String>,
        #[serde(default)]
        targets: Vec<SymbolReference>,
        owner: String,
        span: Span,
    },
    Dataflow {
        source_key: String,
        target_key: String,
        access: DataflowAccess,
        owner: String,
        span: Span,
    },
    Capture {
        source_key: String,
        #[serde(default)]
        target_key: Option<String>,
        target_name: String,
        capture_index: usize,
        owner: String,
        #[serde(default)]
        span: Option<Span>,
    },
    Diagnostic {
        severity: DiagnosticSeverity,
        code: String,
        message: String,
        #[serde(default)]
        owner: Option<String>,
        #[serde(default)]
        span: Option<Span>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SymbolReference {
    pub semantic_key: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub namespace: Option<String>,
    #[serde(default)]
    pub container_key: Option<String>,
    #[serde(default)]
    pub owner: Option<String>,
    #[serde(default)]
    pub span: Option<Span>,
    #[serde(default)]
    pub generated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum DeclarationKind {
    Package,
    Import,
    Namespace,
    Type,
    Function,
    Method,
    Test,
    Parameter,
    Variable,
    Field,
    Constant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum RelationshipKind {
    Implements,
    Extends,
    Overrides,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum CallForm {
    Direct,
    Interface,
    Dynamic,
    Builtin,
    Conversion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum CallResolution {
    Resolved,
    Missing,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum DataflowAccess {
    Read,
    Write,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Span {
    pub start_line: u64,
    pub start_column: u64,
    pub end_line: u64,
    pub end_column: u64,
}
