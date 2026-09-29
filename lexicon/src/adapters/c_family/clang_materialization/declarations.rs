use std::collections::HashMap;

use serde_json::{Map, json};

use crate::node_id;

use super::super::{
    clang_protocol::{DeclarationObservation, FileObservation, MacroObservation},
    macro_syntax,
    model::Declaration,
};

pub(super) fn identity_map(file: &FileObservation) -> HashMap<String, String> {
    let mut result = HashMap::new();
    for value in &file.declarations {
        result.insert(value.compiler_id.clone(), declaration_id(&file.path, value));
    }
    for value in &file.macros {
        result.insert(value.compiler_id.clone(), macro_id(&file.path, value));
    }
    result
}

pub(super) fn materialize(
    file: &FileObservation,
    language: &str,
    module_id: &str,
    ids: &HashMap<String, String>,
) -> Vec<Declaration> {
    let qualified = file
        .declarations
        .iter()
        .map(|value| (value.compiler_id.clone(), value.qualified_name.clone()))
        .collect::<HashMap<_, _>>();
    let mut result = file
        .declarations
        .iter()
        .map(|value| declaration(&file.path, language, module_id, value, ids, &qualified))
        .collect::<Vec<_>>();
    result.extend(
        file.macros
            .iter()
            .map(|value| macro_declaration(&file.path, language, module_id, value)),
    );
    result.sort_by(|left, right| {
        (
            &left.path,
            left.span.start_line,
            left.span.start_column,
            &left.kind,
            &left.qualified_name,
            &left.signature,
        )
            .cmp(&(
                &right.path,
                right.span.start_line,
                right.span.start_column,
                &right.kind,
                &right.qualified_name,
                &right.signature,
            ))
    });
    result
}

fn declaration(
    path: &str,
    language: &str,
    module_id: &str,
    value: &DeclarationObservation,
    ids: &HashMap<String, String>,
    qualified: &HashMap<String, String>,
) -> Declaration {
    let mut attributes = Map::new();
    attributes.insert("language".into(), json!(language));
    if !value.type_name.is_empty() {
        attributes.insert("type".into(), json!(value.type_name));
    }
    if !value.tag.is_empty() {
        attributes.insert("tag".into(), json!(value.tag));
    }
    if value.enum_member {
        attributes.insert("enum_member".into(), json!(true));
    }
    if value.callable {
        attributes.insert("definition".into(), json!(value.definition));
    }
    if value.internal {
        attributes.insert("linkage".into(), json!("internal"));
    }
    if value.template {
        attributes.insert("template".into(), json!(true));
    }
    if value.virtual_member {
        attributes.insert("virtual".into(), json!(true));
    }
    if value.function_pointer {
        attributes.insert("function_pointer".into(), json!(true));
    }
    if value.alias {
        attributes.insert("alias".into(), json!(true));
        if !value.alias_target.is_empty() {
            attributes.insert("target".into(), json!(value.alias_target));
        }
    }
    if let Some(index) = value.parameter_index {
        attributes.insert("index".into(), json!(index));
    }
    if value.callable
        && let Some(count) = value.parameter_count
    {
        attributes.insert("parameter_count".into(), json!(count));
    }

    Declaration {
        id: declaration_id(path, value),
        kind: value.kind.clone(),
        name: value.name.clone(),
        qualified_name: value.qualified_name.clone(),
        path: path.into(),
        container_id: ids
            .get(&value.container_compiler_id)
            .cloned()
            .unwrap_or_else(|| module_id.into()),
        container_qualified: qualified
            .get(&value.container_compiler_id)
            .cloned()
            .unwrap_or_default(),
        parent_type_id: ids
            .get(&value.parent_type_compiler_id)
            .cloned()
            .unwrap_or_default(),
        signature: value.signature.clone(),
        file_language: language.into(),
        span: value.span.clone(),
        attributes,
        callable: value.callable,
        definition: value.definition,
        file_local: value.internal,
        macro_function: false,
        macro_target: String::new(),
        macro_parameters: Vec::new(),
        macro_calls: Vec::new(),
        callable_shape: None,
    }
}

fn macro_declaration(
    path: &str,
    language: &str,
    module_id: &str,
    value: &MacroObservation,
) -> Declaration {
    let target = macro_syntax::direct_target(&value.replacement);
    let mut attributes = Map::from_iter([
        ("language".into(), json!(language)),
        ("macro".into(), json!(true)),
        ("function_like".into(), json!(value.function_like)),
    ]);
    if value.conditional {
        attributes.insert("conditional".into(), json!(true));
    }
    if !value.replacement.is_empty() {
        attributes.insert("replacement".into(), json!(value.replacement));
    }
    if !target.is_empty() {
        attributes.insert("target".into(), json!(target));
    }
    Declaration {
        id: macro_id(path, value),
        kind: "symbol".into(),
        name: value.name.clone(),
        qualified_name: value.name.clone(),
        path: path.into(),
        container_id: module_id.into(),
        container_qualified: String::new(),
        parent_type_id: String::new(),
        signature: String::new(),
        file_language: language.into(),
        span: value.span.clone(),
        attributes,
        callable: false,
        definition: true,
        file_local: false,
        macro_function: value.function_like,
        macro_target: target,
        macro_parameters: value.parameters.clone(),
        macro_calls: Vec::new(),
        callable_shape: None,
    }
}

fn declaration_id(path: &str, value: &DeclarationObservation) -> String {
    let mut canonical = format!("{}::{}::{}", path, value.kind, value.qualified_name);
    if !value.signature.is_empty() {
        canonical.push_str("::");
        canonical.push_str(&value.signature);
    }
    node_id("c-family", &value.kind, &canonical)
}

fn macro_id(path: &str, value: &MacroObservation) -> String {
    node_id(
        "c-family",
        "symbol",
        &format!("{path}::symbol::{}", value.name),
    )
}
