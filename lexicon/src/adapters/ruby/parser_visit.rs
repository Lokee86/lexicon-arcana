fn visit(
    node: Node<'_>,
    bytes: &[u8],
    path: &str,
    file: &str,
    scope: Vec<String>,
    out: &mut Vec<FactRecord>,
) {
    let kind = node.kind();
    let text = node.utf8_text(bytes).unwrap_or("");
    let mut next = scope.clone();
    if matches!(kind, "module" | "class") {
        if let Some(name) = node
            .child_by_field_name("name")
            .and_then(|n| n.utf8_text(bytes).ok())
        {
            let q = if scope.len() > 1 {
                format!("{}::{}", scope[1..].join("::"), name)
            } else {
                name.to_owned()
            };
            let nk = if kind == "class" { "type" } else { "module" };
            let id = node_id("ruby", nk, &q);
            if !out.iter().any(|r| matches!(r, FactRecord::Node(n) if n.id == id)) {
                out.push(FactRecord::Node(NodeRecord {
                    attributes: None,
                    content_id: None,
                    id: id.clone(),
                    kind: nk.into(),
                    name: name.into(),
                    owner: None,
                    path: path.into(),
                    qualified_name: q.clone(),
                    span: Some(node_span(node, path)),
                }));
            }
            if !out.iter().any(|r| matches!(r, FactRecord::Edge(e)
                if e.source == file && e.target == id && e.relation == "defines"))
            {
                out.push(FactRecord::Edge(EdgeRecord {
                    attributes: None,
                    owner: None,
                    relation: "defines".into(),
                    source: file.into(),
                    target: id.clone(),
                    span: Some(node_span(node, path)),
                }));
            }
            if kind == "class" {
                if let Some(base) = node.child_by_field_name("superclass") {
                    if let Ok(base) = base.utf8_text(bytes) {
                        let base = base.trim().trim_start_matches('<').trim();
                        out.push(FactRecord::Edge(EdgeRecord {
                            attributes: None,
                            owner: None,
                            relation: "extends".into(),
                            source: id.clone(),
                            target: node_id("ruby", "type", base),
                            span: Some(node_span(base_node(base, node), path)),
                        }));
                    }
                }
            }
            next.push(name.into());
        }
    }
    if kind == "method" || kind == "singleton_method" {
        if let Some(name) = node
            .child_by_field_name("name")
            .and_then(|n| n.utf8_text(bytes).ok())
        {
            let q = format!("{}#{}", scope[1..].join("::"), name);
            let id = node_id("ruby", "method", &q);
            if !out.iter().any(|r| matches!(r, FactRecord::Node(n) if n.id == id)) {
                out.push(FactRecord::Node(NodeRecord {
                    attributes: None,
                    content_id: None,
                    id: id.clone(),
                    kind: "method".into(),
                    name: name.into(),
                    owner: None,
                    path: path.into(),
                    qualified_name: q,
                    span: Some(node_span(node, path)),
                }));
            }
            if !out.iter().any(|r| matches!(r, FactRecord::Edge(e)
                if e.source == file && e.target == id && e.relation == "defines"))
            {
                out.push(FactRecord::Edge(EdgeRecord {
                    attributes: None,
                    owner: None,
                    relation: "defines".into(),
                    source: file.into(),
                    target: id,
                    span: Some(node_span(node, path)),
                }));
            }
        }
    }
    if kind == "call" && (text.starts_with("require") || text.starts_with("require_relative")) {
        let target = text.split('"').nth(1).or_else(|| text.split('\'').nth(1));
        if let Some(target) = target {
            let import_id = node_id("ruby", "import", &format!("{path}:{target}"));
            out.push(FactRecord::Node(NodeRecord {
                attributes: Some(json!({"target": target})),
                content_id: None,
                id: import_id.clone(),
                kind: "import".into(),
                name: target.into(),
                owner: None,
                path: path.into(),
                qualified_name: format!("import:{path}:{target}"),
                span: Some(node_span(node, path)),
            }));
            out.push(FactRecord::Edge(EdgeRecord {
                attributes: None,
                owner: None,
                relation: "imports".into(),
                source: file.into(),
                target: import_id,
                span: Some(node_span(node, path)),
            }));
        } else {
            out.push(FactRecord::Unresolved(UnresolvedRecord {
                attributes: None,
                candidate_name: None,
                candidate_namespace: None,
                expression: text.into(),
                owner: None,
                reason: "dynamic-target".into(),
                relation: "imports".into(),
                source: file.into(),
                span: Some(node_span(node, path)),
            }));
        }
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        visit(child, bytes, path, file, next.clone(), out);
    }
}
