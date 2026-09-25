use super::{
    model::{AccessObservation, CallObservation, Declaration, RepositoryModel},
    resolution::DeclarationIndex,
};
use crate::{EdgeRecord, FactRecord};
use serde_json::json;

pub fn add_passes_to(
    index: &DeclarationIndex<'_>,
    observation: &CallObservation,
    target: &Declaration,
    records: &mut Vec<FactRecord>,
) {
    let Some(parameters) = index.by_callable_parameters.get(&target.id) else {
        return;
    };
    for (argument_index, expression) in observation.argument_expressions.iter().enumerate() {
        let Some(name) = simple_identifier_name(expression) else {
            continue;
        };
        let Some(parameter) = parameters.iter().find(|parameter| {
            parameter
                .attributes
                .get("index")
                .and_then(|value| value.as_u64())
                == Some(argument_index as u64)
        }) else {
            continue;
        };
        let Some(argument) = resolve_direct_argument(index, observation, &name) else {
            continue;
        };
        records.push(FactRecord::Edge(EdgeRecord {
            attributes: Some(json!({
                "argument_index": argument_index,
                "expression": expression,
                "via_call": target.id,
            })),
            owner: Some(observation.path.clone()),
            relation: "passes-to".into(),
            source: argument.id.clone(),
            span: Some(observation.span.clone()),
            target: parameter.id.clone(),
        }));
    }
}

pub fn add_access_facts(model: &RepositoryModel, records: &mut Vec<FactRecord>) {
    let index = DeclarationIndex::new(model);
    for file in &model.files {
        for observation in &file.accesses {
            let candidates = resolve_access(&index, observation);
            if candidates.len() != 1 {
                continue;
            }
            records.push(FactRecord::Edge(EdgeRecord {
                attributes: None,
                owner: Some(observation.path.clone()),
                relation: observation.relation.clone(),
                source: observation.source_id.clone(),
                span: Some(observation.span.clone()),
                target: candidates[0].id.clone(),
            }));
        }
    }
}

fn resolve_access<'a>(
    index: &'a DeclarationIndex<'a>,
    observation: &AccessObservation,
) -> Vec<&'a Declaration> {
    let accept = |declaration: &Declaration| {
        matches!(
            declaration.kind.as_str(),
            "parameter" | "variable" | "constant" | "field"
        )
    };
    let key = format!("{}\0{}", observation.source_id, observation.candidate);
    let mut candidates = index
        .by_container_name
        .get(&key)
        .into_iter()
        .flatten()
        .copied()
        .filter(|value| accept(value))
        .collect::<Vec<_>>();
    if candidates.is_empty() && !observation.parent_type_id.is_empty() {
        let key = format!("{}\0{}", observation.parent_type_id, observation.candidate);
        candidates = index
            .by_container_name
            .get(&key)
            .into_iter()
            .flatten()
            .copied()
            .filter(|value| accept(value))
            .collect();
    }
    if candidates.is_empty() {
        candidates = index.resolve(
            &observation.candidate,
            &observation.source_scope,
            &observation.path,
            accept,
        );
    }
    candidates
}

fn resolve_direct_argument<'a>(
    index: &'a DeclarationIndex<'a>,
    observation: &CallObservation,
    name: &str,
) -> Option<&'a Declaration> {
    let key = format!("{}\0{name}", observation.source_id);
    let local = index
        .by_container_name
        .get(&key)
        .into_iter()
        .flatten()
        .copied()
        .filter(|value| matches!(value.kind.as_str(), "parameter" | "variable" | "constant"))
        .collect::<Vec<_>>();
    if local.len() == 1 {
        return Some(local[0]);
    }
    if local.len() > 1 {
        return None;
    }

    let source = index.by_id.get(&observation.source_id).copied();
    if let Some(source) = source
        && !source.parent_type_id.is_empty()
    {
        let key = format!("{}\0{name}", source.parent_type_id);
        let fields = index
            .select(
                index.by_container_name.get(&key),
                &observation.path,
                |value| value.kind == "field",
            )
            .unwrap_or_default();
        if fields.len() == 1 {
            return Some(fields[0]);
        }
        if fields.len() > 1 {
            return None;
        }
    }

    let constants = index.resolve(
        name,
        &observation.source_scope,
        &observation.path,
        |value| value.kind == "constant",
    );
    if constants.len() == 1 {
        Some(constants[0])
    } else {
        None
    }
}

fn simple_identifier_name(expression: &str) -> Option<String> {
    let mut value = expression.trim();
    loop {
        if !(value.starts_with('(') && value.ends_with(')')) {
            break;
        }
        let inner = &value[1..value.len() - 1];
        if !balanced_parentheses(inner) {
            break;
        }
        value = inner.trim();
    }
    let mut chars = value.chars();
    let first = chars.next()?;
    if first != '_' && !first.is_ascii_alphabetic() {
        return None;
    }
    if chars.all(|character| character == '_' || character.is_ascii_alphanumeric()) {
        Some(value.into())
    } else {
        None
    }
}

fn balanced_parentheses(value: &str) -> bool {
    let mut depth = 0isize;
    for character in value.chars() {
        match character {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
            _ => {}
        }
    }
    depth == 0
}

#[cfg(test)]
mod tests {
    use super::simple_identifier_name;

    #[test]
    fn direct_argument_identifier_is_conservative() {
        assert_eq!(simple_identifier_name("value").as_deref(), Some("value"));
        assert_eq!(
            simple_identifier_name("((value))").as_deref(),
            Some("value")
        );
        assert_eq!(simple_identifier_name("value + 1"), None);
        assert_eq!(simple_identifier_name("obj.value"), None);
    }
}
