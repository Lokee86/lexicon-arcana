use std::path::Path;

use crate::{AdapterMode, AdapterRequest, Analysis, FACT_SCHEMA_VERSION, FactHeader};

use super::ADAPTER_VERSION;

pub(crate) fn empty_analysis(repository: &Path, request: &AdapterRequest) -> Analysis {
    let incremental = request.mode == AdapterMode::Incremental;
    Analysis::new(
        FactHeader {
            adapter_version: ADAPTER_VERSION.into(),
            changed_files: incremental.then(|| normalized(&request.changed_files)),
            language: "go".into(),
            mode: incremental.then(|| "incremental".into()),
            record: "lexicon".into(),
            removed_files: incremental.then(|| normalized(&request.removed_files)),
            repository: repository_name(repository),
            schema_version: FACT_SCHEMA_VERSION,
            shared_complete: incremental.then_some(false),
        },
        Vec::new(),
    )
}

fn repository_name(repository: &Path) -> String {
    repository
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("repository")
        .to_owned()
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
