use std::mem::size_of;
use std::path::Path;
use std::time::Instant;

use arcana::lexicon::LexiconSnapshot;
use arcana::repository::{
    EdgeFact, NodeFact, RepositoryFacts, SourceSpan, UnresolvedReason, UnresolvedReferenceFact,
    compile_repository_graph,
};

fn main() {
    let root = std::env::args().nth(1).expect("lexicon root");
    println!(
        "sizes NodeFact={} EdgeFact={} Unresolved={} SourceSpan={} UnresolvedReason={} String={}",
        size_of::<NodeFact>(),
        size_of::<EdgeFact>(),
        size_of::<UnresolvedReferenceFact>(),
        size_of::<SourceSpan>(),
        size_of::<UnresolvedReason>(),
        size_of::<String>(),
    );

    println!("MARK load_start");
    let started = Instant::now();
    let snapshot = LexiconSnapshot::current(Path::new(&root)).unwrap();
    println!("MARK load_done {:.3}", started.elapsed().as_secs_f64());

    let facts = snapshot.facts();
    let stats = stats(facts);
    println!(
        "facts nodes={} edges={} unresolved={} node_cap={} edge_cap={} unresolved_cap={}",
        facts.nodes.len(),
        facts.edges.len(),
        facts.unresolved.len(),
        facts.nodes.capacity(),
        facts.edges.capacity(),
        facts.unresolved.capacity()
    );
    println!(
        "inline_bytes nodes={} edges={} unresolved={} total={}",
        facts.nodes.capacity() * size_of::<NodeFact>(),
        facts.edges.capacity() * size_of::<EdgeFact>(),
        facts.unresolved.capacity() * size_of::<UnresolvedReferenceFact>(),
        facts.nodes.capacity() * size_of::<NodeFact>()
            + facts.edges.capacity() * size_of::<EdgeFact>()
            + facts.unresolved.capacity() * size_of::<UnresolvedReferenceFact>(),
    );
    println!(
        "string_len_bytes={} string_capacity_bytes={} string_instances={} span_count={}",
        stats.string_len, stats.string_capacity, stats.string_instances, stats.span_count
    );

    println!("MARK compile_start");
    let started = Instant::now();
    let graph = compile_repository_graph(facts).unwrap();
    println!("MARK compile_done {:.3}", started.elapsed().as_secs_f64());
    println!(
        "graph node_ids={} edges={} edge_cap={} edge_inline_bytes={}",
        graph.node_ids.len(),
        graph.dataset.edges.len(),
        graph.dataset.edges.capacity(),
        graph.dataset.edges.capacity() * size_of::<arcana::synthetic::Edge>()
    );
    std::hint::black_box((&snapshot, &graph));
    println!("MARK hold");
    std::thread::sleep(std::time::Duration::from_secs(3));
}

#[derive(Default)]
struct Stats {
    string_len: usize,
    string_capacity: usize,
    string_instances: usize,
    span_count: usize,
}

fn add_string(stats: &mut Stats, value: &String) {
    stats.string_len += value.len();
    stats.string_capacity += value.capacity();
    stats.string_instances += 1;
}

fn add_optional_string(stats: &mut Stats, value: &Option<String>) {
    if let Some(value) = value {
        add_string(stats, value);
    }
}

fn add_span(stats: &mut Stats, value: &Option<SourceSpan>) {
    if let Some(span) = value {
        stats.span_count += 1;
        add_string(stats, &span.path);
    }
}

fn stats(facts: &RepositoryFacts) -> Stats {
    let mut stats = Stats::default();
    for node in &facts.nodes {
        add_optional_string(&mut stats, &node.external_identity);
        add_string(&mut stats, &node.path);
        add_string(&mut stats, &node.name);
        add_string(&mut stats, &node.qualified_name);
        add_span(&mut stats, &node.span);
    }
    for edge in &facts.edges {
        add_span(&mut stats, &edge.span);
    }
    for reference in &facts.unresolved {
        add_string(&mut stats, &reference.expression);
        add_optional_string(&mut stats, &reference.candidate_namespace);
        add_optional_string(&mut stats, &reference.candidate_name);
        if let UnresolvedReason::Unknown(value) = &reference.reason {
            add_string(&mut stats, value);
        }
        add_span(&mut stats, &reference.span);
    }
    stats
}
