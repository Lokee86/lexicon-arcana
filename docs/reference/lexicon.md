# Lexicon reference

Parent index: [Reference](INDEX.md)

## Purpose

This document defines Lexicon's product-facing commands, state, scan lifecycle, adapters, normalized contracts, consumers, diagnostics, and Grimoire integration boundary.

## Overview

Lexicon converts repository source into immutable normalized facts. Grimoire normally prepares and consumes those snapshots automatically, while direct commands remain available for operation, debugging, export, and adapter development.

Lexicon is the language-analysis component in the Grimoire bundle. It converts repository source into normalized, immutable symbols, relationships, unresolved references, source spans, and dependency evidence.

Grimoire normally prepares and consumes Lexicon automatically. Direct Lexicon commands are for operators, adapter developers, debugging, export, garbage collection, and consumer management.

## Command access

Use either the standalone executable:

```text
lexicon <command> ...
```

or Grimoire's forwarding namespace:

```text
grimoire lexicon <command> ...
```

`grimoire lexicon check` reports the resolved executable and version. Forwarded commands preserve the provider's process output and exit status.

## Primary commands

| Command | Purpose |
| --- | --- |
| `init` | Initialize `.lexicon/`, choose languages/adapters, analyze the repository, and publish the first snapshot. |
| `scan` | Reconcile current relevant source with the previous snapshot and publish a new snapshot when needed. |
| `rebuild` | Force complete analysis for all or selected enabled languages. |
| `demon` | Optional filesystem watch process that schedules the same locked scan transaction. |
| `languages` / `languages set` | Inspect or change enabled languages. |
| `status` | Report repository, current snapshot, detected/enabled languages, and consumers. |
| `doctor` | Validate configuration, private mirror, objects, adapters, runtimes, and consumer commands. |
| `export` | Reconstruct verified standalone JSONL libraries from an immutable snapshot. |
| `gc` | Remove unreachable snapshots and objects while preserving retention and consumer pins. |
| `consumer list|add|remove|run` | Manage deterministic post-publication consumers such as Arcana. |
| `find` | Find snapshot nodes by name, qualified name, path, or kind. |
| `show` | Show one exact snapshot node, rejecting ambiguous selectors. |
| `refs` | Show bounded direct incoming/outgoing relationships and unresolved evidence. |
| `calls` | Show bounded definite, possible, endpoint, and unresolved call evidence without graph traversal. |
| `version` | Report build identity. |

The exact flags and operational semantics are maintained in [`lexicon/docs/APPLICATION.md`](../../lexicon/docs/APPLICATION.md).

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

`CURRENT` names the published immutable snapshot. Readers load the manifest and referenced objects; they do not read adapter transport files or the mutable private source mirror.

`PENDING` supports crash recovery. `LOCK` serializes writers. Consumer state can pin older snapshots so garbage collection does not remove state still in use.

## Scan lifecycle

1. Resolve the repository and enabled languages.
2. Mirror relevant current files into Lexicon's private state repository.
3. Compare with the last successful state.
4. Select complete-language or impacted-file analysis.
5. Run language adapters under the bounded resource scheduler.
6. Validate and store immutable fact objects.
7. Publish the snapshot manifest and atomically advance `CURRENT`.
8. Invoke registered consumers in deterministic order.

A consumer failure does not invalidate the already-published Lexicon snapshot. It is retried on a later scan.

## Incremental correctness boundary

Lexicon reuses unaffected immutable objects when a scoped analysis is safe. It expands modified files through previous dependency information and includes conservative unresolved-reference owners.

It falls back to complete affected-language analysis for additions, deletions, renames, copies, configuration changes, missing prior dependency data, structural changes, uncertain relationship changes, or scoped adapter failure.

Go scopes expand to packages and Rust scopes expand to crates. Identical source, schema, configuration, and adapter versions must produce deterministic facts regardless of valid concurrency.

## Adapters

Adapter implementations live under [`lexicon/adapters/`](../../lexicon/adapters/). The adapter index identifies supported languages and links language-specific behavior.

Adapters own:

- file discovery for the language;
- declaration and source-span extraction;
- calls, dependencies, inheritance, implementation, and other supported relationships;
- unresolved evidence;
- normalized fact emission.

Adapters do not own snapshot publication, global object retention, consumer execution, or Arcana graph storage.

## Normalized contracts

Normative cross-component contracts live under [`lexicon/spec/`](../../lexicon/spec/):

- fact records;
- binary object encoding;
- snapshot manifests;
- runtime evidence.

Changes to these contracts require coordinated compatibility work in Lexicon consumers, especially Arcana and Grimoire's Lexicon reader.

## Arcana consumer

Arcana can be registered as a post-publication consumer. After each successful Lexicon scan, the consumer invokes one bounded Arcana synchronization against the published snapshot.

```text
arcana sync --register
```

The concrete consumer definition is stored under `.lexicon/consumers/`, and the last successful consumed snapshot is recorded under `.lexicon/consumer-state/`.

## Common diagnostics

### No current snapshot

Run `status`, then `doctor`. Initialize the repository if `.lexicon/config.json` does not exist; otherwise run `scan` or `rebuild` according to the reported failure.

### Adapter cannot start

`doctor` checks adapter paths and required runtimes. Release bundles place adapters beside the Lexicon installation; source checkouts resolve them from the configured adapter root.

### Scan reports busy

Another writer owns `.lexicon/LOCK`. Manual scans and the watch process use the same transaction lock. Confirm the existing process is healthy rather than starting competing refresh loops.

### Arcana did not update

Inspect `consumer list`, the Arcana consumer definition, and its consumer-state file. A Lexicon snapshot may be current even when the Arcana consumer failed.

### Export or garbage collection fails

Both operations validate immutable manifests and objects. Malformed consumer pins, missing referenced snapshots, checksum failures, or a changing `CURRENT` abort the operation rather than deleting or exporting uncertain state.

## Code map

| Documented concern | Primary implementation | Related tests |
| --- | --- | --- |
| Grimoire snapshot/fact integration | `internal/lexiconfacts/`, `internal/structure/` | `internal/lexiconfacts/*_test.go` |
| Lexicon command surface | `lexicon/cmd/lexicon/main.go`, `lexicon/internal/cli/` | Lexicon CLI tests |
| Scan and publication lifecycle | `lexicon/internal/scan/`, `lexicon/internal/objectstore/` | scan and object-store tests |
| Adapter execution | `lexicon/internal/adapters/`, `lexicon/internal/languages/` | adapter runner/registry tests |
| Language semantics | `lexicon/adapters/<language>/` | owning adapter test suite |
| Consumers and Arcana handoff | `lexicon/internal/consumer/`, `lexicon/internal/scan/interstack.go` | consumer and integration tests |

Grimoire consumes immutable Lexicon facts. It does not invoke parsers or mutate Lexicon's private state directly.

## Related docs

- [Lexicon application](../../lexicon/docs/APPLICATION.md)
- [Lexicon architecture](../../lexicon/docs/ARCHITECTURE.md)
- [Lexicon maintainer map](../../lexicon/docs/MAINTAINER_MAP.md)
- [Analysis stack](../architecture/analysis-stack.md)

## Notes

Use the owning adapter README for language-specific code maps and semantic limits.
