mod support;

use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;

use lexicon::{
    AdapterRequest, command_spec,
    languages::{definitions, lookup, supports_partitioned_execution, supports_streaming_output},
};

use support::TestDirectory;

#[test]
fn definitions_match_go_directories_and_capabilities() {
    let definitions = definitions();
    assert_eq!(definitions.len(), 12);

    let go = lookup("go").unwrap();
    assert_eq!(go.directory, "go");
    assert!(go.partitioned_execution);
    assert!(!go.streaming_output);

    let python = lookup("python").unwrap();
    assert_eq!(python.directory, "python");
    assert!(python.partitioned_execution);
    assert!(python.streaming_output);

    let typescript = lookup("typescript").unwrap();
    assert_eq!(typescript.directory, "typescript");
    assert!(typescript.extensions.contains(&".js".into()));
    assert!(typescript.extensions.contains(&".svelte".into()));

    let generic = lookup("generic-scala").unwrap();
    assert_eq!(generic.directory, "generic");
    assert_eq!(generic.extensions, vec![".scala"]);
    assert!(!supports_partitioned_execution("generic-scala"));
    assert!(!supports_streaming_output("generic-scala"));
}

#[test]
fn packaged_commands_match_go_argument_contract() {
    for language in [
        "c-family",
        "csharp",
        "go",
        "gdscript",
        "java",
        "kotlin",
        "lotusscript",
        "rust",
    ] {
        let root = TestDirectory::new(&format!("adapter-command-{language}"));
        let executable = packaged(&root.path, language);
        fs::create_dir_all(executable.parent().unwrap()).unwrap();
        fs::write(&executable, b"packaged").unwrap();

        let spec = command_spec(
            &root.path,
            &AdapterRequest {
                language: language.into(),
                repository: PathBuf::from("repo"),
                output: PathBuf::from("facts.jsonl"),
                changed_files: vec!["src\\main.go".into()],
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(spec.program, executable);
        assert!(spec.current_dir.is_none());
        assert_eq!(
            strings(&spec.arguments),
            vec![
                "--repo",
                "repo",
                "--output",
                "facts.jsonl",
                "--changed-file",
                "src/main.go",
            ]
        );
    }
}

#[test]
fn generic_and_partitioned_arguments_match_go() {
    let root = TestDirectory::new("adapter-generic");
    let generic = packaged(&root.path, "generic");
    fs::create_dir_all(generic.parent().unwrap()).unwrap();
    fs::write(&generic, b"packaged").unwrap();

    let generic_spec = command_spec(
        &root.path,
        &AdapterRequest {
            language: "generic-scala".into(),
            repository: "repo".into(),
            output: "facts.jsonl".into(),
            changed_files: vec!["src/Main.scala".into()],
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        strings(&generic_spec.arguments),
        vec![
            "--repo",
            "repo",
            "--output",
            "facts.jsonl",
            "--language",
            "generic-scala",
            "--changed-file",
            "src/Main.scala",
        ]
    );

    let go = packaged(&root.path, "go");
    fs::create_dir_all(go.parent().unwrap()).unwrap();
    fs::write(&go, b"packaged").unwrap();
    let go_spec = command_spec(
        &root.path,
        &AdapterRequest {
            language: "go".into(),
            repository: "repo".into(),
            output: "facts.jsonl".into(),
            workers: 16,
            shards: 64,
            merge_fan_in: 4,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        strings(&go_spec.arguments),
        vec![
            "--repo",
            "repo",
            "--output",
            "facts.jsonl",
            "--workers",
            "16",
            "--shards",
            "64",
            "--merge-fan-in",
            "4",
        ]
    );

    assert!(
        command_spec(
            &root.path,
            &AdapterRequest {
                language: "generic".into(),
                ..Default::default()
            }
        )
        .is_err()
    );
}

fn packaged(root: &std::path::Path, language: &str) -> PathBuf {
    let base = root.join(language).join(format!("lexicon-{language}"));
    if cfg!(windows) {
        base.with_extension("exe")
    } else {
        base
    }
}

fn strings(values: &[OsString]) -> Vec<String> {
    values
        .iter()
        .map(|value| value.to_string_lossy().into_owned())
        .collect()
}
