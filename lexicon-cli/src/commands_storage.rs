use std::io::Write;
use std::path::Path;

use lexicon::{GcOptions, Lexicon, Store, state_root};

use crate::args::{Parser, parse_usize};
use crate::format::{
    display_language_selection, parse_language_selection, parse_optional_languages,
};
use crate::repository::resolve_repository;

pub fn export(arguments: &[String], stdout: &mut dyn Write) -> Result<(), String> {
    let mut parser = Parser::new(arguments);
    let mut repository = None::<String>;
    let mut output = None::<String>;
    let mut snapshot = "CURRENT".to_owned();
    let mut language_text = None::<String>;
    while let Some(argument) = parser.next() {
        match argument {
            "repo" => repository = Some(parser.value("--repo")?.to_owned()),
            "output" => output = Some(parser.value("--output")?.to_owned()),
            "snapshot" => snapshot = parser.value("--snapshot")?.to_owned(),
            "languages" => language_text = Some(parser.value("--languages")?.to_owned()),
            _ => return Err(format!("unknown export option {argument:?}")),
        }
    }
    parser.finish()?;
    let output = output
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "export requires --output".to_owned())?;
    let root = resolve_repository(repository.as_deref())?;
    let languages = parse_optional_languages(language_text.as_deref())?;
    Store::new(state_root(&root))
        .export(&snapshot, Path::new(&output), &languages)
        .map_err(|error| error.to_string())?;
    writeln!(stdout, "exported snapshot {snapshot} to {output}").map_err(|error| error.to_string())
}

pub fn gc(arguments: &[String], stdout: &mut dyn Write) -> Result<(), String> {
    let mut parser = Parser::new(arguments);
    let mut repository = None::<String>;
    let mut retain = 20_usize;
    let mut dry_run = false;
    while let Some(argument) = parser.next() {
        match argument {
            "repo" => repository = Some(parser.value("--repo")?.to_owned()),
            "retain" => retain = parse_usize("--retain", parser.value("--retain")?)?,
            "dry-run" => dry_run = parser.boolean("--dry-run")?,
            _ => return Err(format!("unknown gc option {argument:?}")),
        }
    }
    parser.finish()?;

    let root = resolve_repository(repository.as_deref())?;
    let store = Store::new(state_root(&root));
    let _guard = store.lock().map_err(|error| error.to_string())?;
    let result = store
        .garbage_collect(
            GcOptions {
                keep_snapshots: retain,
            },
            dry_run,
        )
        .map_err(|error| error.to_string())?;
    let mode = if result.dry_run {
        "would delete"
    } else {
        "deleted"
    };
    writeln!(
        stdout,
        "{mode} {} snapshots and {} objects",
        result.deleted_snapshots.len(),
        result.deleted_objects.len()
    )
    .map_err(|error| error.to_string())
}

pub fn languages(arguments: &[String], stdout: &mut dyn Write) -> Result<(), String> {
    if arguments.first().is_some_and(|value| value == "set") {
        return languages_set(&arguments[1..], stdout);
    }
    let arguments = if arguments.first().is_some_and(|value| value == "list") {
        &arguments[1..]
    } else {
        arguments
    };
    let root = repo_only("languages", arguments)?;
    let config = lexicon::load_config(&root)?;
    writeln!(
        stdout,
        "enabled languages: {}",
        display_language_selection(&config.enabled_languages)
    )
    .map_err(|error| error.to_string())
}

fn languages_set(arguments: &[String], stdout: &mut dyn Write) -> Result<(), String> {
    let mut parser = Parser::new(arguments);
    let mut repository = None::<String>;
    let mut language_text = None::<String>;
    while let Some(argument) = parser.next() {
        match argument {
            "repo" => repository = Some(parser.value("--repo")?.to_owned()),
            "languages" => language_text = Some(parser.value("--languages")?.to_owned()),
            _ => return Err(format!("unknown languages set option {argument:?}")),
        }
    }
    parser.finish()?;
    let language_text =
        language_text.ok_or_else(|| "languages set requires --languages".to_owned())?;
    let root = resolve_repository(repository.as_deref())?;
    let selection = parse_language_selection(&language_text)?;
    lexicon::update_enabled_languages(&root, &selection)?;
    let lexicon = Lexicon::open(&root).map_err(|error| error.to_string())?;
    let report = lexicon
        .scan_with_consumer_output(stdout)
        .map_err(|error| error.to_string())?;
    writeln!(
        stdout,
        "enabled languages: {}",
        display_language_selection(&selection)
    )
    .map_err(|error| error.to_string())?;
    writeln!(stdout, "snapshot: {}", report.snapshot_id).map_err(|error| error.to_string())
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
