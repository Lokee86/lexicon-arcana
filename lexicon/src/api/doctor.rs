use std::path::{Path, PathBuf};

use crate::{Store, load_config, state_root};

use super::LexiconError;
use super::doctor_checks::{
    inspect_consumers, manifest_languages, verify_snapshot, verify_state_repository,
};
use super::doctor_runtime::check_runtime;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorCheck {
    pub label: String,
    pub error: Option<String>,
}

impl DoctorCheck {
    pub fn passed(&self) -> bool {
        self.error.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DoctorReport {
    pub checks: Vec<DoctorCheck>,
}

impl DoctorReport {
    pub fn is_healthy(&self) -> bool {
        self.checks.iter().all(DoctorCheck::passed)
    }

    pub fn failures(&self) -> impl Iterator<Item = &DoctorCheck> {
        self.checks.iter().filter(|check| !check.passed())
    }

    pub(super) fn pass(&mut self, label: impl Into<String>) {
        self.checks.push(DoctorCheck {
            label: label.into(),
            error: None,
        });
    }

    pub(super) fn fail(&mut self, label: impl Into<String>, error: impl ToString) {
        self.checks.push(DoctorCheck {
            label: label.into(),
            error: Some(error.to_string().replace('\n', "; ")),
        });
    }
}

pub fn doctor(repository: impl AsRef<Path>) -> Result<DoctorReport, LexiconError> {
    let repository = absolute(repository.as_ref())?;
    let mut report = DoctorReport::default();

    let config = match load_config(&repository) {
        Ok(config) => {
            report.pass("configuration loading");
            Some(config)
        }
        Err(error) => {
            report.fail("configuration loading", error);
            None
        }
    };

    let private_state = state_root(&repository).join("repo");
    match verify_state_repository(&private_state) {
        Ok(()) => report.pass("private Git state repository"),
        Err(error) => report.fail("private Git state repository", error),
    }

    let store = Store::new(state_root(&repository));
    let (manifest, snapshot_error) = verify_snapshot(&store);
    match &snapshot_error {
        None => report.pass("CURRENT snapshot and referenced objects"),
        Some(error) => report.fail("CURRENT snapshot and referenced objects", error),
    }

    let adapter_root = config
        .as_ref()
        .map(|config| PathBuf::from(&config.adapter_root));
    let adapter_available = adapter_root.as_ref().is_some_and(|root| root.is_dir());
    match (&config, &adapter_root, adapter_available) {
        (Some(_), Some(_), true) => report.pass("configured adapter root"),
        (Some(_), Some(root), false) => report.fail(
            "configured adapter root",
            format!("adapter root is not a directory: {}", root.display()),
        ),
        (None, _, _) => report.fail("configured adapter root", "configuration unavailable"),
        _ => report.fail("configured adapter root", "adapter root unavailable"),
    }

    let languages = manifest_languages(&manifest);
    if snapshot_error.is_some() && languages.is_empty() {
        report.fail(
            "detected language adapter directories",
            "snapshot unavailable",
        );
        report.fail("required runtime executables", "snapshot unavailable");
    } else if languages.is_empty() {
        report.pass("detected language adapter directories");
        report.pass("required runtime executables");
    } else {
        inspect_languages(
            &mut report,
            adapter_root.as_deref(),
            adapter_available,
            languages,
        );
    }

    inspect_consumers(&repository, &mut report);
    Ok(report)
}

fn inspect_languages(
    report: &mut DoctorReport,
    adapter_root: Option<&Path>,
    adapter_available: bool,
    languages: Vec<String>,
) {
    for language in languages {
        if !adapter_available {
            report.fail(
                format!("adapter directory: {language}"),
                "configured adapter root is unavailable",
            );
        } else if let Some(root) = adapter_root {
            let directory = root.join(&language);
            if directory.is_dir() {
                report.pass(format!("adapter directory: {language}"));
            } else {
                report.fail(
                    format!("adapter directory: {language}"),
                    format!("adapter directory is missing: {}", directory.display()),
                );
            }
        }

        let runtime = adapter_root
            .ok_or_else(|| "configured adapter root is unavailable".to_owned())
            .and_then(|root| check_runtime(root, &language));
        match runtime {
            Ok(()) => report.pass(format!("runtime executable: {language}")),
            Err(error) => report.fail(format!("runtime executable: {language}"), error),
        }
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
