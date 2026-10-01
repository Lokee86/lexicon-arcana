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
        let scan_started = crate::perf::start();
        let lock_started = crate::perf::start();
        let _guard = self.store.lock()?;
        if let Some(lock_started) = lock_started {
            crate::perf::emit("scan.lock_wait", lock_started.elapsed(), &[]);
        }
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
        let change_detection_started = crate::perf::start();
        self.git.stage_source()?;
        let changes = self.git.source_changes()?;
        if let Some(change_detection_started) = change_detection_started {
            crate::perf::emit(
                "scan.source_change_detection",
                change_detection_started.elapsed(),
                &[("changed_files", changes.len() as u64)],
            );
        }

        let planning_started = crate::perf::start();
        let present_languages = languages_in_tree(self.mirror.root())?;
        let fingerprints = adapter_fingerprints(&self.host, &manifest)?;
        let input = PlanningInput {
            changes: changes.clone(),
            present_languages,
            enabled_languages: self.enabled_languages.clone(),
            adapter_fingerprints: Some(fingerprints),
            python_project_config_unchanged: super::python_config::project_config_unchanged(
                &self.git,
                self.mirror.root(),
                &changes,
            ),
        };
        let plan = plan_scan(&self.store, &manifest, &input)?;
        if let Some(planning_started) = planning_started {
            crate::perf::emit(
                "scan.planning",
                planning_started.elapsed(),
                &[
                    ("present_languages", input.present_languages.len() as u64),
                    ("analysis_plans", plan.analyses.len() as u64),
                    (
                        "full_analysis_plans",
                        plan.analyses
                            .iter()
                            .filter(|analysis| analysis.full)
                            .count() as u64,
                    ),
                ],
            );
        }

        let interstack_drift = interstack_drifted(&plan.manifest);
        if !plan.needs_work()
            && changes.is_empty()
            && !legacy_removed
            && !interstack_drift
            && let Some(id) = current_id
        {
            self.verify_current_state(&plan.manifest)?;
            if let Some(scan_started) = scan_started {
                crate::perf::emit(
                    "scan.total",
                    scan_started.elapsed(),
                    &[("analysis_plans", 0), ("published", 0)],
                );
            }
            return Ok(ScanReport {
                changed: Vec::new(),
                languages: Vec::new(),
                snapshot_id: id,
            });
        }

        let languages = plan.languages();
        let analysis_plan_count = plan.analyses.len() as u64;
        let temporary = self.store.root().join("tmp");
        let analysis_started = crate::perf::start();
        let manifest = execute_analysis_plans(
            &self.store,
            &self.host,
            self.mirror.root(),
            &temporary,
            plan.manifest,
            &plan.analyses,
        )?;
        if let Some(analysis_started) = analysis_started {
            crate::perf::emit(
                "scan.analysis",
                analysis_started.elapsed(),
                &[("analysis_plans", analysis_plan_count)],
            );
        }
        let (manifest, _) = refresh_interstack(&self.store, self.mirror.root(), manifest)?;
        let publication_started = crate::perf::start();
        let snapshot_id = self.commit_manifest(manifest)?;
        if let Some(publication_started) = publication_started {
            crate::perf::emit("scan.publication", publication_started.elapsed(), &[]);
        }
        if let Some(scan_started) = scan_started {
            crate::perf::emit(
                "scan.total",
                scan_started.elapsed(),
                &[("analysis_plans", analysis_plan_count), ("published", 1)],
            );
        }
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
        let staged = crate::perf::start();
        if staged.is_some() {
            crate::perf::emit(
                "scan.publication_stage_all_start",
                std::time::Duration::ZERO,
                &[],
            );
        }
        self.git.stage_all()?;
        if let Some(staged) = staged {
            crate::perf::emit("scan.publication_stage_all", staged.elapsed(), &[]);
        }

        let git_state = crate::perf::start();
        let base = self.git.head_option()?.unwrap_or_default();
        let commit_required = !self.git.has_head() || self.git.has_staged_changes();
        if let Some(git_state) = git_state {
            crate::perf::emit("scan.publication_git_state", git_state.elapsed(), &[]);
        }

        let pending = crate::perf::start();
        if pending.is_some() {
            crate::perf::emit(
                "scan.publication_pending_start",
                std::time::Duration::ZERO,
                &[],
            );
        }
        let transaction = self
            .store
            .begin_scan_publication(&manifest, &base, commit_required)?;
        if let Some(pending) = pending {
            crate::perf::emit("scan.publication_pending", pending.elapsed(), &[]);
        }

        let commit = crate::perf::start();
        if commit.is_some() {
            crate::perf::emit(
                "scan.publication_git_commit_start",
                std::time::Duration::ZERO,
                &[],
            );
        }
        self.git.commit_state()?;
        if let Some(commit) = commit {
            crate::perf::emit("scan.publication_git_commit", commit.elapsed(), &[]);
        }

        let finish = crate::perf::start();
        if finish.is_some() {
            crate::perf::emit(
                "scan.publication_verify_start",
                std::time::Duration::ZERO,
                &[],
            );
        }
        let head = self.git.head()?;
        let result = self
            .store
            .finish_scan_publication(transaction, &head)
            .map_err(ScanExecutionError::from);
        if let Some(finish) = finish {
            crate::perf::emit(
                "scan.publication_verify",
                finish.elapsed(),
                &[("failed", u64::from(result.is_err()))],
            );
        }
        result
    }
}
