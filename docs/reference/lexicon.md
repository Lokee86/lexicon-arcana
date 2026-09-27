# Lexicon reference

Parent index: [Reference](INDEX.md)

## Purpose

Define the current direct Lexicon command surface, generated repository state, scan lifecycle, semantic lookup workflow, Arcana handoff, and implementation ownership during the Rust migration.

## Overview

Lexicon converts repository source into immutable normalized semantic facts. For normal operator use, the recommended runtime is the last optimized Go implementation, pinned at `758af9daf6e71fc0a7ebb837875efe366f6403fd`. The Rust `lexicon` + `lexicon-cli` implementation is the active migration target, but its optimization work is incomplete and it is not yet the recommended runtime.

Grimoire is retired. Operators and consumers invoke Lexicon directly, and Arcana consumes published Lexicon snapshots through the explicit snapshot/consumer boundary.

For a task-oriented first-use guide, start with the [Lexicon operator how-to](../../lexicon/docs/HOWTO.md). For exact command semantics, use [Lexicon application](../../lexicon/docs/APPLICATION.md).

## Command access

Use the standalone executable:

```text
lexicon <command> ...
```

The active command families are:

| Command | Purpose |
| --- | --- |
| `init` | Initialize `.lexicon/`, select languages, analyze the repository, and publish the first snapshot. |
| `scan` | Reconcile source changes and publish or confirm the current immutable snapshot. |
| `rebuild` | Force complete analysis for all or selected enabled languages. |
| `demon` | Optionally watch the repository and feed changes into the same scan transaction. |
| `languages` | Inspect or change enabled languages. |
| `status` | Report repository, snapshot, enabled/detected languages, and consumers. |
| `doctor` | Validate repository state, adapters, storage, and consumer configuration. |
| `export` | Reconstruct verified JSONL from an immutable snapshot. |
| `gc` | Remove unreachable immutable state while preserving current/retained/pinned snapshots. |
| `consumer` | Manage deterministic post-publication consumers such as Arcana. |
| `version` | Report build identity. |

The Rust migration CLI additionally provides `find`, `show`, `refs`, and `calls`. Those commands are not available in the recommended Go runtime at `758af9d`.

For the recommended runtime, use Arcana for symbol resolution, relationships, traversal, reachability, impact, paths, call chains, and architecture-community analysis.

## Normal operator path

First use:

```text
lexicon init --repo /path/to/repository
lexicon status --repo /path/to/repository
lexicon doctor --repo /path/to/repository
```

After source changes:

```text
lexicon scan --repo /path/to/repository
```

For semantic inspection, synchronize Arcana from the published Lexicon snapshot and use Arcana's validated query protocol. The recommended Go Lexicon runtime does not expose direct snapshot lookup commands.

## State layout

```text
.lexicon/
  config.json
  CURRENT
  LOCK
  PENDING
  consumers/
  consumer-state/
  objects/
  snapshots/
  repo/
    .git/
    source/
```

`.lexicon/` is generated Lexicon state and should normally be ignored by the source repository. A repository-root `.lexiconignore` is different: it is authored repository configuration and may be committed when its exclusions are shared project policy.

`CURRENT` names the published immutable snapshot. Readers resolve it and then read the referenced manifest and objects. They do not depend on the mutable private source mirror.

## Scan lifecycle

A normal scan:

1. resolves the repository and configuration;
2. mirrors relevant source into Lexicon-owned private state;
3. calculates source/configuration changes;
4. selects complete or safely scoped analysis;
5. executes the Go runtime's registered language adapters;
6. validates and materializes immutable fact objects;
7. publishes or confirms the immutable snapshot;
8. advances `CURRENT` atomically when publication changes;
9. invokes registered consumers in deterministic order.

A consumer failure does not invalidate the already-published Lexicon snapshot.

## Arcana handoff

Register Arcana after Lexicon has a valid snapshot:

```text
arcana sync \
  --lexicon /path/to/repository/.lexicon \
  --state /path/to/repository/.arcana \
  --register
```

Later successful Lexicon scans can invoke the registered one-shot Arcana sync. Explicit `arcana sync` remains valid.

`.arcana/` is Arcana-owned generated state and should also normally be ignored.

## Runtime recommendation and Rust migration

The optimized Go implementation is the recommended operator runtime for now. Its pinned reference revision is `758af9daf6e71fc0a7ebb837875efe366f6403fd`.

The Rust `lexicon` crate and separate `lexicon-cli` crate are the replacement implementation under active migration. Their semantic parity work is substantial, but optimization is not yet complete. Use them for migration development, parity testing, and performance work rather than as the default user-facing runtime.

Exact migration status belongs in [Rust migration](../../lexicon/docs/RUST_MIGRATION.md).

## Code map

| Concern | Current implementation | Verification |
| --- | --- | --- |
| CLI host and command dispatch | `lexicon-cli/src/` | `lexicon-cli/tests/`, module tests |
| Public library and repository lifecycle | `lexicon/src/api/`, `lexicon/src/repository/`, `lexicon/src/scan/` | `lexicon/tests/public_api.rs`, scan/repository tests |
| Configuration and repository policy | `lexicon/src/config/`, `lexicon/src/repository/` | config/private-state/repository-policy tests |
| Immutable objects, snapshots, export, GC | `lexicon/src/storage/` | storage/publication/recovery/export/GC tests |
| Consumers | `lexicon/src/consumer/`, `lexicon-cli/src/commands_consumer.rs` | consumer execution and CLI tests |
| Semantic lookup | `lexicon/src/lookup/`, `lexicon-cli/src/commands_lookup.rs` | lookup tests |
| Native language adapters | `lexicon/src/adapters/` | adapter-specific Rust parity tests |
| Legacy/parity adapter sources | `lexicon/adapters/` | existing adapter oracle suites during migration |
| Versioned contracts | `lexicon/spec/` | contract/golden compatibility tests |

## Related docs

- [Lexicon operator how-to](../../lexicon/docs/HOWTO.md)
- [Lexicon application](../../lexicon/docs/APPLICATION.md)
- [Lexicon architecture](../../lexicon/docs/ARCHITECTURE.md)
- [Rust migration](../../lexicon/docs/RUST_MIGRATION.md)
- [Arcana operator how-to](../../arcana/docs/HOWTO.md)
- [Installation](installation.md)

## Notes

Direct source inspection remains authoritative for implementation details. Lexicon supplies deterministic semantic evidence; Arcana owns graph traversal; higher-level agent/task orchestration belongs to consumers such as Warlock.
