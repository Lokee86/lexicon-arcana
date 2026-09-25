use std::collections::BTreeMap;

use serde_json::{Map, Value, json};

use super::model::{Declaration, ParameterDecl, ParsedFile};
use super::relationships::{RelationshipTarget, qualify, queue_annotations, queue_supertypes};
use super::state::AnalysisState;
use super::tokens::{contains, nullable_type};

impl AnalysisState {
    pub fn emit_imports(&mut self, file: &ParsedFile) {
        let module = self.module_by_path[&file.path].clone();
        for (index, imported) in file.imports.iter().enumerate() {
            let local = if imported.alias.is_empty() {
                imported.path.rsplit('.').next().unwrap_or(&imported.path)
            } else {
                &imported.alias
            };
            let canonical = format!(
                "{}::import::{}::{}::{index}",
                file.path, imported.path, imported.alias
            );
            let import_id = self.facts.add_node(
                "import",
                &canonical,
                local,
                &file.path,
                &imported.path,
                Some(&file.path),
                Some(imported.span.clone()),
                Some(json!({
                    "alias": imported.alias,
                    "imported": imported.path,
                    "wildcard": imported.wildcard
                })),
            );
            self.facts.add_edge(
                &module,
                &import_id,
                "defines",
                Some(&file.path),
                Some(imported.span.clone()),
                None,
            );
            let target = self.facts.add_node(
                "symbol",
                &format!("external-import:{}", imported.path),
                local,
                "external",
                &imported.path,
                None,
                None,
                Some(json!({"external": true, "resolution": "syntactic-import-target"})),
            );
            self.facts.add_edge(
                &import_id,
                &target,
                "imports",
                Some(&file.path),
                Some(imported.span.clone()),
                Some(json!({"alias": imported.alias, "wildcard": imported.wildcard})),
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn emit_declaration(
        &mut self,
        file: &ParsedFile,
        declaration: &Declaration,
        owner_id: &str,
        owner_qn: &str,
        owner_canonical: &str,
        owner_kind: &str,
        occurrences: &mut BTreeMap<String, usize>,
    ) -> String {
        let mut kind = declaration.kind.as_str();
        if kind == "function" && matches!(owner_kind, "type" | "interface") {
            kind = "method";
        }
        let qualified_base = qualify(owner_qn, &declaration.name);
        let (canonical, qualified_name) = match kind {
            "function" | "method" => {
                let signature = parameter_signature(&declaration.parameters);
                (
                    format!(
                        "{owner_canonical}::callable:{}::receiver:{}({signature})",
                        declaration.name, declaration.receiver
                    ),
                    format!("{qualified_base}({signature})"),
                )
            }
            "constructor" => {
                let signature = parameter_signature(&declaration.parameters);
                (
                    format!("{owner_canonical}::constructor({signature})"),
                    format!("{owner_qn}.<init>({signature})"),
                )
            }
            "field" => (
                format!(
                    "{owner_canonical}::property:{}::receiver:{}",
                    declaration.name, declaration.receiver
                ),
                qualified_base.clone(),
            ),
            _ => (
                format!("{owner_canonical}::{kind}:{}", declaration.name),
                qualified_base.clone(),
            ),
        };
        let canonical = disambiguate(canonical, occurrences);
        let display_name = if kind == "constructor" {
            owner_qn.rsplit('.').next().unwrap_or(owner_qn)
        } else {
            &declaration.name
        };
        let id = self.facts.add_node(
            kind,
            &canonical,
            display_name,
            &file.path,
            &qualified_name,
            Some(&file.path),
            Some(declaration.span.clone()),
            declaration_attributes(declaration),
        );
        for relation in ["contains", "defines"] {
            self.facts.add_edge(
                owner_id,
                &id,
                relation,
                Some(&file.path),
                Some(declaration.span.clone()),
                None,
            );
        }
        if matches!(kind, "type" | "interface") {
            self.relationship_by_qn
                .entry(qualified_base.clone())
                .or_default()
                .push(RelationshipTarget {
                    form: declaration.form.clone(),
                    id: id.clone(),
                    kind: kind.into(),
                });
        }
        self.index_runtime_declaration(
            file,
            declaration,
            &id,
            kind,
            owner_qn,
            owner_kind,
            &qualified_base,
        );
        queue_annotations(
            &mut self.pending_relations,
            file,
            &id,
            owner_qn,
            &declaration.annotations,
            &declaration.span,
        );
        if matches!(declaration.kind.as_str(), "type" | "interface") {
            queue_supertypes(
                &mut self.pending_relations,
                file,
                &id,
                owner_qn,
                &declaration.supertypes,
            );
        }

        if matches!(kind, "function" | "method" | "constructor") {
            for (index, parameter) in declaration.parameters.iter().enumerate() {
                let parameter_canonical =
                    format!("{canonical}::parameter::{index:04}:{}", parameter.name);
                let parameter_qn = format!("{qualified_name}::parameter:{}", parameter.name);
                let parameter_id = self.facts.add_node(
                    "parameter",
                    &parameter_canonical,
                    &parameter.name,
                    &file.path,
                    &parameter_qn,
                    Some(&file.path),
                    Some(parameter.span.clone()),
                    Some(parameter_attributes(parameter, index)),
                );
                for relation in ["contains", "defines"] {
                    self.facts.add_edge(
                        &id,
                        &parameter_id,
                        relation,
                        Some(&file.path),
                        Some(parameter.span.clone()),
                        None,
                    );
                }
                self.index_runtime_parameter(&id, &parameter.name, &parameter_id);
                queue_annotations(
                    &mut self.pending_relations,
                    file,
                    &parameter_id,
                    owner_qn,
                    &parameter.annotations,
                    &parameter.span,
                );
            }
        }

        let mut children = BTreeMap::new();
        for child in &declaration.children {
            self.emit_declaration(
                file,
                child,
                &id,
                &qualified_base,
                &canonical,
                &declaration.kind,
                &mut children,
            );
        }
        if declaration.kind == "type" {
            for parameter in declaration.parameters.iter().filter(|value| value.property) {
                let property = constructor_property(parameter);
                let property_id = self.emit_declaration(
                    file,
                    &property,
                    &id,
                    &qualified_base,
                    &canonical,
                    &declaration.kind,
                    &mut children,
                );
                if let Some(attributes) = self.facts.node_attributes_mut(&property_id) {
                    let object = attributes
                        .get_or_insert_with(|| Value::Object(Map::new()))
                        .as_object_mut()
                        .expect("attributes object");
                    object.insert("constructor_parameter".into(), json!(true));
                }
            }
        }
        id
    }
}

fn constructor_property(parameter: &ParameterDecl) -> Declaration {
    Declaration {
        annotations: Vec::new(),
        body: Default::default(),
        children: Vec::new(),
        delegation: Default::default(),
        delegated: false,
        form: "constructor_parameter_property".into(),
        kind: "field".into(),
        modifiers: Vec::new(),
        mutable: parameter.mutable,
        name: parameter.name.clone(),
        parameters: Vec::new(),
        primary: false,
        receiver: String::new(),
        return_type: String::new(),
        span: parameter.span.clone(),
        supertypes: Vec::new(),
        type_name: parameter.type_name.clone(),
    }
}

fn declaration_attributes(declaration: &Declaration) -> Option<Value> {
    let mut attributes = Map::new();
    if !declaration.form.is_empty() {
        attributes.insert("declaration_kind".into(), json!(declaration.form));
    }
    if !declaration.annotations.is_empty() {
        attributes.insert("annotations".into(), json!(declaration.annotations));
    }
    if !declaration.modifiers.is_empty() {
        attributes.insert("modifiers".into(), json!(declaration.modifiers));
    }
    if declaration.kind == "function" {
        attributes.insert(
            "suspend".into(),
            json!(contains(&declaration.modifiers, "suspend")),
        );
        if !declaration.receiver.is_empty() {
            attributes.insert("extension_receiver".into(), json!(declaration.receiver));
            attributes.insert(
                "extension_receiver_nullable".into(),
                json!(nullable_type(&declaration.receiver)),
            );
        }
        if !declaration.return_type.is_empty() {
            attributes.insert("return_type".into(), json!(declaration.return_type));
            attributes.insert(
                "return_nullable".into(),
                json!(nullable_type(&declaration.return_type)),
            );
        }
    }
    if declaration.kind == "constructor" {
        attributes.insert("primary".into(), json!(declaration.primary));
    }
    if declaration.kind == "field" {
        attributes.insert("mutable".into(), json!(declaration.mutable));
        if !declaration.receiver.is_empty() {
            attributes.insert("extension_receiver".into(), json!(declaration.receiver));
            attributes.insert(
                "extension_receiver_nullable".into(),
                json!(nullable_type(&declaration.receiver)),
            );
        }
        if !declaration.type_name.is_empty() {
            attributes.insert("type".into(), json!(declaration.type_name));
            attributes.insert(
                "nullable".into(),
                json!(nullable_type(&declaration.type_name)),
            );
        }
    }
    (!attributes.is_empty()).then_some(Value::Object(attributes))
}

fn parameter_attributes(parameter: &ParameterDecl, index: usize) -> Value {
    let mut attributes = Map::from_iter([
        ("has_default".into(), json!(parameter.has_default)),
        ("index".into(), json!(index)),
        (
            "nullable".into(),
            json!(nullable_type(&parameter.type_name)),
        ),
        ("property".into(), json!(parameter.property)),
        ("type".into(), json!(parameter.type_name)),
    ]);
    if parameter.property {
        attributes.insert("mutable".into(), json!(parameter.mutable));
    }
    if !parameter.annotations.is_empty() {
        attributes.insert("annotations".into(), json!(parameter.annotations));
    }
    if !parameter.modifiers.is_empty() {
        attributes.insert("modifiers".into(), json!(parameter.modifiers));
    }
    Value::Object(attributes)
}

fn disambiguate(canonical: String, occurrences: &mut BTreeMap<String, usize>) -> String {
    let occurrence = occurrences.entry(canonical.clone()).or_default();
    *occurrence += 1;
    if *occurrence == 1 {
        canonical
    } else {
        format!("{canonical}#{}", *occurrence)
    }
}

pub fn parameter_signature(parameters: &[ParameterDecl]) -> String {
    parameters
        .iter()
        .map(|parameter| parameter.type_name.as_str())
        .collect::<Vec<_>>()
        .join(",")
}
