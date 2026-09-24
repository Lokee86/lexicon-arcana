use std::collections::BTreeMap;

use crate::SnapshotManifest;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub status: String,
    pub old: String,
    pub new: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalysisPlan {
    pub language: String,
    pub full: bool,
    pub known_present: bool,
    pub changed_files: Vec<String>,
    pub removed_files: Vec<String>,
    pub context_files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PlanningInput {
    pub changes: Vec<Change>,
    pub present_languages: Vec<String>,
    pub enabled_languages: Vec<String>,
    pub adapter_fingerprints: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanPlan {
    pub manifest: SnapshotManifest,
    pub analyses: Vec<AnalysisPlan>,
    pub pruned_disabled_languages: bool,
}

impl ScanPlan {
    pub fn needs_work(&self) -> bool {
        self.pruned_disabled_languages || !self.analyses.is_empty()
    }

    pub fn languages(&self) -> Vec<String> {
        self.analyses
            .iter()
            .map(|plan| plan.language.clone())
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageResult {
    pub language: String,
    pub entry: Option<crate::LanguageEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicationTransaction {
    pub(crate) manifest: SnapshotManifest,
    pub(crate) base_state_commit: String,
    pub(crate) commit_required: bool,
}

impl PublicationTransaction {
    pub fn base_state_commit(&self) -> &str {
        &self.base_state_commit
    }

    pub fn commit_required(&self) -> bool {
        self.commit_required
    }

    pub fn manifest(&self) -> &SnapshotManifest {
        &self.manifest
    }
}
