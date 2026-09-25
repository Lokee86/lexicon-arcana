mod discovery;
mod facts;
mod mask;
mod parse;

use crate::{
    AdapterError, AdapterRequest, Analysis, FACT_SCHEMA_VERSION, FactHeader, LanguageAdapter,
};

use discovery::collect_sources;
use facts::Facts;

pub const ADAPTER_VERSION: &str = "0.2.0";

#[derive(Debug, Default)]
pub struct GenericAdapter;

impl LanguageAdapter for GenericAdapter {
    fn implementation_version(&self) -> &'static str {
        ADAPTER_VERSION
    }

    fn implementation_fingerprint(&self) -> String {
        super::fingerprint::source_fingerprint(
            ADAPTER_VERSION,
            &[
                ("mod.rs", include_bytes!("mod.rs")),
                ("discovery.rs", include_bytes!("discovery.rs")),
                ("facts.rs", include_bytes!("facts.rs")),
                ("mask.rs", include_bytes!("mask.rs")),
                ("parse.rs", include_bytes!("parse.rs")),
            ],
        )
    }

    fn analyze(&self, request: &AdapterRequest) -> Result<Analysis, AdapterError> {
        let extension = language_extension(&request.language)?;

        let root = std::fs::canonicalize(&request.repository)
            .map_err(|error| AdapterError::new(format!("resolve repository: {error}")))?;
        if !root.is_dir() {
            return Err(AdapterError::new("repository path is not a directory"));
        }

        let paths = collect_sources(
            &root,
            &extension,
            &request.changed_files,
            request.mode == crate::AdapterMode::Incremental,
        )?;
        let mut facts = Facts::new(request.language.clone());
        for path in paths {
            let absolute = root.join(path.replace('/', std::path::MAIN_SEPARATOR_STR));
            let content = std::fs::read(&absolute)
                .map_err(|error| AdapterError::new(format!("read {path}: {error}")))?;
            if !discovery::source_text(&content) || discovery::generated_source(&content) {
                continue;
            }
            let module_id = facts.add_file(&path, &content);
            parse::parse_source(
                &mut facts,
                &path,
                &module_id,
                std::str::from_utf8(&content).expect("source_text validated UTF-8"),
            );
        }

        let repository = root
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("repository")
            .to_owned();
        let incremental = request.mode == crate::AdapterMode::Incremental;
        let header = FactHeader {
            adapter_version: ADAPTER_VERSION.into(),
            changed_files: incremental.then(|| normalized(&request.changed_files)),
            language: request.language.clone(),
            mode: incremental.then(|| "incremental".into()),
            record: "lexicon".into(),
            removed_files: incremental.then(|| normalized(&request.removed_files)),
            repository,
            schema_version: FACT_SCHEMA_VERSION,
            shared_complete: incremental.then_some(false),
        };
        Ok(Analysis::new(header, facts.into_records()))
    }
}

fn language_extension(language: &str) -> Result<String, AdapterError> {
    let suffix = language
        .strip_prefix("generic-")
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| AdapterError::new(format!("invalid generic language {language:?}")))?;
    const SUPPORTED: &[&str] = &[
        "asm", "bash", "bat", "c", "cc", "clj", "cljs", "cmd", "cpp", "cr", "cs", "dart", "elm",
        "erl", "ex", "exs", "f03", "f90", "f95", "fish", "fs", "fsx", "groovy", "h", "hh", "hpp",
        "hs", "java", "jl", "kt", "kts", "lhs", "lua", "m", "ml", "mli", "mm", "nim", "nims",
        "pas", "php", "pl", "pm", "proto", "ps1", "r", "s", "sc", "scala", "sh", "sol", "sql",
        "sv", "swift", "v", "vb", "vbs", "vim", "zig",
    ];
    if !SUPPORTED.contains(&suffix.as_str()) {
        return Err(AdapterError::new(format!(
            "unsupported generic source extension {suffix:?}"
        )));
    }
    Ok(format!(".{suffix}"))
}

fn normalized(paths: &[String]) -> Vec<String> {
    let mut values = paths
        .iter()
        .map(|path| path.replace('\\', "/"))
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
}
