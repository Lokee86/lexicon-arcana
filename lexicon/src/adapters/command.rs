use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::languages::{is_generic, lookup, supports_partitioned_execution};

use super::executable::{find_executable, packaged_executable};
use super::{AdapterError, AdapterRequest};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterCommand {
    pub program: PathBuf,
    pub arguments: Vec<OsString>,
    pub current_dir: Option<PathBuf>,
    pub environment: Vec<(OsString, OsString)>,
}

impl AdapterCommand {
    pub(crate) fn build(&self) -> Command {
        let mut command = Command::new(&self.program);
        command.args(&self.arguments);
        if let Some(directory) = &self.current_dir {
            command.current_dir(directory);
        }
        for (key, value) in &self.environment {
            command.env(key, value);
        }
        command
    }
}

pub fn command_spec(
    adapter_root: &Path,
    request: &AdapterRequest,
) -> Result<AdapterCommand, AdapterError> {
    let definition = lookup(&request.language)
        .ok_or_else(|| AdapterError::new(format!("unsupported language {:?}", request.language)))?;
    if request.language == "generic" {
        return Err(AdapterError::new(
            "generic adapter requires an extension-qualified language",
        ));
    }

    let arguments = adapter_arguments(request);
    let runtime_language = if is_generic(&request.language) {
        "generic"
    } else {
        request.language.as_str()
    };
    if let Some(executable) = packaged_executable(adapter_root, runtime_language) {
        return Ok(spec(executable, arguments));
    }

    match runtime_language {
        "c-family" | "go" | "gdscript" | "java" | "kotlin" | "lotusscript" | "generic" => {
            let mut value = spec(
                find_executable(&["go"])?,
                prepend_strings(&["run", "."], arguments),
            );
            value.current_dir = Some(adapter_root.join(&definition.directory));
            Ok(value)
        }
        "csharp" => {
            let prefix = vec![
                "run".into(),
                "--project".into(),
                adapter_root
                    .join("csharp")
                    .join("Lexicon.CSharp.csproj")
                    .into_os_string(),
                "--".into(),
            ];
            Ok(spec(
                find_executable(&["dotnet"])?,
                prepend(prefix, arguments),
            ))
        }
        "python" => python_spec(adapter_root, arguments),
        "ruby" => {
            let prefix = vec![
                adapter_root
                    .join("ruby")
                    .join("lexicon_ruby.rb")
                    .into_os_string(),
            ];
            Ok(spec(
                find_executable(&["ruby"])?,
                prepend(prefix, arguments),
            ))
        }
        "rust" => {
            let prefix = vec![
                "run".into(),
                "--quiet".into(),
                "--manifest-path".into(),
                adapter_root
                    .join("rust")
                    .join("Cargo.toml")
                    .into_os_string(),
                "--".into(),
            ];
            Ok(spec(
                find_executable(&["cargo"])?,
                prepend(prefix, arguments),
            ))
        }
        "typescript" => {
            let distribution = adapter_root.join("typescript").join("dist").join("cli.js");
            if !distribution.is_file() {
                return Err(AdapterError::new("TypeScript adapter requires build"));
            }
            Ok(spec(
                find_executable(&["node"])?,
                prepend(vec![distribution.into_os_string()], arguments),
            ))
        }
        _ => Err(AdapterError::new(format!(
            "unsupported language {:?}",
            request.language
        ))),
    }
}

pub(crate) fn adapter_arguments(request: &AdapterRequest) -> Vec<OsString> {
    let mut arguments = vec![
        OsString::from("--repo"),
        request.repository.as_os_str().to_owned(),
        OsString::from("--output"),
        request.output.as_os_str().to_owned(),
    ];
    if is_generic(&request.language) {
        arguments.extend([
            OsString::from("--language"),
            OsString::from(&request.language),
        ]);
    }
    add_paths(&mut arguments, "--changed-file", &request.changed_files);
    add_paths(&mut arguments, "--removed-file", &request.removed_files);
    if supports_partitioned_execution(&request.language) {
        add_positive(&mut arguments, "--workers", request.workers);
        add_positive(&mut arguments, "--shards", request.shards);
        add_positive(&mut arguments, "--merge-fan-in", request.merge_fan_in);
    }
    arguments
}

fn python_spec(
    adapter_root: &Path,
    arguments: Vec<OsString>,
) -> Result<AdapterCommand, AdapterError> {
    let mut value = spec(
        find_executable(&["python", "python3"])?,
        prepend_strings(&["-m", "lexicon_python"], arguments),
    );
    let python_root = adapter_root.join("python");
    let python_path = std::env::var_os("PYTHONPATH")
        .and_then(|existing| {
            let mut paths = vec![python_root.clone()];
            paths.extend(std::env::split_paths(&existing));
            std::env::join_paths(paths).ok()
        })
        .unwrap_or_else(|| python_root.into_os_string());
    value
        .environment
        .push((OsString::from("PYTHONPATH"), python_path));
    Ok(value)
}

fn add_paths(arguments: &mut Vec<OsString>, name: &str, paths: &[String]) {
    for path in paths {
        arguments.extend([
            OsString::from(name),
            OsString::from(path.replace('\\', "/")),
        ]);
    }
}

fn add_positive(arguments: &mut Vec<OsString>, name: &str, value: usize) {
    if value > 0 {
        arguments.extend([OsString::from(name), OsString::from(value.to_string())]);
    }
}

fn spec(program: PathBuf, arguments: Vec<OsString>) -> AdapterCommand {
    AdapterCommand {
        program,
        arguments,
        current_dir: None,
        environment: Vec::new(),
    }
}

fn prepend_strings(prefix: &[&str], arguments: Vec<OsString>) -> Vec<OsString> {
    prepend(prefix.iter().map(OsString::from).collect(), arguments)
}

fn prepend(mut prefix: Vec<OsString>, mut arguments: Vec<OsString>) -> Vec<OsString> {
    prefix.append(&mut arguments);
    prefix
}
