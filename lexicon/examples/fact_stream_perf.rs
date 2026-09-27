use std::hint::black_box;
use std::time::Instant;

use lexicon::{FactHeader, FactRecord, FactStream, NodeRecord};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let small = parse_size(args.next(), 4_096)?;
    let large = parse_size(args.next(), 16_384)?;
    if args.next().is_some() || small == 0 || large <= small {
        return Err("expected optional SMALL LARGE with 0 < SMALL < LARGE".into());
    }

    let small_ms = median_sort_ms(small);
    let large_ms = median_sort_ms(large);
    println!(
        "{}",
        serde_json::json!({
            "small_records": small,
            "large_records": large,
            "small_ms": small_ms,
            "large_ms": large_ms,
            "scale_ratio": large_ms / small_ms.max(0.001),
        })
    );
    Ok(())
}

fn median_sort_ms(count: usize) -> f64 {
    let mut samples = [0.0; 3];
    for sample in &mut samples {
        let mut stream = FactStream {
            header: benchmark_header(),
            records: records(count),
        };
        let started = Instant::now();
        stream.sort_records();
        black_box(&stream.records);
        *sample = started.elapsed().as_secs_f64() * 1_000.0;
    }
    samples.sort_by(f64::total_cmp);
    samples[1]
}

fn benchmark_header() -> FactHeader {
    FactHeader {
        adapter_version: "bench".into(),
        changed_files: None,
        language: "go".into(),
        mode: None,
        record: "header".into(),
        removed_files: None,
        repository: "bench".into(),
        schema_version: 1,
        shared_complete: None,
    }
}

fn records(count: usize) -> Vec<FactRecord> {
    (0..count)
        .rev()
        .map(|index| {
            FactRecord::Node(NodeRecord {
                attributes: None,
                content_id: None,
                id: format!("{index:064x}"),
                kind: "function".into(),
                name: format!("f{index:08}"),
                owner: Some("bench.go".into()),
                path: "bench.go".into(),
                qualified_name: format!("bench::f{index:08}"),
                span: None,
            })
        })
        .collect()
}

fn parse_size(value: Option<String>, default: usize) -> Result<usize, Box<dyn std::error::Error>> {
    value.map_or(Ok(default), |value| Ok(value.parse()?))
}
