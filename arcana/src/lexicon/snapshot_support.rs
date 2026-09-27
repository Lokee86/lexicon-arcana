use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use super::LexiconSnapshotError;

pub(super) fn read_verified_json(
    path: &Path,
    expected: &str,
    domain: &str,
    kind: &'static str,
) -> Result<Vec<u8>, LexiconSnapshotError> {
    let bytes = fs::read(path)?;
    let canonical = bytes.trim_ascii();
    verify_content(canonical, expected, domain, kind)?;
    Ok(canonical.to_vec())
}

pub(super) fn verify_content(
    bytes: &[u8],
    expected: &str,
    domain: &str,
    kind: &'static str,
) -> Result<(), LexiconSnapshotError> {
    let actual = digest(domain, bytes);
    if actual != expected {
        return Err(LexiconSnapshotError::ContentHashMismatch {
            kind,
            expected: expected.to_owned(),
            actual,
        });
    }
    Ok(())
}

pub(super) fn storage_root(root: &Path) -> PathBuf {
    if root.file_name().is_some_and(|name| name == ".lexicon")
        || (root.join("CURRENT").is_file() && root.join("snapshots").is_dir())
    {
        root.to_owned()
    } else {
        root.join(".lexicon")
    }
}

pub(super) fn validate_id(id: &str) -> Result<(), LexiconSnapshotError> {
    let Some(hex) = id.strip_prefix("sha256:") else {
        return Err(LexiconSnapshotError::InvalidId(id.to_owned()));
    };
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(LexiconSnapshotError::InvalidId(id.to_owned()));
    }
    Ok(())
}

pub(super) fn hex_id(id: &str) -> &str {
    id.strip_prefix("sha256:").expect("validated Lexicon ID")
}

fn digest(domain: &str, bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain.as_bytes());
    hasher.update(bytes);
    let mut output = String::from("sha256:");
    for byte in hasher.finalize() {
        write!(output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}
