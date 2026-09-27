#[path = "support/graph.rs"]
mod graph;
mod support;

use lexicon::Store;

use graph::{edge, incremental_analysis, node, publish_language, unresolved, write_file};
use support::TestDirectory;

#[test]
fn dependency_scope_matches_bounded_one_hop_behavior() {
    let directory = TestDirectory::new("dependency-scope");
    let store = Store::new(&directory.path);
    let files = vec![
        write_file(
            &store,
            "a.py",
            vec![
                node("node-a", "a.py"),
                node("node-a-child", "a.py"),
                edge("node-a", "node-a-child", "contains", "a.py"),
            ],
        ),
        write_file(
            &store,
            "b.py",
            vec![
                node("node-b", "b.py"),
                edge("node-b", "node-a", "calls", "b.py"),
            ],
        ),
        write_file(
            &store,
            "c.py",
            vec![
                node("node-c", "c.py"),
                edge("node-c", "node-b", "calls", "c.py"),
            ],
        ),
        write_file(
            &store,
            "d.py",
            vec![
                node("node-d", "d.py"),
                unresolved("node-d", "missing-target", "d.py"),
            ],
        ),
        write_file(
            &store,
            "e.py",
            vec![
                node("node-e", "e.py"),
                unresolved("node-e", "builtin-target", "e.py"),
            ],
        ),
    ];
    publish_language(&store, files);

    let scope = store.incremental_scope("python", &["a.py".into()]).unwrap();
    assert!(!scope.full_required);
    assert_eq!(scope.emit, vec!["a.py", "b.py"]);
    assert_eq!(scope.context, vec!["a.py", "b.py"]);

    for path in ["b.py", "d.py", "e.py"] {
        assert!(
            !store
                .direct_changes_require_full("python", &[path.into()])
                .unwrap(),
            "{path} should be decided after scoped analysis"
        );
    }
    assert!(
        store
            .direct_changes_require_full("python", &["missing.py".into()])
            .unwrap()
    );
}

#[test]
fn resolved_relationship_topology_is_accepted_but_sensitive_unresolved_is_not() {
    let directory = TestDirectory::new("topology");
    let store = Store::new(&directory.path);
    publish_language(
        &store,
        vec![
            write_file(
                &store,
                "a.py",
                vec![
                    node("node-a", "a.py"),
                    edge("node-a", "node-x", "calls", "a.py"),
                ],
            ),
            write_file(&store, "x.py", vec![node("node-x", "x.py")]),
        ],
    );

    let existing = incremental_analysis("node-x");
    assert!(
        !store
            .requires_full_analysis("python", &["a.py".into()], &existing)
            .unwrap()
    );

    let mut resolved = incremental_analysis("node-x");
    let lexicon::FactRecord::Edge(edge) = &mut resolved.records[1] else {
        unreachable!()
    };
    edge.relation = "references".into();
    assert!(
        !store
            .requires_full_analysis("python", &["a.py".into()], &resolved)
            .unwrap()
    );

    let mut sensitive = incremental_analysis("node-x");
    sensitive.records[1] = unresolved("node-a", "ambiguous-target", "a.py");
    assert!(
        store
            .requires_full_analysis("python", &["a.py".into()], &sensitive)
            .unwrap()
    );
}
