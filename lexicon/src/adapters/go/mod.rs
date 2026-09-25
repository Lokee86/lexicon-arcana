mod facts;
mod protocol;

#[cfg(test)]
mod tests;

use std::{collections::BTreeMap, ffi::OsString, path::Path};

use crate::{AdapterError, AdapterRequest, Analysis, LanguageAdapter};

use super::helper::HelperRunner;

pub const ADAPTER_VERSION: &str = "0.1.0";

const HELPER_DIRECTORY: &str = "go-semantic";
const HELPER_EXECUTABLE: &str = "lexicon-go-semantic";
const HELPER_ENVIRONMENT: &str = "LEXICON_GO_SEMANTIC_HELPER";

#[derive(Debug, Clone)]
pub struct GoAdapter {
    helper: HelperRunner,
}

impl GoAdapter {
    pub(crate) fn new(adapter_root: &Path) -> Self {
        Self {
            helper: HelperRunner::discover(
                adapter_root,
                HELPER_DIRECTORY,
                HELPER_EXECUTABLE,
                HELPER_ENVIRONMENT,
            ),
        }
    }

    #[cfg(test)]
    fn with_helper(helper: HelperRunner) -> Self {
        Self { helper }
    }
}

impl LanguageAdapter for GoAdapter {
    fn implementation_version(&self) -> &'static str {
        ADAPTER_VERSION
    }

    fn implementation_fingerprint(&self) -> String {
        super::fingerprint::source_fingerprint(
            ADAPTER_VERSION,
            &[
                ("mod.rs", include_bytes!("mod.rs")),
                ("protocol.rs", include_bytes!("protocol.rs")),
                ("facts.rs", include_bytes!("facts.rs")),
                ("../helper.rs", include_bytes!("../helper.rs")),
                (
                    "../helper_capture.rs",
                    include_bytes!("../helper_capture.rs"),
                ),
                ("go-helper-version", protocol::HELPER_VERSION.as_bytes()),
            ],
        )
    }

    fn analyze(&self, request: &AdapterRequest) -> Result<Analysis, AdapterError> {
        if request.language != "go" {
            return Err(AdapterError::new("go adapter received another language"));
        }
        let repository = std::fs::canonicalize(&request.repository).map_err(|error| {
            AdapterError::new(format!(
                "cannot resolve repository {}: {error}",
                request.repository.display()
            ))
        })?;
        let wire = protocol::Request::shell(
            path_string(&repository)?,
            request.workers,
            request.shards,
            request.merge_fan_in,
        );
        let response: protocol::Response = self.helper.run_json(
            &repository,
            &helper_arguments(),
            &helper_environment(),
            protocol::PROTOCOL_VERSION,
            &wire,
        )?;
        debug_assert_eq!(response.protocol_version, protocol::PROTOCOL_VERSION);
        if !response.records.is_empty() {
            return Err(AdapterError::new(
                "Go semantic helper records are not materialized until the extraction phase",
            ));
        }
        Ok(facts::empty_analysis(&repository, request))
    }
}

fn helper_arguments() -> Vec<OsString> {
    vec![
        OsString::from("--protocol-version"),
        OsString::from(protocol::PROTOCOL_VERSION.to_string()),
    ]
}

fn helper_environment() -> BTreeMap<OsString, OsString> {
    BTreeMap::from([
        (OsString::from("LEXICON_HELPER"), OsString::from("go")),
        (
            OsString::from("LEXICON_HELPER_PROTOCOL"),
            OsString::from(protocol::PROTOCOL_VERSION.to_string()),
        ),
    ])
}

fn path_string(path: &Path) -> Result<String, AdapterError> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| AdapterError::new("repository path is not valid UTF-8"))
}
