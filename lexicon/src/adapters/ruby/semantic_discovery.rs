fn discover_semantics(
    root: &Path,
    path: &Path,
    source: &str,
    records: &mut Vec<FactRecord>,
    mixins: &mut BTreeMap<String, Vec<String>>,
    return_types: &mut BTreeMap<String, String>,
    param_types: &mut BTreeMap<String, BTreeMap<String, BTreeSet<String>>>,
) {
    let rel = relative(root, path);
    let mut frames = Vec::<Frame>::new();
    let mut module_functions = BTreeSet::<String>::new();
    let mut current_method: Option<(String, String)> = None;

    for (line_index, raw) in source.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some((name, accessors)) = struct_assignment(line) {
            ensure_node(records, "type", &name, &name, &rel, None);
            for accessor in accessors {
                ensure_node(
                    records,
                    "method",
                    &accessor,
                    &format!("{name}#{accessor}"),
                    &rel,
                    Some(json!({"generated":"Struct"})),
                );
            }
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

        let namespace = namespace(&frames);
        if line == "module_function" && !namespace.is_empty() {
            module_functions.insert(namespace.clone());
            continue;
        }
        if let Some(module_name) = include_target(line) {
            if !namespace.is_empty() {
                mixins
                    .entry(namespace.clone())
                    .or_default()
                    .push(module_name);
            }
        }
        if let Some((host, module_name)) = explicit_include(line) {
            mixins.entry(host).or_default().push(module_name);
        }

        if let Some((name, params, singleton)) = method_open(line) {
            let owner = namespace.clone();
            let qualified = if singleton || inside_singleton_class(&frames) {
                format!("{owner}.{name}")
            } else if owner.is_empty() {
                name.clone()
            } else {
                format!("{owner}#{name}")
            };
            let id = ensure_node(records, "method", &name, &qualified, &rel, None);
            for param in &params {
                ensure_variable(records, &id, param, &rel, "parameter");
            }
            if module_functions.contains(&owner) && !singleton {
                ensure_node(
                    records,
                    "method",
                    &name,
                    &format!("{owner}.{name}"),
                    &rel,
                    None,
                );
            }
            current_method = Some((qualified.clone(), id.clone()));
            frames.push(Frame::Method {
                id,
                qualified,
                owner,
            });
            if line.contains("; end") {
                frames.pop();
                current_method = None;
            }
            continue;
        }

        if let Some((method_q, _)) = &current_method {
            if let Some(ty) = constructor_type(line) {
                return_types.entry(method_q.clone()).or_insert(ty);
            }
        } else if let Some((lhs, _, _)) = assignment(line) {
            if lhs.chars().next().is_some_and(|c| c.is_uppercase()) {
                let owner = namespace.clone();
                let qualified = if owner.is_empty() {
                    lhs.clone()
                } else {
                    format!("{owner}::{lhs}")
                };
                ensure_node(records, "constant", &lhs, &qualified, &rel, None);
            }
        }

        if current_method.is_none() {
            if let Some((callee, args)) = simple_call(line) {
                if let Some(method_q) = find_top_level_method(records, &callee) {
                    let params = method_params(source, &method_q);
                    for (idx, arg) in args.iter().enumerate() {
                        if let Some(ty) = constructor_type(arg) {
                            if let Some(param) = params.get(idx) {
                                param_types
                                    .entry(method_q.clone())
                                    .or_default()
                                    .entry(param.clone())
                                    .or_default()
                                    .insert(ty);
                            }
                        }
                    }
                }
            }
        }

        if line == "end" {
            if let Some(frame) = frames.pop() {
                if matches!(frame, Frame::Method { .. }) {
                    current_method = frames.iter().rev().find_map(|f| match f {
                        Frame::Method { qualified, id, .. } => {
                            Some((qualified.clone(), id.clone()))
                        }
                        _ => None,
                    });
                }
            }
        }
        let _ = line_index;
    }
}

