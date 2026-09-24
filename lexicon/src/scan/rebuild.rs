use crate::{
    AnalysisPlan, RecoveryOutcome,
    interstack::refresh_interstack,
    languages::{language_enabled, supported},
};

use super::engine::ScanEngine;
use super::engine_support::languages_in_tree;
use super::legacy::remove_legacy_library;
use super::planner::prune_disabled_languages;
use super::{ScanExecutionError, ScanReport, execute_analysis_plans};

impl ScanEngine {
    pub fn rebuild(&self, languages: &[String]) -> Result<ScanReport, ScanExecutionError> {
        let _guard = self.store.lock()?;
        let head = self.git.head_option()?;
        match self.store.recover_pending(head.as_deref())? {
            RecoveryOutcome::NoPending
            | RecoveryOutcome::Discarded
            | RecoveryOutcome::Published(_) => {}
        }
        self.git.reset_index()?;

        let (_, manifest) = self.load_manifest()?;
        remove_legacy_library(self.git.root())?;
        self.mirror.sync_all(&self.repository)?;
        self.git.stage_source()?;
        let changes = self.git.source_changes()?;
        let (manifest, _) = prune_disabled_languages(manifest, &self.enabled_languages);

        let languages = self.rebuild_languages(languages)?;
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
            changed: changes,
            languages,
            snapshot_id,
        })
    }

    fn rebuild_languages(&self, requested: &[String]) -> Result<Vec<String>, ScanExecutionError> {
        if requested.is_empty() {
            return Ok(languages_in_tree(self.mirror.root())?
                .into_iter()
                .filter(|language| language_enabled(language, &self.enabled_languages))
                .collect());
        }

        let mut languages = requested.to_vec();
        languages.sort();
        languages.dedup();
        for language in &languages {
            if !supported(language) {
                return Err(ScanExecutionError::new(format!(
                    "unsupported Lexicon language {language:?}"
                )));
            }
            if !language_enabled(language, &self.enabled_languages) {
                return Err(ScanExecutionError::new(format!(
                    "Lexicon language {language:?} is disabled"
                )));
            }
        }
        Ok(languages)
    }
}
