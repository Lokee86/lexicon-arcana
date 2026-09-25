use super::discovery::{extension, is_header_path};
use regex::Regex;
use serde::Deserialize;
use std::{
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    fs,
    path::{Path, PathBuf},
    sync::OnceLock,
};

#[derive(Debug, Default, Deserialize)]
struct CompileCommand {
    #[serde(default)]
    arguments: Vec<String>,
    #[serde(default)]
    command: String,
    #[serde(default)]
    directory: String,
    file: String,
}

pub fn load_compile_languages(root: &Path) -> HashMap<String, String> {
    let Ok(data) = fs::read(root.join("compile_commands.json")) else {
        return HashMap::new();
    };
    let Ok(commands) = serde_json::from_slice::<Vec<CompileCommand>>(&data) else {
        return HashMap::new();
    };

    let mut result = HashMap::new();
    for command in commands {
        let file = command_path(root, &command);
        let normalized = fs::canonicalize(&file).unwrap_or(file);
        let Ok(relative) = normalized.strip_prefix(root) else {
            continue;
        };
        result.insert(path_text(relative), command_language(&command));
    }
    result
}

pub fn infer_header_languages(
    paths: &[String],
    contents: &HashMap<String, Vec<u8>>,
    compile_languages: &HashMap<String, String>,
) -> HashMap<String, String> {
    let path_set = paths.iter().cloned().collect::<HashSet<_>>();
    let mut by_base = BTreeMap::<String, Vec<String>>::new();
    for path in paths {
        by_base
            .entry(base_name(path).to_ascii_lowercase())
            .or_default()
            .push(path.clone());
    }
    for values in by_base.values_mut() {
        values.sort();
    }

    let pattern = quoted_include_pattern();
    let mut includes = HashMap::<String, Vec<String>>::new();
    for path in paths {
        let source = String::from_utf8_lossy(contents.get(path).map(Vec::as_slice).unwrap_or(&[]));
        let mut targets = Vec::new();
        for captures in pattern.captures_iter(&source) {
            if let Some(target) = resolve_source_include(path, &captures[1], &path_set, &by_base) {
                targets.push(target);
            }
        }
        targets.sort();
        includes.insert(path.clone(), targets);
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
    struct Evidence {
        c: bool,
        cpp: bool,
    }

    let mut evidence = HashMap::<String, Evidence>::new();
    let mut queue = VecDeque::new();
    for path in paths {
        if is_header_path(path) {
            continue;
        }
        let language = source_file_language(path, compile_languages);
        evidence.insert(
            path.clone(),
            Evidence {
                c: language == "c",
                cpp: language == "cpp",
            },
        );
        queue.push_back(path.clone());
    }

    while let Some(path) = queue.pop_front() {
        let current = evidence.get(&path).copied().unwrap_or_default();
        for target in includes.get(&path).into_iter().flatten() {
            if !is_header_path(target) {
                continue;
            }
            let previous = evidence.get(target).copied().unwrap_or_default();
            let next = Evidence {
                c: previous.c || current.c,
                cpp: previous.cpp || current.cpp,
            };
            if next != previous {
                evidence.insert(target.clone(), next);
                queue.push_back(target.clone());
            }
        }
    }

    let mut result = HashMap::new();
    for (path, value) in evidence {
        if !is_header_path(&path) {
            continue;
        }
        if value.c {
            result.insert(path, "c".into());
        } else if value.cpp {
            result.insert(path, "cpp".into());
        }
    }
    result
}

pub fn classify_language(
    path: &str,
    content: &[u8],
    compile_languages: &HashMap<String, String>,
    header_languages: &HashMap<String, String>,
) -> String {
    if let Some(language) = compile_languages.get(path) {
        return language.clone();
    }
    if let Some(language) = header_languages.get(path) {
        return language.clone();
    }

    let suffix = extension(path);
    if suffix == "C" {
        return "cpp".into();
    }
    if suffix == "c" {
        return "c".into();
    }
    if matches!(
        suffix.as_str(),
        "hh" | "hpp" | "hxx" | "h++" | "inl" | "ipp" | "tpp"
    ) {
        return "cpp".into();
    }
    if !matches!(suffix.as_str(), "h" | "inc") {
        return "cpp".into();
    }

    let text = format!("\n{}\n", String::from_utf8_lossy(content));
    const CPP_MARKERS: &[&str] = &[
        "namespace ",
        "class ",
        "template<",
        "template <",
        "constexpr ",
        "std::",
        "public:",
        "private:",
        "protected:",
        " override",
        " virtual ",
        "nullptr",
        "decltype(",
        "using namespace ",
        "::",
    ];
    if CPP_MARKERS.iter().any(|marker| text.contains(marker)) {
        "cpp".into()
    } else {
        "c".into()
    }
}

fn source_file_language(path: &str, compile_languages: &HashMap<String, String>) -> String {
    if let Some(language) = compile_languages.get(path) {
        return language.clone();
    }
    if extension(path) == "C" || extension(path) != "c" {
        "cpp".into()
    } else {
        "c".into()
    }
}

fn resolve_source_include(
    source_path: &str,
    target: &str,
    paths: &HashSet<String>,
    by_base: &BTreeMap<String, Vec<String>>,
) -> Option<String> {
    let target = lexical_path(target);
    if paths.contains(&target) {
        return Some(target);
    }

    let parent = Path::new(source_path)
        .parent()
        .unwrap_or_else(|| Path::new(""));
    let relative = lexical_path(
        &parent
            .join(target.replace('/', std::path::MAIN_SEPARATOR_STR))
            .to_string_lossy(),
    );
    if !relative.starts_with("../") && paths.contains(&relative) {
        return Some(relative);
    }

    let matches = by_base.get(&base_name(&target).to_ascii_lowercase())?;
    (matches.len() == 1).then(|| matches[0].clone())
}

fn command_path(root: &Path, command: &CompileCommand) -> PathBuf {
    let file = PathBuf::from(command.file.replace('/', std::path::MAIN_SEPARATOR_STR));
    if file.is_absolute() {
        return file;
    }
    if command.directory.is_empty() {
        return root.join(file);
    }
    let directory = PathBuf::from(
        command
            .directory
            .replace('/', std::path::MAIN_SEPARATOR_STR),
    );
    if directory.is_absolute() {
        directory.join(file)
    } else {
        root.join(directory).join(file)
    }
}

fn command_language(command: &CompileCommand) -> String {
    let joined =
        format!("{} {}", command.command, command.arguments.join(" ")).to_ascii_lowercase();
    if ["-x c++", "clang++", "g++", " c++", "cpp"]
        .iter()
        .any(|marker| joined.contains(marker))
    {
        "cpp".into()
    } else {
        "c".into()
    }
}

fn quoted_include_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r#"(?m)^\s*#\s*include\s*"([^"]+)""#).unwrap())
}

fn base_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn path_text(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn lexical_path(value: &str) -> String {
    let normalized = value.replace('\\', "/");
    let mut parts = Vec::new();
    for part in normalized.split('/') {
        match part {
            "" | "." => {}
            ".." if parts.last().is_some_and(|last| *last != "..") => {
                parts.pop();
            }
            ".." => parts.push(".."),
            value => parts.push(value),
        }
    }
    parts.join("/")
}
