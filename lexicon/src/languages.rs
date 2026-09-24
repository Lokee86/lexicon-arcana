mod definition;

use std::path::Path;

pub use definition::LanguageDefinition;

use definition::{static_definitions, static_lookup};

const GENERIC_EXTENSIONS: &[&str] = &[
    ".asm", ".bash", ".bat", ".clj", ".cljs", ".cmd", ".cr", ".dart", ".elm", ".erl", ".ex",
    ".exs", ".f03", ".f90", ".f95", ".fish", ".fs", ".fsx", ".groovy", ".hs", ".jl", ".lhs",
    ".lua", ".m", ".ml", ".mli", ".mm", ".nim", ".nims", ".pas", ".php", ".pl", ".pm", ".proto",
    ".ps1", ".r", ".scala", ".sc", ".s", ".sh", ".sol", ".sql", ".swift", ".sv", ".v", ".vb",
    ".vbs", ".vim", ".zig",
];

pub fn definitions() -> Vec<LanguageDefinition> {
    static_definitions().collect()
}

pub fn lookup(language: &str) -> Option<LanguageDefinition> {
    if let Some(definition) = static_lookup(language) {
        return Some(definition);
    }
    generic_extension(language).map(|extension| LanguageDefinition {
        language: language.to_owned(),
        directory: "generic".to_owned(),
        extensions: vec![extension],
        config_files: Vec::new(),
        partitioned_execution: false,
    })
}

pub fn for_path(path: &str) -> Vec<String> {
    let path = Path::new(path);
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let extension = extension(path);
    let mut result = Vec::new();
    for definition in static_definitions().filter(|definition| definition.language != "generic") {
        if definition.extensions.contains(&extension)
            || definition
                .config_files
                .iter()
                .any(|config| config == name || (config.starts_with('.') && *config == extension))
        {
            result.push(definition.language);
        }
    }
    if result.is_empty() && GENERIC_EXTENSIONS.contains(&extension.as_str()) {
        result.push(format!("generic-{}", extension.trim_start_matches('.')));
    }
    result.sort();
    result
}

pub fn owns_source(language: &str, path: &str) -> bool {
    let extension = extension(Path::new(path));
    lookup(language).is_some_and(|definition| definition.extensions.contains(&extension))
}

pub fn supported_languages() -> Vec<String> {
    let mut values: Vec<String> = static_definitions()
        .map(|definition| definition.language)
        .collect();
    values.sort();
    values
}

pub fn supported(language: &str) -> bool {
    lookup(language).is_some()
}

pub fn supports_partitioned_execution(language: &str) -> bool {
    lookup(language).is_some_and(|definition| definition.partitioned_execution)
}

pub fn language_enabled(language: &str, enabled: &[String]) -> bool {
    if !supported(language) {
        return false;
    }
    enabled.is_empty()
        || enabled.iter().any(|candidate| {
            candidate == language || (candidate == "generic" && is_generic(language))
        })
}

pub fn is_generic(language: &str) -> bool {
    generic_extension(language).is_some()
}

fn generic_extension(language: &str) -> Option<String> {
    let suffix = language.strip_prefix("generic-")?.to_ascii_lowercase();
    let extension = format!(".{suffix}");
    GENERIC_EXTENSIONS
        .contains(&extension.as_str())
        .then_some(extension)
}

fn extension(path: &Path) -> String {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|value| format!(".{}", value.to_ascii_lowercase()))
        .unwrap_or_default()
}
