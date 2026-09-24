use std::io::Write;

use crate::args::Parser;
use crate::format::{display_language_selection, display_list};
use crate::repository::resolve_repository;

pub fn status(arguments: &[String], stdout: &mut dyn Write) -> Result<(), String> {
    let root = repo_only("status", arguments)?;
    let report = lexicon::status(&root).map_err(|error| error.to_string())?;
    writeln!(
        stdout,
        "repository root: {}",
        report.repository_root.display()
    )
    .map_err(|error| error.to_string())?;
    writeln!(
        stdout,
        "current snapshot ID: {}",
        report.current_snapshot_id.as_deref().unwrap_or("none")
    )
    .map_err(|error| error.to_string())?;
    writeln!(
        stdout,
        "detected languages: {}",
        display_list(&report.detected_languages)
    )
    .map_err(|error| error.to_string())?;
    writeln!(
        stdout,
        "enabled languages: {}",
        display_language_selection(&report.enabled_languages)
    )
    .map_err(|error| error.to_string())?;
    writeln!(
        stdout,
        "registered consumer names: {}",
        display_list(&report.registered_consumers)
    )
    .map_err(|error| error.to_string())
}

pub fn doctor(arguments: &[String], stdout: &mut dyn Write) -> Result<(), String> {
    let mut parser = Parser::new(arguments);
    let mut repository = None::<String>;
    while let Some(argument) = parser.next() {
        match argument {
            "repo" => repository = Some(parser.value("--repo")?.to_owned()),
            _ => return Err(format!("unknown doctor option {argument:?}")),
        }
    }
    parser.finish()?;
    let root = match resolve_repository(repository.as_deref()) {
        Ok(root) => root,
        Err(error) => {
            writeln!(stdout, "FAIL repository discovery: {error}")
                .map_err(|write_error| write_error.to_string())?;
            return Err(error);
        }
    };
    let report = lexicon::doctor(&root).map_err(|error| error.to_string())?;
    for check in &report.checks {
        match &check.error {
            Some(error) => writeln!(stdout, "FAIL {}: {error}", check.label),
            None => writeln!(stdout, "PASS {}", check.label),
        }
        .map_err(|error| error.to_string())?;
    }
    if report.is_healthy() {
        Ok(())
    } else {
        Err(report
            .failures()
            .filter_map(|check| {
                check
                    .error
                    .as_ref()
                    .map(|error| format!("{}: {error}", check.label))
            })
            .collect::<Vec<_>>()
            .join("\n"))
    }
}

fn repo_only(command: &str, arguments: &[String]) -> Result<std::path::PathBuf, String> {
    let mut parser = Parser::new(arguments);
    let mut repository = None::<String>;
    while let Some(argument) = parser.next() {
        match argument {
            "repo" => repository = Some(parser.value("--repo")?.to_owned()),
            _ => return Err(format!("unknown {command} option {argument:?}")),
        }
    }
    parser.finish()?;
    resolve_repository(repository.as_deref())
}
