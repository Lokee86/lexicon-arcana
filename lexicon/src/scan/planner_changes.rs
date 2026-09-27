use std::collections::BTreeMap;

use crate::languages::{for_path, language_enabled, owns_source};

use super::planner::incremental_plan;
use super::{AnalysisPlan, Change, PlanningInput};

pub(super) fn add_change_plans(plans: &mut BTreeMap<String, AnalysisPlan>, input: &PlanningInput) {
    for change in &input.changes {
        let status = change.status.trim();
        let Some(kind) = status.as_bytes().first().copied() else {
            continue;
        };
        match kind {
            b'M' => add_incremental_path(plans, &change.new, false, input),
            b'A' => add_incremental_path(plans, &change.new, true, input),
            b'R' => add_rename(plans, &change.old, &change.new, input),
            _ => add_structural_paths(plans, change, input),
        }
    }
}

fn add_incremental_path(
    plans: &mut BTreeMap<String, AnalysisPlan>,
    path: &str,
    added: bool,
    input: &PlanningInput,
) {
    for language in for_path(path) {
        if !language_enabled(&language, &input.enabled_languages) {
            continue;
        }
        if !owns_source(&language, path) {
            if !added
                && language == "python"
                && path.replace('\\', "/") == "pyproject.toml"
                && input.python_project_config_unchanged
            {
                continue;
            }
            ensure_plan(plans, language).full = true;
            continue;
        }
        let plan = ensure_plan(plans, language.clone());
        if added && language != "python" {
            plan.full = true;
        } else if added {
            plan.added_files.push(path.to_owned());
        } else {
            plan.changed_files.push(path.to_owned());
        }
    }
}

fn add_rename(
    plans: &mut BTreeMap<String, AnalysisPlan>,
    old_path: &str,
    new_path: &str,
    input: &PlanningInput,
) {
    let old_python = owns_source("python", old_path);
    let new_python = owns_source("python", new_path);
    if language_enabled("python", &input.enabled_languages) && (old_python || new_python) {
        let plan = ensure_plan(plans, "python".into());
        if old_python && new_python {
            plan.removed_files.push(old_path.to_owned());
            plan.added_files.push(new_path.to_owned());
        } else {
            plan.full = true;
        }
    }
    for path in [old_path, new_path] {
        for language in for_path(path) {
            if language == "python" || !language_enabled(&language, &input.enabled_languages) {
                continue;
            }
            ensure_plan(plans, language).full = true;
        }
    }
}

fn add_structural_paths(
    plans: &mut BTreeMap<String, AnalysisPlan>,
    change: &Change,
    input: &PlanningInput,
) {
    for path in [&change.new, &change.old] {
        if path.is_empty() {
            continue;
        }
        for language in for_path(path) {
            if language_enabled(&language, &input.enabled_languages) {
                ensure_plan(plans, language).full = true;
            }
        }
    }
}

fn ensure_plan(plans: &mut BTreeMap<String, AnalysisPlan>, language: String) -> &mut AnalysisPlan {
    plans
        .entry(language.clone())
        .or_insert_with(|| incremental_plan(language))
}

#[cfg(test)]
#[path = "planner_changes_tests.rs"]
mod tests;
