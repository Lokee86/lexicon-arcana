use std::io::Write;
use std::time::Duration;

use lexicon::{
    CONSUMER_VERSION, ConsumerDefinition, Store, add_consumer_definition, list_consumer_paths,
    remove_consumer_definition, run_consumer, state_root,
};

use crate::args::{Parser, parse_duration};
use crate::format::consumer_file_name;
use crate::repository::resolve_repository;

pub fn consumer(arguments: &[String], stdout: &mut dyn Write) -> Result<(), String> {
    let Some(command) = arguments.first() else {
        return Err("consumer requires list, add, remove, or run".into());
    };
    match command.as_str() {
        "list" => list(&arguments[1..], stdout),
        "add" => add(&arguments[1..], stdout),
        "remove" => remove(&arguments[1..], stdout),
        "run" => run(&arguments[1..], stdout),
        _ => Err(format!("unknown consumer command {command:?}")),
    }
}

fn list(arguments: &[String], stdout: &mut dyn Write) -> Result<(), String> {
    let root = repo_only("consumer list", arguments)?;
    for path in list_consumer_paths(&state_root(&root)).map_err(|error| error.to_string())? {
        if let Some(stem) = path.file_stem().and_then(|value| value.to_str()) {
            writeln!(stdout, "{stem}").map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

fn add(arguments: &[String], stdout: &mut dyn Write) -> Result<(), String> {
    let mut parser = Parser::new(arguments);
    let mut repository = None::<String>;
    let mut name = None::<String>;
    let mut command = None::<String>;
    let mut timeout = Duration::ZERO;
    let mut args = Vec::<String>::new();

    while let Some(argument) = parser.next() {
        match argument {
            "repo" => repository = Some(parser.value("--repo")?.to_owned()),
            "name" => name = Some(parser.value("--name")?.to_owned()),
            "command" => command = Some(parser.value("--command")?.to_owned()),
            "timeout" => timeout = parse_duration("--timeout", parser.value("--timeout")?)?,
            "arg" => args.push(parser.value("--arg")?.to_owned()),
            _ => return Err(format!("unknown consumer add option {argument:?}")),
        }
    }
    parser.finish()?;
    let name = name
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "consumer add requires --name and --command".to_owned())?;
    let command = command
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "consumer add requires --name and --command".to_owned())?;
    let root = resolve_repository(repository.as_deref())?;
    let file_name = consumer_file_name(&name)?;
    let timeout_nanos = u64::try_from(timeout.as_nanos())
        .map_err(|_| "consumer timeout is too large".to_owned())?;
    add_consumer_definition(
        &state_root(&root),
        &file_name,
        &ConsumerDefinition {
            version: CONSUMER_VERSION,
            command,
            args,
            timeout_nanos,
        },
    )
    .map_err(|error| error.to_string())?;
    writeln!(
        stdout,
        "registered consumer: {}",
        file_name.trim_end_matches(".json")
    )
    .map_err(|error| error.to_string())
}

fn remove(arguments: &[String], stdout: &mut dyn Write) -> Result<(), String> {
    let mut parser = Parser::new(arguments);
    let mut repository = None::<String>;
    let mut name = None::<String>;
    while let Some(argument) = parser.next() {
        match argument {
            "repo" => repository = Some(parser.value("--repo")?.to_owned()),
            "name" => name = Some(parser.value("--name")?.to_owned()),
            _ => return Err(format!("unknown consumer remove option {argument:?}")),
        }
    }
    parser.finish()?;
    let name = name
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "consumer remove requires --name".to_owned())?;
    let root = resolve_repository(repository.as_deref())?;
    let file_name = consumer_file_name(&name)?;
    remove_consumer_definition(&state_root(&root), &file_name)
        .map_err(|error| error.to_string())?;
    writeln!(
        stdout,
        "removed consumer: {}",
        file_name.trim_end_matches(".json")
    )
    .map_err(|error| error.to_string())
}

fn run(arguments: &[String], stdout: &mut dyn Write) -> Result<(), String> {
    let mut parser = Parser::new(arguments);
    let mut repository = None::<String>;
    let mut name = None::<String>;
    let mut snapshot = "CURRENT".to_owned();
    while let Some(argument) = parser.next() {
        match argument {
            "repo" => repository = Some(parser.value("--repo")?.to_owned()),
            "name" => name = Some(parser.value("--name")?.to_owned()),
            "snapshot" => snapshot = parser.value("--snapshot")?.to_owned(),
            _ => return Err(format!("unknown consumer run option {argument:?}")),
        }
    }
    parser.finish()?;
    let name = name
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "consumer run requires --name".to_owned())?;
    let root = resolve_repository(repository.as_deref())?;
    let state = state_root(&root);
    let store = Store::new(&state);
    let snapshot_id = resolve_snapshot(&store, &snapshot)?;
    let file_name = consumer_file_name(&name)?;
    run_consumer(&root, &state, &file_name, &snapshot_id, Some(stdout))
        .map_err(|error| error.to_string())?;
    writeln!(
        stdout,
        "consumer {} processed {snapshot_id}",
        file_name.trim_end_matches(".json")
    )
    .map_err(|error| error.to_string())
}

fn resolve_snapshot(store: &Store, requested: &str) -> Result<String, String> {
    let requested = requested.trim();
    if requested.is_empty() || requested.eq_ignore_ascii_case("CURRENT") {
        return store
            .current()
            .map(|(id, _)| id)
            .map_err(|error| error.to_string());
    }
    store
        .load_snapshot(requested)
        .map_err(|error| error.to_string())?;
    Ok(requested.to_owned())
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
