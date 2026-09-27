use std::collections::BTreeMap;

use super::add_change_plans;
use crate::{Change, PlanningInput};

fn input(changes: Vec<Change>) -> PlanningInput {
    PlanningInput {
        changes,
        present_languages: Vec::new(),
        enabled_languages: vec!["python".into(), "go".into()],
        adapter_fingerprints: None,
        python_project_config_unchanged: false,
    }
}

fn change(status: &str, old: &str, new: &str) -> Change {
    Change {
        status: status.into(),
        old: old.into(),
        new: new.into(),
    }
}

#[test]
fn python_addition_stays_incremental() {
    let input = input(vec![change("A", "", "pkg/new.py")]);
    let mut plans = BTreeMap::new();
    add_change_plans(&mut plans, &input);
    let plan = plans.get("python").expect("python plan");
    assert!(!plan.full);
    assert_eq!(plan.added_files, vec!["pkg/new.py"]);
}

#[test]
fn non_python_addition_requires_full_analysis() {
    let input = input(vec![change("A", "", "pkg/new.go")]);
    let mut plans = BTreeMap::new();
    add_change_plans(&mut plans, &input);
    assert!(plans.get("go").expect("go plan").full);
}

#[test]
fn python_rename_tracks_removed_and_added_paths() {
    let input = input(vec![change("R100", "pkg/old.py", "pkg/new.py")]);
    let mut plans = BTreeMap::new();
    add_change_plans(&mut plans, &input);
    let plan = plans.get("python").expect("python plan");
    assert!(!plan.full);
    assert_eq!(plan.removed_files, vec!["pkg/old.py"]);
    assert_eq!(plan.added_files, vec!["pkg/new.py"]);
}

#[test]
fn non_semantic_pyproject_change_is_ignored() {
    let mut input = input(vec![change("M", "", "pyproject.toml")]);
    input.python_project_config_unchanged = true;
    let mut plans = BTreeMap::new();
    add_change_plans(&mut plans, &input);
    assert!(!plans.contains_key("python"));
}

#[test]
fn semantic_pyproject_change_requires_full_analysis() {
    let input = input(vec![change("M", "", "pyproject.toml")]);
    let mut plans = BTreeMap::new();
    add_change_plans(&mut plans, &input);
    assert!(plans.get("python").expect("python plan").full);
}
