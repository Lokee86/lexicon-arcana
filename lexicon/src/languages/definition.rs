#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageDefinition {
    pub language: String,
    pub directory: String,
    pub extensions: Vec<String>,
    pub config_files: Vec<String>,
    pub partitioned_execution: bool,
}

struct StaticDefinition {
    language: &'static str,
    directory: &'static str,
    extensions: &'static [&'static str],
    config_files: &'static [&'static str],
    partitioned_execution: bool,
}

const DEFINITIONS: &[StaticDefinition] = &[
    def(
        "c-family",
        "c-family",
        &[
            ".c", ".cc", ".cp", ".cpp", ".cxx", ".c++", ".h", ".hh", ".hpp", ".hxx", ".h++",
            ".inc", ".inl", ".ipp", ".tpp",
        ],
        &["compile_commands.json", "CMakeLists.txt"],
        false,
    ),
    def("gdscript", "gdscript", &[".gd"], &["project.godot"], false),
    def("go", "go", &[".go"], &["go.mod", "go.sum"], true),
    def(
        "csharp",
        "csharp",
        &[".cs"],
        &[
            ".sln",
            ".csproj",
            "Directory.Build.props",
            "Directory.Build.targets",
            "global.json",
        ],
        false,
    ),
    def(
        "java",
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
        false,
    ),
    def(
        "kotlin",
        "kotlin",
        &[".kt", ".kts"],
        &["build.gradle.kts", "settings.gradle.kts"],
        false,
    ),
    def(
        "lotusscript",
        "lotusscript",
        &[".ls", ".lsa", ".lsdb", ".lss"],
        &[],
        false,
    ),
    def(
        "python",
        "python",
        &[".py"],
        &["pyproject.toml", "setup.cfg", "requirements.txt"],
        true,
    ),
    def(
        "ruby",
        "ruby",
        &[".rb", ".gemspec"],
        &["Gemfile", "Gemfile.lock"],
        false,
    ),
    def(
        "rust",
        "rust",
        &[".rs"],
        &["Cargo.toml", "Cargo.lock"],
        false,
    ),
    def(
        "typescript",
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
        false,
    ),
    def("generic", "generic", &[], &[], false),
];

const fn def(
    language: &'static str,
    directory: &'static str,
    extensions: &'static [&'static str],
    config_files: &'static [&'static str],
    partitioned_execution: bool,
) -> StaticDefinition {
    StaticDefinition {
        language,
        directory,
        extensions,
        config_files,
        partitioned_execution,
    }
}

pub(crate) fn static_definitions() -> impl Iterator<Item = LanguageDefinition> {
    DEFINITIONS.iter().map(owned)
}

pub(crate) fn static_lookup(language: &str) -> Option<LanguageDefinition> {
    DEFINITIONS
        .iter()
        .find(|definition| definition.language == language)
        .map(owned)
}

fn owned(value: &StaticDefinition) -> LanguageDefinition {
    LanguageDefinition {
        language: value.language.to_owned(),
        directory: value.directory.to_owned(),
        extensions: value
            .extensions
            .iter()
            .map(|value| (*value).to_owned())
            .collect(),
        config_files: value
            .config_files
            .iter()
            .map(|value| (*value).to_owned())
            .collect(),
        partitioned_execution: value.partitioned_execution,
    }
}
