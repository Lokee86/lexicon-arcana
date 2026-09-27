use std::env;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::time::Instant;

use lexicon::{AdapterHost, ScanEngine, StateRepository, Store};
use serde_json::json;
use sha2::{Digest, Sha256};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    match arguments.as_slice() {
        [command, repository, state_root, adapter_root, language] if command == "scan" => {
            scan(
                Path::new(repository),
                Path::new(state_root),
                Path::new(adapter_root),
                language,
            )
        }
        [command, state_root, output_root, snapshot, language] if command == "export" => {
            export(
                Path::new(state_root),
                Path::new(output_root),
                snapshot,
                language,
            )
        }
        _ => Err(
            "usage: lifecycle_profile scan <repository> <state-root> <adapter-root> <language>\n       lifecycle_profile export <state-root> <output-root> <snapshot> <language>"
                .into(),
        ),
    }
}

fn scan(
    repository: &Path,
    state_root: &Path,
    adapter_root: &Path,
    language: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(state_root)?;
    let state_repository_root = state_root.join("repo");
    let git = if state_repository_root.join(".git").is_dir() {
        StateRepository::open(&state_repository_root)?
    } else {
        StateRepository::ensure(&state_repository_root)?
    };
    let store = Store::new(state_root);
    let host = AdapterHost::new(adapter_root);
    let engine = ScanEngine::new(repository, git, store, host, vec![language.to_owned()]);

    let started = Instant::now();
    let report = engine.scan()?;
    let wall_ms = started.elapsed().as_secs_f64() * 1000.0;
    let (cas_object_count, cas_bytes) = tree_stats(&state_root.join("objects"))?;

    println!(
        "{}",
        json!({
            "mode": "scan",
            "wall_ms": wall_ms,
            "snapshot_id": report.snapshot_id,
            "changed_files": report.changed.len(),
            "languages": report.languages,
            "cas_object_count": cas_object_count,
            "cas_bytes": cas_bytes,
        })
    );
    Ok(())
}

fn export(
    state_root: &Path,
    output_root: &Path,
    snapshot: &str,
    language: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(output_root)?;
    let store = Store::new(state_root);
    store.export(snapshot, output_root, &[language.to_owned()])?;
    let path = output_root.join(format!("{language}.jsonl"));
    let (jsonl_bytes, jsonl_lines, sha256) = hash_file(&path)?;
    let fact_count = jsonl_lines.saturating_sub(1);
    let (cas_object_count, cas_bytes) = tree_stats(&state_root.join("objects"))?;

    println!(
        "{}",
        json!({
            "mode": "export",
            "snapshot_id": snapshot,
            "language": language,
            "jsonl_bytes": jsonl_bytes,
            "jsonl_lines": jsonl_lines,
            "fact_count": fact_count,
            "sha256": sha256,
            "cas_object_count": cas_object_count,
            "cas_bytes": cas_bytes,
        })
    );
    Ok(())
}

fn hash_file(path: &Path) -> Result<(u64, u64, String), io::Error> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut bytes = 0_u64;
    let mut lines = 0_u64;
    let mut buffer = vec![0_u8; 1024 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        let chunk = &buffer[..read];
        bytes += read as u64;
        lines += chunk.iter().filter(|byte| **byte == b'\n').count() as u64;
        hasher.update(chunk);
    }
    Ok((bytes, lines, format!("{:x}", hasher.finalize())))
}

fn tree_stats(root: &Path) -> Result<(u64, u64), io::Error> {
    if !root.exists() {
        return Ok((0, 0));
    }
    let mut count = 0_u64;
    let mut bytes = 0_u64;
    let mut stack = vec![PathBuf::from(root)];
    while let Some(directory) = stack.pop() {
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            if metadata.is_dir() {
                stack.push(entry.path());
            } else if metadata.is_file() {
                count += 1;
                bytes += metadata.len();
            }
        }
    }
    Ok((count, bytes))
}
