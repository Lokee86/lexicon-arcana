use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;
use serde_json::json;

use crate::EdgeRecord;

use super::model::SourceFile;
use super::paths::last_identifier;
use super::resolver::{Resolver, attributes, line_span};

static EXEC_COMMAND: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\bexec\.Command\(\s*([^,]+)(.*)\)").unwrap());
static EXEC_COMMAND_CONTEXT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\bexec\.CommandContext\(\s*[^,]+,\s*([^,]+)(.*)\)").unwrap());
static QUOTED_ARGUMENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"["']([^"']+)["']"#).unwrap());

pub(crate) fn boundary_commands() -> HashMap<&'static str, HashSet<&'static str>> {
    HashMap::from([
        (
            "lexicon",
            HashSet::from([
                "init",
                "scan",
                "demon",
                "rebuild",
                "export",
                "gc",
                "languages",
                "consumer",
                "status",
                "doctor",
                "version",
            ]),
        ),
        (
            "arcana",
            HashSet::from([
                "benchmark",
                "import-facts",
                "sync",
                "update-facts",
                "query",
                "vectorize",
                "semantic-query",
                "protocol",
                "version",
            ]),
        ),
    ])
}

impl Resolver<'_> {
    pub(crate) fn detect_process_contracts(&mut self, file: &SourceFile) {
        self.detect_process_invocations(file);
        self.detect_cli_command_ownership(file);
        self.detect_arcana_protocol(file);
    }

    fn detect_process_invocations(&mut self, file: &SourceFile) {
        let commands = boundary_commands();
        for (index, line) in file.lines.iter().enumerate() {
            let Some((executable, arguments)) = parse_exec_command(line) else {
                continue;
            };
            let Some(process_name) =
                self.resolve_boundary_process(&file.path, &executable, &commands)
            else {
                continue;
            };
            let Some(owner) = self.index.owner_at(&file.path, (index + 1) as u64) else {
                continue;
            };
            let process = self.add_process(&process_name);
            self.add_edge(EdgeRecord {
                attributes: attributes([
                    ("confidence", json!(1.0)),
                    ("evidence", json!([line.trim()])),
                ]),
                owner: None,
                relation: "invokes-process".into(),
                source: owner.id.clone(),
                span: Some(line_span(&file.path, index + 1, line)),
                target: process.id,
            });
            self.result.summary.process_links += 1;

            if let Some(command) = first_boundary_command(&process_name, &arguments, &commands) {
                self.link_command_invocation(&owner, &process_name, &command, file, index, line);
            }
        }

        let Some(process_name) = boundary_process_for_path(&file.path) else {
            return;
        };
        for (index, line) in file.lines.iter().enumerate() {
            for capture in QUOTED_ARGUMENT.captures_iter(line) {
                let command = capture[1].to_ascii_lowercase();
                if !commands
                    .get(process_name)
                    .is_some_and(|values| values.contains(command.as_str()))
                {
                    continue;
                }
                let trimmed = line.trim();
                if !line.contains("Arguments")
                    && !line.contains("[]string")
                    && !trimmed.starts_with(&format!("\"{command}\""))
                {
                    continue;
                }
                if let Some(owner) = self.index.owner_at(&file.path, (index + 1) as u64) {
                    self.link_command_invocation(&owner, process_name, &command, file, index, line);
                }
            }
        }
    }

    fn resolve_boundary_process(
        &self,
        path: &str,
        token: &str,
        commands: &HashMap<&str, HashSet<&str>>,
    ) -> Option<String> {
        let mut value = token
            .trim()
            .trim_matches(|character| character == '"' || character == '\'')
            .to_owned();
        if let Some(constant) = self.unique_string(&last_identifier(&value)) {
            value = constant;
        }
        let base = Path::new(&value)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(&value)
            .trim_end_matches(".exe")
            .to_ascii_lowercase();
        if commands.contains_key(base.as_str()) {
            return Some(base);
        }
        boundary_process_for_path(path).map(str::to_owned)
    }
}

fn parse_exec_command(line: &str) -> Option<(String, Vec<String>)> {
    let capture = EXEC_COMMAND_CONTEXT
        .captures(line)
        .or_else(|| EXEC_COMMAND.captures(line))?;
    let arguments = QUOTED_ARGUMENT
        .captures_iter(&capture[2])
        .map(|value| value[1].to_owned())
        .collect();
    Some((capture[1].trim().to_owned(), arguments))
}

fn boundary_process_for_path(path: &str) -> Option<&'static str> {
    let path = path.replace('\\', "/").to_ascii_lowercase();
    if path.contains("/arcanagraph/") || path.starts_with("internal/arcanagraph/") {
        Some("arcana")
    } else if path.contains("/lexiconfacts/") || path.starts_with("internal/lexiconfacts/") {
        Some("lexicon")
    } else {
        None
    }
}

fn first_boundary_command(
    process: &str,
    arguments: &[String],
    commands: &HashMap<&str, HashSet<&str>>,
) -> Option<String> {
    arguments.iter().find_map(|argument| {
        let argument = argument.trim().to_ascii_lowercase();
        commands
            .get(process)
            .is_some_and(|values| values.contains(argument.as_str()))
            .then_some(argument)
    })
}
