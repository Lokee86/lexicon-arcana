use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use anyhow::Result;

use crate::{Analysis, FACT_SCHEMA_VERSION, FactHeader};

use super::contract::Facts;
use super::model::Context;
use super::{
    dataflow, dependencies, discovery, extractor, parser, semantic_facts, semantic_outcomes,
};

pub(crate) fn analyze(
    repo: &Path,
    changed_files: Option<&[String]>,
    removed_files: Option<&[String]>,
) -> Result<Analysis> {
    let metadata = discovery::load_metadata(repo)?;
    let repository = discovery::repository_identity(repo, &metadata);
    let sources = parser::parse_sources(repo)?;
    let mut context = Context {
        repo: repo.to_path_buf(),
        repository: repository.clone(),
        sources,
        facts: Facts::new(),
        crates: Vec::new(),
        modules: BTreeMap::new(),
        symbols: BTreeMap::new(),
        types: BTreeMap::new(),
        traits: BTreeMap::new(),
        macros: BTreeMap::new(),
        constructors: BTreeMap::new(),
        constructor_types: BTreeMap::new(),
        type_aliases: BTreeMap::new(),
        value_types: BTreeMap::new(),
        type_qn_by_id: BTreeMap::new(),
        trait_qn_by_id: BTreeMap::new(),
        function_qn_by_id: BTreeMap::new(),
        functions: BTreeMap::new(),
        methods: Vec::new(),
        method_index: BTreeMap::new(),
        trait_method_index: BTreeMap::new(),
        trait_method_ids: Default::default(),
        type_traits: BTreeMap::new(),
        fields: BTreeMap::new(),
        field_ids: BTreeMap::new(),
        pending_impls: Vec::new(),
        pending_imports: Vec::new(),
        imports: BTreeMap::new(),
        closure_ids: BTreeMap::new(),
        propagated_parameters: BTreeMap::new(),
        propagated_captures: BTreeMap::new(),
        return_values: BTreeMap::new(),
        processed: HashSet::new(),
    };

    discovery::add_repository_and_files(&mut context);
    discovery::add_crates(&mut context, &metadata);
    dependencies::add_dependencies(&mut context, &metadata);
    extractor::extract(&mut context);
    semantic_facts::emit(&mut context);
    semantic_outcomes::emit(&mut context);
    dataflow::emit(&mut context);

    let incremental = changed_files.is_some() || removed_files.is_some();
    Ok(Analysis::new(
        FactHeader {
            adapter_version: super::ADAPTER_VERSION.into(),
            changed_files: incremental.then(|| normalized(changed_files.unwrap_or_default())),
            language: "rust".into(),
            mode: incremental.then(|| "incremental".into()),
            record: "lexicon".into(),
            removed_files: incremental.then(|| normalized(removed_files.unwrap_or_default())),
            repository,
            schema_version: FACT_SCHEMA_VERSION,
            shared_complete: incremental.then_some(true),
        },
        context.facts.into_records(),
    ))
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
