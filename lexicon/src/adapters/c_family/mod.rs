//! Native C/C++ adapter foundation.
//!
//! C and C++ share one repository model and one stable `c-family` identity
//! namespace. Step 1 owns discovery, language selection, parsing, and the
//! file/module fact foundation; semantic extraction is layered on later.

mod discovery;
mod facts;
mod language;
mod parser;

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
                ("discovery.rs", include_bytes!("discovery.rs")),
                ("language.rs", include_bytes!("language.rs")),
                ("parser.rs", include_bytes!("parser.rs")),
                ("facts.rs", include_bytes!("facts.rs")),
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

        let files = parser::parse_repository(&repository)?;
        Ok(facts::analysis(&repository, request, files))
    }
}
