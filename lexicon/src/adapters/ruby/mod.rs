//! Native Ruby semantic adapter. Parsing is performed by tree-sitter; no Ruby
//! runtime is required by the production analysis path.

mod dependencies;
mod parser;
mod semantic;

use crate::{AdapterError, AdapterRequest, Analysis, LanguageAdapter};

pub const ADAPTER_VERSION: &str = "0.3.0";

#[derive(Debug, Default)]
pub struct RubyAdapter;

impl LanguageAdapter for RubyAdapter {
    fn implementation_version(&self) -> &'static str {
        ADAPTER_VERSION
    }
    fn implementation_fingerprint(&self) -> String {
        super::fingerprint::source_fingerprint(
            ADAPTER_VERSION,
            &[
                ("mod.rs", include_bytes!("mod.rs")),
                ("parser.rs", include_bytes!("parser.rs")),
                ("parser_visit.rs", include_bytes!("parser_visit.rs")),
                ("parser_support.rs", include_bytes!("parser_support.rs")),
                ("semantic.rs", include_bytes!("semantic.rs")),
                (
                    "semantic_discovery.rs",
                    include_bytes!("semantic_discovery.rs"),
                ),
                (
                    "semantic_emission.rs",
                    include_bytes!("semantic_emission.rs"),
                ),
                (
                    "semantic_resolution.rs",
                    include_bytes!("semantic_resolution.rs"),
                ),
                ("semantic_syntax.rs", include_bytes!("semantic_syntax.rs")),
                ("semantic_support.rs", include_bytes!("semantic_support.rs")),
                ("dependencies.rs", include_bytes!("dependencies.rs")),
            ],
        )
    }
    fn analyze(&self, request: &AdapterRequest) -> Result<Analysis, AdapterError> {
        if request.language != "ruby" {
            return Err(AdapterError::new("ruby adapter received another language"));
        }
        parser::analyze(&request.repository, request)
    }
}
