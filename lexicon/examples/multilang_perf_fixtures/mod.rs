use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub(crate) struct FixtureSet {
    pub(crate) root: PathBuf,
    pub(crate) cases: Vec<(&'static str, &'static str, PathBuf)>,
}

impl FixtureSet {
    pub(crate) fn create() -> io::Result<Self> {
        let root = std::env::temp_dir().join(format!(
            "lexicon-multilang-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&root)?;

        let cases = vec![
            case(
                &root,
                "c-family",
                "c-family",
                &[(
                    "src/main.c",
                    "int helper(void) { return 1; }\nint main(void) { return helper(); }\n",
                )],
            )?,
            case(
                &root,
                "python",
                "python",
                &[
                    ("pkg/__init__.py", ""),
                    ("pkg/helper.py", "def helper():\n    return 1\n"),
                    (
                        "main.py",
                        "from pkg.helper import helper\n\ndef main():\n    return helper()\n",
                    ),
                    (
                        "pyproject.toml",
                        "[project]\nname = \"fixture\"\ndependencies = [\"requests>=2\"]\n",
                    ),
                ],
            )?,
            case(
                &root,
                "gdscript",
                "gdscript",
                &[
                    ("project.godot", "[application]\nconfig/name=\"Fixture\"\n"),
                    (
                        "main.gd",
                        "class_name Main\nsignal changed\nfunc helper():\n    return 1\nfunc run():\n    return helper()\n",
                    ),
                ],
            )?,
            case(
                &root,
                "go",
                "go",
                &[
                    ("go.mod", "module example.com/fixture\n\ngo 1.22\n"),
                    (
                        "main.go",
                        "package main\n\nfunc helper() int { return 1 }\nfunc main() { _ = helper() }\n",
                    ),
                ],
            )?,
            case(
                &root,
                "lotusscript",
                "lotusscript",
                &[(
                    "main.lss",
                    "Sub Helper\nEnd Sub\n\nSub Main\n    Call Helper\nEnd Sub\n",
                )],
            )?,
            case(
                &root,
                "kotlin",
                "kotlin",
                &[(
                    "src/main/kotlin/demo/Main.kt",
                    "package demo\nclass Main { fun helper(): Int = 1; fun run(): Int = helper() }\n",
                )],
            )?,
            rust_case(&root)?,
            case(
                &root,
                "ruby",
                "ruby",
                &[
                    (
                        "lib/main.rb",
                        "module Demo\n  class Main\n    def helper\n      1\n    end\n    def run\n      helper\n    end\n  end\nend\n",
                    ),
                    (
                        "Gemfile",
                        "source \"https://rubygems.org\"\ngem \"rack\", \"~> 3.0\"\n",
                    ),
                ],
            )?,
            case(
                &root,
                "generic-lua",
                "generic-lua",
                &[(
                    "main.lua",
                    "local dep = require(\"dep\")\nfunction helper() return 1 end\nfunction run() return helper() end\n",
                )],
            )?,
        ];
        Ok(Self { root, cases })
    }
}

impl Drop for FixtureSet {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn case(
    root: &Path,
    name: &'static str,
    language: &'static str,
    files: &[(&str, &str)],
) -> io::Result<(&'static str, &'static str, PathBuf)> {
    let directory = root.join(name);
    for (relative, content) in files {
        write(&directory, relative, content)?;
    }
    Ok((name, language, directory))
}

fn rust_case(root: &Path) -> io::Result<(&'static str, &'static str, PathBuf)> {
    case(
        root,
        "rust",
        "rust",
        &[
            (
                "Cargo.toml",
                "[package]\nname = \"lexicon_fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
            ),
            (
                "src/lib.rs",
                "pub fn helper() -> i32 { 1 }\npub fn run() -> i32 { helper() }\n",
            ),
        ],
    )
}

fn write(root: &Path, relative: &str, content: &str) -> io::Result<()> {
    let path = root.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, content)
}
