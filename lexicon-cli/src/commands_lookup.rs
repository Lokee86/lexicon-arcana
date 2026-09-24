use std::io::Write;

use lexicon::{LookupDirection, LookupNode, LookupReference, SnapshotLookup, Store, state_root};

use crate::args::{Parser, parse_usize};
use crate::repository::resolve_repository;

const DEFAULT_LIMIT: usize = 20;
const MAX_LIMIT: usize = 200;

pub fn find(arguments: &[String], stdout: &mut dyn Write) -> Result<(), String> {
    let options = parse_options("find", arguments, true)?;
    let lookup = load(&options, false)?;
    for node in lookup.find(&options.term, options.limit) {
        write_node_line(stdout, &node)?;
    }
    Ok(())
}

pub fn show(arguments: &[String], stdout: &mut dyn Write) -> Result<(), String> {
    let options = parse_options("show", arguments, false)?;
    let lookup = load(&options, false)?;
    let node = lookup
        .resolve(&options.term)
        .map_err(|error| error.to_string())?;
    writeln!(stdout, "id: {}", node.node.id).map_err(io_error)?;
    writeln!(stdout, "language: {}", node.language).map_err(io_error)?;
    writeln!(stdout, "kind: {}", node.node.kind).map_err(io_error)?;
    writeln!(stdout, "name: {}", node.node.name).map_err(io_error)?;
    writeln!(stdout, "qualified_name: {}", node.node.qualified_name).map_err(io_error)?;
    writeln!(stdout, "path: {}", node.node.path).map_err(io_error)?;
    if let Some(owner) = &node.node.owner {
        writeln!(stdout, "owner: {owner}").map_err(io_error)?;
    }
    if let Some(span) = &node.node.span {
        writeln!(
            stdout,
            "span: {}:{}:{}-{}:{}",
            span.path, span.start_line, span.start_column, span.end_line, span.end_column
        )
        .map_err(io_error)?;
    }
    if let Some(attributes) = &node.node.attributes {
        writeln!(
            stdout,
            "attributes: {}",
            serde_json::to_string(attributes).map_err(|error| error.to_string())?
        )
        .map_err(io_error)?;
    }
    Ok(())
}

pub fn refs(arguments: &[String], stdout: &mut dyn Write) -> Result<(), String> {
    relationships("refs", arguments, stdout, false)
}

pub fn calls(arguments: &[String], stdout: &mut dyn Write) -> Result<(), String> {
    relationships("calls", arguments, stdout, true)
}

fn relationships(
    command: &str,
    arguments: &[String],
    stdout: &mut dyn Write,
    calls_only: bool,
) -> Result<(), String> {
    let options = parse_options(command, arguments, true)?;
    let lookup = load(&options, true)?;
    let references = if calls_only {
        lookup.calls(&options.term, options.limit)
    } else {
        lookup.refs(&options.term, options.limit)
    }
    .map_err(|error| error.to_string())?;
    for reference in references {
        write_reference(stdout, reference)?;
    }
    Ok(())
}

fn write_reference(output: &mut dyn Write, reference: LookupReference) -> Result<(), String> {
    match reference {
        LookupReference::Edge(value) => {
            let (direction, other) = match value.direction {
                LookupDirection::Outgoing => ("out", value.target),
                LookupDirection::Incoming => ("in", value.source),
            };
            let other_id = match value.direction {
                LookupDirection::Outgoing => &value.edge.target,
                LookupDirection::Incoming => &value.edge.source,
            };
            let label = other
                .as_ref()
                .map(|node| node.node.qualified_name.as_str())
                .unwrap_or("<external>");
            writeln!(
                output,
                "{direction}\t{}\t{label}\t{other_id}\t{}",
                value.edge.relation, value.language
            )
            .map_err(io_error)
        }
        LookupReference::Unresolved(value) => writeln!(
            output,
            "out\t{}\t? {}\t{}\t{}",
            value.record.relation, value.record.expression, value.record.reason, value.language
        )
        .map_err(io_error),
    }
}

fn write_node_line(output: &mut dyn Write, node: &LookupNode) -> Result<(), String> {
    let location = node.node.span.as_ref().map_or_else(
        || node.node.path.clone(),
        |span| format!("{}:{}", span.path, span.start_line),
    );
    writeln!(
        output,
        "{}\t{}\t{}\t{}\t{}",
        node.node.id, node.language, node.node.kind, node.node.qualified_name, location
    )
    .map_err(io_error)
}

struct LookupOptions {
    repository: Option<String>,
    snapshot: String,
    limit: usize,
    term: String,
}

fn parse_options(
    command: &str,
    arguments: &[String],
    allow_limit: bool,
) -> Result<LookupOptions, String> {
    let mut parser = Parser::new(arguments);
    let mut repository = None;
    let mut snapshot = "CURRENT".to_owned();
    let mut limit = DEFAULT_LIMIT;
    let mut term = None;
    while let Some(argument) = parser.next() {
        if parser.is_option() {
            match argument {
                "repo" => repository = Some(parser.value("--repo")?.to_owned()),
                "snapshot" => snapshot = parser.value("--snapshot")?.to_owned(),
                "limit" if allow_limit => limit = parse_usize("--limit", parser.value("--limit")?)?,
                _ => return Err(format!("unknown {command} option {argument:?}")),
            }
        } else if term.is_none() {
            term = Some(argument.to_owned());
        } else {
            return Err(format!("unexpected {command} argument {argument:?}"));
        }
    }
    parser.finish()?;
    if limit > MAX_LIMIT {
        return Err(format!("--limit must not exceed {MAX_LIMIT}"));
    }
    Ok(LookupOptions {
        repository,
        snapshot,
        limit,
        term: term.ok_or_else(|| format!("{command} requires a query or node selector"))?,
    })
}

fn load(options: &LookupOptions, relationships: bool) -> Result<SnapshotLookup, String> {
    let root = resolve_repository(options.repository.as_deref())?;
    let store = Store::new(state_root(&root));
    if relationships {
        SnapshotLookup::load(&store, &options.snapshot)
    } else {
        SnapshotLookup::load_nodes(&store, &options.snapshot)
    }
    .map_err(|error| error.to_string())
}

fn io_error(error: std::io::Error) -> String {
    error.to_string()
}
