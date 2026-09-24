mod support;

use std::fs;
use std::io::Write;
use std::sync::Arc;

use lexicon::{
    AdapterError, AdapterHost, AdapterRequest, NativeAdapter, adapter_fingerprint,
    adapter_fingerprint_with_versions,
};

use support::TestDirectory;

struct EchoAdapter;

impl NativeAdapter for EchoAdapter {
    fn run(&self, request: &AdapterRequest, output: &mut dyn Write) -> Result<(), AdapterError> {
        writeln!(output, "{{\"language\":\"{}\"}}", request.language).map_err(AdapterError::from)
    }
}

#[test]
fn native_adapter_supports_file_and_stream_execution() {
    let root = TestDirectory::new("native-adapter");
    let mut host = AdapterHost::new(root.path.join("adapters"));
    host.register_native("python", Arc::new(EchoAdapter));

    let request = AdapterRequest {
        language: "python".into(),
        repository: root.path.join("repo"),
        output: root.path.join("out").join("facts.jsonl"),
        ..Default::default()
    };
    host.run(&request).unwrap();
    assert_eq!(
        fs::read_to_string(&request.output).unwrap(),
        "{\"language\":\"python\"}\n"
    );

    let streamed = host
        .run_stream(&request, |reader| {
            let mut text = String::new();
            reader
                .read_to_string(&mut text)
                .map_err(AdapterError::from)?;
            Ok(text)
        })
        .unwrap();
    assert_eq!(streamed, "{\"language\":\"python\"}\n");
}

#[test]
fn fingerprint_matches_go_oracle_and_ignores_test_state_files() {
    let root = TestDirectory::new("adapter-fingerprint");
    write(&root.path, "python/z.py", "z = 1\n");
    write(&root.path, "python/nested/a.py", "a = 1\n");
    write(&root.path, "python/tests/test_adapter.py", "ignored = 1\n");
    write(&root.path, "python/.git/generated.txt", "ignored\n");

    let first = adapter_fingerprint_with_versions(&root.path, "python", 1, 1).unwrap();
    assert_eq!(
        first,
        "sha256:65691dba13735e6d977fbab119af6160c7726bad76bd4f5a02f2b442ac8a1444"
    );
    assert_eq!(adapter_fingerprint(&root.path, "python").unwrap(), first);

    write(
        &root.path,
        "python/tests/test_adapter.py",
        "changed but ignored\n",
    );
    write(
        &root.path,
        "python/.git/generated.txt",
        "changed but ignored\n",
    );
    assert_eq!(adapter_fingerprint(&root.path, "python").unwrap(), first);

    write(&root.path, "python/z.py", "z = 2\n");
    assert_ne!(adapter_fingerprint(&root.path, "python").unwrap(), first);
}

#[test]
fn fingerprint_includes_schema_and_config_versions_and_rejects_missing_adapters() {
    let root = TestDirectory::new("adapter-fingerprint-version");
    write(&root.path, "ruby/adapter.rb", "puts 'ok'\n");

    let base = adapter_fingerprint_with_versions(&root.path, "ruby", 1, 1).unwrap();
    assert_ne!(
        adapter_fingerprint_with_versions(&root.path, "ruby", 2, 1).unwrap(),
        base
    );
    assert_ne!(
        adapter_fingerprint_with_versions(&root.path, "ruby", 1, 2).unwrap(),
        base
    );
    assert!(adapter_fingerprint(&root.path, "unknown-language").is_err());
    assert!(adapter_fingerprint(&root.path, "go").is_err());
}

fn write(root: &std::path::Path, relative: &str, content: &str) {
    let path = root.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}
