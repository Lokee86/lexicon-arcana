use crate::synthetic::{EdgeKind, NodeId};

use super::{
    CatalogueEntry, ContentId, EdgeFact, NodeFact, NodeKey, NodeKind, RelationKind,
    RepositoryCatalogue, RepositoryCompileError, RepositoryFacts, SourceSpan, UnresolvedReason,
    UnresolvedReferenceFact, compile_repository_facts, compile_repository_facts_owned,
    edge_kind_to_relation, read_catalogue, relation_to_edge_kind, write_catalogue,
};

#[test]
fn compiler_assigns_dense_ids_and_stable_relation_codes() {
    let first = NodeKey::from_identity("first");
    let second = NodeKey::from_identity("second");
    let facts = RepositoryFacts {
        nodes: vec![node(second, "b.rs"), node(first, "a.rs")],
        edges: vec![EdgeFact {
            source: first,
            target: second,
            relation: RelationKind::Calls,
            span: None,
        }],
        unresolved: Vec::new(),
    };
    let compiled = compile_repository_facts(&facts).unwrap();
    assert_eq!(compiled.dataset.node_count, 2);
    assert_eq!(
        compiled.dataset.edges,
        vec![crate::synthetic::Edge {
            source: NodeId(0),
            target: NodeId(1),
            kind: EdgeKind(5),
        }]
    );
    assert_eq!(compiled.node_ids[&first], NodeId(0));
    assert_eq!(compiled.node_ids[&second], NodeId(1));
    assert_eq!(
        edge_kind_to_relation(EdgeKind(5)),
        Some(RelationKind::Calls)
    );
    assert_eq!(relation_to_edge_kind(&RelationKind::Calls), EdgeKind(5));
    assert_eq!(
        relation_to_edge_kind(&RelationKind::PossibleCalls),
        EdgeKind(13)
    );
    assert_eq!(
        edge_kind_to_relation(EdgeKind(13)),
        Some(RelationKind::PossibleCalls)
    );
    assert_eq!(
        relation_to_edge_kind(&RelationKind::ConvertsTo),
        EdgeKind(14)
    );
    assert_eq!(
        edge_kind_to_relation(EdgeKind(14)),
        Some(RelationKind::ConvertsTo)
    );
    assert_eq!(relation_to_edge_kind(&RelationKind::PassesTo), EdgeKind(20));
    assert_eq!(
        edge_kind_to_relation(EdgeKind(20)),
        Some(RelationKind::PassesTo)
    );
    for (relation, code) in [
        (RelationKind::CallsEndpoint, 25),
        (RelationKind::HandledBy, 26),
        (RelationKind::Publishes, 27),
        (RelationKind::Consumes, 28),
        (RelationKind::ReadsConfig, 29),
        (RelationKind::InvokesProcess, 30),
        (RelationKind::ProducesMessage, 31),
        (RelationKind::ConsumesMessage, 32),
    ] {
        assert_eq!(relation_to_edge_kind(&relation), EdgeKind(code));
        assert_eq!(edge_kind_to_relation(EdgeKind(code)), Some(relation));
    }
}

#[test]
fn interstack_node_kinds_round_trip() {
    for (value, kind) in [
        ("http-endpoint", NodeKind::HttpEndpoint),
        ("message-channel", NodeKind::MessageChannel),
        ("config-key", NodeKind::ConfigKey),
        ("process", NodeKind::Process),
        ("cli-command", NodeKind::CliCommand),
        ("protocol", NodeKind::Protocol),
        ("state-path", NodeKind::StatePath),
    ] {
        assert_eq!(NodeKind::parse(value), Some(kind.clone()));
        assert_eq!(kind.as_str(), value);
    }
}

#[test]
fn consuming_compiler_matches_borrowed_compiler() {
    let first = NodeKey::from_identity("owned-first");
    let second = NodeKey::from_identity("owned-second");
    let facts = RepositoryFacts::with_unresolved(
        vec![node(second, "b.rs"), node(first, "a.rs")],
        vec![EdgeFact {
            source: first,
            target: second,
            relation: RelationKind::Calls,
            span: None,
        }],
        vec![UnresolvedReferenceFact {
            source: first,
            relation: RelationKind::Calls,
            expression: "dynamic()".to_owned(),
            candidate_namespace: None,
            candidate_name: Some("dynamic".to_owned()),
            reason: UnresolvedReason::DynamicTarget,
            span: None,
        }],
    );

    let borrowed = compile_repository_facts(&facts).unwrap();
    let owned = compile_repository_facts_owned(facts).unwrap();
    assert_eq!(owned, borrowed);
}

#[test]
fn consuming_compiler_moves_owned_strings_without_cloning() {
    let key = NodeKey::from_identity("moved-node");
    let node = node(key, "src/moved.rs");
    let node_name_ptr = node.name.as_ptr();
    let reference = UnresolvedReferenceFact {
        source: key,
        relation: RelationKind::Calls,
        expression: "moved_dynamic()".to_owned(),
        candidate_namespace: None,
        candidate_name: None,
        reason: UnresolvedReason::DynamicTarget,
        span: None,
    };
    let expression_ptr = reference.expression.as_ptr();

    let compiled = compile_repository_facts_owned(RepositoryFacts::with_unresolved(
        vec![node],
        vec![],
        vec![reference],
    ))
    .unwrap();

    assert_eq!(
        compiled.catalogue.entries()[0].fact.name.as_ptr(),
        node_name_ptr
    );
    assert_eq!(compiled.unresolved[0].expression.as_ptr(), expression_ptr);
}

#[test]
fn compiler_collapses_repeated_call_sites_to_one_graph_edge() {
    let caller = NodeKey::from_identity("caller");
    let callee = NodeKey::from_identity("callee");
    let facts = RepositoryFacts {
        nodes: vec![node(caller, "caller.rs"), node(callee, "callee.rs")],
        edges: vec![
            EdgeFact {
                source: caller,
                target: callee,
                relation: RelationKind::Calls,
                span: Some(SourceSpan::new("caller.rs", 10, 5, 10, 12).unwrap()),
            },
            EdgeFact {
                source: caller,
                target: callee,
                relation: RelationKind::Calls,
                span: Some(SourceSpan::new("caller.rs", 20, 5, 20, 12).unwrap()),
            },
        ],
        unresolved: Vec::new(),
    };

    let compiled = compile_repository_facts(&facts).unwrap();
    assert_eq!(compiled.dataset.edges.len(), 1);
    assert_eq!(compiled.dataset.edges[0].kind, EdgeKind(5));
}

#[test]
fn compiler_preserves_recursive_self_edges() {
    let key = NodeKey::from_identity("recursive");
    let facts = RepositoryFacts {
        nodes: vec![node(key, "recursive.go")],
        edges: vec![EdgeFact {
            source: key,
            target: key,
            relation: RelationKind::Calls,
            span: None,
        }],
        unresolved: Vec::new(),
    };

    let compiled = compile_repository_facts(&facts).unwrap();
    assert_eq!(
        compiled.dataset.edges,
        vec![crate::synthetic::Edge {
            source: NodeId(0),
            target: NodeId(0),
            kind: EdgeKind(5),
        }]
    );
}

#[test]
fn compiler_rejects_invalid_facts() {
    let key = NodeKey::from_identity("node");
    let base = node(key, "a.rs");
    let conflicting = NodeFact {
        name: "other".to_owned(),
        ..base.clone()
    };
    assert!(
        compile_repository_facts(&RepositoryFacts {
            nodes: vec![base.clone(), conflicting],
            edges: vec![],
            unresolved: Vec::new(),
        })
        .is_err()
    );
    assert!(
        compile_repository_facts(&RepositoryFacts {
            nodes: vec![base.clone()],
            edges: vec![edge(key, NodeKey::from_u64(99))],
            unresolved: Vec::new(),
        })
        .is_err()
    );
}

#[test]
fn compiler_preserves_and_validates_unresolved_references() {
    let source = NodeKey::from_identity("source");
    let reference = UnresolvedReferenceFact {
        source,
        relation: RelationKind::Calls,
        expression: "pkg.Call".to_owned(),
        candidate_namespace: Some("pkg".to_owned()),
        candidate_name: Some("Call".to_owned()),
        reason: UnresolvedReason::UnsupportedForm,
        span: Some(SourceSpan::new("src/main.rs", 1, 1, 1, 9).unwrap()),
    };
    let facts = RepositoryFacts::with_unresolved(
        vec![node(source, "src/main.rs")],
        vec![],
        vec![reference.clone()],
    );
    let compiled = compile_repository_facts(&facts).unwrap();
    assert_eq!(compiled.unresolved, vec![reference.clone()]);

    let error = compile_repository_facts(&RepositoryFacts::with_unresolved(
        vec![],
        vec![],
        vec![reference],
    ))
    .unwrap_err();
    assert!(matches!(
        error,
        RepositoryCompileError::MissingUnresolvedSource { key } if key == source
    ));
}

#[test]
fn catalogue_round_trips_and_supports_exact_lookups() {
    let catalogue = sample_catalogue();
    let key = catalogue.entries()[0].fact.key;
    let encoded = catalogue.encode().unwrap();
    assert_eq!(RepositoryCatalogue::decode(&encoded).unwrap(), catalogue);
    assert_eq!(catalogue.lookup_by_key(key).unwrap().node_id, NodeId(0));
    assert_eq!(catalogue.lookup_by_path("src\\main.rs").unwrap().len(), 1);
    assert_eq!(catalogue.lookup_by_name("main").len(), 1);
}

#[test]
fn catalogue_indexes_duplicate_names_paths_kinds_keys_and_prefix_boundaries() {
    let catalogue = RepositoryCatalogue::new(vec![
        catalogue_entry(3, 40, NodeKind::Function, "src/library.rs", "same"),
        catalogue_entry(1, 20, NodeKind::Function, "src/lib/a.rs", "same"),
        catalogue_entry(4, 50, NodeKind::File, "docs/readme.md", "readme"),
        catalogue_entry(0, 10, NodeKind::Function, "src/a.rs", "same"),
        catalogue_entry(2, 30, NodeKind::Type, "src/lib/a.rs", "same"),
    ])
    .unwrap();

    assert_eq!(
        catalogue.node_ids_by_name("same"),
        &[NodeId(0), NodeId(1), NodeId(2), NodeId(3)]
    );
    assert_eq!(
        catalogue.node_ids_by_path("src/lib/a.rs").unwrap(),
        &[NodeId(1), NodeId(2)]
    );
    assert_eq!(
        catalogue.node_ids_by_path_prefix("src/lib").unwrap(),
        &[NodeId(1), NodeId(2)]
    );
    assert_eq!(
        catalogue.node_ids_by_kind(&NodeKind::Function),
        &[NodeId(0), NodeId(1), NodeId(3)]
    );
    assert_eq!(
        catalogue.node_id_by_key(NodeKey::from_u64(30)),
        Some(NodeId(2))
    );
}

#[test]
fn catalogue_file_is_immutable_and_validated() {
    let catalogue = sample_catalogue();
    let path = std::env::temp_dir().join(format!("arcana-catalogue-{}.txt", std::process::id()));
    let _ = std::fs::remove_file(&path);
    write_catalogue(&path, &catalogue).unwrap();
    assert_eq!(read_catalogue(&path).unwrap(), catalogue);
    assert!(write_catalogue(&path, &catalogue).is_err());
    let encoded = std::fs::read_to_string(&path).unwrap();
    assert!(encoded.starts_with("version\t3\n"));
    assert!(RepositoryCatalogue::decode(encoded.trim_end_matches('\n')).is_err());
    std::fs::remove_file(path).unwrap();
}

fn sample_catalogue() -> RepositoryCatalogue {
    RepositoryCatalogue::new(vec![CatalogueEntry {
        node_id: NodeId(0),
        fact: NodeFact {
            key: NodeKey::from_identity("node"),
            external_identity: None,
            kind: NodeKind::File,
            path: "src/main.rs".to_owned(),
            name: "main".to_owned(),
            qualified_name: "demo::main".to_owned(),
            content_id: Some(ContentId::from_bytes(b"content")),
            span: Some(SourceSpan::new("src/main.rs", 1, 2, 3, 4).unwrap()),
        },
    }])
    .unwrap()
}

fn catalogue_entry(
    node_id: u32,
    key: u64,
    kind: NodeKind,
    path: &str,
    name: &str,
) -> CatalogueEntry {
    CatalogueEntry {
        node_id: NodeId(node_id),
        fact: NodeFact {
            key: NodeKey::from_u64(key),
            external_identity: None,
            kind,
            path: path.to_owned(),
            name: name.to_owned(),
            qualified_name: name.to_owned(),
            content_id: None,
            span: None,
        },
    }
}

fn node(key: NodeKey, path: &str) -> NodeFact {
    NodeFact {
        key,
        external_identity: None,
        kind: NodeKind::File,
        path: path.to_owned(),
        name: String::new(),
        qualified_name: String::new(),
        content_id: None,
        span: None,
    }
}

fn edge(source: NodeKey, target: NodeKey) -> EdgeFact {
    EdgeFact {
        source,
        target,
        relation: RelationKind::References,
        span: None,
    }
}

#[test]
fn catalogue_reads_version_one_without_external_identities() {
    let input = concat!(
        "version\t1\n",
        "N\t0\t0000000000000001\tfunction\tsrc/lib.rs\tlegacy\t-\t-\t-\t-\t-\t-\n"
    );
    let catalogue = RepositoryCatalogue::decode(input).unwrap();
    assert_eq!(catalogue.entries().len(), 1);
    assert_eq!(catalogue.entries()[0].fact.external_identity, None);
}
