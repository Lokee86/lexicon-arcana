use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::Analysis;

use super::{
    ADAPTER_CONTRACT_VERSION, AdapterError, AdapterRequest, LanguageAdapter, fingerprint,
    gdscript::GdscriptAdapter, generic::GenericAdapter, lotusscript::LotusScriptAdapter,
    python::PythonAdapter,
};

pub struct AdapterHost {
    root: PathBuf,
    adapters: BTreeMap<String, Arc<dyn LanguageAdapter>>,
    generic: Arc<dyn LanguageAdapter>,
}

impl AdapterHost {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let mut host = Self {
            root: root.into(),
            adapters: BTreeMap::new(),
            generic: Arc::new(GenericAdapter),
        };
        host.register("python", Arc::new(PythonAdapter));
        host.register("gdscript", Arc::new(GdscriptAdapter));
        host.register("lotusscript", Arc::new(LotusScriptAdapter));
        host
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn register(&mut self, language: impl Into<String>, adapter: Arc<dyn LanguageAdapter>) {
        self.adapters.insert(language.into(), adapter);
    }

    pub fn register_generic(&mut self, adapter: Arc<dyn LanguageAdapter>) {
        self.generic = adapter;
    }

    pub fn has_adapter(&self, language: &str) -> bool {
        self.adapters.contains_key(language) || crate::languages::is_generic(language)
    }

    pub fn fingerprint(&self, language: &str) -> Result<String, AdapterError> {
        let adapter = self.adapter(language)?;
        fingerprint::adapter_fingerprint(language, adapter.as_ref())
    }

    pub fn analyze(&self, request: &AdapterRequest) -> Result<Analysis, AdapterError> {
        let adapter = self.adapter(&request.language)?;

        let contract = adapter.contract();
        if contract.version != ADAPTER_CONTRACT_VERSION {
            return Err(AdapterError::new(format!(
                "{} adapter contract version {} is unsupported; expected {}",
                request.language, contract.version, ADAPTER_CONTRACT_VERSION
            )));
        }
        if contract.fact_schema_version != crate::FACT_SCHEMA_VERSION {
            return Err(AdapterError::new(format!(
                "{} adapter fact schema version {} is unsupported; expected {}",
                request.language,
                contract.fact_schema_version,
                crate::FACT_SCHEMA_VERSION
            )));
        }

        let mut analysis = adapter.analyze(request)?;
        analysis.restrict_incremental_ownership();
        analysis
            .canonicalize()
            .map_err(|error| AdapterError::new(error.to_string()))?;
        analysis
            .validate()
            .map_err(|error| AdapterError::new(error.to_string()))?;
        if analysis.header.language != request.language {
            return Err(AdapterError::new(format!(
                "{} adapter emitted language {:?}",
                request.language, analysis.header.language
            )));
        }
        Ok(analysis)
    }

    fn adapter(&self, language: &str) -> Result<&Arc<dyn LanguageAdapter>, AdapterError> {
        if let Some(adapter) = self.adapters.get(language) {
            return Ok(adapter);
        }
        if crate::languages::is_generic(language) {
            return Ok(&self.generic);
        }
        Err(AdapterError::new(format!(
            "no adapter registered for {language:?}"
        )))
    }
}
