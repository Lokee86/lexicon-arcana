mod support;

use std::fs;

use lexicon::{AdapterHost, AdapterRequest, Analysis, FactRecord};
use support::TestDirectory;

#[test]
fn simple_identifier_initializers_match_oracle_dataflow_boundary() {
    let root = TestDirectory::new("c-family-access-initializer");
    write(
        &root,
        "main.c",
        "int run(int input) { int local = input; return local; }\n",
    );

    let analysis = analyze(&root);
    assert_access(&analysis, "run", "writes", "run::local");
    assert_no_access(&analysis, "run", "reads", "run::input");
    assert_access(&analysis, "run", "reads", "run::local");
}

#[test]
fn declarator_size_expressions_emit_reads_without_fabricating_writes() {
    let root = TestDirectory::new("c-family-access-declarator-size");
    write(
        &root,
        "main.c",
        "int run(int input) { char buffer[sizeof(input)]; return 0; }\n",
    );

    let analysis = analyze(&root);
    assert_access(&analysis, "run", "reads", "run::input");
    assert_no_access(&analysis, "run", "writes", "run::buffer");
}

#[test]
fn compound_assignments_and_updates_emit_reads_and_writes() {
    let root = TestDirectory::new("c-family-access-update");
    write(
        &root,
        "main.c",
        r#"int run(int input) {
  int local = 0;
  local += input;
  local++;
  return local;
}
"#,
    );

    let analysis = analyze(&root);
    assert_access(&analysis, "run", "writes", "run::local");
    assert_access(&analysis, "run", "reads", "run::local");
    assert_access(&analysis, "run", "reads", "run::input");
}

#[test]
fn fields_subscripts_and_receiver_values_are_conservative_dataflow() {
    let root = TestDirectory::new("c-family-access-fields");
    write(
        &root,
        "main.c",
        r#"struct Point { int x; int y; };
int run(struct Point *point, int *values, int index) {
  point->x = values[index];
  point->y++;
  return point->x;
}
"#,
    );

    let analysis = analyze(&root);
    assert_access(&analysis, "run", "writes", "Point::x");
    assert_access(&analysis, "run", "reads", "Point::x");
    assert_access(&analysis, "run", "writes", "Point::y");
    assert_access(&analysis, "run", "reads", "Point::y");
    assert_access(&analysis, "run", "reads", "run::point");
    assert_access(&analysis, "run", "reads", "run::values");
    assert_access(&analysis, "run", "reads", "run::index");
}

#[test]
fn ambiguous_field_ownership_does_not_invent_access_edges() {
    let root = TestDirectory::new("c-family-access-ambiguous-field");
    write(
        &root,
        "main.c",
        r#"struct A { int value; };
struct B { int value; };
int run(struct A *item) { return item->value; }
"#,
    );

    let analysis = analyze(&root);
    assert_no_access(&analysis, "run", "reads", "A::value");
    assert_no_access(&analysis, "run", "reads", "B::value");
    assert_access(&analysis, "run", "reads", "run::item");
}

fn analyze(root: &TestDirectory) -> Analysis {
    AdapterHost::new(root.path.join("adapters"))
        .analyze(&AdapterRequest {
            language: "c-family".into(),
            repository: root.path.clone(),
            ..Default::default()
        })
        .unwrap()
}

fn assert_access(analysis: &Analysis, source_qualified: &str, relation: &str, target: &str) {
    let source = node(analysis, source_qualified);
    let target = node(analysis, target);
    assert!(
        analysis.records.iter().any(|record| matches!(
            record,
            FactRecord::Edge(edge)
                if edge.relation == relation
                    && edge.source == source.id
                    && edge.target == target.id
        )),
        "missing {relation} {source_qualified} -> {}",
        target.qualified_name
    );
}

fn assert_no_access(analysis: &Analysis, source_qualified: &str, relation: &str, target: &str) {
    let source = node(analysis, source_qualified);
    let target = node(analysis, target);
    assert!(!analysis.records.iter().any(|record| matches!(
        record,
        FactRecord::Edge(edge)
            if edge.relation == relation
                && edge.source == source.id
                && edge.target == target.id
    )));
}

fn node<'a>(analysis: &'a Analysis, qualified: &str) -> &'a lexicon::NodeRecord {
    analysis
        .records
        .iter()
        .find_map(|record| match record {
            FactRecord::Node(node) if node.qualified_name == qualified => Some(node),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing node {qualified}"))
}

fn write(root: &TestDirectory, relative: &str, content: &str) {
    let path = root
        .path
        .join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}
