mod call_builtins;
mod call_model;
mod call_resolution;
mod call_support;
mod contract;
mod dataflow;
mod declaration_special;
mod declarations;
mod dependencies;
mod discovery;
mod expr_callbacks;
mod expr_calls;
mod expr_eval;
mod expr_values;
mod extractor;
mod flow;
mod function_index;
mod implementations;
mod imports;
mod items;
mod model;
mod orchestrator;
mod parser;
mod paths;
mod relationships;
mod resolve;
mod semantic;
mod semantic_actions;
mod semantic_error_flow;
mod semantic_facts;
mod semantic_outcomes;
mod syntax;
mod type_resolution;

use crate::{AdapterError, AdapterRequest, Analysis, LanguageAdapter};

pub const ADAPTER_VERSION: &str = "0.5.0";

#[derive(Debug, Default)]
pub struct RustAdapter;

impl LanguageAdapter for RustAdapter {
    fn implementation_version(&self) -> &'static str {
        ADAPTER_VERSION
    }

    fn implementation_fingerprint(&self) -> String {
        super::fingerprint::source_fingerprint(
            ADAPTER_VERSION,
            &[
                ("mod.rs", include_bytes!("mod.rs")),
                ("contract.rs", include_bytes!("contract.rs")),
                ("orchestrator.rs", include_bytes!("orchestrator.rs")),
                ("call_builtins.rs", include_bytes!("call_builtins.rs")),
                ("call_model.rs", include_bytes!("call_model.rs")),
                ("call_resolution.rs", include_bytes!("call_resolution.rs")),
                ("call_support.rs", include_bytes!("call_support.rs")),
                ("dataflow.rs", include_bytes!("dataflow.rs")),
                (
                    "declaration_special.rs",
                    include_bytes!("declaration_special.rs"),
                ),
                ("declarations.rs", include_bytes!("declarations.rs")),
                ("dependencies.rs", include_bytes!("dependencies.rs")),
                ("discovery.rs", include_bytes!("discovery.rs")),
                ("expr_callbacks.rs", include_bytes!("expr_callbacks.rs")),
                ("expr_calls.rs", include_bytes!("expr_calls.rs")),
                ("expr_eval.rs", include_bytes!("expr_eval.rs")),
                ("expr_values.rs", include_bytes!("expr_values.rs")),
                ("extractor.rs", include_bytes!("extractor.rs")),
                ("flow.rs", include_bytes!("flow.rs")),
                ("function_index.rs", include_bytes!("function_index.rs")),
                ("implementations.rs", include_bytes!("implementations.rs")),
                ("imports.rs", include_bytes!("imports.rs")),
                ("items.rs", include_bytes!("items.rs")),
                ("model.rs", include_bytes!("model.rs")),
                ("parser.rs", include_bytes!("parser.rs")),
                ("paths.rs", include_bytes!("paths.rs")),
                ("relationships.rs", include_bytes!("relationships.rs")),
                ("resolve.rs", include_bytes!("resolve.rs")),
                ("semantic.rs", include_bytes!("semantic.rs")),
                ("semantic_actions.rs", include_bytes!("semantic_actions.rs")),
                (
                    "semantic_error_flow.rs",
                    include_bytes!("semantic_error_flow.rs"),
                ),
                ("semantic_facts.rs", include_bytes!("semantic_facts.rs")),
                (
                    "semantic_outcomes.rs",
                    include_bytes!("semantic_outcomes.rs"),
                ),
                ("syntax.rs", include_bytes!("syntax.rs")),
                ("type_resolution.rs", include_bytes!("type_resolution.rs")),
            ],
        )
    }

    fn analyze(&self, request: &AdapterRequest) -> Result<Analysis, AdapterError> {
        if request.language != "rust" {
            return Err(AdapterError::new(format!(
                "Rust adapter received language {:?}",
                request.language
            )));
        }
        let changed = (request.mode == crate::AdapterMode::Incremental)
            .then_some(request.changed_files.as_slice());
        let removed = (request.mode == crate::AdapterMode::Incremental)
            .then_some(request.removed_files.as_slice());
        let repository = request.repository.canonicalize().map_err(|error| {
            AdapterError::new(format!(
                "cannot resolve repository {}: {error}",
                request.repository.display()
            ))
        })?;
        orchestrator::analyze(&repository, changed, removed)
            .map_err(|error| AdapterError::new(error.to_string()))
    }
}
