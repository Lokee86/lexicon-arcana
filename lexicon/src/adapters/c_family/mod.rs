//! Native C/C++ adapter foundation.
//!
//! C and C++ share one repository model and one stable `c-family` identity
//! namespace. The current native slice owns discovery, language selection,
//! parsing, repository/declaration extraction, and typed foundation facts.
//! Cross-file semantic relationships are layered on later.

mod callables;
mod declaration_helpers;
mod declaration_records;
mod declarations;
mod discovery;
mod facts;
mod include_facts;
mod includes;
mod language;
mod macro_declarations;
mod model;
#[cfg(test)]
mod model_tests;
mod parser;
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
                ("facts.rs", include_bytes!("facts.rs")),
                ("include_facts.rs", include_bytes!("include_facts.rs")),
                ("includes.rs", include_bytes!("includes.rs")),
                ("language.rs", include_bytes!("language.rs")),
                (
                    "macro_declarations.rs",
                    include_bytes!("macro_declarations.rs"),
                ),
                ("model.rs", include_bytes!("model.rs")),
                ("parser.rs", include_bytes!("parser.rs")),
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
