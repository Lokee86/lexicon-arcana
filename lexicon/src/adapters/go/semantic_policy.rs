use super::observations::{CallForm, CallResolution, DataflowAccess, RelationshipKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TargetRepresentation {
    Conversion,
    Builtin,
    Interface,
    Dynamic,
    Closure,
    Callable,
}

pub(super) fn inferred_call_form(canonical_identity: &str) -> CallForm {
    if canonical_identity.starts_with("type:") || canonical_identity.starts_with("type-expression:")
    {
        CallForm::Conversion
    } else if canonical_identity.starts_with("function:go:builtins:") {
        CallForm::Builtin
    } else if canonical_identity.starts_with("interface-method:") {
        CallForm::Interface
    } else if canonical_identity.starts_with("dynamic-method:")
        || canonical_identity.starts_with("ssa-function:")
    {
        CallForm::Dynamic
    } else {
        CallForm::Direct
    }
}

pub(super) fn target_representation(
    form: CallForm,
    canonical_identity: &str,
) -> TargetRepresentation {
    if matches!(form, CallForm::Conversion)
        || canonical_identity.starts_with("type:")
        || canonical_identity.starts_with("type-expression:")
    {
        TargetRepresentation::Conversion
    } else if matches!(form, CallForm::Builtin)
        || canonical_identity.starts_with("function:go:builtins:")
    {
        TargetRepresentation::Builtin
    } else if matches!(form, CallForm::Interface)
        || canonical_identity.starts_with("interface-method:")
    {
        TargetRepresentation::Interface
    } else if canonical_identity.starts_with("dynamic-method:")
        || canonical_identity.starts_with("ssa-function:")
    {
        TargetRepresentation::Dynamic
    } else if canonical_identity.starts_with("closure:") {
        TargetRepresentation::Closure
    } else {
        TargetRepresentation::Callable
    }
}

pub(super) fn call_relation(form: CallForm, target_count: usize) -> &'static str {
    if matches!(form, CallForm::Conversion) {
        "converts-to"
    } else if target_count == 1 {
        "calls"
    } else {
        "possible-calls"
    }
}

pub(super) fn unresolved_reason(form: CallForm, resolution: CallResolution) -> &'static str {
    match resolution {
        CallResolution::Ambiguous => "ambiguous-target",
        CallResolution::Unsupported => "unsupported-form",
        CallResolution::Missing => match form {
            CallForm::Interface | CallForm::Dynamic => "dynamic-target",
            CallForm::Builtin => "builtin-target",
            CallForm::Conversion => "type-conversion",
            CallForm::Direct => "missing-target",
        },
        CallResolution::Resolved => unreachable!("resolved callsite has no unresolved reason"),
    }
}

pub(super) fn relationship_relation(kind: RelationshipKind) -> &'static str {
    match kind {
        RelationshipKind::Implements => "implements",
        RelationshipKind::Extends => "extends",
        RelationshipKind::Overrides => "overrides",
    }
}

pub(super) fn dataflow_relation(access: DataflowAccess) -> &'static str {
    match access {
        DataflowAccess::Read => "reads",
        DataflowAccess::Write => "writes",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_owns_call_and_unresolved_policy() {
        assert_eq!(call_relation(CallForm::Direct, 1), "calls");
        assert_eq!(call_relation(CallForm::Dynamic, 2), "possible-calls");
        assert_eq!(call_relation(CallForm::Conversion, 1), "converts-to");
        assert_eq!(
            unresolved_reason(CallForm::Interface, CallResolution::Missing),
            "dynamic-target"
        );
        assert_eq!(
            unresolved_reason(CallForm::Direct, CallResolution::Ambiguous),
            "ambiguous-target"
        );
    }

    #[test]
    fn rust_owns_relationship_and_dataflow_policy() {
        assert_eq!(
            relationship_relation(RelationshipKind::Implements),
            "implements"
        );
        assert_eq!(dataflow_relation(DataflowAccess::Read), "reads");
        assert_eq!(dataflow_relation(DataflowAccess::Write), "writes");
    }
}
