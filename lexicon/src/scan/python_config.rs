use std::fs;
use std::path::Path;

use crate::{Change, StateRepository};

pub(super) fn project_config_unchanged(
    git: &StateRepository,
    source_root: &Path,
    changes: &[Change],
) -> bool {
    let relevant = changes.iter().any(|change| {
        change.status.trim().starts_with('M') && change.new.replace('\\', "/") == "pyproject.toml"
    });
    if !relevant {
        return false;
    }
    let Ok(previous) = git.head_source("pyproject.toml") else {
        return false;
    };
    let Ok(current) = fs::read(source_root.join("pyproject.toml")) else {
        return false;
    };
    project_analysis_config(&previous) == project_analysis_config(&current)
}

fn project_analysis_config(data: &[u8]) -> String {
    let text = String::from_utf8_lossy(data)
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    let mut result = String::new();
    let mut capture = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(table) = toml_table_header(trimmed) {
            capture = table == "project" || table == "project.optional-dependencies";
            if capture {
                result.push('[');
                result.push_str(table);
                result.push_str("]\n");
            }
            continue;
        }
        if capture || trimmed.starts_with("project.") {
            result.push_str(line.trim_end_matches([' ', '\t']));
            result.push('\n');
        }
    }
    result
}

fn toml_table_header(line: &str) -> Option<&str> {
    if !line.starts_with('[') || line.starts_with("[[") {
        return None;
    }
    let end = line.find(']')?;
    if end <= 1 {
        return None;
    }
    let rest = line[end + 1..].trim();
    if !rest.is_empty() && !rest.starts_with('#') {
        return None;
    }
    Some(line[1..end].trim())
}

#[cfg(test)]
mod tests {
    use super::project_analysis_config;

    #[test]
    fn ignores_non_project_pyproject_changes() {
        let left = b"[tool.ruff]\nline-length = 88\n[project]\nname = \"x\"\n";
        let right = b"[tool.ruff]\nline-length = 120\n[project]\nname = \"x\"\n";
        assert_eq!(
            project_analysis_config(left),
            project_analysis_config(right)
        );
    }

    #[test]
    fn detects_project_dependency_changes() {
        let left = b"[project]\ndependencies = [\"a\"]\n";
        let right = b"[project]\ndependencies = [\"b\"]\n";
        assert_ne!(
            project_analysis_config(left),
            project_analysis_config(right)
        );
    }
}
