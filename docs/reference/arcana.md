# Arcana reference

Parent index: [Reference](INDEX.md)

## Purpose

Define Arcana's current direct command surface, managed graph state, Lexicon synchronization, deterministic query protocol, optional semantic vectors, and implementation ownership.

## Overview

Arcana consumes a verified immutable Lexicon snapshot and publishes immutable graph state for deterministic structural queries. It is directly operable and does not require Grimoire.

For a task-oriented first-use guide, start with the [Arcana operator how-to](../../arcana/docs/HOWTO.md). Exact command/state semantics live in [Arcana application](../../arcana/docs/APPLICATION.md).

## Command access

Use the standalone executable:

```text
arcana <command> ...
```

The main command families are:

| Command | Purpose |
| --- | --- |
| `sync` | Synchronize managed `.arcana/` state from the current Lexicon snapshot. |
| `sync --register` | Synchronize and register Arcana as a Lexicon post-publication consumer. |
| `protocol --snapshot <path>` | Serve overlay-aware `arcana.query.v1` JSONL requests against one verified snapshot. |
| `query` | Inspect an explicitly supplied packed graph/catalogue directly; this does not apply overlays. |
| `vectorize` | Explicitly build the optional semantic entry-point index. |
| `semantic-query` | Query that optional semantic index for graph entry points. |
| `import-facts` | Build a standalone repository snapshot from canonical fact input. |
| `update-facts` | Produce a new standalone generation from declared changed-file facts when node identity permits an overlay. |
| `benchmark` | Exercise deterministic packed/overlay graph workloads. |

## Normal operator path

Prepare Lexicon first:

```text
lexicon init --repo /path/to/repository
```

Synchronize and register Arcana:

```text
arcana sync \
  --lexicon /path/to/repository/.lexicon \
  --state /path/to/repository/.arcana \
  --register
```

Later repository updates normally begin with:

```text
lexicon scan --repo /path/to/repository
```

When registered, Arcana can be synchronized automatically after successful Lexicon publication. Explicit `arcana sync` remains valid.

## Managed state

Arcana publishes generated state under `.arcana/`:

```text
.arcana/
  CURRENT
  LOCK
  snapshots/
    <lexicon-digest>/
      graph.arcana
      overlay.arcana              # optional
      graph.manifest
      repository.arcana
      repository.manifest
      lexicon.snapshot
      compatibility.warnings      # optional
  vector-cache/                   # optional
  vectors/                        # optional
```

`.arcana/` is generated state and should normally be ignored by the source repository. `CURRENT` contains the full Lexicon snapshot identity; the matching directory under `snapshots/` uses its digest.

## Query protocol

The stable machine boundary is `arcana.query.v1`. Start a session with:

```text
arcana protocol --snapshot /path/to/repository/.arcana/snapshots/<digest>
```

Then send one JSON object per line. Begin with capabilities:

```json
{"id":"cap","op":"capabilities"}
```

Common operations include `search_nodes`, `resolve_symbol`, `resolve_file`, `neighbors`, `paths`, `reachability`, `impact`, `shortest_call_chain`, `dead_symbols`, `operational_role`, `architecture_summary`, `unresolved`, `stats`, and `diff`.

The protocol validates the complete repository snapshot and is overlay-aware. Consumers should use it instead of reading packed bytes directly when they need authoritative managed-state graph results.

## Direct query

`arcana query` is intentionally narrower:

```text
arcana query \
  --graph <snapshot>/graph.arcana \
  --catalogue <debug-catalogue.tsv> \
  --name ExactSymbolName
```

It reads the packed base plus an explicitly supplied debug/export TSV catalogue and does not merge `overlay.arcana`. Published snapshots use `repository.arcana`; use `protocol` for normal managed snapshots.

## Lexicon boundary

Lexicon owns language parsing, normalized semantic facts, durable identities, source spans, and immutable semantic snapshots. Arcana owns graph ingestion, packed graph storage, overlays, traversal, graph algorithms, and graph-local query semantics.

Arcana records the exact Lexicon snapshot it consumed and rejects corrupt or incompatible input rather than silently substituting stale graph state.

See [Lexicon ingestion contract](../../arcana/docs/LEXICON_CONTRACT.md).

## Optional semantic vectors

Arcana can build an optional graph-entry-point index through a compatible external OpenAI-style embedding endpoint:

```text
arcana vectorize --state /path/to/repository/.arcana
arcana semantic-query --state /path/to/repository/.arcana --query "profile persistence"
```

Semantic matches are entry points. Exact relationships, paths, reachability, and impact remain deterministic graph operations.

## Common diagnostics

If no graph snapshot exists, verify Lexicon with `lexicon status` and `lexicon doctor`, then run explicit `arcana sync`.

If a snapshot is rejected, rebuild/synchronize from verified Lexicon state rather than editing `.arcana/` files manually.

If automatic synchronization does not occur, inspect the Lexicon consumer registration and rerun `arcana sync --register`.

If direct `query` disagrees with a managed snapshot containing an overlay, use `protocol`; direct query intentionally sees only the packed base.

## Code map

| Concern | Current implementation | Verification |
| --- | --- | --- |
| CLI parsing/dispatch | `arcana/src/main.rs`, `arcana/src/cli.rs`, `arcana/src/cli_*.rs` | Arcana CLI tests |
| Lexicon synchronization | `arcana/src/cli_sync.rs`, `arcana/src/cli_sync_state.rs`, `arcana/src/lexicon/` | sync/Lexicon tests |
| Repository compilation/catalogue | `arcana/src/repository/` | repository tests |
| Packed graph/snapshots | `arcana/src/storage/`, `arcana/src/snapshot/` | storage/snapshot tests |
| Query protocol | `arcana/src/protocol/` | protocol tests |
| Optional vectors | `arcana/src/vector/`, `arcana/src/cli_vectors.rs` | vector tests |
| Deterministic synthetic benchmarks | `arcana/src/benchmark/`, `arcana/src/synthetic/` | benchmark/synthetic tests |

## Related docs

- [Arcana operator how-to](../../arcana/docs/HOWTO.md)
- [Arcana application](../../arcana/docs/APPLICATION.md)
- [Arcana architecture](../../arcana/docs/ARCHITECTURE.md)
- [Lexicon ingestion contract](../../arcana/docs/LEXICON_CONTRACT.md)
- [Repository snapshots](../../arcana/docs/repository-snapshots.md)
- [Installation](installation.md)

## Notes

Arcana is a deterministic graph provider. Higher-level task interpretation, ranking, and agent orchestration belong to consumers such as Warlock or Pitlord.
