#[test]
fn compact_loader_has_no_repository_wide_rich_fallback() {
    let loader = include_str!("snapshot_compact.rs");
    let visitor = include_str!("snapshot_compact_visit.rs");
    assert!(!loader.contains("snapshot::load("));
    assert!(!loader.contains("into_facts("));
    assert!(!visitor.contains("snapshot::load("));
    assert!(!visitor.contains("RepositoryFacts"));
}

#[test]
fn compact_delta_loader_uses_only_selected_object_visitors() {
    let source = include_str!("snapshot_compact.rs");
    let start = source.find("pub fn load_compact_delta(").unwrap();
    let end = source[start..]
        .find("\nfn profile(")
        .map(|offset| start + offset)
        .unwrap();
    let delta = &source[start..end];

    assert!(delta.contains("visit_node_pass_selected("));
    assert!(delta.contains("visit_relation_pass_selected("));
    assert!(!delta.contains("visit_node_pass("));
    assert!(!delta.contains("visit_relation_pass("));
}
