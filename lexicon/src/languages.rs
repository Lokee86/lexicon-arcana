use std::path::Path;

const DEFINITIONS: &[(&str, &[&str], &[&str])] = &[
    (
        "c-family",
        &[
            ".c", ".cc", ".cp", ".cpp", ".cxx", ".c++", ".h", ".hh", ".hpp", ".hxx", ".h++",
            ".inc", ".inl", ".ipp", ".tpp",
        ],
        &["compile_commands.json", "CMakeLists.txt"],
    ),
    ("gdscript", &[".gd"], &["project.godot"]),
    ("go", &[".go"], &["go.mod", "go.sum"]),
    (
        "csharp",
        &[".cs"],
        &[
            ".sln",
            ".csproj",
            "Directory.Build.props",
            "Directory.Build.targets",
            "global.json",
        ],
    ),
    (
        "java",
        &[".java"],
        &[
            "pom.xml",
            "build.gradle",
            "settings.gradle",
            "gradlew",
            "gradlew.bat",
            "mvnw",
            "mvnw.cmd",
        ],
    ),
    (
        "kotlin",
        &[".kt", ".kts"],
        &["build.gradle.kts", "settings.gradle.kts"],
    ),
    ("lotusscript", &[".ls", ".lsa", ".lsdb", ".lss"], &[]),
    (
        "python",
        &[".py"],
        &["pyproject.toml", "setup.cfg", "requirements.txt"],
    ),
    ("ruby", &[".rb", ".gemspec"], &["Gemfile", "Gemfile.lock"]),
    ("rust", &[".rs"], &["Cargo.toml", "Cargo.lock"]),
    (
        "typescript",
        &[
            ".ts", ".tsx", ".mts", ".cts", ".js", ".jsx", ".mjs", ".cjs", ".svelte",
        ],
        &[
            "package.json",
            "package-lock.json",
            "tsconfig.json",
            "jsconfig.json",
        ],
    ),
];

const GENERIC_EXTENSIONS: &[&str] = &[
    ".asm", ".bash", ".bat", ".clj", ".cljs", ".cmd", ".cr", ".dart", ".elm", ".erl", ".ex",
    ".exs", ".f03", ".f90", ".f95", ".fish", ".fs", ".fsx", ".groovy", ".hs", ".jl", ".lhs",
    ".lua", ".m", ".ml", ".mli", ".mm", ".nim", ".nims", ".pas", ".php", ".pl", ".pm", ".proto",
    ".ps1", ".r", ".scala", ".sc", ".s", ".sh", ".sol", ".sql", ".swift", ".sv", ".v", ".vb",
    ".vbs", ".vim", ".zig",
];

pub fn for_path(path: &str) -> Vec<String> {
    let path = Path::new(path);
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let extension = extension(path);
    let mut result = Vec::new();
    for (language, extensions, config_files) in DEFINITIONS {
        if extensions.contains(&extension.as_str())
            || config_files
                .iter()
                .any(|config| *config == name || (config.starts_with('.') && *config == extension))
        {
            result.push((*language).to_owned());
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
    if let Some(generic) = generic_extension(language) {
        return generic == extension;
    }
    DEFINITIONS
        .iter()
        .find(|(candidate, _, _)| *candidate == language)
        .is_some_and(|(_, extensions, _)| extensions.contains(&extension.as_str()))
}

pub fn supported_languages() -> Vec<String> {
    let mut values: Vec<String> = DEFINITIONS
        .iter()
        .map(|(language, _, _)| (*language).to_owned())
        .collect();
    values.push("generic".to_owned());
    values.sort();
    values
}

pub fn supported(language: &str) -> bool {
    language == "generic"
        || is_generic(language)
        || DEFINITIONS
            .iter()
            .any(|(candidate, _, _)| *candidate == language)
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
