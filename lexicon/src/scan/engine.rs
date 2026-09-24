use std::path::{Path, PathBuf};

use crate::{
    AdapterHost, Change, RecoveryOutcome, SnapshotManifest, SourceMirror, StateRepository,
    StorageError, Store,
    interstack::{interstack_drifted, refresh_interstack},
};

use super::engine_support::{adapter_fingerprints, languages_in_tree};
use super::legacy::{build_legacy_manifest, legacy_library_exists, remove_legacy_library};
use super::{PlanningInput, ScanExecutionError, execute_analysis_plans, plan_scan};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanReport {
    pub changed: Vec<Change>,
    pub languages: Vec<String>,
    pub snapshot_id: String,
}

pub struct ScanEngine {
    pub(super) repository: PathBuf,
    pub(super) git: StateRepository,
    pub(super) mirror: SourceMirror,
    pub(super) store: Store,
    pub(super) host: AdapterHost,
    pub(super) enabled_languages: Vec<String>,
}

impl ScanEngine {
    pub fn new(
        repository: impl Into<PathBuf>,
        git: StateRepository,
        store: Store,
        host: AdapterHost,
        enabled_languages: Vec<String>,
    ) -> Self {
        let mirror = SourceMirror::new(git.root().join("source"));
        Self {
            repository: repository.into(),
            git,
            mirror,
            store,
            host,
            enabled_languages,
        }
    }

    pub fn scan(&self) -> Result<ScanReport, ScanExecutionError> {
        self.scan_with(|mirror, repository| mirror.sync_all(repository))
    }

    pub fn scan_paths(&self, paths: &[PathBuf]) -> Result<ScanReport, ScanExecutionError> {
        self.scan_with(|mirror, repository| mirror.sync_paths(repository, paths))
    }

    pub fn store(&self) -> &Store {
        &self.store
    }

    pub fn state_repository(&self) -> &StateRepository {
        &self.git
    }

    fn scan_with(
        &self,
        synchronize: impl FnOnce(&SourceMirror, &Path) -> Result<(), crate::RepositoryError>,
    ) -> Result<ScanReport, ScanExecutionError> {
        let _guard = self.store.lock()?;
        let head = self.git.head_option()?;
        match self.store.recover_pending(head.as_deref())? {
            RecoveryOutcome::NoPending
            | RecoveryOutcome::Discarded
            | RecoveryOutcome::Published(_) => {}
        }
        self.git.reset_index()?;

        let (current_id, manifest) = self.load_manifest()?;
        let legacy_removed = remove_legacy_library(self.git.root())?;
        synchronize(&self.mirror, &self.repository)?;
        self.git.stage_source()?;
        let changes = self.git.source_changes()?;

        let present_languages = languages_in_tree(self.mirror.root())?;
        let fingerprints = adapter_fingerprints(&self.host, &manifest)?;
        let input = PlanningInput {
            changes: changes.clone(),
            present_languages,
            enabled_languages: self.enabled_languages.clone(),
            adapter_fingerprints: Some(fingerprints),
        };
        let plan = plan_scan(&self.store, &manifest, &input)?;

        let interstack_drift = interstack_drifted(&plan.manifest);
        if !plan.needs_work()
            && changes.is_empty()
            && !legacy_removed
            && !interstack_drift
            && let Some(id) = current_id
        {
            self.verify_current_state(&plan.manifest)?;
            return Ok(ScanReport {
                changed: Vec::new(),
                languages: Vec::new(),
                snapshot_id: id,
            });
        }

        let languages = plan.languages();
        let temporary = self.store.root().join("tmp");
        let manifest = execute_analysis_plans(
            &self.store,
            &self.host,
            self.mirror.root(),
            &temporary,
            plan.manifest,
            &plan.analyses,
        )?;
        let (manifest, _) = refresh_interstack(&self.store, self.mirror.root(), manifest)?;
        let snapshot_id = self.commit_manifest(manifest)?;
        Ok(ScanReport {
            changed: changes,
            languages,
            snapshot_id,
        })
    }

    pub(super) fn load_manifest(
        &self,
    ) -> Result<(Option<String>, SnapshotManifest), ScanExecutionError> {
        match self.store.current() {
            Ok((id, manifest)) => {
                if let Some(head) = self.git.head_option()?
                    && manifest.state_commit != head
                {
                    if legacy_library_exists(self.git.root())
                        && let Ok(migrated) =
                            build_legacy_manifest(&self.store, self.git.root(), &head, &self.host)
                    {
                        let id = self.store.publish(&migrated)?;
                        return Ok((Some(id), migrated));
                    }
                    return Err(ScanExecutionError::new(format!(
                        "Lexicon snapshot state {} does not match private state {} and no recoverable publication exists",
                        manifest.state_commit, head
                    )));
                }
                self.verify_current_state(&manifest)?;
                Ok((Some(id), manifest))
            }
            Err(StorageError::NoCurrentSnapshot) => {
                if let Some(head) = self.git.head_option()?
                    && legacy_library_exists(self.git.root())
                    && let Ok(migrated) =
                        build_legacy_manifest(&self.store, self.git.root(), &head, &self.host)
                {
                    let id = self.store.publish(&migrated)?;
                    return Ok((Some(id), migrated));
                }
                Ok((
                    None,
                    SnapshotManifest {
                        version: crate::storage::SNAPSHOT_VERSION,
                        state_commit: String::new(),
                        languages: Some(Vec::new()),
                    },
                ))
            }
            Err(error) => Err(error.into()),
        }
    }

    fn verify_current_state(&self, manifest: &SnapshotManifest) -> Result<(), ScanExecutionError> {
        let head = self.git.head_option()?;
        match head {
            Some(head) if manifest.state_commit != head => Err(ScanExecutionError::new(format!(
                "Lexicon snapshot state {} does not match private state {}",
                manifest.state_commit, head
            ))),
            None if !manifest.state_commit.is_empty() => Err(ScanExecutionError::new(
                "Lexicon snapshot references private state but the state repository has no commit",
            )),
            _ => Ok(()),
        }
    }

    pub(super) fn commit_manifest(
        &self,
        manifest: SnapshotManifest,
    ) -> Result<String, ScanExecutionError> {
        self.git.stage_all()?;
        let base = self.git.head_option()?.unwrap_or_default();
        let commit_required = !self.git.has_head() || self.git.has_staged_changes();
        let transaction = self
            .store
            .begin_scan_publication(&manifest, &base, commit_required)?;
        self.git.commit_state()?;
        let head = self.git.head()?;
        self.store
            .finish_scan_publication(transaction, &head)
            .map_err(ScanExecutionError::from)
    }
}
