use crate::adapters::rust::model::{Context, ValueSet};

pub(crate) fn common_method_return(receiver: &ValueSet, name: &str) -> ValueSet {
    if matches!(
        name,
        "unwrap"
            | "expect"
            | "unwrap_or"
            | "unwrap_or_default"
            | "unwrap_or_else"
            | "lock"
            | "into_inner"
    ) {
        let mut result = receiver
            .contained_values
            .first()
            .cloned()
            .unwrap_or_else(|| receiver.clone());
        if receiver.contained_values.is_empty() {
            result
                .types
                .extend(receiver.contained_types.iter().cloned());
        }
        result.unknown = result.types.is_empty()
            && result.traits.is_empty()
            && result.callables.is_empty()
            && !result.builtin
            && !result.external;
        return result;
    }
    if matches!(
        name,
        "as_ref"
            | "as_mut"
            | "as_deref"
            | "as_deref_mut"
            | "borrow"
            | "borrow_mut"
            | "deref"
            | "deref_mut"
            | "clone"
            | "default"
            | "to_owned"
    ) {
        return receiver.clone();
    }
    if matches!(
        name,
        "checked_add" | "checked_mul" | "checked_sub" | "checked_div" | "checked_rem"
    ) {
        return ValueSet {
            contained_types: receiver
                .types
                .union(&receiver.contained_types)
                .cloned()
                .collect(),
            contained_values: vec![receiver.clone()],
            builtin: true,
            ..ValueSet::default()
        };
    }
    if matches!(
        name,
        "iter"
            | "iter_mut"
            | "into_iter"
            | "take"
            | "skip"
            | "map"
            | "filter"
            | "inspect"
            | "enumerate"
            | "peekable"
            | "rev"
            | "fuse"
            | "cycle"
    ) {
        return ValueSet {
            contained_types: receiver
                .types
                .union(&receiver.contained_types)
                .cloned()
                .collect(),
            contained_values: receiver.contained_values.clone(),
            builtin: true,
            ..ValueSet::default()
        };
    }
    if matches!(
        name,
        "collect"
            | "count"
            | "len"
            | "is_empty"
            | "is_none"
            | "is_some"
            | "is_ok"
            | "is_err"
            | "is_some_and"
            | "is_ok_and"
            | "is_err_and"
            | "contains"
            | "starts_with"
            | "ends_with"
            | "trim"
            | "try_exists"
            | "as_bytes"
            | "as_str"
            | "as_nanos"
            | "to_le_bytes"
    ) {
        return ValueSet {
            builtin: true,
            ..ValueSet::default()
        };
    }
    if matches!(name, "ok_or" | "strip_prefix" | "strip_suffix") {
        let mut result = receiver.clone();
        result.builtin = true;
        if result.contained_values.is_empty() {
            result.contained_values.push(receiver.clone());
        }
        return result;
    }
    ValueSet {
        builtin: receiver.builtin,
        external: receiver.external,
        unknown: !receiver.builtin && !receiver.external,
        ..ValueSet::default()
    }
}

pub(crate) fn builtin_unknown_method(name: &str) -> bool {
    matches!(
        name,
        "as_nanos" | "contains" | "iter" | "starts_with" | "trim" | "try_exists" | "write_all"
    )
}

pub(crate) fn generated_unknown_method(name: &str) -> bool {
    matches!(name, "cmp" | "into" | "then" | "then_with" | "to_string")
}

pub(crate) fn builtin_method(name: &str) -> bool {
    matches!(
        name,
        "as_ref"
            | "as_mut"
            | "borrow"
            | "borrow_mut"
            | "clone"
            | "cmp"
            | "collect"
            | "count"
            | "default"
            | "deref"
            | "deref_mut"
            | "eq"
            | "fmt"
            | "hash"
            | "into"
            | "into_iter"
            | "map"
            | "partial_cmp"
            | "take"
            | "to_owned"
            | "to_string"
            | "try_from"
    )
}

pub(crate) fn node_callable(context: &Context, id: &str) -> bool {
    context
        .facts
        .nodes
        .get(id)
        .is_some_and(|node| matches!(node.kind.as_str(), "function" | "method"))
}

pub(crate) fn builtin_function(text: &str) -> bool {
    matches!(
        text,
        "Some" | "None" | "Ok" | "Err" | "drop" | "size_of" | "align_of"
    )
}

pub(crate) fn builtin_macro(name: &str) -> bool {
    matches!(
        name,
        "assert"
            | "assert_eq"
            | "assert_ne"
            | "cfg"
            | "column"
            | "compile_error"
            | "concat"
            | "dbg"
            | "debug_assert"
            | "debug_assert_eq"
            | "debug_assert_ne"
            | "eprint"
            | "eprintln"
            | "env"
            | "file"
            | "format"
            | "format_args"
            | "include"
            | "include_bytes"
            | "include_str"
            | "line"
            | "matches"
            | "module_path"
            | "option_env"
            | "panic"
            | "print"
            | "println"
            | "stringify"
            | "thread_local"
            | "todo"
            | "try"
            | "unimplemented"
            | "unreachable"
            | "vec"
            | "write"
            | "writeln"
    )
}
