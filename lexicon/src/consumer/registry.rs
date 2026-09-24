use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::LexiconError;

use super::ConsumerDefinition;
use super::atomic::write_atomic;

pub fn list_consumer_paths(root: &Path) -> Result<Vec<PathBuf>, LexiconError> {
    let entries = match fs::read_dir(root.join("consumers")) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(LexiconError::new(format!(
                "read Lexicon consumers: {error}"
            )));
        }
    };
    let mut paths = Vec::new();
    for entry in entries {
        let entry = entry?;
        if entry.file_type()?.is_dir()
            || entry.path().extension().and_then(|value| value.to_str()) != Some("json")
        {
            continue;
        }
        paths.push(entry.path());
    }
    paths.sort();
    Ok(paths)
}

pub fn load_consumer_definition(path: &Path) -> Result<ConsumerDefinition, LexiconError> {
    let data = fs::read(path).map_err(|error| {
        LexiconError::new(format!("read Lexicon consumer {}: {error}", path.display()))
    })?;
    ConsumerDefinition::parse(&data)
        .map_err(|error| LexiconError::new(format!("Lexicon consumer {}: {error}", path.display())))
}

pub fn add_consumer_definition(
    state_root: &Path,
    name: &str,
    definition: &ConsumerDefinition,
) -> Result<(), LexiconError> {
    let name = validate_consumer_name(name)?;
    definition.validate()?;
    #[derive(Serialize)]
    struct Wire<'a> {
        version: u64,
        command: &'a str,
        #[serde(skip_serializing_if = "slice_empty")]
        args: &'a [String],
        #[serde(skip_serializing_if = "Option::is_none")]
        timeout: Option<String>,
    }

    let value = Wire {
        version: definition.version,
        command: &definition.command,
        args: &definition.args,
        timeout: (definition.timeout_nanos != 0).then(|| format_duration(definition.timeout_nanos)),
    };
    let mut data = serde_json::to_vec_pretty(&value)
        .map_err(|error| LexiconError::new(format!("encode Lexicon consumer {name}: {error}")))?;
    data.push(b'\n');
    write_atomic(&state_root.join("consumers").join(name), &data)
}

pub fn remove_consumer_definition(state_root: &Path, name: &str) -> Result<(), LexiconError> {
    let name = validate_consumer_name(name)?;
    let mut failures = Vec::new();
    for path in [
        state_root.join("consumers").join(&name),
        state_root.join("consumer-state").join(&name),
    ] {
        if let Err(error) = fs::remove_file(&path)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            failures.push(error.to_string());
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(LexiconError::new(failures.join("; ")))
    }
}

pub fn validate_consumer_name(name: &str) -> Result<String, LexiconError> {
    let path = Path::new(name);
    let plain = !name.is_empty()
        && path.components().count() == 1
        && !name.contains('/')
        && !name.contains('\\')
        && path.extension().and_then(|value| value.to_str()) == Some("json")
        && path.file_stem().is_some_and(|value| !value.is_empty());
    if plain {
        Ok(name.to_owned())
    } else {
        Err(LexiconError::new(format!(
            "invalid Lexicon consumer name {name:?}"
        )))
    }
}

fn format_duration(nanos: u64) -> String {
    const HOUR: u64 = 3_600_000_000_000;
    const MINUTE: u64 = 60_000_000_000;
    const SECOND: u64 = 1_000_000_000;
    const MILLISECOND: u64 = 1_000_000;
    const MICROSECOND: u64 = 1_000;

    if nanos >= SECOND {
        let hours = nanos / HOUR;
        let remainder = nanos % HOUR;
        let minutes = remainder / MINUTE;
        let seconds_nanos = remainder % MINUTE;
        let mut result = String::new();
        if hours > 0 {
            result.push_str(&format!("{hours}h"));
        }
        if minutes > 0 || hours > 0 {
            result.push_str(&format!("{minutes}m"));
        }
        let seconds = seconds_nanos / SECOND;
        let fraction = seconds_nanos % SECOND;
        if fraction == 0 {
            result.push_str(&format!("{seconds}s"));
        } else {
            result.push_str(&format!(
                "{seconds}.{}s",
                format!("{fraction:09}").trim_end_matches('0')
            ));
        }
        return result;
    }
    if nanos >= MILLISECOND {
        return scaled_duration(nanos, MILLISECOND, 6, "ms");
    }
    if nanos >= MICROSECOND {
        return scaled_duration(nanos, MICROSECOND, 3, "µs");
    }
    format!("{nanos}ns")
}

fn scaled_duration(nanos: u64, divisor: u64, width: usize, unit: &str) -> String {
    let whole = nanos / divisor;
    let remainder = nanos % divisor;
    if remainder == 0 {
        return format!("{whole}{unit}");
    }
    let fraction = format!("{remainder:0width$}", width = width)
        .trim_end_matches('0')
        .to_owned();
    format!("{whole}.{fraction}{unit}")
}

fn slice_empty(values: &&[String]) -> bool {
    values.is_empty()
}
