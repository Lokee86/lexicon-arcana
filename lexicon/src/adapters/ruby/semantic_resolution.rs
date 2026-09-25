#[allow(clippy::too_many_arguments)]
fn resolve_call(
    records: &mut Vec<FactRecord>,
    source_id: &str,
    method_q: &str,
    owner: &str,
    expr: &str,
    inheritance: &BTreeMap<String, String>,
    mixins: &BTreeMap<String, Vec<String>>,
    return_types: &BTreeMap<String, String>,
    param_types: &BTreeMap<String, BTreeMap<String, BTreeSet<String>>>,
    locals: &BTreeMap<String, String>,
) {
    if expr == "yield" || expr.starts_with("send(") || expr.contains("@clock.call") {
        return;
    }
    if expr.starts_with("require") || expr.starts_with("include ") || expr.contains(".include(") {
        return;
    }

    if let Some((receiver, name)) = expr.rsplit_once('.') {
        if receiver.is_empty() {
            return;
        }
        if receiver == "self" {
            let q = format!("{owner}#{name}");
            if node_exists_q(records, &q) {
                add_edge(records, source_id, &node_id("ruby", "method", &q), "calls");
                return;
            }
        }
        if receiver.chars().next().is_some_and(|c| c.is_uppercase()) {
            let q = format!("{receiver}.{name}");
            if node_exists_q(records, &q) {
                add_edge(records, source_id, &node_id("ruby", "method", &q), "calls");
                return;
            }
        }
        if let Some(ty) = locals.get(&format!("$type:{receiver}")) {
            let q = format!("{ty}#{name}");
            if node_exists_q(records, &q) {
                add_edge(records, source_id, &node_id("ruby", "method", &q), "calls");
                return;
            }
        }
        if let Some(types) = param_types.get(method_q).and_then(|m| m.get(receiver)) {
            let mut linked = false;
            for ty in types {
                let q = format!("{ty}#{name}");
                if node_exists_q(records, &q) {
                    add_edge(
                        records,
                        source_id,
                        &node_id("ruby", "method", &q),
                        "possible-calls",
                    );
                    linked = true;
                }
            }
            if linked {
                add_unresolved(records, source_id, "calls", expr, "dynamic-target");
                return;
            }
        }
        if let Some((factory, tail)) = receiver.rsplit_once('.') {
            let factory_q = format!("{factory}.{tail}");
            if node_exists_q(records, &factory_q) {
                add_edge(
                    records,
                    source_id,
                    &node_id("ruby", "method", &factory_q),
                    "calls",
                );
                if let Some(ty) = return_types
                    .get(&factory_q)
                    .or_else(|| return_types.get(&format!("{factory}#{tail}")))
                {
                    let q = format!("{ty}#{name}");
                    if node_exists_q(records, &q) {
                        add_edge(records, source_id, &node_id("ruby", "method", &q), "calls");
                        return;
                    }
                }
            }
        }
        add_unresolved(records, source_id, "calls", expr, "dynamic-target");
        return;
    }

    let name = expr.trim_end_matches("()");
    let mut candidates = Vec::<String>::new();
    let direct = if owner.is_empty() {
        name.to_string()
    } else {
        format!("{owner}#{name}")
    };
    if node_exists_q(records, &direct) {
        candidates.push(direct);
    }

    let mut cursor = owner.to_string();
    while let Some(parent) = inheritance.get(&cursor) {
        let q = format!("{parent}#{name}");
        if node_exists_q(records, &q) {
            candidates.push(q);
        }
        cursor = parent.clone();
    }
    for module_name in inherited_mixins(owner, inheritance, mixins) {
        let q = format!("{module_name}#{name}");
        if node_exists_q(records, &q) {
            candidates.push(q);
        }
    }

    if node_kind_q(records, owner) == Some("module") {
        let hosts = mixins
            .iter()
            .filter_map(|(host, modules)| modules.iter().any(|m| m == owner).then_some(host))
            .collect::<Vec<_>>();
        let host_candidates = hosts
            .iter()
            .filter_map(|host| {
                let q = format!("{host}#{name}");
                node_exists_q(records, &q).then_some(q)
            })
            .collect::<Vec<_>>();
        if host_candidates.len() == 1 {
            candidates.extend(host_candidates);
        } else if host_candidates.len() > 1 {
            add_unresolved(records, source_id, "calls", expr, "external-target");
            return;
        }
    }

    candidates.sort();
    candidates.dedup();
    if let Some(target) = candidates.last() {
        add_edge(
            records,
            source_id,
            &node_id("ruby", "method", target),
            "calls",
        );
    } else if !is_non_call_keyword(name) {
        add_unresolved(records, source_id, "calls", expr, "missing-target");
    }
}

fn emit_read(
    records: &mut Vec<FactRecord>,
    source: &str,
    owner: &str,
    rel: &str,
    locals: &BTreeMap<String, String>,
    token: &str,
) {
    if token.starts_with('@') || locals.contains_key(token) {
        let id = ensure_variable(
            records,
            source,
            token,
            rel,
            if token.starts_with('@') {
                "instance"
            } else {
                "local"
            },
        );
        add_edge(records, source, &id, "reads");
    } else if token.chars().next().is_some_and(|c| c.is_uppercase()) {
        let q = if owner.is_empty() {
            token.to_string()
        } else {
            format!("{owner}::{token}")
        };
        if node_exists_q(records, &q) {
            add_edge(records, source, &node_id("ruby", "constant", &q), "reads");
        }
    }
}

fn inherited_mixins(
    owner: &str,
    inheritance: &BTreeMap<String, String>,
    mixins: &BTreeMap<String, Vec<String>>,
) -> Vec<String> {
    let mut result = mixins.get(owner).cloned().unwrap_or_default();
    let mut cursor = owner;
    while let Some(parent) = inheritance.get(cursor) {
        result.extend(mixins.get(parent).cloned().unwrap_or_default());
        cursor = parent;
    }
    result
}

fn ensure_variable(
    records: &mut Vec<FactRecord>,
    owner: &str,
    name: &str,
    path: &str,
    scope: &str,
) -> String {
    let q = format!("{owner}:{name}");
    ensure_node(
        records,
        "variable",
        name,
        &q,
        path,
        Some(json!({"scope":scope,"owner":owner})),
    )
}

fn ensure_node(
    records: &mut Vec<FactRecord>,
    kind: &str,
    name: &str,
    qualified: &str,
    path: &str,
    attributes: Option<serde_json::Value>,
) -> String {
    let id = node_id("ruby", kind, qualified);
    if !records
        .iter()
        .any(|r| matches!(r, FactRecord::Node(n) if n.id == id))
    {
        records.push(FactRecord::Node(NodeRecord {
            attributes,
            content_id: None,
            id: id.clone(),
            kind: kind.into(),
            name: name.into(),
            owner: None,
            path: path.into(),
            qualified_name: qualified.into(),
            span: None,
        }));
    }
    id
}

fn add_edge(records: &mut Vec<FactRecord>, source: &str, target: &str, relation: &str) {
    if !node_exists_id(records, source) || !node_exists_id(records, target) {
        return;
    }
    if relation != "calls"
        && records.iter().any(|r| matches!(r, FactRecord::Edge(e) if e.source == source && e.target == target && e.relation == relation))
    {
        return;
    }
    records.push(FactRecord::Edge(EdgeRecord {
        attributes: None,
        owner: None,
        relation: relation.into(),
        source: source.into(),
        target: target.into(),
        span: None,
    }));
}

fn add_unresolved(
    records: &mut Vec<FactRecord>,
    source: &str,
    relation: &str,
    expression: &str,
    reason: &str,
) {
    if records.iter().any(|r| matches!(r, FactRecord::Unresolved(u) if u.source == source && u.relation == relation && u.expression == expression && u.reason == reason)) { return; }
    records.push(FactRecord::Unresolved(UnresolvedRecord {
        attributes: None,
        candidate_name: None,
        candidate_namespace: None,
        expression: expression.into(),
        owner: None,
        reason: reason.into(),
        relation: relation.into(),
        source: source.into(),
        span: None,
    }));
}