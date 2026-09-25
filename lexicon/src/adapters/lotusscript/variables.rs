use serde_json::{Value, json};

use crate::SourceSpan;

use super::declarations::{
    declaration_public, declared_type, normalize_type, visibility_attributes,
};
use super::facts::Facts;
use super::model::{AnalysisState, Declaration, UseEvidence, VariableSymbol};
use super::syntax::{identifier_prefix, literal_value, split_top_level};

impl AnalysisState {
    #[allow(clippy::too_many_arguments)]
    pub fn add_variables(
        &mut self,
        facts: &mut Facts,
        file_path: &str,
        module_id: &str,
        span: &SourceSpan,
        text: &str,
        class: Option<&Declaration>,
        callable: Option<&Declaration>,
    ) -> bool {
        let trimmed = text.trim();
        let (kind, body, visibility) = if let Some(body) = redim_body(trimmed) {
            ("variable", body, String::new())
        } else if let Some((visibility, body)) = constant_body(trimmed) {
            ("constant", body, visibility)
        } else {
            let fields = trimmed.split_whitespace().collect::<Vec<_>>();
            let mut index = 0;
            let mut visibility = String::new();
            while index < fields.len() && variable_modifier(fields[index]) {
                if !fields[index].eq_ignore_ascii_case("Dim") {
                    visibility.push_str(fields[index]);
                    visibility.push(' ');
                }
                index += 1;
            }
            if index == 0 {
                if class.is_none() || callable.is_some() || identifier_prefix(trimmed).is_empty() {
                    return false;
                }
                ("field", trimmed.to_owned(), visibility)
            } else {
                if index >= fields.len() {
                    return false;
                }
                (
                    if callable.is_some() {
                        "variable"
                    } else {
                        "field"
                    },
                    fields[index..].join(" "),
                    visibility,
                )
            }
        };

        let (owner_id, owner_qualified) = callable
            .map(|value| (value.id.as_str(), value.qualified_name.as_str()))
            .or_else(|| class.map(|value| (value.id.as_str(), value.qualified_name.as_str())))
            .unwrap_or((module_id, file_path));

        for part in split_top_level(&body, ',') {
            let name = identifier_prefix(&part);
            if name.is_empty() {
                continue;
            }
            let data_type = declared_type(&part);
            let mut attributes = visibility_attributes(&visibility);
            if !data_type.is_empty() {
                attributes.insert("type".into(), json!(data_type));
            }
            let identity = format!(
                "{owner_qualified}::{kind}::{}::{}",
                name.to_ascii_lowercase(),
                span.start_line
            );
            let id = facts.add_node(
                kind,
                &name,
                file_path,
                &format!("{owner_qualified}::{name}"),
                &identity,
                Some(file_path),
                Some(span.clone()),
                (!attributes.is_empty()).then_some(Value::Object(attributes)),
                None,
            );
            let public = declaration_public(
                &visibility,
                class.map_or_else(
                    || self.module_public.get(file_path).copied().unwrap_or(false),
                    |value| value.type_members_public,
                ),
            );
            let symbol = VariableSymbol {
                data_type: normalize_type(&data_type),
                id: id.clone(),
                public,
            };
            if let Some(callable) = callable {
                self.variable_symbols
                    .entry(callable.id.clone())
                    .or_default()
                    .insert(name.to_ascii_lowercase(), symbol);
            } else if let Some(class) = class {
                self.field_symbols
                    .entry(class.id.clone())
                    .or_default()
                    .insert(name.to_ascii_lowercase(), symbol);
            } else {
                self.module_symbols
                    .entry(file_path.into())
                    .or_default()
                    .insert(name.to_ascii_lowercase(), symbol);
            }
            facts.add_edge(
                owner_id,
                &id,
                if callable.is_some() {
                    "contains"
                } else {
                    "defines"
                },
                Some(file_path),
                Some(span.clone()),
                None,
            );
        }
        true
    }

    pub fn add_use(
        &mut self,
        facts: &mut Facts,
        file_path: &str,
        module_id: &str,
        span: &SourceSpan,
        keyword: &str,
        expression: &str,
    ) {
        let literal = literal_value(expression);
        let identity_target = literal.as_deref().unwrap_or_else(|| expression.trim());
        let identity = format!(
            "{file_path}::import::{}::{}::{}",
            span.start_line,
            keyword.to_ascii_lowercase(),
            identity_target.to_ascii_lowercase()
        );
        let id = facts.add_node(
            "import",
            identity_target,
            file_path,
            &identity,
            &identity,
            Some(file_path),
            Some(span.clone()),
            Some(json!({"keyword": keyword, "target": identity_target})),
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
        self.uses.push(UseEvidence {
            dynamic: literal.is_none(),
            expression: expression.trim().into(),
            import_id: id,
            keyword: keyword.into(),
            owner_path: file_path.into(),
            span: span.clone(),
            target: literal.unwrap_or_default(),
        });
    }
}

fn variable_modifier(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "dim" | "global" | "public" | "private" | "protected" | "static"
    )
}

fn redim_body(value: &str) -> Option<String> {
    let fields = value.split_whitespace().collect::<Vec<_>>();
    if fields.len() < 2 || !fields[0].eq_ignore_ascii_case("ReDim") {
        return None;
    }
    let mut index = 1;
    if fields
        .get(index)
        .is_some_and(|value| value.eq_ignore_ascii_case("Preserve"))
    {
        index += 1;
    }
    (index < fields.len()).then(|| fields[index..].join(" "))
}

fn constant_body(value: &str) -> Option<(String, String)> {
    let lower = value.to_ascii_lowercase();
    let index = lower.find("const ")?;
    let prefix = value[..index].trim();
    if !prefix.is_empty()
        && !prefix.split_whitespace().all(|value| {
            matches!(
                value.to_ascii_lowercase().as_str(),
                "public" | "private" | "protected"
            )
        })
    {
        return None;
    }
    Some((prefix.into(), value[index + 6..].trim().into()))
}
