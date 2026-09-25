//! Native C/C++ adapter foundation.
//!
//! C and C++ share one repository model and one stable `c-family` identity
//! namespace. The current native slice owns discovery, language selection,
//! parsing, repository/declaration extraction, and typed foundation facts.
//! Cross-file semantic relationships are layered on later.

mod call_candidates;
mod call_facts;
mod call_references;
mod callables;
mod declaration_helpers;
mod declaration_records;
mod declarations;
mod discovery;
mod expressions;
mod facts;
mod include_facts;
mod includes;
mod indirect_calls;
mod language;
mod macro_declarations;
mod macro_expanded_call;
mod macro_fact_records;
mod macro_facts;
mod macro_resolution;
mod macro_substitution;
mod macro_syntax;
mod model;
#[cfg(test)]
mod model_tests;
mod parser;
mod pointer_aliases;
mod pointer_bindings;
mod receiver_resolution;
mod relationship_facts;
mod resolution;
#[cfg(test)]
mod resolution_tests;
mod syntax;
mod type_declarations;
mod visibility;
#[cfg(test)]
mod visibility_tests;

use crate::{AdapterError, AdapterRequest, Analysis, LanguageAdapter};

pub const ADAPTER_VERSION: &str = "0.5.0";

#[derive(Debug, Default)]
pub struct CFamilyAdapter;

impl LanguageAdapter for CFamilyAdapter {
    fn implementation_version(&self) -> &'static str {
        ADAPTER_VERSION
    }

    fn implementation_fingerprint(&self) -> String {
        super::fingerprint::source_fingerprint(
            ADAPTER_VERSION,
            &[
                ("mod.rs", include_bytes!("mod.rs")),
                ("call_candidates.rs", include_bytes!("call_candidates.rs")),
                ("call_facts.rs", include_bytes!("call_facts.rs")),
                ("call_references.rs", include_bytes!("call_references.rs")),
                ("callables.rs", include_bytes!("callables.rs")),
                (
                    "declaration_helpers.rs",
                    include_bytes!("declaration_helpers.rs"),
                ),
                (
                    "declaration_records.rs",
                    include_bytes!("declaration_records.rs"),
                ),
                ("declarations.rs", include_bytes!("declarations.rs")),
                ("discovery.rs", include_bytes!("discovery.rs")),
                ("expressions.rs", include_bytes!("expressions.rs")),
                ("facts.rs", include_bytes!("facts.rs")),
                ("include_facts.rs", include_bytes!("include_facts.rs")),
                ("includes.rs", include_bytes!("includes.rs")),
                ("indirect_calls.rs", include_bytes!("indirect_calls.rs")),
                ("language.rs", include_bytes!("language.rs")),
                (
                    "macro_declarations.rs",
                    include_bytes!("macro_declarations.rs"),
                ),
                (
                    "macro_expanded_call.rs",
                    include_bytes!("macro_expanded_call.rs"),
                ),
                (
                    "macro_fact_records.rs",
                    include_bytes!("macro_fact_records.rs"),
                ),
                ("macro_facts.rs", include_bytes!("macro_facts.rs")),
                ("macro_resolution.rs", include_bytes!("macro_resolution.rs")),
                (
                    "macro_substitution.rs",
                    include_bytes!("macro_substitution.rs"),
                ),
                ("macro_syntax.rs", include_bytes!("macro_syntax.rs")),
                ("model.rs", include_bytes!("model.rs")),
                ("parser.rs", include_bytes!("parser.rs")),
                ("pointer_aliases.rs", include_bytes!("pointer_aliases.rs")),
                ("pointer_bindings.rs", include_bytes!("pointer_bindings.rs")),
                (
                    "receiver_resolution.rs",
                    include_bytes!("receiver_resolution.rs"),
                ),
                (
                    "relationship_facts.rs",
                    include_bytes!("relationship_facts.rs"),
                ),
                ("resolution.rs", include_bytes!("resolution.rs")),
                ("syntax.rs", include_bytes!("syntax.rs")),
                (
                    "type_declarations.rs",
                    include_bytes!("type_declarations.rs"),
                ),
                ("visibility.rs", include_bytes!("visibility.rs")),
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

        let model = parser::parse_repository(&repository)?;
        Ok(facts::analysis(request, model))
    }
}
