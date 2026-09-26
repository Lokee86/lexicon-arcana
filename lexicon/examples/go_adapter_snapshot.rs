use std::{env, fs, path::PathBuf, time::Instant};

use lexicon::{AdapterHost, AdapterRequest, FactStream};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args_os().skip(1);
    let repository = PathBuf::from(args.next().ok_or("repository path is required")?);
    let output = PathBuf::from(args.next().ok_or("output path is required")?);
    let workers = parse_usize(args.next(), "workers", 1)?;
    let shards = parse_usize(args.next(), "shards", 1)?;
    let merge_fan_in = parse_usize(args.next(), "merge fan-in", 2)?;
    if args.next().is_some() {
        return Err("too many arguments".into());
    }

    let adapter_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("adapters");
    let host = AdapterHost::new(adapter_root);
    let analysis = host.analyze(&AdapterRequest {
        language: "go".into(),
        repository,
        workers,
        shards,
        merge_fan_in,
        ..AdapterRequest::default()
    })?;
    let stream = FactStream {
        header: analysis.header,
        records: analysis.records,
    };
    let storage_started = performance_enabled().then(Instant::now);
    let final_fact_count = stream.records.len();
    let bytes = stream.canonical_jsonl()?;
    let output_bytes = bytes.len();
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(output, bytes)?;
    if let Some(storage_started) = storage_started {
        eprintln!(
            "[lexicon-perf] stage=go.final_serialization_storage elapsed_ms={:.3} final_fact_count={} output_bytes={}",
            storage_started.elapsed().as_secs_f64() * 1000.0,
            final_fact_count,
            output_bytes
        );
    }
    Ok(())
}

fn performance_enabled() -> bool {
    let Some(value) = env::var_os("LEXICON_PERF") else {
        return false;
    };
    !matches!(
        value.to_string_lossy().trim().to_ascii_lowercase().as_str(),
        "" | "0" | "false" | "off" | "no"
    )
}

fn parse_usize(
    value: Option<std::ffi::OsString>,
    label: &str,
    default: usize,
) -> Result<usize, Box<dyn std::error::Error>> {
    let Some(value) = value else {
        return Ok(default);
    };
    let value = value
        .into_string()
        .map_err(|_| format!("{label} is not valid UTF-8"))?;
    let parsed = value
        .parse::<usize>()
        .map_err(|error| format!("invalid {label} {value:?}: {error}"))?;
    Ok(parsed)
}
