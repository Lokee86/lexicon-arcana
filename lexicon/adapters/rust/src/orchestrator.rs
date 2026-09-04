use crate::contract::Facts;
use crate::discovery;
use crate::emit;
use crate::extractor;
use crate::model::Context;
use crate::parser;
use anyhow::Result;
use std::collections::{BTreeMap, HashSet};
use std::path::Path;

pub(crate) fn generate(
    repo: &Path,
    changed_files: Option<&[String]>,
    removed_files: Option<&[String]>,
) -> Result<String> {
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
    crate::dependencies::add_dependencies(&mut context, &metadata);
    extractor::extract(&mut context);
    crate::semantic_facts::emit(&mut context);
    crate::semantic_outcomes::emit(&mut context);
    crate::dataflow::emit(&mut context);
    emit::render(&context, &repository, changed_files, removed_files)
}
