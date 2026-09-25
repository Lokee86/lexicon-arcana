mod call_emission;
mod dataflow;
mod declarations;
mod dependencies;
mod facts;
mod inheritance;
mod lexer;
mod model;
mod parser;
mod project;
mod relationships;
mod repository;
mod semantic;
mod syntax;

use crate::{
    AdapterError, AdapterRequest, Analysis, FACT_SCHEMA_VERSION, FactHeader, LanguageAdapter,
};

use facts::Facts;

pub const ADAPTER_VERSION: &str = "0.3.0";

#[derive(Debug, Default)]
pub struct GdscriptAdapter;

impl LanguageAdapter for GdscriptAdapter {
    fn implementation_version(&self) -> &'static str {
        ADAPTER_VERSION
    }

    fn implementation_fingerprint(&self) -> String {
        super::fingerprint::source_fingerprint(
            ADAPTER_VERSION,
            &[
                ("mod.rs", include_bytes!("mod.rs")),
                ("model.rs", include_bytes!("model.rs")),
                ("facts.rs", include_bytes!("facts.rs")),
                ("lexer.rs", include_bytes!("lexer.rs")),
                ("syntax.rs", include_bytes!("syntax.rs")),
                ("parser/mod.rs", include_bytes!("parser/mod.rs")),
                (
                    "parser/declarations.rs",
                    include_bytes!("parser/declarations.rs"),
                ),
                ("parser/calls.rs", include_bytes!("parser/calls.rs")),
                ("parser/imports.rs", include_bytes!("parser/imports.rs")),
                ("repository.rs", include_bytes!("repository.rs")),
                ("declarations.rs", include_bytes!("declarations.rs")),
                ("inheritance.rs", include_bytes!("inheritance.rs")),
                ("relationships.rs", include_bytes!("relationships.rs")),
                ("call_emission.rs", include_bytes!("call_emission.rs")),
                ("dataflow.rs", include_bytes!("dataflow.rs")),
                ("project.rs", include_bytes!("project.rs")),
                ("dependencies.rs", include_bytes!("dependencies.rs")),
                ("semantic/mod.rs", include_bytes!("semantic/mod.rs")),
                (
                    "semantic/bindings.rs",
                    include_bytes!("semantic/bindings.rs"),
                ),
                (
                    "semantic/expressions.rs",
                    include_bytes!("semantic/expressions.rs"),
                ),
                ("semantic/calls.rs", include_bytes!("semantic/calls.rs")),
                (
                    "semantic/callables.rs",
                    include_bytes!("semantic/callables.rs"),
                ),
                (
                    "semantic/callable_maps.rs",
                    include_bytes!("semantic/callable_maps.rs"),
                ),
            ],
        )
    }

    fn analyze(&self, request: &AdapterRequest) -> Result<Analysis, AdapterError> {
        if request.language != "gdscript" {
            return Err(AdapterError::new(format!(
                "gdscript adapter received language {:?}",
                request.language
            )));
        }

        let mut facts = Facts::new();
        let mut repository = repository::load(&request.repository, &mut facts)?;
        for file in &mut repository.files {
            declarations::process(&mut facts, file);
        }
        for file in &repository.files {
            inheritance::process_file(&mut facts, file);
        }
        inheritance::process_overrides(&mut facts);
        project::process_autoloads(&repository, &mut facts)?;
        dependencies::process_project(&repository, &mut facts)?;
        let model = semantic::SemanticModel::build(&facts, &repository.files);
        for index in 0..repository.files.len() {
            relationships::process_imports(&mut facts, &repository.files, index, &model);
            dataflow::process(&mut facts, &model, &repository.files, index);
            call_emission::process(&mut facts, &model, &repository.files, index);
        }

        let incremental = request.mode == crate::AdapterMode::Incremental;
        Ok(Analysis::new(
            FactHeader {
                adapter_version: ADAPTER_VERSION.into(),
                changed_files: incremental.then(|| normalized(&request.changed_files)),
                language: "gdscript".into(),
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

fn normalized(paths: &[String]) -> Vec<String> {
    let mut values = paths
        .iter()
        .map(|path| path.replace('\\', "/"))
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
}
