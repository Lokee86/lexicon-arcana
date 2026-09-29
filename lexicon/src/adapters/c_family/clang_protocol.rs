use serde::{Deserialize, Serialize};

use crate::adapters::frontend::ProtocolResponse;

pub(crate) const PROTOCOL_VERSION: u32 = 1;
pub(crate) const HELPER_VERSION: &str = include_str!("../../../adapters/c-family-clang/VERSION");

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CapabilitiesRequest {
    pub protocol_version: u32,
    pub operation: &'static str,
    pub repository_root: String,
}

impl CapabilitiesRequest {
    pub(crate) fn new(repository_root: String) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            operation: "capabilities",
            repository_root,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CapabilitiesResponse {
    pub protocol_version: u32,
    pub helper_version: String,
    pub clang_version: String,
    pub capabilities: Vec<String>,
    pub compilation_database: bool,
    #[serde(default)]
    pub compilation_database_error: Option<String>,
}

impl ProtocolResponse for CapabilitiesResponse {
    fn protocol_version(&self) -> u32 {
        self.protocol_version
    }
}
