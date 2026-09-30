//! Native C/C++ adapter backed by the private Clang/LibTooling frontend.
//!
//! Clang owns C/C++ syntax and compiler semantics. Rust owns repository
//! discovery, canonical Lexicon identities, graph policy, fact materialization,
//! determinism, and publication.

mod clang_frontend;
mod clang_materialization;
mod clang_protocol;
mod discovery;
mod fact_dedup;
mod facts;
mod include_facts;
mod includes;
mod model;
mod relationship_facts;
mod resolution;
mod semantic_call_facts;
mod semantic_call_records;
mod semantic_dataflow_facts;
mod semantic_pointer_index;
mod visibility;

#[cfg(test)]
mod tests;

use std::path::Path;

use crate::{AdapterError, AdapterRequest, Analysis, LanguageAdapter};

pub const ADAPTER_VERSION: &str = "0.6.0";

#[derive(Debug, Clone)]
pub struct CFamilyAdapter {
    frontend: clang_frontend::ClangFrontend,
}

impl CFamilyAdapter {
    pub(crate) fn new(adapter_root: &Path) -> Self {
        Self {
            frontend: clang_frontend::ClangFrontend::discover(adapter_root),
        }
    }

    #[cfg(test)]
    pub(crate) fn with_frontend(frontend: clang_frontend::ClangFrontend) -> Self {
        Self { frontend }
    }
}

pub(crate) fn verify_runtime_helper(adapter_root: &Path) -> Result<(), AdapterError> {
    CFamilyAdapter::new(adapter_root)
        .frontend
        .resolve()
        .map(|_| ())
}

impl LanguageAdapter for CFamilyAdapter {
    fn implementation_version(&self) -> &'static str {
        ADAPTER_VERSION
    }

    fn implementation_fingerprint(&self) -> String {
        super::fingerprint::source_fingerprint(
            ADAPTER_VERSION,
            &[
                ("mod.rs", include_bytes!("mod.rs")),
                ("clang_frontend.rs", include_bytes!("clang_frontend.rs")),
                ("clang_protocol.rs", include_bytes!("clang_protocol.rs")),
                (
                    "clang_materialization.rs",
                    include_bytes!("clang_materialization.rs"),
                ),
                (
                    "clang_materialization/declarations.rs",
                    include_bytes!("clang_materialization/declarations.rs"),
                ),
                (
                    "clang_materialization/references.rs",
                    include_bytes!("clang_materialization/references.rs"),
                ),
                (
                    "clang_materialization/semantics.rs",
                    include_bytes!("clang_materialization/semantics.rs"),
                ),
                (
                    "clang_materialization/value_flow.rs",
                    include_bytes!("clang_materialization/value_flow.rs"),
                ),
                ("discovery.rs", include_bytes!("discovery.rs")),
                ("fact_dedup.rs", include_bytes!("fact_dedup.rs")),
                ("facts.rs", include_bytes!("facts.rs")),
                ("include_facts.rs", include_bytes!("include_facts.rs")),
                ("includes.rs", include_bytes!("includes.rs")),
                ("model.rs", include_bytes!("model.rs")),
                (
                    "relationship_facts.rs",
                    include_bytes!("relationship_facts.rs"),
                ),
                ("resolution.rs", include_bytes!("resolution.rs")),
                (
                    "semantic_call_facts.rs",
                    include_bytes!("semantic_call_facts.rs"),
                ),
                (
                    "semantic_call_records.rs",
                    include_bytes!("semantic_call_records.rs"),
                ),
                (
                    "semantic_dataflow_facts.rs",
                    include_bytes!("semantic_dataflow_facts.rs"),
                ),
                (
                    "semantic_pointer_index.rs",
                    include_bytes!("semantic_pointer_index.rs"),
                ),
                ("visibility.rs", include_bytes!("visibility.rs")),
                (
                    "clang-helper-version",
                    clang_protocol::HELPER_VERSION.as_bytes(),
                ),
                ("../frontend/mod.rs", include_bytes!("../frontend/mod.rs")),
                (
                    "../frontend/runner.rs",
                    include_bytes!("../frontend/runner.rs"),
                ),
                (
                    "../frontend/capture.rs",
                    include_bytes!("../frontend/capture.rs"),
                ),
            ],
        )
    }

    fn analyze(&self, request: &AdapterRequest) -> Result<Analysis, AdapterError> {
        if request.language != "c-family" {
            return Err(AdapterError::new(format!(
                "c-family adapter received language {:?}",
                request.language
            )));
        }

        let repository = request.repository.canonicalize().map_err(|error| {
            AdapterError::new(format!(
                "cannot resolve C-family repository {}: {error}",
                request.repository.display()
            ))
        })?;
        if !repository.is_dir() {
            return Err(AdapterError::new(
                "C-family repository path is not a directory",
            ));
        }

        let discovery_started = crate::perf::start();
        let files = discovery::collect_sources(&repository)?;
        if let Some(started) = discovery_started {
            crate::perf::emit(
                "c-family.repository_discovery",
                started.elapsed(),
                &[("discovered_files", files.len() as u64)],
            );
        }

        let frontend_started = crate::perf::start();
        let response = self.frontend.structural(&repository, files)?;
        if let Some(started) = frontend_started {
            crate::perf::emit(
                "c-family.frontend.semantic_analysis",
                started.elapsed(),
                &[
                    ("observed_files", response.files.len() as u64),
                    ("translation_units", response.translation_units.len() as u64),
                ],
            );
        }

        let materialization_started = crate::perf::start();
        let model = clang_materialization::materialize(&repository, &response)?;
        if let Some(started) = materialization_started {
            crate::perf::emit(
                "c-family.lexicon.materialization",
                started.elapsed(),
                &[("files", model.files.len() as u64)],
            );
        }
        Ok(facts::analysis(request, model))
    }
}
