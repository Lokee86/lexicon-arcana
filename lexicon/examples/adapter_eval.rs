use std::{env, fs, path::PathBuf};

use lexicon::{AdapterHost, AdapterMode, AdapterRequest, FactStream};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let language = args.next().ok_or("missing language")?;
    let repository = PathBuf::from(args.next().ok_or("missing repository")?);
    let output = PathBuf::from(args.next().ok_or("missing output")?);
    if args.next().is_some() {
        return Err("usage: adapter_eval <language> <repository> <output>".into());
    }

    let host = AdapterHost::new(repository.join(".lexicon-adapters"));
    let analysis = host.analyze(&AdapterRequest {
        language,
        mode: AdapterMode::Full,
        repository,
        ..Default::default()
    })?;
    let stream = FactStream {
        header: analysis.header,
        records: analysis.records,
    };
    let jsonl = stream.canonical_jsonl()?;

    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(output, jsonl)?;
    Ok(())
}
