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
use extract::{ExtractionMetrics, extract_repository};
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
                ("extract/lifetime.rs", include_bytes!("extract/lifetime.rs")),
                ("extract/parallel.rs", include_bytes!("extract/parallel.rs")),
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

        crate::perf::emit(
            "python.execution_plan",
            std::time::Duration::ZERO,
            &[
                ("workers", request.workers as u64),
                ("logical_shards", request.shards as u64),
                ("merge_fan_in", request.merge_fan_in as u64),
            ],
        );

        let discovery_started = crate::perf::start();
        let repository = discover(&request.repository)?;
        if let Some(discovery_started) = discovery_started {
            let source_bytes = repository.files.iter().map(|file| file.size).sum::<u64>();
            crate::perf::emit(
                "python.adapter_discovery",
                discovery_started.elapsed(),
                &[
                    ("discovered_files", repository.files.len() as u64),
                    ("source_bytes", source_bytes),
                    ("retained_source_bytes", 0),
                    ("retained_files", 0),
                    ("retained_ast_files", 0),
                    ("file_descriptors", repository.files.len() as u64),
                ],
            );
        }

        let mut facts = Facts::new(repository.name.clone());
        let extraction_started = crate::perf::start();
        let extraction_metrics = extract_repository(
            &repository,
            &mut facts,
            request.workers,
            request.shards,
            request.merge_fan_in,
        )?;
        if let Some(extraction_started) = extraction_started {
            emit_python_state(
                "python.extraction",
                extraction_started.elapsed(),
                &facts,
                &extraction_metrics,
            );
        }

        let facts_before_resolution = emitted_fact_count(&facts);
        let resolution_started = crate::perf::start();
        resolve(&mut facts);
        if let Some(resolution_started) = resolution_started {
            let mut counters = python_state_counters(&facts);
            counters.push(("facts_before_resolution", facts_before_resolution as u64));
            counters.push(("facts_after_resolution", emitted_fact_count(&facts) as u64));
            crate::perf::emit(
                "python.repository_resolution",
                resolution_started.elapsed(),
                &counters,
            );
        }

        let final_emission_started = crate::perf::start();
        let call_record_clones = facts.calls.len() as u64;
        semantic::emit_outcome_facts(&facts.calls.clone(), &mut facts);
        dependencies::add_dependency_facts(&repository, &mut facts);
        if let Some(final_emission_started) = final_emission_started {
            crate::perf::emit(
                "python.final_fact_emission",
                final_emission_started.elapsed(),
                &[
                    ("final_fact_count", emitted_fact_count(&facts) as u64),
                    ("record_clones", call_record_clones),
                ],
            );
        }

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

fn emitted_fact_count(facts: &Facts) -> usize {
    facts.nodes.len() + facts.edges.len() + facts.unresolved.len()
}

fn python_state_counters(facts: &Facts) -> Vec<(&'static str, u64)> {
    let bindings = facts.module_bindings.len()
        + facts.scope_bindings.len()
        + facts.local_assignments.len()
        + facts.loop_bindings.len()
        + facts.data_symbols.len();
    let extraction_state_cardinality = facts.modules.len()
        + facts.symbols.len()
        + facts.qnames.len()
        + facts.imports.len()
        + facts.inheritances.len()
        + facts.functions.len()
        + facts.classes.len()
        + facts.lambda_ids.len()
        + facts.calls.len()
        + facts.local_assignments.len()
        + facts.loop_bindings.len()
        + facts.module_bindings.len()
        + facts.scope_bindings.len()
        + facts.scope_parents.len()
        + facts.data_symbols.len();

    vec![
        ("fact_count", emitted_fact_count(facts) as u64),
        (
            "extraction_state_cardinality",
            extraction_state_cardinality as u64,
        ),
        ("calls_retained", facts.calls.len() as u64),
        ("imports_retained", facts.imports.len() as u64),
        ("functions_retained", facts.functions.len() as u64),
        ("classes_retained", facts.classes.len() as u64),
        ("bindings_retained", bindings as u64),
    ]
}

fn emit_python_state(
    stage: &str,
    elapsed: std::time::Duration,
    facts: &Facts,
    metrics: &ExtractionMetrics,
) {
    let mut counters = python_state_counters(facts);
    counters.extend([
        (
            "peak_retained_source_bytes",
            metrics.peak_retained_source_bytes,
        ),
        ("peak_retained_files", metrics.peak_retained_files),
        ("peak_retained_ast_files", metrics.peak_retained_ast_files),
        ("extraction_workers", metrics.workers),
        ("extraction_shards", metrics.logical_shards),
    ]);
    crate::perf::emit(stage, elapsed, &counters);
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

#[cfg(test)]
mod tests;
