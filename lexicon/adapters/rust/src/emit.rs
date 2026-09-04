use crate::contract::JsonMap;
use crate::model::Context;
use anyhow::{Context as AnyhowContext, Result};
use serde_json::Value;

pub(crate) fn render(
    context: &Context,
    repository: &str,
    changed_files: Option<&[String]>,
    removed_files: Option<&[String]>,
) -> Result<String> {
    let incremental = changed_files.is_some() || removed_files.is_some();
    let selected: std::collections::HashSet<String> = changed_files
        .unwrap_or_default()
        .iter()
        .map(|path| path.replace('\\', "/"))
        .collect();
    let mut header = JsonMap::new();
    header.insert("adapter_version".into(), Value::String("0.5.0".into()));
    header.insert("language".into(), Value::String("rust".into()));
    header.insert("record".into(), Value::String("lexicon".into()));
    header.insert("repository".into(), Value::String(repository.into()));
    header.insert("schema_version".into(), Value::Number(1.into()));
    if incremental {
        header.insert("mode".into(), Value::String("incremental".into()));
        let mut changed: Vec<_> = selected.iter().cloned().collect();
        changed.sort();
        let mut removed: Vec<_> = removed_files
            .unwrap_or_default()
            .iter()
            .map(|path| path.replace('\\', "/"))
            .collect();
        removed.sort();
        header.insert("changed_files".into(), serde_json::json!(changed));
        header.insert("removed_files".into(), serde_json::json!(removed));
        header.insert("shared_complete".into(), Value::Bool(true));
    }
    let mut facts: Vec<Value> = context
        .facts
        .nodes
        .values()
        .chain(context.facts.edges.values())
        .chain(context.facts.unresolved.values())
        .cloned()
        .collect();
    if incremental {
        let owners = node_owners(&facts);
        facts.retain(|fact| include_fact(fact, &owners, &selected));
    }
    facts.sort_by_key(fact_sort_key);
    let mut values = vec![Value::Object(header)];
    values.extend(facts);
    values
        .into_iter()
        .map(|value| serde_json::to_string(&value).context("cannot serialize fact"))
        .collect::<Result<Vec<_>>>()
        .map(|lines| format!("{}\n", lines.join("\n")))
}

fn node_owners(facts: &[Value]) -> std::collections::HashMap<String, String> {
    facts
        .iter()
        .filter(|fact| fact.get("record").and_then(Value::as_str) == Some("node"))
        .filter_map(|fact| {
            let id = fact.get("id")?.as_str()?.to_string();
            Some((id, direct_owner(fact)))
        })
        .collect()
}

fn include_fact(
    fact: &Value,
    owners: &std::collections::HashMap<String, String>,
    selected: &std::collections::HashSet<String>,
) -> bool {
    let mut owner = direct_owner(fact);
    if owner.is_empty() {
        if let Some(source) = fact.get("source").and_then(Value::as_str) {
            owner = owners.get(source).cloned().unwrap_or_default();
        }
    }
    owner.is_empty() || selected.contains(&owner)
}

fn direct_owner(fact: &Value) -> String {
    if let Some(owner) = fact.get("owner").and_then(Value::as_str) {
        return owner.replace('\\', "/");
    }
    if let Some(path) = fact
        .get("span")
        .and_then(Value::as_object)
        .and_then(|span| span.get("path"))
        .and_then(Value::as_str)
    {
        return path.replace('\\', "/");
    }
    if fact.get("record").and_then(Value::as_str) == Some("node")
        && fact.get("kind").and_then(Value::as_str) == Some("file")
    {
        return fact
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .replace('\\', "/");
    }
    String::new()
}

type FactSortKey = (u8, String, String, String, String, Option<SpanSortKey>);
type SpanSortKey = (String, u64, u64, u64, u64);

pub(crate) fn fact_sort_key(value: &Value) -> FactSortKey {
    let object = value.as_object().expect("fact records are objects");
    match object.get("record").and_then(Value::as_str).unwrap_or("") {
        "node" => (
            0,
            field(object, "id"),
            field(object, "kind"),
            field(object, "path"),
            field(object, "qualified_name"),
            None,
        ),
        "edge" => (
            1,
            field(object, "source"),
            field(object, "target"),
            field(object, "relation"),
            String::new(),
            span_sort_key(object.get("span")),
        ),
        _ => (
            2,
            field(object, "source"),
            field(object, "relation"),
            field(object, "expression"),
            field(object, "reason"),
            span_sort_key(object.get("span")),
        ),
    }
}

fn field(object: &JsonMap, name: &str) -> String {
    object
        .get(name)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn span_sort_key(value: Option<&Value>) -> Option<SpanSortKey> {
    let span = value.and_then(Value::as_object)?;
    Some((
        span.get("path")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        span.get("start_line")
            .and_then(Value::as_u64)
            .unwrap_or_default(),
        span.get("start_column")
            .and_then(Value::as_u64)
            .unwrap_or_default(),
        span.get("end_line")
            .and_then(Value::as_u64)
            .unwrap_or_default(),
        span.get("end_column")
            .and_then(Value::as_u64)
            .unwrap_or_default(),
    ))
}
