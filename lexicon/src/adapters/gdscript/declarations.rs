use std::collections::BTreeMap;

use serde_json::json;

use super::facts::Facts;
use super::model::{ParsedFile, Scope};

pub fn process(facts: &mut Facts, file: &mut ParsedFile) {
    prepare_script_class(facts, file);
    facts.script_owner_by_path.insert(
        super::facts::normalize_path(&file.path),
        file.script_owner_id.clone(),
    );

    let mut occurrences = BTreeMap::<String, usize>::new();
    let mut scopes: Vec<Scope> = Vec::new();

    for declaration in &mut file.declarations {
        while scopes
            .last()
            .is_some_and(|scope| declaration.indent <= scope.indent)
        {
            scopes.pop();
        }

        let (mut parent_id, mut parent_key) = (file.script_owner_id.clone(), file.path.clone());
        let (mut owner_class, mut owner_function) = (file.script_owner_id.clone(), String::new());
        if let Some(scope) = scopes.last() {
            parent_id = scope.id.clone();
            parent_key = scope.key.clone();
            owner_class = scope.class_id.clone();
            owner_function = scope.function.clone();
        }
        if declaration.keyword == "class_name" {
            parent_id = file.module_id.clone();
            parent_key = file.path.clone();
            owner_class = declaration.node_id.clone();
        }

        declaration.owner_id = parent_id.clone();
        declaration.owner_class_id = owner_class.clone();
        declaration.owner_function = owner_function.clone();
        if declaration.kind == "extends" {
            continue;
        }

        if declaration.kind == "type"
            && declaration.keyword == "class_name"
            && !declaration.node_id.is_empty()
        {
            occurrences
                .entry(format!("{parent_key}::type::{}", declaration.name))
                .or_insert(1);
        }
        if declaration.kind == "type" && declaration.node_id.is_empty() {
            declaration.key = next_key(
                &mut occurrences,
                &format!("{parent_key}::type::{}", declaration.name),
            );
            declaration.node_id = crate::node_id("gdscript", "type", &declaration.key);
            facts
                .class_by_name
                .entry(declaration.name.clone())
                .or_default()
                .push(declaration.node_id.clone());
            facts.index_class(&file.path, &declaration.name, &declaration.node_id);
        }
        if declaration.kind == "type"
            && declaration.keyword == "class_name"
            && declaration.indent == 0
        {
            facts
                .script_owner_candidates_by_path
                .entry(super::facts::normalize_path(&file.path))
                .or_default()
                .push(declaration.node_id.clone());
        }
        if declaration.node_id.is_empty() {
            declaration.key = next_key(
                &mut occurrences,
                &format!("{parent_key}::{}::{}", declaration.kind, declaration.name),
            );
            declaration.node_id = crate::node_id("gdscript", &declaration.kind, &declaration.key);
        }
        if !declaration.preload_path.is_empty() {
            facts.index_preload_alias(&file.path, &declaration.name, &declaration.preload_path);
        }

        let attributes = declaration_attributes(declaration);
        facts.add_node(
            &declaration.kind,
            &declaration.name,
            &file.path,
            &qualified(&file.path, &parent_key, &declaration.name),
            &declaration.key,
            Some(declaration.span.clone()),
            None,
            attributes,
        );
        facts.add_edge(
            &parent_id,
            &declaration.node_id,
            "contains",
            Some(declaration.span.clone()),
            None,
        );
        facts.add_edge(
            &parent_id,
            &declaration.node_id,
            "defines",
            Some(declaration.span.clone()),
            None,
        );
        facts.index_declaration(declaration);

        if declaration.kind == "type" && declaration.keyword != "class_name" {
            scopes.push(Scope {
                indent: declaration.indent,
                id: declaration.node_id.clone(),
                key: declaration.key.clone(),
                class_id: declaration.node_id.clone(),
                function: owner_function,
            });
        } else if declaration.kind == "function" {
            scopes.push(Scope {
                indent: declaration.indent,
                id: declaration.node_id.clone(),
                key: declaration.key.clone(),
                class_id: owner_class,
                function: declaration.node_id.clone(),
            });
        }
    }
}

fn prepare_script_class(facts: &mut Facts, file: &mut ParsedFile) {
    file.script_owner_id = file.module_id.clone();
    for declaration in &mut file.declarations {
        if declaration.kind != "type"
            || declaration.keyword != "class_name"
            || declaration.indent != 0
        {
            continue;
        }
        declaration.key = format!("{}::type::{}", file.path, declaration.name);
        declaration.node_id = crate::node_id("gdscript", "type", &declaration.key);
        file.class_id = declaration.node_id.clone();
        file.script_owner_id = declaration.node_id.clone();
        facts
            .class_by_name
            .entry(declaration.name.clone())
            .or_default()
            .push(declaration.node_id.clone());
        facts.index_class(&file.path, &declaration.name, &declaration.node_id);
        break;
    }
}

fn next_key(occurrences: &mut BTreeMap<String, usize>, base: &str) -> String {
    let occurrence = occurrences.entry(base.into()).or_default();
    *occurrence += 1;
    if *occurrence == 1 {
        base.into()
    } else {
        format!("{base}#{}", *occurrence)
    }
}

fn declaration_attributes(declaration: &super::model::Declaration) -> Option<serde_json::Value> {
    let mut attributes = serde_json::Map::new();
    for (key, value) in &declaration.attributes {
        attributes.insert(key.clone(), value.clone());
    }
    if declaration.kind == "function" {
        attributes.insert("parameters".into(), json!(declaration.parameters));
        if declaration.keyword == "lambda" {
            attributes.insert("anonymous".into(), json!(true));
        }
        if !declaration.return_type.is_empty() {
            attributes.insert("return_type".into(), json!(declaration.return_type));
        }
        if declaration.is_static {
            attributes.insert("static".into(), json!(true));
        }
        if declaration.is_async {
            attributes.insert("async".into(), json!(true));
        }
    }
    if !declaration.extends.is_empty() {
        attributes.insert("extends".into(), json!(declaration.extends));
    }
    (!attributes.is_empty()).then_some(serde_json::Value::Object(attributes))
}

fn qualified(path: &str, parent_key: &str, name: &str) -> String {
    if parent_key == path {
        format!("{path}::{name}")
    } else {
        format!("{parent_key}::{name}")
    }
}
