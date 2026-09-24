#![allow(dead_code)]

use std::fs;
use std::path::Path;

use lexicon::SourceSpan;
use lexicon::interstack::{Node, ResolveResult};

pub fn write_fixture(root: &Path, relative: &str, content: &str) {
    let path = root.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
    fs::create_dir_all(path.parent().expect("fixture parent")).unwrap();
    fs::write(path, content).unwrap();
}

pub fn test_id(character: char) -> String {
    format!("sha256:{}", character.to_string().repeat(64))
}

pub fn file_node(character: char, path: &str) -> Node {
    Node {
        id: test_id(character),
        kind: "file".into(),
        name: Path::new(path)
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap()
            .into(),
        path: path.into(),
        qualified_name: path.into(),
        span: None,
        attributes: Default::default(),
    }
}

pub fn callable_node(character: char, kind: &str, name: &str, path: &str, line: u64) -> Node {
    Node {
        id: test_id(character),
        kind: kind.into(),
        name: name.into(),
        path: path.into(),
        qualified_name: name.into(),
        span: Some(SourceSpan {
            path: path.into(),
            start_line: line,
            start_column: 1,
            end_line: line,
            end_column: 2,
        }),
        attributes: Default::default(),
    }
}

pub fn qualified_callable(
    character: char,
    kind: &str,
    name: &str,
    qualified_name: &str,
    path: &str,
    line: u64,
) -> Node {
    let mut node = callable_node(character, kind, name, path, line);
    node.qualified_name = qualified_name.into();
    node
}

pub fn node_id(result: &ResolveResult, kind: &str, name: &str) -> String {
    result
        .nodes
        .iter()
        .find(|node| node.kind == kind && node.name == name)
        .unwrap_or_else(|| panic!("missing {kind} node {name:?}"))
        .id
        .clone()
}

pub fn assert_edge(result: &ResolveResult, source: &str, target: &str, relation: &str) {
    assert!(
        result.edges.iter().any(|edge| {
            edge.source == source && edge.target == target && edge.relation == relation
        }),
        "missing {relation} edge {source} -> {target}"
    );
}
