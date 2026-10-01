use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::git_bytes;

/// Git blob equality proves working-tree byte equality only when checkout
/// transformations are absent. A clean Git status alone is insufficient.
pub(super) fn eligible_paths(repo: &Path, prefix: &str) -> Option<BTreeSet<PathBuf>> {
    let tracked = normal_tracked_paths(repo, prefix)?;
    let matching_eol = matching_eol_paths(repo, prefix)?;
    let transformed = transformed_attribute_paths(repo, prefix, &tracked)?;
    Some(
        tracked
            .intersection(&matching_eol)
            .filter(|path| !transformed.contains(*path))
            .cloned()
            .collect(),
    )
}

fn normal_tracked_paths(repo: &Path, prefix: &str) -> Option<BTreeSet<PathBuf>> {
    let mut args = vec!["ls-files", "-v", "-z", "--"];
    if !prefix.is_empty() {
        args.push(prefix);
    }
    let bytes = git_bytes(repo, &args)?;
    let mut paths = BTreeSet::new();
    for record in bytes
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
    {
        let (&tag, remainder) = record.split_first()?;
        if remainder.first().copied() != Some(b' ') {
            return None;
        }
        // Exclude assume-unchanged and skip-worktree, even when diff HEAD is clean.
        if tag != b'H' {
            continue;
        }
        let path = std::str::from_utf8(&remainder[1..]).ok()?;
        if let Some(relative) = path.strip_prefix(prefix)
            && !relative.is_empty()
        {
            paths.insert(PathBuf::from(relative));
        }
    }
    Some(paths)
}

fn matching_eol_paths(repo: &Path, prefix: &str) -> Option<BTreeSet<PathBuf>> {
    let mut args = vec!["ls-files", "--eol", "-z", "--"];
    if !prefix.is_empty() {
        args.push(prefix);
    }
    let bytes = git_bytes(repo, &args)?;
    let mut paths = BTreeSet::new();
    for record in bytes
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
    {
        let tab = record.iter().position(|byte| *byte == b'\t')?;
        let metadata = std::str::from_utf8(&record[..tab]).ok()?;
        let mut fields = metadata.split_whitespace();
        let index_eol = fields.next()?.strip_prefix("i/")?;
        let working_eol = fields.next()?.strip_prefix("w/")?;
        // Mixed, ambiguous, absent, and converted line endings fall back.
        if index_eol != working_eol || !matches!(index_eol, "lf" | "crlf" | "none" | "-text") {
            continue;
        }
        let path = std::str::from_utf8(&record[tab + 1..]).ok()?;
        if let Some(relative) = path.strip_prefix(prefix)
            && !relative.is_empty()
        {
            paths.insert(PathBuf::from(relative));
        }
    }
    Some(paths)
}

fn transformed_attribute_paths(
    repo: &Path,
    prefix: &str,
    tracked: &BTreeSet<PathBuf>,
) -> Option<BTreeSet<PathBuf>> {
    if tracked.is_empty() {
        return Some(BTreeSet::new());
    }
    let paths = tracked
        .iter()
        .map(|path| Some(format!("{prefix}{}", path.to_str()?)))
        .collect::<Option<Vec<_>>>()?;
    let mut child = Command::new("git")
        .args([
            "check-attr",
            "-z",
            "--stdin",
            "filter",
            "ident",
            "working-tree-encoding",
        ])
        .current_dir(repo)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut stdin = child.stdin.take()?;
    // Git can emit considerably more attribute output than input. Feed stdin
    // concurrently while wait_with_output drains stdout to avoid pipe deadlock.
    let writer = std::thread::spawn(move || -> std::io::Result<()> {
        for path in paths {
            stdin.write_all(path.as_bytes())?;
            stdin.write_all(&[0])?;
        }
        Ok(())
    });
    let output = child.wait_with_output().ok()?;
    writer.join().ok()?.ok()?;
    if !output.status.success() {
        return None;
    }

    let mut fields = output.stdout.split(|byte| *byte == 0);
    let mut transformed = BTreeSet::new();
    loop {
        let path = fields.next()?;
        if path.is_empty() {
            break;
        }
        let attribute = fields.next()?;
        let value = fields.next()?;
        let path = std::str::from_utf8(path).ok()?;
        let attribute = std::str::from_utf8(attribute).ok()?;
        let value = std::str::from_utf8(value).ok()?;
        if matches!(attribute, "filter" | "ident" | "working-tree-encoding")
            && !matches!(value, "unspecified" | "unset")
            && let Some(relative) = path.strip_prefix(prefix)
        {
            transformed.insert(PathBuf::from(relative));
        }
    }
    Some(transformed)
}
