use std::io::Write;
use std::path::Path;

use crate::LexiconError;

use super::process::invoke;
use super::registry::{list_consumer_paths, load_consumer_definition, validate_consumer_name};
use super::state::save_snapshot;

pub fn run_consumers(
    repository: &Path,
    state_root: &Path,
    snapshot_id: &str,
    output: Option<&mut dyn Write>,
) -> Result<(), LexiconError> {
    let paths = list_consumer_paths(state_root)?;
    let mut failures = Vec::new();
    match output {
        Some(output) => {
            for path in paths {
                run_path(
                    repository,
                    state_root,
                    snapshot_id,
                    &path,
                    Some(&mut *output),
                    &mut failures,
                );
            }
        }
        None => {
            for path in paths {
                run_path(
                    repository,
                    state_root,
                    snapshot_id,
                    &path,
                    None,
                    &mut failures,
                );
            }
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(LexiconError::new(failures.join("; ")))
    }
}

pub fn run_consumer(
    repository: &Path,
    state_root: &Path,
    name: &str,
    snapshot_id: &str,
    output: Option<&mut dyn Write>,
) -> Result<(), LexiconError> {
    let name = validate_consumer_name(name)?;
    run_consumer_inner(repository, state_root, &name, snapshot_id, output)
}

fn run_path(
    repository: &Path,
    state_root: &Path,
    snapshot_id: &str,
    path: &Path,
    output: Option<&mut dyn Write>,
    failures: &mut Vec<String>,
) {
    let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
        failures.push("Lexicon consumer has non-UTF-8 filename".to_owned());
        return;
    };
    if let Err(error) = run_consumer_inner(repository, state_root, name, snapshot_id, output) {
        failures.push(format!("Lexicon consumer {name}: {error}"));
    }
}

fn run_consumer_inner(
    repository: &Path,
    state_root: &Path,
    name: &str,
    snapshot_id: &str,
    output: Option<&mut dyn Write>,
) -> Result<(), LexiconError> {
    let definition = load_consumer_definition(&state_root.join("consumers").join(name))?;
    invoke(&definition, repository, state_root, snapshot_id, output)?;
    save_snapshot(state_root, name, snapshot_id)
}
