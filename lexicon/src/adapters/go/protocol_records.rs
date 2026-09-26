use std::collections::BTreeMap;

use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "record", deny_unknown_fields)]
pub(crate) enum Record {
    #[serde(rename = "declaration")]
    Declaration {
        identity: String,
        kind: DeclarationKind,
        name: String,
        owner: String,
        span: Span,
        #[serde(default)]
        metadata: BTreeMap<String, String>,
    },
    #[serde(rename = "relationship")]
    Relationship {
        source: String,
        target: String,
        kind: RelationshipKind,
        owner: String,
        span: Span,
    },
    #[serde(rename = "call")]
    Call {
        source: String,
        target: String,
        kind: CallKind,
        class: CallClass,
        #[serde(default)]
        target_name: Option<String>,
        #[serde(default)]
        target_namespace: Option<String>,
        #[serde(default)]
        target_container: Option<String>,
        owner: String,
        span: Span,
    },
    #[serde(rename = "dataflow")]
    Dataflow {
        source: String,
        target: String,
        kind: DataflowKind,
        owner: String,
        span: Span,
    },
    #[serde(rename = "unresolved")]
    Unresolved {
        source: String,
        relation: String,
        expression: String,
        #[serde(default)]
        candidate_namespace: Option<String>,
        #[serde(default)]
        candidate_name: Option<String>,
        reason: UnresolvedReason,
        class: CallClass,
        owner: String,
        span: Span,
    },
    #[serde(rename = "diagnostic")]
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
    References,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum CallKind {
    Definite,
    Possible,
    Conversion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum CallClass {
    Internal,
    External,
    Builtin,
    Conversion,
    Dynamic,
    Interface,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum DataflowKind {
    Read,
    Write,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum UnresolvedReason {
    MissingTarget,
    AmbiguousTarget,
    UnsupportedForm,
    DynamicTarget,
    ExternalTarget,
    BuiltinTarget,
    TypeConversion,
    SelfTarget,
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
