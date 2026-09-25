mod support;

use std::fs;

use lexicon::{AdapterHost, AdapterRequest, Analysis, FactRecord};
use support::TestDirectory;

#[test]
fn resolves_scoped_nested_and_explicitly_qualified_base_types() {
    let root = TestDirectory::new("c-family-inheritance-scopes");
    write(
        &root,
        "base.hpp",
        r#"namespace demo {
class Base {};
namespace nested {
class NestedDerived : public Base {};
}
}
namespace other { class Base {}; }
"#,
    );
    write(
        &root,
        "derived.cpp",
        r#"#include "base.hpp"
namespace demo {
class Derived : public Base {};
class Qualified : public other::Base {};
}
"#,
    );

    let analysis = analyze(&root);
    assert_extends(&analysis, "demo::Derived", "demo::Base");
    assert_extends(&analysis, "demo::nested::NestedDerived", "demo::Base");
    assert_extends(&analysis, "demo::Qualified", "other::Base");
}

#[test]
fn template_arguments_are_removed_before_base_resolution() {
    let root = TestDirectory::new("c-family-inheritance-template");
    write(
        &root,
        "main.cpp",
        r#"template <typename T> class Box {};
class Item : public Box<int> {};
"#,
    );

    let analysis = analyze(&root);
    assert_extends(&analysis, "Item", "Box");
    assert!(!has_unresolved_extends(
        &analysis,
        "Box<int>",
        "missing-target"
    ));
}

#[test]
fn ambiguous_short_base_names_remain_explicitly_unresolved() {
    let root = TestDirectory::new("c-family-inheritance-ambiguous");
    write(
        &root,
        "main.cpp",
        r#"namespace left { class Base {}; }
namespace right { class Base {}; }
class Derived : public Base {};
"#,
    );

    let analysis = analyze(&root);
    assert_unresolved_extends(&analysis, "Base", "ambiguous-target");
}

#[test]
fn missing_base_types_remain_explicitly_unresolved() {
    let root = TestDirectory::new("c-family-inheritance-missing");
    write(&root, "main.cpp", "class Derived : public Missing {};\n");

    let analysis = analyze(&root);
    assert_unresolved_extends(&analysis, "Missing", "missing-target");
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

fn assert_extends(analysis: &Analysis, source_qualified: &str, target_qualified: &str) {
    let source = node(analysis, "type", source_qualified);
    let target = node(analysis, "type", target_qualified);
    assert!(analysis.records.iter().any(|record| matches!(
        record,
        FactRecord::Edge(edge)
            if edge.relation == "extends"
                && edge.source == source.id
                && edge.target == target.id
    )));
}

fn assert_unresolved_extends(analysis: &Analysis, expression: &str, reason: &str) {
    assert!(analysis.records.iter().any(|record| matches!(
        record,
        FactRecord::Unresolved(value)
            if value.relation == "extends"
                && value.expression == expression
                && value.reason == reason
    )));
}

fn has_unresolved_extends(analysis: &Analysis, expression: &str, reason: &str) -> bool {
    analysis.records.iter().any(|record| {
        matches!(
            record,
            FactRecord::Unresolved(value)
                if value.relation == "extends"
                    && value.expression == expression
                    && value.reason == reason
        )
    })
}

fn node<'a>(analysis: &'a Analysis, kind: &str, qualified: &str) -> &'a lexicon::NodeRecord {
    analysis
        .records
        .iter()
        .find_map(|record| match record {
            FactRecord::Node(node) if node.kind == kind && node.qualified_name == qualified => {
                Some(node)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing {kind} {qualified}"))
}

fn write(root: &TestDirectory, relative: &str, content: &str) {
    let path = root
        .path
        .join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}
