use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::Duration;

use serde_json::{Map, Value};

use crate::{EdgeRecord, SourceSpan, UnresolvedRecord};

use super::http::{HttpContract, HttpProducer};
use super::model::{Library, Node, SourceIndex};
use super::source::collect_source_files;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Summary {
    pub http_contracts: usize,
    pub http_links: usize,
    pub message_channels: usize,
    pub message_links: usize,
    pub config_keys: usize,
    pub processes: usize,
    pub process_links: usize,
    pub commands: usize,
    pub command_links: usize,
    pub protocols: usize,
    pub protocol_links: usize,
    pub state_paths: usize,
    pub state_links: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolveResult {
    pub repository: String,
    pub nodes: Vec<Node>,
    pub edges: Vec<EdgeRecord>,
    pub unresolved: Vec<UnresolvedRecord>,
    pub summary: Summary,
}

pub(crate) struct Resolver<'a> {
    pub(crate) index: SourceIndex<'a>,
    pub(crate) result: ResolveResult,
    pub(crate) nodes: HashMap<String, Node>,
    pub(crate) edges: HashMap<String, EdgeRecord>,
    pub(crate) unresolved: HashMap<String, UnresolvedRecord>,
    pub(crate) constants: HashMap<String, HashSet<String>>,
    pub(crate) http: Vec<HttpContract>,
    pub(crate) http_sources: Vec<HttpProducer>,
    pub(crate) http_providers: HashMap<String, Vec<HttpProducer>>,
}

pub fn resolve(
    source_root: &Path,
    libraries: &[Library],
) -> Result<ResolveResult, crate::ScanExecutionError> {
    let allowed_languages = libraries
        .iter()
        .map(|library| library.language.clone())
        .collect::<HashSet<_>>();
    let source_started = crate::perf::start();
    let files = collect_source_files(source_root, &allowed_languages)?;
    if let Some(source_started) = source_started {
        crate::perf::emit(
            "interstack.source_loading",
            source_started.elapsed(),
            &[("files", files.len() as u64)],
        );
    }
    let repository = libraries
        .iter()
        .find_map(|library| (!library.repository.is_empty()).then(|| library.repository.clone()))
        .unwrap_or_else(|| {
            source_root
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("repository")
                .to_owned()
        });
    let index_started = crate::perf::start();
    let index = SourceIndex::new(libraries);
    if let Some(index_started) = index_started {
        crate::perf::emit(
            "interstack.source_index",
            index_started.elapsed(),
            &[
                ("libraries", libraries.len() as u64),
                (
                    "nodes",
                    libraries
                        .iter()
                        .map(|library| library.nodes.len() as u64)
                        .sum(),
                ),
            ],
        );
    }
    let mut resolver = Resolver {
        index,
        result: ResolveResult {
            repository,
            nodes: Vec::new(),
            edges: Vec::new(),
            unresolved: Vec::new(),
            summary: Summary::default(),
        },
        nodes: HashMap::new(),
        edges: HashMap::new(),
        unresolved: HashMap::new(),
        constants: HashMap::new(),
        http: Vec::new(),
        http_sources: Vec::new(),
        http_providers: HashMap::new(),
    };

    let detection_started = crate::perf::start();
    let phase = crate::perf::start();
    for file in &files {
        resolver.collect_constants(file);
    }
    if let Some(phase) = phase {
        crate::perf::emit(
            "interstack.constants",
            phase.elapsed(),
            &[("files", files.len() as u64)],
        );
    }
    let phase = crate::perf::start();
    for file in &files {
        resolver.detect_http_consumers(file);
        resolver.detect_message_consumers(file);
    }
    if let Some(phase) = phase {
        crate::perf::emit(
            "interstack.consumers",
            phase.elapsed(),
            &[("files", files.len() as u64)],
        );
    }
    let phase = crate::perf::start();
    for file in &files {
        resolver.collect_http_path_providers(file);
    }
    if let Some(phase) = phase {
        crate::perf::emit(
            "interstack.http_providers",
            phase.elapsed(),
            &[("files", files.len() as u64)],
        );
    }
    let mut stages = [Duration::ZERO; 6];
    for file in &files {
        let started = crate::perf::start();
        resolver.detect_http_producers(file);
        if let Some(started) = started {
            stages[0] += started.elapsed();
        }
        let started = crate::perf::start();
        resolver.detect_message_producers(file);
        if let Some(started) = started {
            stages[1] += started.elapsed();
        }
        let started = crate::perf::start();
        resolver.detect_config_reads(file);
        if let Some(started) = started {
            stages[2] += started.elapsed();
        }
        let started = crate::perf::start();
        resolver.detect_boundary_config(file);
        if let Some(started) = started {
            stages[3] += started.elapsed();
        }
        let started = crate::perf::start();
        resolver.detect_process_contracts(file);
        if let Some(started) = started {
            stages[4] += started.elapsed();
        }
        let started = crate::perf::start();
        resolver.detect_state_contracts(file);
        if let Some(started) = started {
            stages[5] += started.elapsed();
        }
    }
    if detection_started.is_some() {
        for (label, elapsed) in [
            ("interstack.http_producers", stages[0]),
            ("interstack.message_producers", stages[1]),
            ("interstack.config_reads", stages[2]),
            ("interstack.boundary_config", stages[3]),
            ("interstack.process_contracts", stages[4]),
            ("interstack.state_contracts", stages[5]),
        ] {
            crate::perf::emit(label, elapsed, &[("files", files.len() as u64)]);
        }
    }
    if let Some(detection_started) = detection_started {
        crate::perf::emit(
            "interstack.contract_detection",
            detection_started.elapsed(),
            &[("files", files.len() as u64)],
        );
    }

    let linking_started = crate::perf::start();
    resolver.resolve_http_producers();
    resolver.finish();
    if let Some(linking_started) = linking_started {
        crate::perf::emit(
            "interstack.linking",
            linking_started.elapsed(),
            &[
                ("nodes", resolver.result.nodes.len() as u64),
                ("edges", resolver.result.edges.len() as u64),
                ("unresolved", resolver.result.unresolved.len() as u64),
            ],
        );
    }
    Ok(resolver.result)
}

impl Resolver<'_> {
    fn finish(&mut self) {
        self.result.nodes = self.nodes.values().cloned().collect();
        self.result.edges = self.edges.values().cloned().collect();
        self.result.unresolved = self.unresolved.values().cloned().collect();
    }

    pub(crate) fn add_node(&mut self, node: Node) {
        if !node.id.is_empty() {
            self.nodes.insert(node.id.clone(), node);
        }
    }

    pub(crate) fn add_edge(&mut self, edge: EdgeRecord) {
        if edge.source.is_empty() || edge.target.is_empty() || edge.relation.is_empty() {
            return;
        }
        self.edges.insert(edge_sort_key(&edge), edge);
    }

    pub(crate) fn add_unresolved(&mut self, item: UnresolvedRecord) {
        if item.source.is_empty()
            || item.relation.is_empty()
            || item.expression.is_empty()
            || item.reason.is_empty()
        {
            return;
        }
        self.unresolved.insert(unresolved_sort_key(&item), item);
    }

    pub(crate) fn unique_string(&self, name: &str) -> Option<String> {
        let values = self.constants.get(name)?;
        (values.len() == 1).then(|| values.iter().next().expect("one value").clone())
    }
}

pub(crate) fn line_span(path: &str, line: usize, text: &str) -> SourceSpan {
    let end_column = text.chars().count().saturating_add(1).max(2) as u64;
    SourceSpan {
        path: path.to_owned(),
        start_line: line as u64,
        start_column: 1,
        end_line: line as u64,
        end_column,
    }
}

pub(crate) fn attributes(values: impl IntoIterator<Item = (&'static str, Value)>) -> Option<Value> {
    let map = values
        .into_iter()
        .map(|(key, value)| (key.to_owned(), value))
        .collect::<Map<_, _>>();
    (!map.is_empty()).then_some(Value::Object(map))
}

pub(crate) fn span_sort_key(span: Option<&SourceSpan>) -> String {
    span.map(|span| {
        format!(
            "{}:{:010}:{:010}:{:010}:{:010}",
            span.path, span.start_line, span.start_column, span.end_line, span.end_column
        )
    })
    .unwrap_or_default()
}

fn edge_sort_key(edge: &EdgeRecord) -> String {
    format!(
        "{}\0{}\0{}\0{}",
        edge.source,
        edge.target,
        edge.relation,
        span_sort_key(edge.span.as_ref())
    )
}

fn unresolved_sort_key(item: &UnresolvedRecord) -> String {
    format!(
        "{}\0{}\0{}\0{}\0{}",
        item.source,
        item.relation,
        item.expression,
        item.reason,
        span_sort_key(item.span.as_ref())
    )
}
