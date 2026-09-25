mod dependencies;
mod discovery;
mod extract;
mod facts;
mod model;
mod resolve;
mod semantic;
mod source;

use crate::{
    AdapterError, AdapterMode, AdapterRequest, Analysis, FACT_SCHEMA_VERSION, FactHeader,
    LanguageAdapter,
};

use discovery::discover;
use extract::extract_repository;
use facts::Facts;
use resolve::resolve;

pub const ADAPTER_VERSION: &str = "0.5.0";

#[derive(Debug, Default)]
pub struct PythonAdapter;

impl LanguageAdapter for PythonAdapter {
    fn implementation_version(&self) -> &'static str {
        ADAPTER_VERSION
    }

    fn implementation_fingerprint(&self) -> String {
        super::fingerprint::source_fingerprint(
            ADAPTER_VERSION,
            &[
                ("mod.rs", include_bytes!("mod.rs")),
                ("dependencies.rs", include_bytes!("dependencies.rs")),
                ("discovery.rs", include_bytes!("discovery.rs")),
                ("extract.rs", include_bytes!("extract.rs")),
                ("extract/dataflow.rs", include_bytes!("extract/dataflow.rs")),
                (
                    "extract/declarations.rs",
                    include_bytes!("extract/declarations.rs"),
                ),
                (
                    "extract/expressions.rs",
                    include_bytes!("extract/expressions.rs"),
                ),
                ("extract/imports.rs", include_bytes!("extract/imports.rs")),
                (
                    "extract/statements.rs",
                    include_bytes!("extract/statements.rs"),
                ),
                ("facts.rs", include_bytes!("facts.rs")),
                ("model.rs", include_bytes!("model.rs")),
                ("resolve.rs", include_bytes!("resolve.rs")),
                ("resolve/bindings.rs", include_bytes!("resolve/bindings.rs")),
                (
                    "resolve/relationships.rs",
                    include_bytes!("resolve/relationships.rs"),
                ),
                ("resolve/shapes.rs", include_bytes!("resolve/shapes.rs")),
                (
                    "resolve/calls/mod.rs",
                    include_bytes!("resolve/calls/mod.rs"),
                ),
                (
                    "resolve/calls/annotation.rs",
                    include_bytes!("resolve/calls/annotation.rs"),
                ),
                (
                    "resolve/calls/callbacks.rs",
                    include_bytes!("resolve/calls/callbacks.rs"),
                ),
                (
                    "resolve/calls/dispatch.rs",
                    include_bytes!("resolve/calls/dispatch.rs"),
                ),
                (
                    "resolve/calls/expression.rs",
                    include_bytes!("resolve/calls/expression.rs"),
                ),
                (
                    "resolve/calls/scope.rs",
                    include_bytes!("resolve/calls/scope.rs"),
                ),
                ("semantic/mod.rs", include_bytes!("semantic/mod.rs")),
                (
                    "semantic/error_flow.rs",
                    include_bytes!("semantic/error_flow.rs"),
                ),
                (
                    "semantic/handlers.rs",
                    include_bytes!("semantic/handlers.rs"),
                ),
                (
                    "semantic/outcomes.rs",
                    include_bytes!("semantic/outcomes.rs"),
                ),
                ("source.rs", include_bytes!("source.rs")),
            ],
        )
    }

    fn analyze(&self, request: &AdapterRequest) -> Result<Analysis, AdapterError> {
        if request.language != "python" {
            return Err(AdapterError::new(format!(
                "python adapter received language {:?}",
                request.language
            )));
        }

        let repository = discover(&request.repository)?;
        let mut facts = Facts::new(repository.name.clone());
        extract_repository(&repository, &mut facts)?;
        semantic::emit_semantic_facts(&repository, &mut facts);
        resolve(&mut facts);
        semantic::emit_outcome_facts(&facts.calls.clone(), &mut facts);
        dependencies::add_dependency_facts(&repository, &mut facts);

        let incremental = request.mode == AdapterMode::Incremental;
        let header = FactHeader {
            adapter_version: ADAPTER_VERSION.into(),
            changed_files: incremental.then(|| normalized(&request.changed_files)),
            language: "python".into(),
            mode: incremental.then(|| "incremental".into()),
            record: "lexicon".into(),
            removed_files: incremental.then(|| normalized(&request.removed_files)),
            repository: repository.name,
            schema_version: FACT_SCHEMA_VERSION,
            shared_complete: incremental.then_some(true),
        };
        Ok(Analysis::new(header, facts.into_records()))
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
