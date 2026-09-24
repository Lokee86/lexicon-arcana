use std::io::Write;
use std::path::Path;
use std::time::Duration;

use lexicon::{Lexicon, WatchOptions, WatchStop, find_adapter_root};

use crate::args::{Parser, parse_duration};
use crate::format::{
    parse_language_selection, parse_optional_languages, write_scan_report, write_watch_notice,
};
use crate::repository::{init_repository, resolve_repository};

pub fn init(arguments: &[String], stdout: &mut dyn Write) -> Result<(), String> {
    let mut parser = Parser::new(arguments);
    let mut repository = None::<String>;
    let mut adapters = None::<String>;
    let mut languages = None::<String>;
    while let Some(argument) = parser.next() {
        match argument {
            "repo" => repository = Some(parser.value("--repo")?.to_owned()),
            "adapters" => adapters = Some(parser.value("--adapters")?.to_owned()),
            "languages" => languages = Some(parser.value("--languages")?.to_owned()),
            _ => return Err(format!("unknown init option {argument:?}")),
        }
    }
    parser.finish()?;

    let root = init_repository(repository.as_deref())?;
    let adapter_root = find_adapter_root(&root, adapters.as_deref().map(Path::new))?;
    let (_, report) = match languages {
        Some(value) => {
            let selection = parse_language_selection(&value)?;
            Lexicon::initialize_with_languages(&root, &adapter_root, &selection)
                .map_err(|error| error.to_string())?
        }
        None => Lexicon::initialize(&root, &adapter_root).map_err(|error| error.to_string())?,
    };
    writeln!(stdout, "initialized Lexicon: {}", root.display())
        .map_err(|error| error.to_string())?;
    if !report.languages.is_empty() {
        writeln!(stdout, "libraries: {}", report.languages.join(", "))
            .map_err(|error| error.to_string())?;
    }
    writeln!(stdout, "snapshot: {}", report.snapshot_id).map_err(|error| error.to_string())
}

pub fn scan(arguments: &[String], stdout: &mut dyn Write) -> Result<(), String> {
    let root = repo_only("scan", arguments)?;
    let lexicon = Lexicon::open(&root).map_err(|error| error.to_string())?;
    let report = lexicon
        .scan_with_consumer_output(stdout)
        .map_err(|error| error.to_string())?;
    write_scan_report(stdout, &report)
}

pub fn rebuild(arguments: &[String], stdout: &mut dyn Write) -> Result<(), String> {
    let mut parser = Parser::new(arguments);
    let mut repository = None::<String>;
    let mut language_text = None::<String>;
    while let Some(argument) = parser.next() {
        match argument {
            "repo" => repository = Some(parser.value("--repo")?.to_owned()),
            "languages" => language_text = Some(parser.value("--languages")?.to_owned()),
            _ => return Err(format!("unknown rebuild option {argument:?}")),
        }
    }
    parser.finish()?;

    let root = resolve_repository(repository.as_deref())?;
    let languages = parse_optional_languages(language_text.as_deref())?;
    let lexicon = Lexicon::open(&root).map_err(|error| error.to_string())?;
    let report = lexicon
        .rebuild_with_consumer_output(&languages, stdout)
        .map_err(|error| error.to_string())?;
    writeln!(
        stdout,
        "rebuilt libraries: {}",
        crate::format::display_list(&report.languages)
    )
    .map_err(|error| error.to_string())?;
    writeln!(stdout, "snapshot: {}", report.snapshot_id).map_err(|error| error.to_string())
}

pub fn demon(
    arguments: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Result<(), String> {
    let mut parser = Parser::new(arguments);
    let mut repository = None::<String>;
    let mut debounce = Duration::from_millis(150);
    let mut reconcile = Duration::from_secs(30);
    while let Some(argument) = parser.next() {
        match argument {
            "repo" => repository = Some(parser.value("--repo")?.to_owned()),
            "debounce" => debounce = parse_duration("--debounce", parser.value("--debounce")?)?,
            "reconcile" => reconcile = parse_duration("--reconcile", parser.value("--reconcile")?)?,
            _ => return Err(format!("unknown demon option {argument:?}")),
        }
    }
    parser.finish()?;

    let root = resolve_repository(repository.as_deref())?;
    let lexicon = Lexicon::open(&root).map_err(|error| error.to_string())?;
    writeln!(stdout, "Lexicon demon watching {}", root.display())
        .map_err(|error| error.to_string())?;

    let stop = WatchStop::default();
    let signal = stop.clone();
    ctrlc::set_handler(move || signal.stop())
        .map_err(|error| format!("install signal handler: {error}"))?;
    lexicon
        .watch(
            WatchOptions {
                debounce,
                reconcile,
            },
            &stop,
            |notice| write_watch_notice(stderr, notice),
        )
        .map_err(|error| error.to_string())
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
