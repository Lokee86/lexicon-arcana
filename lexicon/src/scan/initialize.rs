use crate::{AnalysisPlan, RecoveryOutcome, SnapshotManifest, interstack::refresh_interstack};

use super::engine::ScanEngine;
use super::engine_support::languages_in_tree;
use super::legacy::remove_legacy_library;
use super::{ScanExecutionError, ScanReport, execute_analysis_plans};

impl ScanEngine {
    pub(crate) fn initialize_full_locked(&self) -> Result<ScanReport, ScanExecutionError> {
        let head = self.git.head_option()?;
        match self.store.recover_pending(head.as_deref())? {
            RecoveryOutcome::NoPending
            | RecoveryOutcome::Discarded
            | RecoveryOutcome::Published(_) => {}
        }
        self.mirror.sync_all(&self.repository)?;
        remove_legacy_library(self.git.root())?;

        let languages = languages_in_tree(self.mirror.root())?
            .into_iter()
            .filter(|language| {
                crate::languages::language_enabled(language, &self.enabled_languages)
            })
            .collect::<Vec<_>>();
        let plans = languages
            .iter()
            .map(|language| AnalysisPlan {
                language: language.clone(),
                full: true,
                known_present: true,
                changed_files: Vec::new(),
                removed_files: Vec::new(),
                context_files: Vec::new(),
            })
            .collect::<Vec<_>>();
        let manifest = SnapshotManifest {
            version: crate::storage::SNAPSHOT_VERSION,
            state_commit: String::new(),
            languages: Some(Vec::new()),
        };
        let temporary = self.store.root().join("tmp");
        let manifest = execute_analysis_plans(
            &self.store,
            &self.host,
            self.mirror.root(),
            &temporary,
            manifest,
            &plans,
        )?;
        let (manifest, _) = refresh_interstack(&self.store, self.mirror.root(), manifest)?;
        let snapshot_id = self.commit_manifest(manifest)?;
        Ok(ScanReport {
            changed: Vec::new(),
            languages,
            snapshot_id,
        })
    }
}
