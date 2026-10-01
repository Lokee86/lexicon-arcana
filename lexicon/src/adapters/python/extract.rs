mod dataflow;
mod declarations;
mod expressions;
mod imports;
mod lifetime;
mod parallel;
mod statements;

use std::path::Path;

use serde_json::json;

use crate::{AdapterError, node_id};

use super::facts::Facts;
use super::model::{Repository, SourceFile};
use super::source::{generated, span};

pub use lifetime::ExtractionMetrics;

pub fn extract_repository(
    repository: &Repository,
    facts: &mut Facts,
    workers: usize,
    shards: usize,
    merge_fan_in: usize,
) -> Result<ExtractionMetrics, AdapterError> {
    add_structure(repository, facts);
    parallel::extract_repository(repository, facts, workers, shards, merge_fan_in)
}

fn add_structure(repository: &Repository, facts: &mut Facts) {
    let root = facts.add_node(
        "repository",
        &repository.name,
        ".",
        &repository.name,
        Some(&repository.name),
        None,
        None,
        None,
    );
    let mut directories = std::collections::BTreeMap::from([(".".to_owned(), root.clone())]);
    for path in repository
        .directories
        .iter()
        .filter(|path| path.as_str() != ".")
    {
        let name = Path::new(path)
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or(path);
        let id = facts.add_node("directory", name, path, path, Some(path), None, None, None);
        directories.insert(path.clone(), id);
    }
    for (path, id) in &directories {
        if path == "." {
            continue;
        }
        let parent = Path::new(path)
            .parent()
            .map(|value| value.to_string_lossy().replace('\\', "/"))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| ".".into());
        facts.add_edge(
            directories.get(&parent).unwrap_or(&root),
            id,
            "contains",
            None,
            None,
        );
    }
}

pub(super) fn extract_file(file: &SourceFile, facts: &mut Facts) -> Result<(), AdapterError> {
    let file_id = facts.add_node(
        "file",
        file.path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or(&file.relative),
        &file.relative,
        &file.relative,
        Some(&file.relative),
        None,
        None,
        Some(&file.bytes),
    );
    let parent = Path::new(&file.relative)
        .parent()
        .map(|value| value.to_string_lossy().replace('\\', "/"))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| ".".into());
    let parent_id = if parent == "." {
        node_id("python", "repository", &facts.repository)
    } else {
        node_id("python", "directory", &parent)
    };
    facts.add_edge(&parent_id, &file_id, "contains", None, None);

    let module_id = facts.add_node(
        "module",
        file.module.rsplit('.').next().unwrap_or(&file.module),
        &file.relative,
        &file.module,
        Some(&file.module),
        None,
        None,
        None,
    );
    facts.modules.insert(file.module.clone(), module_id.clone());
    facts.symbols.insert(file.module.clone(), module_id.clone());
    facts.add_edge(&file_id, &module_id, "contains", None, None);

    if let Some(error) = &file.parse_error {
        facts.add_unresolved(
            &module_id,
            "parses",
            &file.relative,
            "unsupported-form",
            None,
            Some(error.clone()),
        );
        return Ok(());
    }
    let Some(suite) = &file.suite else {
        return Ok(());
    };

    let mut visitor = Visitor {
        facts,
        file,
        owner: vec![module_id],
        lexical: Vec::new(),
        classes: Vec::new(),
        branch_depth: 0,
        direct_class_statement: false,
        import_index: 0,
        bare_call: None,
        semantic_outcomes_enabled: !generated(&file.source),
    };
    visitor.visit_suite(suite);
    Ok(())
}

struct Visitor<'a> {
    facts: &'a mut Facts,
    file: &'a SourceFile,
    owner: Vec<String>,
    lexical: Vec<(String, bool)>,
    classes: Vec<String>,
    branch_depth: usize,
    direct_class_statement: bool,
    import_index: usize,
    bare_call: Option<u32>,
    semantic_outcomes_enabled: bool,
}

impl Visitor<'_> {
    fn owner(&self) -> &str {
        self.owner.last().expect("module owner")
    }

    fn class_qname(&self) -> Option<&str> {
        self.classes.last().map(String::as_str)
    }

    fn attributes(
        &self,
        decorators: &[rustpython_parser::ast::Expr],
        is_async: bool,
    ) -> Option<serde_json::Value> {
        let mut map = serde_json::Map::new();
        if !decorators.is_empty() {
            let mut values = decorators
                .iter()
                .map(|value| super::source::expression_text(value, &self.file.source))
                .collect::<Vec<_>>();
            values.sort();
            map.insert("decorators".into(), json!(values));
        }
        if is_async {
            map.insert("async".into(), json!(true));
        }
        (!map.is_empty()).then_some(serde_json::Value::Object(map))
    }

    fn node_span<T: rustpython_parser::ast::Ranged>(&self, node: &T) -> Option<crate::SourceSpan> {
        span(node, self.file)
    }
}
