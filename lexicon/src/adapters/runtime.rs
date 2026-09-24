use std::path::Path;
use std::process::Command;

use super::executable::{find_executable, npm_executable};
use super::{AdapterError, AdapterRequest};

pub(crate) fn prepare_runtime(
    adapter_root: &Path,
    request: &AdapterRequest,
) -> Result<(), AdapterError> {
    if request.language != "typescript" {
        return Ok(());
    }
    let directory = adapter_root.join("typescript");
    let distribution = directory.join("dist").join("cli.js");
    if distribution.is_file() {
        return Ok(());
    }
    let npm = find_executable(&[npm_executable()])?;
    if !directory.join("node_modules").exists() {
        run(
            &npm,
            &directory,
            &["ci", "--silent"],
            "prepare TypeScript dependencies",
        )?;
    }
    run(
        &npm,
        &directory,
        &["run", "build", "--silent"],
        "build TypeScript adapter",
    )
}

fn run(
    program: &Path,
    directory: &Path,
    arguments: &[&str],
    action: &str,
) -> Result<(), AdapterError> {
    let output = Command::new(program)
        .args(arguments)
        .current_dir(directory)
        .output()
        .map_err(AdapterError::from)?;
    if output.status.success() {
        Ok(())
    } else {
        Err(AdapterError::new(format!(
            "{action}: {}\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )))
    }
}
