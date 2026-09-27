use std::fs;

use super::{ADAPTER_VERSION, PythonAdapter};
use crate::{AdapterMode, AdapterRequest, LanguageAdapter};

#[test]
fn partitioned_execution_matches_serial_output() {
    let directory =
        std::env::temp_dir().join(format!("lexicon-python-partition-{}", std::process::id()));
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).expect("create temporary Python repository");
    fs::write(
        directory.join("a.py"),
        "from b import run\n\ndef main(value: int):\n    try:\n        return run(value)\n    except ValueError:\n        return 0\n",
    )
    .expect("write a.py");
    fs::write(
        directory.join("b.py"),
        "def run(value: int):\n    return value + 1\n",
    )
    .expect("write b.py");
    fs::write(
        directory.join("c.py"),
        "class Worker:\n    def execute(self, value):\n        return value\n",
    )
    .expect("write c.py");

    let adapter = PythonAdapter;
    let base = AdapterRequest {
        language: "python".into(),
        mode: AdapterMode::Full,
        repository: directory.clone(),
        changed_files: Vec::new(),
        removed_files: Vec::new(),
        workers: 1,
        shards: 1,
        merge_fan_in: 2,
    };
    let serial = adapter.analyze(&base).expect("serial Python analysis");

    let parallel = adapter
        .analyze(&AdapterRequest {
            workers: 3,
            shards: 3,
            merge_fan_in: 2,
            ..base
        })
        .expect("partitioned Python analysis");

    assert_eq!(serial.header.adapter_version, ADAPTER_VERSION);
    assert_eq!(serial.header, parallel.header);
    assert_eq!(serial.records, parallel.records);
    fs::remove_dir_all(directory).expect("remove temporary Python repository");
}
