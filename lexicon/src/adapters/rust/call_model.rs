use crate::adapters::rust::model::ValueSet;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Default)]
pub(crate) struct CallResolution {
    pub(crate) targets: BTreeSet<String>,
    pub(crate) possible: bool,
    pub(crate) reason: Option<&'static str>,
    pub(crate) return_value: ValueSet,
}

#[derive(Clone)]
pub(crate) struct CallEvent {
    pub(crate) expression: String,
    pub(crate) span: Option<Value>,
    pub(crate) resolution: CallResolution,
}

#[derive(Clone, Default)]
pub(crate) struct AnalysisResult {
    pub(crate) return_value: ValueSet,
    pub(crate) parameter_updates: BTreeMap<(String, usize), ValueSet>,
    pub(crate) capture_updates: BTreeMap<(String, String), ValueSet>,
    pub(crate) calls: Vec<CallEvent>,
}
