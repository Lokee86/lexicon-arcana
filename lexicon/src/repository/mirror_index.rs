use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

#[path = "mirror_index_worktree.rs"]
mod worktree;

/// Git's two immutable trees are the baseline: the published private mirror
/// HEAD and the source HEAD. Only a path clean relative to BOTH trees can be
/// skipped, and only if the trees contain identical content object IDs.
/// Untracked, staged, dirty, and ambiguous paths retain byte comparison.
pub(crate) fn unchanged_files(
    mirror_root: &Path,
    source_root: &Path,
    desired: &BTreeMap<PathBuf, PathBuf>,
) -> Option<BTreeSet<PathBuf>> {
    let Some(state_root) = mirror_root.parent() else {
        return fallback(1);
    };
    let Some(private_tree) = tree_blobs(state_root, "source/") else {
        return fallback(1);
    };
    let Some(private_dirty) = dirty_paths(state_root, "source/") else {
        return fallback(2);
    };
    let clean = private_dirty.is_empty();
    if crate::perf::enabled() {
        crate::perf::emit(
            "scan.mirror_index_state",
            Duration::ZERO,
            &[
                ("mirror_clean", u64::from(clean)),
                ("mirror_dirty", u64::from(!clean)),
                ("mirror_status_failed", 0),
            ],
        );
    }

    // Source repositories may be nested inside a larger Git worktree. Git's
    // --show-prefix supplies the path relative to that worktree's root.
    let Some(source_top) = git_text(source_root, &["rev-parse", "--show-toplevel"]) else {
        return fallback(3);
    };
    let Some(source_prefix) = git_text(source_root, &["rev-parse", "--show-prefix"]) else {
        return fallback(3);
    };
    let source_top = PathBuf::from(source_top.trim());
    let source_prefix = source_prefix.trim().to_owned();
    let Some(source_tree) = tree_blobs(&source_top, &source_prefix) else {
        return fallback(4);
    };
    let Some(source_dirty) = dirty_paths(&source_top, &source_prefix) else {
        return fallback(5);
    };
    // A Git-clean file can still be transformed on checkout (CRLF, smudge
    // filters, ident, encoding). Only byte-equivalent working-tree paths qualify.
    let Some(source_normal) = worktree::eligible_paths(&source_top, &source_prefix) else {
        return fallback(6);
    };

    let mut unchanged = BTreeSet::new();
    let mut matching_blobs = 0_u64;
    for relative in desired.keys() {
        let Some(private_oid) = private_tree.get(relative) else {
            continue;
        };
        let Some(source_oid) = source_tree.get(relative) else {
            continue;
        };
        if private_oid != source_oid
            || private_dirty.contains(relative)
            || source_dirty.contains(relative)
            || !source_normal.contains(relative)
        {
            continue;
        }
        matching_blobs += 1;
        let destination = mirror_root.join(relative);
        if std::fs::symlink_metadata(destination)
            .is_ok_and(|metadata| metadata.file_type().is_file())
        {
            unchanged.insert(relative.clone());
        }
    }
    if crate::perf::enabled() {
        crate::perf::emit(
            "scan.source_index_metadata",
            Duration::ZERO,
            &[
                ("desired_files", desired.len() as u64),
                ("matching_blobs", matching_blobs),
                ("private_dirty", private_dirty.len() as u64),
                ("source_dirty", source_dirty.len() as u64),
                ("indexed_skips", unchanged.len() as u64),
                ("source_content_reads", 0),
            ],
        );
    }
    Some(unchanged)
}

// Diagnostic reason codes: private baseline missing=1, private status failure=2,
// source not Git=3, source tree failure=4, source status failure=5,
// source tracked-file metadata failure=6. Fallback always compares source bytes.
fn fallback(reason: u64) -> Option<BTreeSet<PathBuf>> {
    if crate::perf::enabled() {
        crate::perf::emit(
            "scan.mirror_index_fallback",
            Duration::ZERO,
            &[("reason", reason)],
        );
    }
    None
}

fn tree_blobs(repo: &Path, prefix: &str) -> Option<BTreeMap<PathBuf, String>> {
    let mut args = vec!["ls-tree", "-r", "-z", "--full-tree", "HEAD", "--"];
    if !prefix.is_empty() {
        args.push(prefix);
    }
    let bytes = git_bytes(repo, &args)?;
    let mut result = BTreeMap::new();
    for record in bytes
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
    {
        let tab = record.iter().position(|byte| *byte == b'\t')?;
        let metadata = std::str::from_utf8(&record[..tab]).ok()?;
        let mut fields = metadata.split_whitespace();
        let mode = fields.next()?;
        let kind = fields.next()?;
        let oid = fields.next()?;
        if kind != "blob" || !matches!(mode, "100644" | "100755") {
            continue;
        }
        let path = std::str::from_utf8(&record[tab + 1..]).ok()?;
        if let Some(relative) = path.strip_prefix(prefix)
            && !relative.is_empty()
        {
            result.insert(PathBuf::from(relative), oid.to_owned());
        }
    }
    Some(result)
}

fn dirty_paths(repo: &Path, prefix: &str) -> Option<BTreeSet<PathBuf>> {
    // diff HEAD includes staged AND working tree edits; the source tree is
    // authoritative for tracked files, while untracked paths are never skipped.
    let mut args = vec!["diff", "--no-ext-diff", "--name-only", "-z", "HEAD", "--"];
    if !prefix.is_empty() {
        args.push(prefix);
    }
    let bytes = git_bytes(repo, &args)?;
    paths_with_prefix(&bytes, prefix)
}

fn paths_with_prefix(bytes: &[u8], prefix: &str) -> Option<BTreeSet<PathBuf>> {
    let mut paths = BTreeSet::new();
    for raw in bytes
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
    {
        let path = std::str::from_utf8(raw).ok()?;
        if let Some(relative) = path.strip_prefix(prefix)
            && !relative.is_empty()
        {
            paths.insert(PathBuf::from(relative));
        }
    }
    Some(paths)
}

fn git_text(repo: &Path, args: &[&str]) -> Option<String> {
    String::from_utf8(git_bytes(repo, args)?).ok()
}

fn git_bytes(repo: &Path, args: &[&str]) -> Option<Vec<u8>> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .ok()?;
    output.status.success().then_some(output.stdout)
}

#[cfg(test)]
#[path = "mirror_index_tests.rs"]
mod tests;
