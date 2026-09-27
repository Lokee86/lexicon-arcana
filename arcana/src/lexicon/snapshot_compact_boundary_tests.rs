#[test]
fn compact_loader_has_no_repository_wide_rich_fallback() {
    let loader = include_str!("snapshot_compact.rs");
    let visitor = include_str!("snapshot_compact_visit.rs");
    assert!(!loader.contains("snapshot::load("));
    assert!(!loader.contains("into_facts("));
    assert!(!visitor.contains("snapshot::load("));
    assert!(!visitor.contains("RepositoryFacts"));
}
