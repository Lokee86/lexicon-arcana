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
    assert!(source.contains("load_compact_delta("));
    assert!(source.contains("plan_compact_delta_edge_changes_from_store("));
    assert!(source.contains("rewrite_repository_store("));
    for forbidden in [
        "materialize_base_dataset(",
        "plan_verified_compact_snapshot_update_from_store(",
        "compile_compact_repository_graph(",
    ] {
        assert!(
            !source.contains(forbidden),
            "managed sync must not use obsolete incremental marker {forbidden:?}"
        );
    }

    let incremental_start = source.find("fn write_incremental_snapshot(").unwrap();
    let incremental_end = source.find("fn rebuild_loaded_snapshot(").unwrap();
    let incremental = &source[incremental_start..incremental_end];
    assert!(incremental.contains("load_compact_delta("));
    assert!(!incremental.contains("load_compact("));

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
