use super::{
    discovery::{collect_sources, is_ambiguous_header_path},
    language::{classify_language, infer_header_languages, load_compile_languages},
};
use crate::AdapterError;
use std::{collections::HashMap, fs, path::Path};
use tree_sitter::{Node, Parser};

#[derive(Debug)]
pub struct ParsedFile {
    pub path: String,
    pub language: String,
    pub parser_language: String,
    pub content: Vec<u8>,
    pub parse_error: bool,
}

pub fn parse_repository(root: &Path) -> Result<Vec<ParsedFile>, AdapterError> {
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
        let (parser_language, parse_error) = parse_source(
            &content,
            &language,
            is_ambiguous_header_path(&path),
            &mut c_parser,
            &mut cpp_parser,
        )?;
        files.push(ParsedFile {
            path,
            language,
            parser_language,
            content,
            parse_error,
        });
    }
    Ok(files)
}

fn parse_source(
    content: &[u8],
    language: &str,
    allow_fallback: bool,
    c_parser: &mut Parser,
    cpp_parser: &mut Parser,
) -> Result<(String, bool), AdapterError> {
    let (primary, alternate, alternate_language) = if language == "cpp" {
        (cpp_parser, c_parser, "c")
    } else {
        (c_parser, cpp_parser, "cpp")
    };

    let tree = primary
        .parse(content, None)
        .ok_or_else(|| AdapterError::new("tree-sitter returned no tree"))?;
    let primary_error = tree.root_node().has_error();
    if !allow_fallback || !primary_error {
        return Ok((language.into(), primary_error));
    }

    let fallback = alternate
        .parse(content, None)
        .ok_or_else(|| AdapterError::new("tree-sitter returned no fallback tree"))?;
    let primary_score = syntax_error_score(tree.root_node());
    let fallback_score = syntax_error_score(fallback.root_node());
    if fallback_score < primary_score {
        Ok((alternate_language.into(), fallback.root_node().has_error()))
    } else {
        Ok((language.into(), primary_error))
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
