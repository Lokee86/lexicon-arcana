mod support;

use std::fs;

use lexicon::{AdapterHost, AdapterRequest, Analysis, FactRecord};
use serde_json::Value;
use support::TestDirectory;

#[test]
fn extracts_c_and_cpp_declaration_surface() {
    let root = TestDirectory::new("c-family-declarations");
    write(
        &root,
        "include/model.hpp",
        r#"#define SCALE 2
namespace demo {
enum class Mode { A, B };
using Count = int;
class Item {
public:
  Item(int value);
  virtual int value() const;
private:
  int value_;
};
}
"#,
    );
    write(
        &root,
        "include/point.h",
        r#"typedef struct Point { int x; int y; } Point;
typedef int (*callback_fn)(int value);
struct callbacks { callback_fn run; };
int point_sum(const Point *point);
"#,
    );
    write(
        &root,
        "src/model.cpp",
        r#"static int helper(int value) {
  int local = value;
  return local;
}
const int LIMIT = 4;
"#,
    );

    let analysis = analyze(&root);
    assert_node(&analysis, "namespace", "demo");
    assert_attr(&analysis, "symbol", "SCALE", "macro", Value::Bool(true));
    assert_attr(
        &analysis,
        "symbol",
        "SCALE",
        "replacement",
        Value::String("2".into()),
    );

    assert_attr(
        &analysis,
        "type",
        "demo::Mode",
        "tag",
        Value::String("enum".into()),
    );
    assert_attr(
        &analysis,
        "constant",
        "demo::Mode::A",
        "enum_member",
        Value::Bool(true),
    );
    assert_attr(&analysis, "type", "demo::Count", "alias", Value::Bool(true));
    assert_attr(
        &analysis,
        "type",
        "demo::Item",
        "tag",
        Value::String("class".into()),
    );

    let constructor = node(&analysis, "constructor", "demo::Item::Item");
    assert_eq!(attr(constructor, "definition"), Some(&Value::Bool(false)));
    assert_eq!(attr(constructor, "parameter_count"), Some(&Value::from(1)));
    assert_defined_by(&analysis, "demo::Item", "demo::Item::Item");

    let method = node(&analysis, "method", "demo::Item::value");
    assert_eq!(attr(method, "virtual"), Some(&Value::Bool(true)));
    assert_eq!(attr(method, "parameter_count"), Some(&Value::from(0)));
    assert_defined_by(&analysis, "demo::Item", "demo::Item::value");
    assert_attr(
        &analysis,
        "field",
        "demo::Item::value_",
        "type",
        Value::String("int".into()),
    );

    assert_attr(
        &analysis,
        "type",
        "Point",
        "tag",
        Value::String("struct".into()),
    );
    assert_attr(
        &analysis,
        "type",
        "callback_fn",
        "function_pointer",
        Value::Bool(true),
    );
    assert_attr(
        &analysis,
        "field",
        "callbacks::run",
        "type",
        Value::String("callback_fn".into()),
    );

    assert_attr(
        &analysis,
        "function",
        "helper",
        "linkage",
        Value::String("internal".into()),
    );
    assert_attr(
        &analysis,
        "variable",
        "helper::local",
        "type",
        Value::String("int".into()),
    );
    assert_defined_by(&analysis, "helper", "helper::local");
    assert_node(&analysis, "constant", "LIMIT");
}

#[test]
fn overloads_prototypes_and_parameters_have_stable_distinct_identities() {
    let root = TestDirectory::new("c-family-overloads");
    write(
        &root,
        "main.cpp",
        r#"int select(int value);
int select(int first, int second);
int select(int value) { return value; }
"#,
    );

    let first = analyze(&root);
    let second = analyze(&root);
    assert_eq!(first.records, second.records);

    let select = first
        .records
        .iter()
        .filter_map(|record| match record {
            FactRecord::Node(node) if node.kind == "function" && node.name == "select" => {
                Some(node)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    // The one-parameter prototype and definition share a facts-v1 identity;
    // emission keeps the later definition, matching the Go oracle.
    assert_eq!(select.len(), 2);
    assert_eq!(
        select
            .iter()
            .map(|node| node.id.as_str())
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        2
    );
    assert_eq!(
        select
            .iter()
            .filter(|node| attr(node, "definition") == Some(&Value::Bool(true)))
            .count(),
        1
    );
    assert!(select.iter().any(|node| {
        attr(node, "parameter_count") == Some(&Value::from(1))
            && attr(node, "definition") == Some(&Value::Bool(true))
    }));
    assert!(select.iter().any(|node| {
        attr(node, "parameter_count") == Some(&Value::from(2))
            && attr(node, "definition") == Some(&Value::Bool(false))
    }));
}

#[test]
fn c_function_pointer_members_remain_fields() {
    let root = TestDirectory::new("c-family-pointer-field");
    write(
        &root,
        "callbacks.h",
        "struct callbacks { int (*run)(int value); };\n",
    );

    let analysis = analyze(&root);
    let field = node(&analysis, "field", "callbacks::run");
    assert_eq!(attr(field, "function_pointer"), Some(&Value::Bool(true)));
    assert!(!analysis.records.iter().any(|record| matches!(
        record,
        FactRecord::Node(value) if value.kind == "method" && value.name == "run"
    )));
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

fn assert_node(analysis: &Analysis, kind: &str, qualified: &str) {
    let _ = node(analysis, kind, qualified);
}

fn assert_attr(analysis: &Analysis, kind: &str, qualified: &str, key: &str, expected: Value) {
    assert_eq!(attr(node(analysis, kind, qualified), key), Some(&expected));
}

fn attr<'a>(node: &'a lexicon::NodeRecord, key: &str) -> Option<&'a Value> {
    node.attributes.as_ref()?.as_object()?.get(key)
}

fn assert_defined_by(analysis: &Analysis, container: &str, child: &str) {
    let container = analysis
        .records
        .iter()
        .find_map(|record| match record {
            FactRecord::Node(node) if node.qualified_name == container => Some(node),
            _ => None,
        })
        .unwrap();
    let child = analysis
        .records
        .iter()
        .find_map(|record| match record {
            FactRecord::Node(node) if node.qualified_name == child => Some(node),
            _ => None,
        })
        .unwrap();
    assert!(analysis.records.iter().any(|record| matches!(
        record,
        FactRecord::Edge(edge)
            if edge.relation == "defines"
                && edge.source == container.id
                && edge.target == child.id
    )));
}

fn write(root: &TestDirectory, relative: &str, content: &str) {
    let path = root
        .path
        .join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}
