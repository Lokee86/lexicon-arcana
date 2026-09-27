#[path = "multilang_perf_fixtures/mod.rs"]
mod multilang_perf_fixtures;

use std::collections::BTreeMap;
use std::time::Instant;

use lexicon::{AdapterHost, AdapterRequest, FactRecord, FactStream};
use multilang_perf_fixtures::FixtureSet;
use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Serialize)]
struct ResultRow {
    wall_ms: f64,
    fact_count: usize,
    nodes: usize,
    edges: usize,
    unresolved: usize,
    jsonl_bytes: usize,
    sha256: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fixtures = FixtureSet::create()?;
    let adapter_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("adapters");
    let host = AdapterHost::new(adapter_root);
    let mut results = BTreeMap::new();

    for (name, language, repository) in &fixtures.cases {
        let started = Instant::now();
        let analysis = host.analyze(&AdapterRequest {
            language: (*language).into(),
            repository: repository.clone(),
            workers: 2,
            shards: 4,
            merge_fan_in: 2,
            ..Default::default()
        })?;
        let wall_ms = started.elapsed().as_secs_f64() * 1000.0;
        let (nodes, edges, unresolved) = counts(&analysis.records);
        let stream = FactStream {
            header: analysis.header,
            records: analysis.records,
        };
        let jsonl = stream.canonical_jsonl()?;
        let digest = format!("{:X}", Sha256::digest(&jsonl));
        results.insert(
            *name,
            ResultRow {
                wall_ms,
                fact_count: nodes + edges + unresolved,
                nodes,
                edges,
                unresolved,
                jsonl_bytes: jsonl.len(),
                sha256: digest,
            },
        );
    }

    println!("{}", serde_json::to_string(&results)?);
    Ok(())
}

fn counts(records: &[FactRecord]) -> (usize, usize, usize) {
    let mut nodes = 0;
    let mut edges = 0;
    let mut unresolved = 0;
    for record in records {
        match record {
            FactRecord::Node(_) => nodes += 1,
            FactRecord::Edge(_) => edges += 1,
            FactRecord::Unresolved(_) => unresolved += 1,
        }
    }
    (nodes, edges, unresolved)
}
