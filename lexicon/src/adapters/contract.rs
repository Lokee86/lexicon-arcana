use crate::{Analysis, FACT_SCHEMA_VERSION};

use super::{AdapterError, AdapterRequest};

pub const ADAPTER_CONTRACT_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdapterContract {
    pub version: u32,
    pub fact_schema_version: u32,
}

impl AdapterContract {
    pub const CURRENT: Self = Self {
        version: ADAPTER_CONTRACT_VERSION,
        fact_schema_version: FACT_SCHEMA_VERSION,
    };
}

pub trait LanguageAdapter: Send + Sync {
    fn contract(&self) -> AdapterContract {
        AdapterContract::CURRENT
    }

    fn implementation_version(&self) -> &'static str;

    fn implementation_fingerprint(&self) -> String;

    fn analyze(&self, request: &AdapterRequest) -> Result<Analysis, AdapterError>;
}
