# Lexicon operator how-to

Parent index: [Lexicon Documentation](README.md)

## Purpose

Use this guide when you want to install Lexicon, prepare a repository, query semantic facts, refresh state after edits, or connect Arcana without first learning Lexicon's storage internals.

For the exact command and storage contracts, use [Application](APPLICATION.md). For adapter coverage and known limits, use [Status](STATUS.md).

## Overview

The normal lifecycle is: install Lexicon, initialize a repository once, inspect or query the published semantic snapshot, run `scan` after source changes, and optionally register Arcana as a post-publication consumer. `.lexicon/` is generated state; repository-owned exclusions belong in `.lexiconignore` instead.

## 1. Install and verify

For normal use, install or build the last optimized Go Lexicon implementation. The Rust port remains a migration/development target and is not yet the recommended operator runtime because its optimization work is incomplete.

The pinned Go reference revision is `758af9daf6e71fc0a7ebb837875efe366f6403fd`. Build it from the `lexicon/` directory:

```text
go build -o ../bin/lexicon ./cmd/lexicon
```

Then verify:

```text
../bin/lexicon version
```

The shared [installation guide](../../docs/reference/installation.md) documents the Go operator path and the separate Rust migration path.

## 2. Initialize a repository

From anywhere, point Lexicon at the repository root:

```text
lexicon init --repo /path/to/repository
```

This creates generated repository state under:

```text
/path/to/repository/.lexicon/
```

`.lexicon/` is generated analysis state. Do not commit it. Add it to the target repository's ignore rules if the repository does not already ignore it.

If only selected supported languages should be active:

```text
lexicon init --repo /path/to/repository --languages rust,python,typescript
```

## 3. Verify the prepared state

Run:

```text
lexicon status --repo /path/to/repository
lexicon doctor --repo /path/to/repository
```

Use `status` for the current snapshot, detected/enabled languages, and consumers. Use `doctor` when adapters, runtimes, storage, or registered consumers may be misconfigured.

## 4. Inspect semantic structure

The recommended Go runtime at `758af9d` does not include the newer Rust-migration-only `find`, `show`, `refs`, or `calls` commands.

For normal operator work, publish the Lexicon snapshot and use Arcana for symbol resolution, relationships, traversal, reachability, impact, call chains, and architecture queries. See [Arcana operator how-to](../../arcana/docs/HOWTO.md).

When a consumer needs raw normalized facts instead of graph queries, use `lexicon export`.

## 5. Refresh after source changes

Run a normal incremental scan:

```text
lexicon scan --repo /path/to/repository
```

Lexicon reuses unchanged immutable objects and expands to a larger analysis boundary when the safe incremental boundary is not sufficient.

To force complete analysis of enabled languages:

```text
lexicon rebuild --repo /path/to/repository
```

To change the enabled language set:

```text
lexicon languages set --repo /path/to/repository --languages rust,python
```

## 6. Optional watch mode

For a repository you are actively editing:

```text
lexicon demon --repo /path/to/repository
```

The watcher feeds changes into the same scan transaction used by `lexicon scan`. It is optional; consumers do not require a resident Lexicon process.

A repository-root `.lexiconignore` can exclude additional paths from analysis. It is repository configuration and may be committed when the exclusions are shared project policy.

## 7. Connect Arcana

After Lexicon has a valid snapshot:

```text
arcana sync \
  --lexicon /path/to/repository/.lexicon \
  --state /path/to/repository/.arcana \
  --register
```

`--register` installs Arcana as a Lexicon post-publication consumer. Later successful Lexicon scans can then synchronize Arcana automatically.

Verify the registration with:

```text
lexicon consumer list --repo /path/to/repository
```

`.arcana/` is also generated state and should not be committed.

## 8. Export facts when another tool needs them

Export a verified snapshot as JSONL without modifying current state:

```text
lexicon export \
  --repo /path/to/repository \
  --output /tmp/lexicon-export
```

Restrict the export when required:

```text
lexicon export \
  --repo /path/to/repository \
  --output /tmp/lexicon-export \
  --languages rust,python
```

Treat exports as derived artifacts unless a downstream workflow explicitly requires them to be versioned.

## 9. Reclaim old generated state

Preview garbage collection first:

```text
lexicon gc --repo /path/to/repository --dry-run
```

Then apply it:

```text
lexicon gc --repo /path/to/repository
```

Lexicon preserves the current snapshot, configured retention, and consumer-pinned snapshots.

## Troubleshooting

If initialization or scans fail:

1. run `lexicon doctor --repo <repo>`;
2. confirm required language runtimes are installed;
3. confirm the installed adapter tree belongs to the same Lexicon distribution;
4. use `lexicon status` to confirm the last published snapshot remains valid;
5. use a full `rebuild` when adapter or configuration changes require complete analysis.

Do not manually edit `.lexicon/CURRENT`, snapshot manifests, objects, or the private mirror. They are Lexicon-owned generated state.

## Related docs

- [Application](APPLICATION.md)
- [Architecture](ARCHITECTURE.md)
- [Status](STATUS.md)
- [Development](DEVELOPMENT.md)
- [Adapter index](../adapters/README.md)
- [Arcana operator how-to](../../arcana/docs/HOWTO.md)

## Notes

This guide intentionally favors the shortest supported operator path. Exact flags, storage invariants, compatibility behavior, and development procedures remain owned by the linked reference documents.
