use super::model::{Declaration, SourceFile};
use serde_json::{Value, json};
use std::collections::BTreeSet;

pub fn propagate(files: &mut [SourceFile]) {
    let mut aliases = BTreeSet::<String>::new();
    for declaration in files.iter().flat_map(|file| &file.declarations) {
        if declaration.kind == "type" && is_function_pointer(declaration) {
            aliases.insert(declaration.name.clone());
        }
    }

    loop {
        let mut changed = false;
        for declaration in files
            .iter_mut()
            .flat_map(|file| file.declarations.iter_mut())
        {
            if is_function_pointer(declaration) {
                continue;
            }
            let type_text = declaration_type_text(declaration);
            if !contains_any_identifier(type_text, &aliases) {
                continue;
            }
            declaration
                .attributes
                .insert("function_pointer".into(), json!(true));
            changed = true;
            if declaration.kind == "type" {
                aliases.insert(declaration.name.clone());
            }
        }
        if !changed {
            break;
        }
    }
}

pub fn is_function_pointer(declaration: &Declaration) -> bool {
    declaration
        .attributes
        .get("function_pointer")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn declaration_type_text(declaration: &Declaration) -> &str {
    declaration
        .attributes
        .get("type")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .or_else(|| declaration.attributes.get("target").and_then(Value::as_str))
        .unwrap_or_default()
}

fn contains_any_identifier(value: &str, identifiers: &BTreeSet<String>) -> bool {
    value
        .split(|character: char| !(character.is_alphanumeric() || character == '_'))
        .filter(|part| !part.is_empty())
        .any(|part| identifiers.contains(part))
}

#[cfg(test)]
mod tests {
    use super::contains_any_identifier;
    use std::collections::BTreeSet;

    #[test]
    fn alias_matching_uses_identifier_boundaries() {
        let values = BTreeSet::from(["callback_fn".to_owned()]);
        assert!(contains_any_identifier("const callback_fn *", &values));
        assert!(!contains_any_identifier("callback_fn_extra *", &values));
    }
}
