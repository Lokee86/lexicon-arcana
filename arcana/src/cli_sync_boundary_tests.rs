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
    assert!(source.contains("load_compact("));
    assert!(source.contains("write_compiled_compact_owned("));
    assert!(source.contains("plan_verified_compact_snapshot_update_from_store("));
}
