#[path = "support/graph.rs"]
mod graph;
mod support;

use lexicon::Store;

use graph::{edge, incremental_analysis, node, publish_language, unresolved, write_file};
use support::TestDirectory;

#[test]
fn dependency_scope_matches_go_transitive_and_unresolved_behavior() {
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
    assert_eq!(scope.emit, vec!["a.py", "b.py", "c.py", "d.py", "e.py"]);
    assert_eq!(scope.context, scope.emit);

    assert!(
        !store
            .direct_changes_require_full("python", &["e.py".into()])
            .unwrap()
    );
    assert!(
        store
            .direct_changes_require_full("python", &["b.py".into()])
            .unwrap()
    );
    assert!(
        store
            .direct_changes_require_full("python", &["d.py".into()])
            .unwrap()
    );
}

#[test]
fn new_relationship_topology_requires_full_analysis() {
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

    let changed = incremental_analysis("node-y");
    assert!(
        store
            .requires_full_analysis("python", &["a.py".into()], &changed)
            .unwrap()
    );
}
