use serde::{Deserialize, Serialize};

use crate::adapters::helper::ProtocolResponse;

use super::protocol_records::Record;

pub(crate) const PROTOCOL_VERSION: u32 = 1;
pub(crate) const HELPER_VERSION: &str = "0.13.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Request {
    pub protocol_version: u32,
    pub repository_root: String,
    pub files: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modules: Vec<Module>,
    pub execution: Execution,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Module {
    pub root: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Execution {
    pub workers: usize,
    pub shards: usize,
    pub merge_fan_in: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Response {
    pub protocol_version: u32,
    #[serde(default)]
    pub records: Vec<Record>,
}

impl ProtocolResponse for Response {
    fn protocol_version(&self) -> u32 {
        self.protocol_version
    }
}

impl Request {
    pub(crate) fn new(
        repository_root: String,
        files: Vec<String>,
        modules: Vec<Module>,
        workers: usize,
        shards: usize,
        fan_in: usize,
    ) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            repository_root,
            files,
            modules,
            execution: Execution {
                workers: workers.max(1),
                shards: shards.max(1),
                merge_fan_in: fan_in.max(2),
            },
        }
    }
}
