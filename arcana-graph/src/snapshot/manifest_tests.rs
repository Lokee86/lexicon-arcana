use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::{ManifestError, SnapshotManifest};

static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn manifest() -> SnapshotManifest {
    SnapshotManifest {
        snapshot_id: 0x12,
        created_unix_seconds: 1_725_000_001,
        node_count: 42,
        base_edge_count: 100,
        visible_edge_count: 113,
        base_dataset_checksum: 0xabcdef,
        overlay_checksum: 0x1234,
        visible_dataset_checksum: 0xfedcba,
        base_file: PathBuf::from("snapshots/base.pack"),
        overlay_file: Some(PathBuf::from("snapshots/overlay.pack")),
    }
}

fn temporary_path(label: &str) -> PathBuf {
    let sequence = TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "arcana-manifest-{label}-{}-{sequence}.manifest",
        std::process::id()
    ))
}

struct TempPath(PathBuf);

impl TempPath {
    fn new(label: &str) -> Self {
        Self(temporary_path(label))
    }

    fn as_path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempPath {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[test]
fn encoding_is_canonical_and_round_trips() {
    let expected = "version=1\nsnapshot_id=0000000000000012\ncreated_unix_seconds=1725000001\nnode_count=42\nbase_edge_count=100\nvisible_edge_count=113\nbase_dataset_checksum=0000000000abcdef\noverlay_checksum=0000000000001234\nvisible_dataset_checksum=0000000000fedcba\nbase_file=snapshots/base.pack\noverlay_file=snapshots/overlay.pack\n";
    let encoded = manifest().encode().unwrap();

    assert_eq!(encoded, expected);
    assert_eq!(SnapshotManifest::decode(&encoded).unwrap(), manifest());
}

#[test]
fn absent_overlay_is_omitted_and_round_trips() {
    let mut value = manifest();
    value.overlay_file = None;
    let encoded = value.encode().unwrap();

    assert!(!encoded.contains("overlay_file="));
    assert_eq!(SnapshotManifest::decode(&encoded).unwrap(), value);
}

#[test]
fn parser_rejects_strict_format_violations() {
    let encoded = manifest().encode().unwrap();
    let cases = [
        (
            "missing",
            encoded.replace("base_file=snapshots/base.pack\n", ""),
            "missing",
        ),
        (
            "duplicate",
            encoded.replace(
                "base_file=snapshots/base.pack\n",
                "base_file=snapshots/base.pack\nbase_file=again.pack\n",
            ),
            "duplicate",
        ),
        (
            "unknown",
            encoded.replacen("version=1\n", "version=1\nunknown=value\n", 1),
            "unknown",
        ),
        (
            "malformed",
            encoded.replace(
                "base_dataset_checksum=0000000000abcdef",
                "base_dataset_checksum=ABC",
            ),
            "malformed",
        ),
        (
            "unsupported",
            encoded.replacen("version=1", "version=2", 1),
            "unsupported",
        ),
    ];

    for (label, input, _) in cases {
        assert!(
            SnapshotManifest::decode(&input).is_err(),
            "{label} input was accepted"
        );
    }
    assert!(matches!(
        SnapshotManifest::decode(&encoded.replace("base_file=snapshots/base.pack\n", "")),
        Err(ManifestError::MissingField("base_file"))
    ));
    assert!(matches!(
        SnapshotManifest::decode(&encoded.replacen("version=1\n", "version=1\nunknown=value\n", 1)),
        Err(ManifestError::UnknownField(_))
    ));
    assert!(matches!(
        SnapshotManifest::decode(&encoded.replacen("version=1", "version=2", 1)),
        Err(ManifestError::UnsupportedVersion { found: 2 })
    ));
    assert!(matches!(
        SnapshotManifest::decode(&encoded[..encoded.len() - 1]),
        Err(ManifestError::MissingFinalNewline)
    ));
}

#[test]
fn paths_must_be_relative_without_parent_traversal() {
    let mut traversal = manifest();
    traversal.base_file = PathBuf::from("snapshots/../base.pack");
    assert!(matches!(
        traversal.encode(),
        Err(ManifestError::InvalidPath {
            field: "base_file",
            ..
        })
    ));

    let mut absolute = manifest();
    absolute.base_file = std::env::temp_dir().join("base.pack");
    assert!(matches!(
        absolute.encode(),
        Err(ManifestError::InvalidPath {
            field: "base_file",
            ..
        })
    ));
}

#[test]
fn writer_commits_once_and_refuses_replacement() {
    let path = TempPath::new("write");
    let value = manifest();

    value.write_to(path.as_path()).unwrap();
    assert_eq!(SnapshotManifest::read_from(path.as_path()).unwrap(), value);
    assert!(matches!(
        value.write_to(path.as_path()),
        Err(ManifestError::Io(error)) if error.kind() == io::ErrorKind::AlreadyExists
    ));
}
