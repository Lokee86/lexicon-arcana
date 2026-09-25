use serde_json::{Map, Value, json};

use crate::SourceSpan;

use super::facts::Facts;
use super::model::{AnalysisState, Declaration, ExtendsEvidence, VariableSymbol};
use super::syntax::{contains_word, identifier_prefix, split_top_level};

impl AnalysisState {
    #[allow(clippy::too_many_arguments)]
    pub fn add_type(
        &mut self,
        facts: &mut Facts,
        file_path: &str,
        module_id: &str,
        span: &SourceSpan,
        name: &str,
        visibility: &str,
        base: &str,
        type_members_public: bool,
    ) -> Declaration {
        let qualified = format!("{file_path}::{name}");
        let mut attributes = visibility_attributes(visibility);
        if !base.is_empty() {
            attributes.insert("base".into(), json!(base));
        }
        let id = facts.add_node(
            "type",
            name,
            file_path,
            &qualified,
            &qualified,
            Some(file_path),
            Some(span.clone()),
            map_value(attributes),
            None,
        );
        facts.add_edge(
            module_id,
            &id,
            "defines",
            Some(file_path),
            Some(span.clone()),
            None,
        );
        let declaration = Declaration {
            class_id: None,
            id: id.clone(),
            owner_path: file_path.into(),
            public: declaration_public(
                visibility,
                self.module_public.get(file_path).copied().unwrap_or(false),
            ),
            qualified_name: qualified,
            span: span.clone(),
            type_members_public,
        };
        let key = name.to_ascii_lowercase();
        self.classes_by_name
            .entry(key.clone())
            .or_default()
            .push(declaration.clone());
        if !base.is_empty() {
            self.extends.push(ExtendsEvidence {
                base: base.into(),
                class_id: id,
                owner_path: file_path.into(),
                span: span.clone(),
            });
        }
        declaration
    }

    #[allow(clippy::too_many_arguments)]
    pub fn add_callable(
        &mut self,
        facts: &mut Facts,
        file_path: &str,
        module_id: &str,
        span: &SourceSpan,
        class: Option<&Declaration>,
        form: &str,
        name: &str,
        modifiers: &str,
        parameters: &str,
        external: bool,
        tail: &str,
    ) -> Declaration {
        let (owner_id, owner_qualified, kind, class_id) = if let Some(class) = class {
            (
                class.id.clone(),
                class.qualified_name.clone(),
                if name.eq_ignore_ascii_case("New") {
                    "constructor"
                } else {
                    "method"
                },
                Some(class.id.clone()),
            )
        } else {
            (module_id.into(), file_path.into(), "function", None)
        };

        let lower_form = form.to_ascii_lowercase();
        let mut attributes = visibility_attributes(modifiers);
        attributes.insert("form".into(), json!(lower_form));
        if lower_form.starts_with("property") {
            attributes.insert(
                "property_accessor".into(),
                json!(lower_form.trim_start_matches("property").trim()),
            );
        }
        if external {
            attributes.insert("external".into(), json!(true));
            if let Some(library) = library_name(tail) {
                attributes.insert("library".into(), json!(library));
            }
        }

        let identity = format!(
            "{owner_qualified}::{kind}::{}::{lower_form}",
            name.to_ascii_lowercase()
        );
        let qualified = format!("{owner_qualified}::{name}");
        let id = facts.add_node(
            kind,
            name,
            file_path,
            &qualified,
            &identity,
            Some(file_path),
            Some(span.clone()),
            map_value(attributes),
            None,
        );
        facts.add_edge(
            &owner_id,
            &id,
            "defines",
            Some(file_path),
            Some(span.clone()),
            None,
        );

        let declaration = Declaration {
            class_id: class_id.clone(),
            id: id.clone(),
            owner_path: file_path.into(),
            public: declaration_public(
                modifiers,
                if class.is_some() {
                    true
                } else {
                    self.module_public.get(file_path).copied().unwrap_or(false)
                },
            ),
            qualified_name: qualified,
            span: span.clone(),
            type_members_public: false,
        };

        let key = name.to_ascii_lowercase();
        self.callables_by_name
            .entry(key.clone())
            .or_default()
            .push(declaration.clone());
        if let Some(class_id) = class_id {
            self.methods_by_class
                .entry(class_id)
                .or_default()
                .entry(key)
                .or_default()
                .push(declaration.clone());
        }
        self.add_parameters(facts, &declaration, parameters);
        declaration
    }

    fn add_parameters(&mut self, facts: &mut Facts, callable: &Declaration, parameters: &str) {
        let parameters = parameters.trim();
        if parameters.len() < 2 {
            return;
        }
        for (index, part) in split_top_level(&parameters[1..parameters.len() - 1], ',')
            .into_iter()
            .enumerate()
        {
            let name = parameter_name(&part);
            if name.is_empty() {
                continue;
            }
            let data_type = declared_type(&part);
            let mut attributes = Map::new();
            attributes.insert("position".into(), json!(index));
            if !data_type.is_empty() {
                attributes.insert("type".into(), json!(data_type));
            }
            for modifier in ["byval", "byref", "optional", "paramarray"] {
                if contains_word(&part, modifier) {
                    attributes.insert(modifier.into(), json!(true));
                }
            }
            let identity = format!(
                "{}::parameter::{index}::{}",
                callable.qualified_name,
                name.to_ascii_lowercase()
            );
            let id = facts.add_node(
                "parameter",
                &name,
                &callable.owner_path,
                &format!("{}::{name}", callable.qualified_name),
                &identity,
                Some(&callable.owner_path),
                Some(callable.span.clone()),
                Some(Value::Object(attributes)),
                None,
            );
            self.record_variable_symbol(&callable.id, &name, &data_type, &id, true);
            facts.add_edge(
                &callable.id,
                &id,
                "contains",
                Some(&callable.owner_path),
                Some(callable.span.clone()),
                None,
            );
        }
    }

    pub fn record_variable_symbol(
        &mut self,
        owner: &str,
        name: &str,
        data_type: &str,
        id: &str,
        public: bool,
    ) {
        self.variable_symbols
            .entry(owner.into())
            .or_default()
            .insert(
                name.to_ascii_lowercase(),
                VariableSymbol {
                    data_type: normalize_type(data_type),
                    id: id.into(),
                    public,
                },
            );
    }
}

pub fn visibility_attributes(value: &str) -> Map<String, Value> {
    let mut attributes = Map::new();
    for word in value.split_whitespace().map(str::to_ascii_lowercase) {
        match word.as_str() {
            "public" | "private" | "protected" => {
                attributes.insert("visibility".into(), json!(word));
            }
            "static" => {
                attributes.insert("static".into(), json!(true));
            }
            _ => {}
        }
    }
    attributes
}

pub fn declaration_public(modifiers: &str, default_public: bool) -> bool {
    for word in modifiers.split_whitespace() {
        if word.eq_ignore_ascii_case("public") {
            return true;
        }
        if word.eq_ignore_ascii_case("private") || word.eq_ignore_ascii_case("protected") {
            return false;
        }
    }
    default_public
}

pub fn parameter_name(value: &str) -> String {
    value
        .split_whitespace()
        .find(|word| {
            !matches!(
                word.to_ascii_lowercase().as_str(),
                "byval" | "byref" | "optional" | "paramarray"
            )
        })
        .map(identifier_prefix)
        .unwrap_or_default()
}

pub fn declared_type(value: &str) -> String {
    let lower = value.to_ascii_lowercase();
    let Some(index) = lower.find(" as ") else {
        return String::new();
    };
    let mut tail = value[index + 4..].trim();
    if let Some(equal) = tail.find('=') {
        tail = tail[..equal].trim();
    }
    if tail.len() >= 4 && tail[..4].eq_ignore_ascii_case("new ") {
        tail = tail[4..].trim();
    }
    tail.trim().to_owned()
}

pub fn normalize_type(value: &str) -> String {
    let mut value = value.trim();
    if value.len() >= 4 && value[..4].eq_ignore_ascii_case("new ") {
        value = value[4..].trim();
    }
    value = value.trim_end_matches("()").trim();
    value
        .rsplit('.')
        .next()
        .unwrap_or(value)
        .to_ascii_lowercase()
}

fn library_name(tail: &str) -> Option<String> {
    let lower = tail.to_ascii_lowercase();
    let index = lower.find("lib")?;
    super::syntax::literal_value(
        tail[index + 3..]
            .split_whitespace()
            .next()
            .unwrap_or_default(),
    )
}

fn map_value(value: Map<String, Value>) -> Option<Value> {
    (!value.is_empty()).then_some(Value::Object(value))
}
