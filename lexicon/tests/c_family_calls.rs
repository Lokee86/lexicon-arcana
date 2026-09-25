mod support;

use std::fs;

use lexicon::{AdapterHost, AdapterRequest, Analysis, FactRecord};
use serde_json::Value;
use support::TestDirectory;

#[test]
fn resolves_direct_free_calls_static_translation_units_and_constructors() {
    let root = TestDirectory::new("c-family-calls-direct");
    write(
        &root,
        "bundle.c",
        "#include \"helper.c\"\n#include \"user.c\"\n",
    );
    write(
        &root,
        "helper.c",
        "static int helper(int value) { return value + 1; }\n",
    );
    write(
        &root,
        "user.c",
        "int run(int value) { return helper(value); }\n",
    );
    write(
        &root,
        "thing.cpp",
        "class Thing { public: Thing() {} };\nThing make() { return Thing(); }\n",
    );

    let analysis = analyze(&root);
    assert_call(&analysis, "run", "helper", "calls", "direct-scoped-name");
    assert_call_kind(
        &analysis,
        "make",
        "constructor",
        "Thing",
        "calls",
        "direct-scoped-name",
    );
}

#[test]
fn member_calls_prefer_enclosing_and_direct_receiver_types() {
    let root = TestDirectory::new("c-family-calls-members");
    write(
        &root,
        "members.cpp",
        r#"class A {
public:
  int foo() { return 1; }
  int own() { return foo() + this->foo() + self.foo(); }
};
class B { public: int foo() { return 2; } };
class Holder {
  A *member;
public:
  int parameter(A *value) { return value->foo(); }
  int local() { A value; return value.foo(); }
  int field_receiver() { return member->foo(); }
};
"#,
    );

    let analysis = analyze(&root);
    let source = "A::own";
    assert_call(
        &analysis,
        source,
        "A::foo",
        "calls",
        "enclosing-type-ownership",
    );
    assert_no_call_relation(&analysis, source, "possible-calls");
    for source in [
        "Holder::parameter",
        "Holder::local",
        "Holder::field_receiver",
    ] {
        assert_call(&analysis, source, "A::foo", "calls", "direct-receiver-type");
        assert_no_call_relation(&analysis, source, "possible-calls");
    }
}

#[test]
fn inherited_receiver_falls_back_conservatively_and_unknown_receivers_stay_ambiguous() {
    let root = TestDirectory::new("c-family-calls-inherited");
    write(
        &root,
        "members.cpp",
        r#"class A { public: int run() { return 1; } };
class B { public: int run() { return 2; } };
class Base { public: int inherited() { return 3; } };
class Derived : public Base {};
namespace left { class Thing {}; }
namespace right { class Thing {}; }
class Holder {
public:
  int inherited_receiver(Derived *value) { return value->inherited(); }
  int unknown(Missing *value) { return value->run(); }
  int ambiguous(Thing *value) { return value->run(); }
};
"#,
    );

    let analysis = analyze(&root);
    assert_call(
        &analysis,
        "Holder::inherited_receiver",
        "Base::inherited",
        "calls",
        "direct-scoped-name",
    );
    for source in ["Holder::unknown", "Holder::ambiguous"] {
        assert_call(
            &analysis,
            source,
            "A::run",
            "possible-calls",
            "direct-scoped-name",
        );
        assert_call(
            &analysis,
            source,
            "B::run",
            "possible-calls",
            "direct-scoped-name",
        );
        assert_unresolved_call(&analysis, source, "ambiguous-target");
    }
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

fn assert_call(
    analysis: &Analysis,
    source_qualified: &str,
    target_qualified: &str,
    relation: &str,
    evidence: &str,
) {
    let source = node(analysis, source_qualified);
    let target = node(analysis, target_qualified);
    let edge = analysis
        .records
        .iter()
        .find_map(|record| match record {
            FactRecord::Edge(edge)
                if edge.relation == relation
                    && edge.source == source.id
                    && edge.target == target.id =>
            {
                Some(edge)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing {relation} {source_qualified} -> {target_qualified}"));
    let attributes = edge.attributes.as_ref().and_then(Value::as_object).unwrap();
    let values = attributes
        .get("evidence")
        .and_then(Value::as_array)
        .unwrap();
    assert!(
        values.iter().any(|value| value.as_str() == Some(evidence)),
        "evidence for {source_qualified} -> {target_qualified} = {values:?}"
    );
    assert_eq!(
        attributes.get("resolution").and_then(Value::as_str),
        Some(if relation == "calls" {
            "definite"
        } else {
            "possible"
        })
    );
}

fn assert_call_kind(
    analysis: &Analysis,
    source_qualified: &str,
    target_kind: &str,
    target_qualified: &str,
    relation: &str,
    evidence: &str,
) {
    let source = node(analysis, source_qualified);
    let target = analysis
        .records
        .iter()
        .find_map(|record| match record {
            FactRecord::Node(node)
                if node.kind == target_kind && node.qualified_name == target_qualified =>
            {
                Some(node)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing {target_kind} {target_qualified}"));
    let edge = analysis
        .records
        .iter()
        .find_map(|record| match record {
            FactRecord::Edge(edge)
                if edge.relation == relation
                    && edge.source == source.id
                    && edge.target == target.id =>
            {
                Some(edge)
            }
            _ => None,
        })
        .unwrap_or_else(|| {
            panic!("missing {relation} {source_qualified} -> {target_kind} {target_qualified}")
        });
    let values = edge
        .attributes
        .as_ref()
        .and_then(Value::as_object)
        .and_then(|attributes| attributes.get("evidence"))
        .and_then(Value::as_array)
        .unwrap();
    assert!(values.iter().any(|value| value.as_str() == Some(evidence)));
}

fn assert_no_call_relation(analysis: &Analysis, source_qualified: &str, relation: &str) {
    let source = node(analysis, source_qualified);
    assert!(!analysis.records.iter().any(|record| matches!(
        record,
        FactRecord::Edge(edge) if edge.relation == relation && edge.source == source.id
    )));
}

fn assert_unresolved_call(analysis: &Analysis, source_qualified: &str, reason: &str) {
    let source = node(analysis, source_qualified);
    assert!(analysis.records.iter().any(|record| matches!(
        record,
        FactRecord::Unresolved(value)
            if value.relation == "calls"
                && value.source == source.id
                && value.reason == reason
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
