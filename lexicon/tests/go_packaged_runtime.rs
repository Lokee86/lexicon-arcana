use std::{fs, path::PathBuf, process::Command};

use lexicon::{AdapterHost, AdapterRequest};

mod support;
use support::TestDirectory;

#[test]
fn packaged_go_semantic_helper_runs_from_adapter_root() {
    let root = TestDirectory::new("go-packaged-runtime");
    let adapter_root = root.path.join("adapters");
    let helper_dir = adapter_root.join("go-semantic");
    let repository = root.path.join("repository");
    fs::create_dir_all(&helper_dir).unwrap();
    fs::create_dir_all(&repository).unwrap();

    let helper = helper_dir.join(format!(
        "lexicon-go-semantic{}",
        std::env::consts::EXE_SUFFIX
    ));
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("adapters/go-semantic");
    let output = Command::new("go")
        .args(["build", "-trimpath", "-buildvcs=false", "-o"])
        .arg(&helper)
        .arg(".")
        .current_dir(source)
        .output()
        .expect("build packaged Go semantic helper");
    assert!(
        output.status.success(),
        "helper build failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::write(
        repository.join("go.mod"),
        "module example.com/packaged\n\ngo 1.22\n",
    )
    .unwrap();
    fs::write(
        repository.join("main.go"),
        "package packaged\n\nfunc Value() int { return 1 }\n",
    )
    .unwrap();

    let host = AdapterHost::new(&adapter_root);
    let analysis = host
        .analyze(&AdapterRequest {
            language: "go".into(),
            repository,
            workers: 1,
            shards: 1,
            merge_fan_in: 2,
            ..AdapterRequest::default()
        })
        .unwrap();

    assert_eq!(analysis.header.language, "go");
    assert_eq!(analysis.header.repository, "example.com/packaged");
    assert!(!analysis.records.is_empty());
}
