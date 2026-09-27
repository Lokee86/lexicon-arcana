use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::Change;

use super::RepositoryError;

#[derive(Debug, Clone)]
pub struct StateRepository {
    root: PathBuf,
}

impl StateRepository {
    pub fn ensure(root: impl Into<PathBuf>) -> Result<Self, RepositoryError> {
        let repository = Self { root: root.into() };
        fs::create_dir_all(&repository.root).map_err(|error| {
            RepositoryError::new(format!("create Lexicon state repository: {error}"))
        })?;
        repository.run(&["init", "--quiet", "--initial-branch=state"])?;
        for (key, value) in [
            ("user.name", "Lexicon"),
            ("user.email", "lexicon@local"),
            ("core.autocrlf", "false"),
            ("core.filemode", "false"),
        ] {
            repository.run(&["config", key, value])?;
        }
        Ok(repository)
    }

    pub fn open(root: impl Into<PathBuf>) -> Result<Self, RepositoryError> {
        let repository = Self { root: root.into() };
        repository
            .output(&["rev-parse", "--git-dir"])
            .map_err(|error| {
                RepositoryError::new(format!("open Lexicon state repository: {error}"))
            })?;
        Ok(repository)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn reset_index(&self) -> Result<(), RepositoryError> {
        if self.has_head() {
            self.run(&["reset", "--quiet", "--mixed", "HEAD"])?;
        }
        Ok(())
    }

    pub fn stage_source(&self) -> Result<(), RepositoryError> {
        self.run(&["add", "-A", "--", "source"])
    }

    pub fn stage_all(&self) -> Result<(), RepositoryError> {
        self.run(&["add", "-A"])
    }

    pub fn has_head(&self) -> bool {
        self.output(&["rev-parse", "--verify", "HEAD"]).is_ok()
    }

    pub fn head(&self) -> Result<String, RepositoryError> {
        if !self.has_head() {
            return Err(RepositoryError::new(
                "Lexicon state repository has no commit",
            ));
        }
        self.output(&["rev-parse", "HEAD"])
    }

    pub fn head_option(&self) -> Result<Option<String>, RepositoryError> {
        if self.has_head() {
            self.head().map(Some)
        } else {
            Ok(None)
        }
    }

    pub fn head_source(&self, path: &str) -> Result<Vec<u8>, RepositoryError> {
        if !self.has_head() {
            return Err(RepositoryError::new(
                "Lexicon state repository has no commit",
            ));
        }
        let normalized = path.replace('\\', "/");
        let spec = format!("HEAD:source/{normalized}");
        self.output_bytes(&["show", &spec])
    }

    pub fn has_staged_changes(&self) -> bool {
        match Command::new("git")
            .args(["diff", "--cached", "--quiet"])
            .current_dir(&self.root)
            .status()
        {
            Ok(status) => !status.success(),
            Err(_) => true,
        }
    }

    pub fn commit_state(&self) -> Result<(), RepositoryError> {
        if !self.has_head() {
            return self.run(&["commit", "--quiet", "--allow-empty", "-m", "Lexicon state"]);
        }
        if !self.has_staged_changes() {
            return Ok(());
        }
        self.run(&["commit", "--quiet", "--amend", "--no-edit"])?;
        let _ = self.run(&["reflog", "expire", "--expire=now", "--all"]);
        Ok(())
    }

    pub fn source_changes(&self) -> Result<Vec<Change>, RepositoryError> {
        if !self.has_head() {
            return Ok(Vec::new());
        }
        let data = self.output_bytes(&[
            "diff",
            "--cached",
            "--name-status",
            "-z",
            "-M",
            "HEAD",
            "--",
            "source",
        ])?;
        Ok(parse_changes(&data))
    }

    fn run(&self, arguments: &[&str]) -> Result<(), RepositoryError> {
        self.output(arguments).map(|_| ())
    }

    fn output(&self, arguments: &[&str]) -> Result<String, RepositoryError> {
        let data = self.output_bytes(arguments)?;
        Ok(String::from_utf8_lossy(&data).trim().to_owned())
    }

    fn output_bytes(&self, arguments: &[&str]) -> Result<Vec<u8>, RepositoryError> {
        let output = Command::new("git")
            .args(arguments)
            .current_dir(&self.root)
            .output()
            .map_err(RepositoryError::from)?;
        if output.status.success() {
            return Ok(output.stdout);
        }
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let detail = if detail.is_empty() {
            output.status.to_string()
        } else {
            detail
        };
        Err(RepositoryError::new(format!(
            "git {}: {detail}",
            arguments.join(" ")
        )))
    }
}

pub(crate) fn parse_changes(data: &[u8]) -> Vec<Change> {
    let parts: Vec<&[u8]> = data.split(|byte| *byte == 0).collect();
    let mut changes = Vec::new();
    let mut index = 0;
    while index + 1 < parts.len() {
        let status = String::from_utf8_lossy(parts[index]).into_owned();
        index += 1;
        if status.is_empty() || index + 1 > parts.len() {
            break;
        }
        let first = source_relative(parts[index]);
        index += 1;
        let mut change = Change {
            status: status.clone(),
            old: String::new(),
            new: first.clone(),
        };
        if status.starts_with('R') || status.starts_with('C') {
            change.old = first;
            if index < parts.len() {
                change.new = source_relative(parts[index]);
                index += 1;
            }
        }
        changes.push(change);
    }
    changes
}

fn source_relative(value: &[u8]) -> String {
    String::from_utf8_lossy(value)
        .trim_start_matches("source/")
        .replace('\\', "/")
}
