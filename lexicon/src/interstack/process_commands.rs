use std::sync::LazyLock;

use regex::Regex;
use serde_json::json;

use crate::EdgeRecord;

use super::model::{Node, SourceFile};
use super::paths::{camel_to_kebab, synthetic_path};
use super::process::boundary_commands;
use super::resolver::{Resolver, attributes, line_span};

static GO_CLI_COMMAND: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^\s*case\s+["']([a-z][a-z0-9-]*)["']\s*:"#).unwrap());
static RUST_CLI_COMMAND: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"cli::Command::([A-Z][A-Za-z0-9]*)").unwrap());

impl Resolver<'_> {
    pub(crate) fn link_command_invocation(
        &mut self,
        owner: &Node,
        process_name: &str,
        command: &str,
        file: &SourceFile,
        index: usize,
        line: &str,
    ) {
        let process = self.add_process(process_name);
        let command_node = self.add_cli_command(process_name, command);
        self.add_edge(EdgeRecord {
            attributes: None,
            owner: None,
            relation: "contains".into(),
            source: process.id,
            span: None,
            target: command_node.id.clone(),
        });
        self.add_edge(EdgeRecord {
            attributes: attributes([
                ("confidence", json!(1.0)),
                ("evidence", json!([line.trim()])),
            ]),
            owner: None,
            relation: "calls".into(),
            source: owner.id.clone(),
            span: Some(line_span(&file.path, index + 1, line)),
            target: command_node.id,
        });
        self.result.summary.command_links += 1;
    }

    pub(crate) fn detect_cli_command_ownership(&mut self, file: &SourceFile) {
        let path = file.path.replace('\\', "/").to_ascii_lowercase();
        let (process_name, pattern) = if path.contains("lexicon/internal/cli/") {
            ("lexicon", &*GO_CLI_COMMAND)
        } else if path == "arcana/src/main.rs" || path.ends_with("/arcana/src/main.rs") {
            ("arcana", &*RUST_CLI_COMMAND)
        } else {
            return;
        };
        let commands = boundary_commands();
        let process = self.add_process(process_name);

        for (index, line) in file.lines.iter().enumerate() {
            let Some(capture) = pattern.captures(line) else {
                continue;
            };
            let mut command = capture[1].to_ascii_lowercase();
            if process_name == "arcana" {
                command = camel_to_kebab(&capture[1]);
            }
            if command == "help"
                || !commands
                    .get(process_name)
                    .is_some_and(|values| values.contains(command.as_str()))
            {
                continue;
            }
            let Some(owner) = self.index.owner_at(&file.path, (index + 1) as u64) else {
                continue;
            };
            let command_node = self.add_cli_command(process_name, &command);
            self.add_edge(EdgeRecord {
                attributes: None,
                owner: None,
                relation: "contains".into(),
                source: process.id.clone(),
                span: None,
                target: command_node.id.clone(),
            });
            self.add_edge(EdgeRecord {
                attributes: attributes([
                    ("confidence", json!(1.0)),
                    ("evidence", json!([line.trim()])),
                ]),
                owner: None,
                relation: "defines".into(),
                source: owner.id,
                span: Some(line_span(&file.path, index + 1, line)),
                target: command_node.id,
            });
            self.result.summary.command_links += 1;
        }
    }

    pub(crate) fn add_process(&mut self, name: &str) -> Node {
        let identity = format!("process\0{name}");
        let id = crate::node_id(super::LANGUAGE, "process", &identity);
        if let Some(node) = self.nodes.get(&id) {
            return node.clone();
        }
        let node = Node {
            id,
            kind: "process".into(),
            name: name.to_owned(),
            path: synthetic_path("processes", &identity),
            qualified_name: format!("process:{name}"),
            span: None,
            attributes: Default::default(),
        };
        self.add_node(node.clone());
        self.result.summary.processes += 1;
        node
    }

    pub(crate) fn add_cli_command(&mut self, process: &str, command: &str) -> Node {
        let identity = format!("cli-command\0{process}\0{command}");
        let id = crate::node_id(super::LANGUAGE, "cli-command", &identity);
        if let Some(node) = self.nodes.get(&id) {
            return node.clone();
        }
        let name = format!("{process} {command}");
        let node = Node {
            id,
            kind: "cli-command".into(),
            name: name.clone(),
            path: synthetic_path("commands", &identity),
            qualified_name: format!("cli:{name}"),
            span: None,
            attributes: Default::default(),
        };
        self.add_node(node.clone());
        self.result.summary.commands += 1;
        node
    }
}
