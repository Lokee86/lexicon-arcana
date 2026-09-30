#[test]
fn managed_sync_has_no_rich_repository_facts_path() {
    let source = include_str!("cli_sync_build.rs");
    for forbidden in [
        "LexiconSnapshot::",
        "into_facts(",
        "write_compiled_owned",
        "plan_verified_snapshot_update_from_store(",
    ] {
        assert!(
            !source.contains(forbidden),
            "managed sync must not use rich path marker {forbidden:?}"
        );
    }
    assert!(source.contains("enum SyncPlan"));
    assert!(source.contains("load_compact("));
    assert!(source.contains("write_compiled_compact_owned("));
    assert!(source.contains("plan_verified_compact_snapshot_update_from_store("));
    assert!(
        !source.contains("materialize_base_dataset("),
        "managed sync must keep the packed graph as the base without materializing it"
    );

    let planner_start = source.find("pub(super) fn plan_snapshot(").unwrap();
    let planner_end = source.find("pub(super) fn build_snapshot(").unwrap();
    let planner = &source[planner_start..planner_end];
    for forbidden in [
        "load_compact(",
        "materialize_base_dataset(",
        "compile_compact_repository_graph(",
    ] {
        assert!(
            !planner.contains(forbidden),
            "sync planning must remain metadata-only; found {forbidden:?}"
        );
    }
}
