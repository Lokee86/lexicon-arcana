#[path = "support/dependency_delta_fixture.rs"]
mod fixture;
mod support;

use fixture::{State, apply_step, check_parity, edge, header, node, publish, unresolved};
use lexicon::{Analysis, Store};
use support::TestDirectory;

#[test]
fn repeated_edits_delete_add_shared_ownership_match_full_rebuild() {
    let directory = TestDirectory::new("delta-parity");
    let indexed = Store::new(directory.path.join("incremental"));
    let oracle = Store::new(directory.path.join("oracle"));
    let mut state = State::initial();
    let full = Analysis::new(header("full", &[], &[]), state.all_records());
    let initial = indexed
        .build_full_language(&full, &state.sources(), "python", "cfg", "fp")
        .unwrap();
    let full_initial = oracle
        .build_full_language(&full, &state.sources(), "python", "cfg", "fp")
        .unwrap();
    publish(&indexed, initial, 0);
    publish(&oracle, full_initial, 0);
    check_parity(&indexed, &oracle, "initial");

    state.revisions.insert("a.py".into(), 2);
    state.files.insert(
        "a.py".into(),
        vec![node("node-a-new", Some("a.py"), "a.py")],
    );
    apply_step(&indexed, &oracle, &state, 1, &["a.py"], &[], "edit a");
    state.revisions.insert("b.py".into(), 2);
    state.files.insert(
        "b.py".into(),
        vec![
            node("node-b", Some("b.py"), "b.py"),
            edge("node-b", "shared-e", "b.py"),
        ],
    );
    apply_step(&indexed, &oracle, &state, 2, &["b.py"], &[], "relink b");
    state.revisions.remove("b.py");
    state.files.remove("b.py");
    apply_step(&indexed, &oracle, &state, 3, &[], &["b.py"], "remove b");
    state.revisions.insert("e.py".into(), 1);
    state
        .files
        .insert("e.py".into(), vec![node("node-e", Some("e.py"), "e.py")]);
    apply_step(
        &indexed,
        &oracle,
        &state,
        4,
        &["e.py"],
        &[],
        "shared target becomes owned",
    );
    state.revisions.insert("e.py".into(), 2);
    state
        .files
        .insert("e.py".into(), vec![node("node-e2", Some("e.py"), "e.py")]);
    apply_step(
        &indexed,
        &oracle,
        &state,
        5,
        &["e.py"],
        &[],
        "second edit without bootstrap",
    );
    state.revisions.insert("pkg/old.py".into(), 2);
    state.files.insert(
        "pkg/old.py".into(),
        vec![
            node("node-pkg/old.py", Some("pkg/old.py"), "pkg/old.py"),
            unresolved("pkg/old.py", "pkg.other"),
        ],
    );
    apply_step(
        &indexed,
        &oracle,
        &state,
        6,
        &["pkg/old.py"],
        &[],
        "changed unresolved module candidate",
    );
}
