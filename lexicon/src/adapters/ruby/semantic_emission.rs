#[allow(clippy::too_many_arguments)]
fn emit_semantics(
    root: &Path,
    path: &Path,
    source: &str,
    records: &mut Vec<FactRecord>,
    inheritance: &BTreeMap<String, String>,
    mixins: &BTreeMap<String, Vec<String>>,
    return_types: &BTreeMap<String, String>,
    param_types: &BTreeMap<String, BTreeMap<String, BTreeSet<String>>>,
) {
    let rel = relative(root, path);
    let mut frames = Vec::<Frame>::new();
    let mut locals = BTreeMap::<String, String>::new();

    for (line_index, raw) in source.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some((name, _)) = struct_assignment(line) {
            frames.push(Frame::Namespace(name));
            continue;
        }
        if let Some(name) = namespace_open(line, &frames) {
            if !line.contains("; end") {
                frames.push(Frame::Namespace(name));
            }
            continue;
        }
        if line == "class << self" {
            frames.push(Frame::Other);
            continue;
        }

        if let Some((name, params, singleton)) = method_open(line) {
            let owner = namespace(&frames);
            let qualified = if singleton || inside_singleton_class(&frames) {
                format!("{owner}.{name}")
            } else if owner.is_empty() {
                name.clone()
            } else {
                format!("{owner}#{name}")
            };
            let id = node_id("ruby", "method", &qualified);
            locals.clear();
            for param in params {
                locals.insert(param.clone(), "parameter".into());
                ensure_variable(records, &id, &param, &rel, "parameter");
            }
            if let Some(parent) = inheritance.get(&owner) {
                let target_q = format!("{parent}#{name}");
                if node_exists_q(records, &target_q) {
                    add_edge(
                        records,
                        &id,
                        &node_id("ruby", "method", &target_q),
                        "overrides",
                    );
                }
            }
            frames.push(Frame::Method {
                id,
                qualified,
                owner,
            });
            if line.contains("; end") {
                frames.pop();
            }
            continue;
        }

        let current = current_source(&frames);
        if let Some((source_id, method_q, owner)) = current.clone() {
            if line == "yield" || line.contains("yield") {
                add_unresolved(records, &source_id, "calls", line, "dynamic-target");
            }
            if line.contains("send(") {
                add_unresolved(records, &source_id, "defines", line, "dynamic-target");
            }
            if line.contains("@clock.call") {
                add_unresolved(records, &source_id, "calls", line, "dynamic-target");
            }

            if let Some((lhs, rhs, compound)) = assignment(line) {
                let kind = if lhs.chars().next().is_some_and(|c| c.is_uppercase()) {
                    "constant"
                } else {
                    "variable"
                };
                let target = if kind == "constant" {
                    let q = if owner.is_empty() {
                        lhs.clone()
                    } else {
                        format!("{owner}::{lhs}")
                    };
                    ensure_node(records, "constant", &lhs, &q, &rel, None)
                } else {
                    locals.insert(lhs.clone(), "local".into());
                    ensure_variable(records, &source_id, &lhs, &rel, "local")
                };
                add_edge(records, &source_id, &target, "writes");
                if compound {
                    add_edge(records, &source_id, &target, "reads");
                }
                for token in identifiers(&rhs) {
                    emit_read(records, &source_id, &owner, &rel, &locals, &token);
                }
                if let Some(ty) = constructor_type(&rhs) {
                    locals.insert(format!("$type:{lhs}"), ty);
                }
            } else {
                for token in identifiers(line) {
                    emit_read(records, &source_id, &owner, &rel, &locals, &token);
                }
            }

            if line == "super" || line.starts_with("super(") {
                if let Some(parent) = inheritance.get(&owner) {
                    if let Some(name) = method_q.split(['#', '.']).next_back() {
                        let q = format!("{parent}#{name}");
                        if node_exists_q(records, &q) {
                            add_edge(records, &source_id, &node_id("ruby", "method", &q), "calls");
                        }
                    }
                }
            }

            for expression in call_expressions(line) {
                resolve_call(
                    records,
                    &source_id,
                    &method_q,
                    &owner,
                    &expression,
                    inheritance,
                    mixins,
                    return_types,
                    param_types,
                    &locals,
                );
            }

            if opens_block(line) {
                let block_q = format!("{method_q}#block@{}", line_index + 1);
                let block_id = ensure_node(
                    records,
                    "function",
                    "block",
                    &block_q,
                    &rel,
                    Some(json!({"block":true})),
                );
                frames.push(Frame::Block { id: block_id });
                continue;
            }
        }

        if line == "end" {
            frames.pop();
        }
    }
}

