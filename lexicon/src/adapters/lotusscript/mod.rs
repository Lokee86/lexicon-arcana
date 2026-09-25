mod calls;
mod dataflow;
mod declarations;
mod dxl;
mod facts;
mod model;
mod parser;
mod repository;
mod resolve_calls;
mod scope;
mod syntax;
mod variables;

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::json;

use crate::{
    AdapterError, AdapterRequest, Analysis, FACT_SCHEMA_VERSION, FactHeader, LanguageAdapter,
};

use facts::Facts;
use model::AnalysisState;

pub const ADAPTER_VERSION: &str = "0.3.0";

#[derive(Debug, Default)]
pub struct LotusScriptAdapter;

impl LanguageAdapter for LotusScriptAdapter {
    fn implementation_version(&self) -> &'static str {
        ADAPTER_VERSION
    }

    fn implementation_fingerprint(&self) -> String {
        super::fingerprint::source_fingerprint(
            ADAPTER_VERSION,
            &[
                ("mod.rs", include_bytes!("mod.rs")),
                ("calls.rs", include_bytes!("calls.rs")),
                ("dataflow.rs", include_bytes!("dataflow.rs")),
                ("declarations.rs", include_bytes!("declarations.rs")),
                ("dxl.rs", include_bytes!("dxl.rs")),
                ("facts.rs", include_bytes!("facts.rs")),
                ("model.rs", include_bytes!("model.rs")),
                ("parser.rs", include_bytes!("parser.rs")),
                ("repository.rs", include_bytes!("repository.rs")),
                ("resolve_calls.rs", include_bytes!("resolve_calls.rs")),
                ("scope.rs", include_bytes!("scope.rs")),
                ("syntax.rs", include_bytes!("syntax.rs")),
                ("variables.rs", include_bytes!("variables.rs")),
            ],
        )
    }

    fn analyze(&self, request: &AdapterRequest) -> Result<Analysis, AdapterError> {
        if request.language != "lotusscript" {
            return Err(AdapterError::new(format!(
                "LotusScript adapter received language {:?}",
                request.language
            )));
        }
        let repository = repository::discover(&request.repository)?;
        let mut facts = Facts::default();
        let mut state = AnalysisState::default();

        let repository_id = facts.add_node(
            "repository",
            &repository.name,
            ".",
            &repository.name,
            &repository.name,
            None,
            None,
            None,
            None,
        );
        let directories = add_directories(
            &mut facts,
            &repository_id,
            &repository.name,
            &repository.directories,
        );

        for source in &repository.sources {
            let file_name = Path::new(&source.path)
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or(&source.path);
            let file_id = facts.add_node(
                "file",
                file_name,
                &source.path,
                &source.path,
                &source.path,
                Some(&source.path),
                None,
                None,
                Some(&source.raw_content_id),
            );
            facts.add_edge(
                parent_directory(&directories, &source.path),
                &file_id,
                "contains",
                Some(&source.path),
                None,
                None,
            );

            let module_name = Path::new(&source.path)
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or(file_name);
            let module_id = facts.add_node(
                "module",
                module_name,
                &source.path,
                &source.path,
                &source.path,
                Some(&source.path),
                None,
                Some(json!({"script_library": module_name})),
                None,
            );
            facts.add_edge(
                &file_id,
                &module_id,
                "contains",
                Some(&source.path),
                None,
                None,
            );

            let key = module_name.to_ascii_lowercase();
            state
                .modules_by_name
                .entry(key.clone())
                .or_default()
                .push(module_id.clone());
            state
                .module_paths_by_name
                .entry(key)
                .or_default()
                .push(source.path.clone());
            state
                .module_id_by_path
                .insert(source.path.clone(), module_id.clone());

            parser::parse_file(&mut state, &mut facts, source, &module_id);
        }

        state.resolve_uses(&mut facts);
        state.resolve_extends(&mut facts);
        state.resolve_calls(&mut facts);
        state.resolve_accesses(&mut facts);

        let incremental = request.mode == crate::AdapterMode::Incremental;
        Ok(Analysis::new(
            FactHeader {
                adapter_version: ADAPTER_VERSION.into(),
                changed_files: incremental.then(|| normalized(&request.changed_files)),
                language: "lotusscript".into(),
                mode: incremental.then(|| "incremental".into()),
                record: "lexicon".into(),
                removed_files: incremental.then(|| normalized(&request.removed_files)),
                repository: repository.name,
                schema_version: FACT_SCHEMA_VERSION,
                shared_complete: incremental.then_some(true),
            },
            facts.into_records(),
        ))
    }
}

fn add_directories(
    facts: &mut Facts,
    repository_id: &str,
    repository_name: &str,
    directories: &[String],
) -> BTreeMap<String, String> {
    let mut ids = BTreeMap::new();
    for directory in directories {
        let name = if directory == "." {
            repository_name
        } else {
            Path::new(directory)
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or(directory)
        };
        let id = facts.add_node(
            "directory",
            name,
            directory,
            directory,
            directory,
            None,
            None,
            None,
            None,
        );
        ids.insert(directory.clone(), id.clone());
        if directory == "." {
            facts.add_edge(repository_id, &id, "contains", None, None, None);
        }
    }
    for directory in directories.iter().filter(|value| value.as_str() != ".") {
        let parent = Path::new(directory)
            .parent()
            .map(|value| value.to_string_lossy().replace('\\', "/"))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| ".".into());
        if let (Some(parent), Some(child)) = (ids.get(&parent), ids.get(directory)) {
            facts.add_edge(parent, child, "contains", None, None, None);
        }
    }
    ids
}

fn parent_directory<'a>(directories: &'a BTreeMap<String, String>, path: &str) -> &'a str {
    let parent = Path::new(path)
        .parent()
        .map(|value| value.to_string_lossy().replace('\\', "/"))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| ".".into());
    directories
        .get(&parent)
        .or_else(|| directories.get("."))
        .map(String::as_str)
        .expect("repository root directory")
}

fn normalized(paths: &[String]) -> Vec<String> {
    let mut result = paths
        .iter()
        .map(|path| path.replace('\\', "/"))
        .collect::<Vec<_>>();
    result.sort();
    result.dedup();
    result
}
