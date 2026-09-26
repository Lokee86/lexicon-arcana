mod dependencies;
mod discovery;
mod facts;
mod identities;
mod module_ownership;
mod protocol;
mod protocol_records;
mod semantic_call_contract_targets;
mod semantic_call_facts;
mod semantic_call_target_support;
mod semantic_call_targets;
mod semantic_capture_facts;
mod semantic_dataflow_facts;
mod semantic_fact_index;
mod semantic_facts;
mod semantic_facts_support;
mod semantic_relationship_facts;
mod semantic_ssa_target_support;

#[cfg(test)]
mod anonymous_interface_tests;
#[cfg(test)]
mod capture_position_tests;
#[cfg(test)]
mod capture_tests;
#[cfg(test)]
mod dataflow_tests;
#[cfg(test)]
mod dependencies_tests;
#[cfg(test)]
mod differential_compare;
#[cfg(test)]
mod differential_tests;
#[cfg(test)]
mod discovery_boundary_tests;
#[cfg(test)]
mod discovery_tests;
#[cfg(test)]
mod identities_tests;
#[cfg(test)]
mod incremental_tests;
#[cfg(test)]
mod relationship_tests;
#[cfg(test)]
mod semantic_parity_tests;
#[cfg(test)]
mod ssa_tests;
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
                ("dependencies.rs", include_bytes!("dependencies.rs")),
                ("discovery.rs", include_bytes!("discovery.rs")),
                ("identities.rs", include_bytes!("identities.rs")),
                ("module_ownership.rs", include_bytes!("module_ownership.rs")),
                ("protocol.rs", include_bytes!("protocol.rs")),
                ("protocol_records.rs", include_bytes!("protocol_records.rs")),
                ("facts.rs", include_bytes!("facts.rs")),
                (
                    "semantic_call_contract_targets.rs",
                    include_bytes!("semantic_call_contract_targets.rs"),
                ),
                (
                    "semantic_call_facts.rs",
                    include_bytes!("semantic_call_facts.rs"),
                ),
                (
                    "semantic_call_target_support.rs",
                    include_bytes!("semantic_call_target_support.rs"),
                ),
                (
                    "semantic_call_targets.rs",
                    include_bytes!("semantic_call_targets.rs"),
                ),
                (
                    "semantic_capture_facts.rs",
                    include_bytes!("semantic_capture_facts.rs"),
                ),
                (
                    "semantic_dataflow_facts.rs",
                    include_bytes!("semantic_dataflow_facts.rs"),
                ),
                (
                    "semantic_fact_index.rs",
                    include_bytes!("semantic_fact_index.rs"),
                ),
                ("semantic_facts.rs", include_bytes!("semantic_facts.rs")),
                (
                    "semantic_facts_support.rs",
                    include_bytes!("semantic_facts_support.rs"),
                ),
                (
                    "semantic_relationship_facts.rs",
                    include_bytes!("semantic_relationship_facts.rs"),
                ),
                (
                    "semantic_ssa_target_support.rs",
                    include_bytes!("semantic_ssa_target_support.rs"),
                ),
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
        let discovery_started = crate::perf::start();
        let inventory = discovery::discover(&repository)?;
        if let Some(discovery_started) = discovery_started {
            crate::perf::emit(
                "go.repository_discovery",
                discovery_started.elapsed(),
                &[
                    ("discovered_files", inventory.files.len() as u64),
                    (
                        "retained_source_bytes",
                        inventory
                            .files
                            .iter()
                            .map(|file| file.content.len() as u64)
                            .sum(),
                    ),
                    ("modules", inventory.modules.len() as u64),
                ],
            );
        }
        let wire = semantic_request(&repository, &inventory, request)?;
        let response: protocol::Response = self.helper.run_json(
            &repository,
            &helper_arguments(),
            &helper_environment(),
            protocol::PROTOCOL_VERSION,
            &wire,
        )?;
        debug_assert_eq!(response.protocol_version, protocol::PROTOCOL_VERSION);
        facts::structural_analysis(request, &inventory, &response.records)
    }
}

fn semantic_request(
    repository: &Path,
    inventory: &discovery::Inventory,
    request: &AdapterRequest,
) -> Result<protocol::Request, AdapterError> {
    Ok(protocol::Request::new(
        path_string(repository)?,
        inventory.semantic_files(),
        inventory
            .modules
            .iter()
            .map(|module| protocol::Module {
                root: module.root.clone(),
                path: module.path.clone(),
            })
            .collect(),
        request.workers,
        request.shards,
        request.merge_fan_in,
    ))
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
