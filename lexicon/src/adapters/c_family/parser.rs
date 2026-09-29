use super::{
    declarations,
    discovery::{collect_sources, is_ambiguous_header_path},
    includes::FileIndex,
    language::{classify_language, infer_header_languages, load_compile_languages},
    model::{RepositoryModel, SourceFile},
    visibility::VisibilityIndex,
};
use crate::AdapterError;
use std::{collections::HashMap, fs, path::Path};
use tree_sitter::{Node, Parser, Tree};

pub fn parse_repository(root: &Path) -> Result<RepositoryModel, AdapterError> {
    let paths = collect_sources(root)?;
    let compile_languages = load_compile_languages(root);
    let mut contents = HashMap::with_capacity(paths.len());
    for path in &paths {
        let content = fs::read(root.join(path.replace('/', std::path::MAIN_SEPARATOR_STR)))
            .map_err(|error| AdapterError::new(format!("read {path}: {error}")))?;
        contents.insert(path.clone(), content);
    }
    let header_languages = infer_header_languages(&paths, &contents, &compile_languages);

    let mut c_parser = Parser::new();
    c_parser
        .set_language(&tree_sitter_c::LANGUAGE.into())
        .map_err(|error| AdapterError::new(format!("configure C parser: {error}")))?;
    let mut cpp_parser = Parser::new();
    cpp_parser
        .set_language(&tree_sitter_cpp::LANGUAGE.into())
        .map_err(|error| AdapterError::new(format!("configure C++ parser: {error}")))?;

    let mut files = Vec::with_capacity(paths.len());
    for path in paths {
        let content = contents.remove(&path).unwrap_or_default();
        let language = classify_language(&path, &content, &compile_languages, &header_languages);
        let (tree, parser_language) = parse_source(
            &content,
            &language,
            is_ambiguous_header_path(&path),
            &mut c_parser,
            &mut cpp_parser,
        )?;
        let mut file = SourceFile {
            path,
            language,
            parser: "tree-sitter".into(),
            parser_language,
            parse_error: tree.root_node().has_error(),
            content,
            declarations: Vec::new(),
            includes: Vec::new(),
            inheritance: Vec::new(),
            calls: Vec::new(),
            semantic_relationships: Vec::new(),
            semantic_calls: Vec::new(),
            pointer_bindings: Vec::new(),
            accesses: Vec::new(),
        };
        declarations::extract(&mut file, tree.root_node());
        files.push(file);
    }
    super::pointer_aliases::propagate(&mut files);
    let file_index = FileIndex::new(&files);
    let visibility = VisibilityIndex::new(&files, &file_index);
    Ok(RepositoryModel {
        repository: root
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("repository")
            .into(),
        files,
        visibility,
    })
}

fn parse_source(
    content: &[u8],
    language: &str,
    allow_fallback: bool,
    c_parser: &mut Parser,
    cpp_parser: &mut Parser,
) -> Result<(Tree, String), AdapterError> {
    let (primary, alternate, alternate_language) = if language == "cpp" {
        (cpp_parser, c_parser, "c")
    } else {
        (c_parser, cpp_parser, "cpp")
    };

    let tree = primary
        .parse(content, None)
        .ok_or_else(|| AdapterError::new("tree-sitter returned no tree"))?;
    if !allow_fallback || !tree.root_node().has_error() {
        return Ok((tree, language.into()));
    }

    let fallback = alternate
        .parse(content, None)
        .ok_or_else(|| AdapterError::new("tree-sitter returned no fallback tree"))?;
    if syntax_error_score(fallback.root_node()) < syntax_error_score(tree.root_node()) {
        Ok((fallback, alternate_language.into()))
    } else {
        Ok((tree, language.into()))
    }
}

fn syntax_error_score(node: Node<'_>) -> usize {
    let mut score = usize::from(node.is_missing());
    if node.is_error() {
        score += 10;
    }
    for index in 0..node.child_count() {
        if let Some(child) = node.child(index) {
            score += syntax_error_score(child);
        }
    }
    score
}
