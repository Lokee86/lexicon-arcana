use std::path::{Path, PathBuf};

use crate::{
    AdapterHost, GcOptions, GcResult, ScanEngine, ScanReport, StateRepository, Store, load_config,
    save_config, save_config_with_languages, state_root,
};

use super::LexiconError;

pub struct Lexicon {
    pub(super) repository: PathBuf,
    pub(super) state_root: PathBuf,
    adapter_root: PathBuf,
    pub(super) engine: ScanEngine,
}

impl Lexicon {
    pub fn open(repository: impl AsRef<Path>) -> Result<Self, LexiconError> {
        let repository = absolute(repository.as_ref())?;
        let config = load_config(&repository).map_err(LexiconError::new)?;
        let host = AdapterHost::new(&config.adapter_root);
        Self::open_with_host(repository, host)
    }

    pub fn open_with_host(
        repository: impl AsRef<Path>,
        host: AdapterHost,
    ) -> Result<Self, LexiconError> {
        let repository = absolute(repository.as_ref())?;
        let config = load_config(&repository).map_err(LexiconError::new)?;
        let state_root = state_root(&repository);
        let git = StateRepository::open(state_root.join("repo"))?;
        let adapter_root = host.root().to_path_buf();
        let engine = ScanEngine::new(
            &repository,
            git,
            Store::new(&state_root),
            host,
            config.enabled_languages,
        );
        Ok(Self {
            repository,
            state_root,
            adapter_root,
            engine,
        })
    }

    pub fn initialize(
        repository: impl AsRef<Path>,
        adapter_root: impl AsRef<Path>,
    ) -> Result<(Self, ScanReport), LexiconError> {
        let repository = absolute(repository.as_ref())?;
        let adapter_root = absolute(adapter_root.as_ref())?;
        let host = AdapterHost::new(adapter_root);
        Self::initialize_with_host(repository, host)
    }

    pub fn initialize_with_languages(
        repository: impl AsRef<Path>,
        adapter_root: impl AsRef<Path>,
        enabled_languages: &[String],
    ) -> Result<(Self, ScanReport), LexiconError> {
        let repository = absolute(repository.as_ref())?;
        let adapter_root = absolute(adapter_root.as_ref())?;
        let host = AdapterHost::new(adapter_root);
        Self::initialize_with_host_and_languages(repository, host, enabled_languages)
    }

    pub fn initialize_with_host(
        repository: impl AsRef<Path>,
        host: AdapterHost,
    ) -> Result<(Self, ScanReport), LexiconError> {
        Self::initialize_configured(repository.as_ref(), host, None)
    }

    pub fn initialize_with_host_and_languages(
        repository: impl AsRef<Path>,
        host: AdapterHost,
        enabled_languages: &[String],
    ) -> Result<(Self, ScanReport), LexiconError> {
        Self::initialize_configured(repository.as_ref(), host, Some(enabled_languages))
    }

    fn initialize_configured(
        repository: &Path,
        host: AdapterHost,
        enabled_languages: Option<&[String]>,
    ) -> Result<(Self, ScanReport), LexiconError> {
        let repository = absolute(repository)?;
        let state_root = state_root(&repository);
        let store = Store::new(&state_root);
        let _guard = store.lock()?;

        let config = match enabled_languages {
            Some(languages) => save_config_with_languages(&repository, host.root(), languages),
            None => save_config(&repository, host.root()),
        }
        .map_err(LexiconError::new)?;

        let git = StateRepository::ensure(state_root.join("repo"))?;
        let adapter_root = PathBuf::from(&config.adapter_root);
        let engine = ScanEngine::new(&repository, git, store, host, config.enabled_languages);
        let report = engine.initialize_full_locked()?;
        let value = Self {
            repository,
            state_root,
            adapter_root,
            engine,
        };
        Ok((value, report))
    }

    pub fn repository(&self) -> &Path {
        &self.repository
    }

    pub fn state_root(&self) -> &Path {
        &self.state_root
    }

    pub fn adapter_root(&self) -> &Path {
        &self.adapter_root
    }

    pub fn export(
        &self,
        snapshot: &str,
        destination: impl AsRef<Path>,
        languages: &[String],
    ) -> Result<(), LexiconError> {
        self.engine
            .store()
            .export(snapshot, destination.as_ref(), languages)
            .map_err(Into::into)
    }

    pub fn garbage_collect(
        &self,
        options: GcOptions,
        dry_run: bool,
    ) -> Result<GcResult, LexiconError> {
        self.engine
            .store()
            .garbage_collect(options, dry_run)
            .map_err(Into::into)
    }

    pub fn store(&self) -> &Store {
        self.engine.store()
    }

    pub fn state_repository(&self) -> &StateRepository {
        self.engine.state_repository()
    }
}

fn absolute(path: &Path) -> Result<PathBuf, LexiconError> {
    if path.is_absolute() {
        return Ok(crate::config::clean_path(path));
    }
    std::env::current_dir()
        .map(|current| crate::config::clean_path(&current.join(path)))
        .map_err(Into::into)
}
