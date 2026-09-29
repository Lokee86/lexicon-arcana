use std::{collections::HashMap, fs, path::Path};

use crate::{AdapterError, node_id};

mod declarations;
mod references;
mod semantics;
#[cfg(test)]
mod tests;
mod value_flow;

use super::{
    clang_protocol::{FileObservation, StructuralResponse},
    includes::FileIndex,
    model::{RepositoryModel, SourceFile},
    visibility::VisibilityIndex,
};

pub(crate) fn materialize(
    root: &Path,
    response: &StructuralResponse,
) -> Result<RepositoryModel, AdapterError> {
    let root = root.canonicalize().map_err(|error| {
        AdapterError::new(format!(
            "resolve C-family repository {}: {error}",
            root.display()
        ))
    })?;
    let mut files = response.files.iter().collect::<Vec<_>>();
    files.sort_by(|left, right| left.path.cmp(&right.path));

    let ids = files
        .iter()
        .map(|file| (file.path.clone(), declarations::identity_map(file)))
        .collect::<HashMap<_, _>>();
    let references = references::ReferenceIndex::new(&files, &ids);

    let mut materialized = Vec::with_capacity(files.len());
    for file in files {
        let local_ids = ids
            .get(&file.path)
            .expect("C-family Clang local identity map");
        materialized.push(materialize_file(&root, file, local_ids, &ids, &references)?);
    }
    let index = FileIndex::new(&materialized);
    let visibility = VisibilityIndex::new(&materialized, &index);
    Ok(RepositoryModel {
        repository: root
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("repository")
            .into(),
        files: materialized,
        visibility,
    })
}

fn materialize_file(
    root: &Path,
    observation: &FileObservation,
    ids: &HashMap<String, String>,
    all_ids: &references::IdentityMaps,
    references: &references::ReferenceIndex,
) -> Result<SourceFile, AdapterError> {
    validate_relative(&observation.path)?;
    let content = fs::read(root.join(observation.path.replace('/', std::path::MAIN_SEPARATOR_STR)))
        .map_err(|error| AdapterError::new(format!("read {}: {error}", observation.path)))?;
    let language = source_language(observation, &content);
    let module_id = node_id("c-family", "module", &observation.path);
    let parse_error = observation
        .diagnostics
        .iter()
        .any(|value| matches!(value.severity.as_str(), "error" | "fatal"));

    let declarations = declarations::materialize(observation, &language, &module_id, ids);
    let (semantic_relationships, semantic_calls) =
        semantics::materialize(observation, all_ids, references)?;
    let (semantic_pointer_bindings, semantic_accesses) =
        value_flow::materialize(observation, ids, references)?;
    let mut includes = observation
        .includes
        .iter()
        .map(|value| super::model::IncludeObservation {
            id: node_id(
                "c-family",
                "import",
                &format!(
                    "{}::include::{}::{}",
                    observation.path, value.target, value.offset
                ),
            ),
            module_id: module_id.clone(),
            path: observation.path.clone(),
            target: value.target.clone(),
            resolved_path: value.resolved_path.clone(),
            expression: value.expression.clone(),
            system: value.system,
            span: value.span.clone(),
        })
        .collect::<Vec<_>>();
    includes.sort_by(|left, right| {
        (left.span.start_line, left.span.start_column, &left.target).cmp(&(
            right.span.start_line,
            right.span.start_column,
            &right.target,
        ))
    });

    Ok(SourceFile {
        path: observation.path.clone(),
        language: language.clone(),
        parser: "clang".into(),
        parser_language: language,
        content,
        parse_error,
        declarations,
        includes,
        inheritance: Vec::new(),
        calls: Vec::new(),
        semantic_relationships,
        semantic_calls,
        semantic_pointer_bindings,
        semantic_accesses,
        pointer_bindings: Vec::new(),
        accesses: Vec::new(),
    })
}

fn source_language(file: &FileObservation, content: &[u8]) -> String {
    let has_c = file.languages.iter().any(|value| value == "c");
    let has_cpp = file.languages.iter().any(|value| value == "cpp");
    match (has_c, has_cpp) {
        (true, false) => "c".into(),
        (false, true) => "cpp".into(),
        (true, true)
            if file
                .translation_units
                .iter()
                .any(|translation_unit| translation_unit != &file.path) =>
        {
            "c".into()
        }
        _ => super::language::classify_language(
            &file.path,
            content,
            &HashMap::new(),
            &HashMap::new(),
        ),
    }
}

fn validate_relative(path: &str) -> Result<(), AdapterError> {
    let value = Path::new(path);
    if path.is_empty() || value.is_absolute() || path.contains('\\') {
        return Err(AdapterError::new(format!(
            "Clang observation path is not canonical repository-relative form: {path:?}"
        )));
    }
    if value
        .components()
        .any(|part| matches!(part, std::path::Component::ParentDir))
    {
        return Err(AdapterError::new(format!(
            "Clang observation path escapes repository: {path:?}"
        )));
    }
    Ok(())
}
