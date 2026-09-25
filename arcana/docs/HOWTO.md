# Arcana operator how-to

Parent index: [Arcana Documentation](README.md)

## Purpose

Use this guide when you want to turn a Lexicon snapshot into a queryable graph, keep it synchronized, run common graph queries, or diagnose generated Arcana state.

For exact command and state semantics, use [Application and operations](APPLICATION.md). For the Lexicon handoff contract, use [Lexicon contract](LEXICON_CONTRACT.md).

## Overview

The normal lifecycle is: prepare Lexicon, synchronize Arcana once, optionally register automatic synchronization, then use the validated protocol for graph-shaped questions. `.arcana/` is generated state and is rebuilt or advanced from immutable Lexicon snapshots rather than hand-maintained.

## 1. Prepare Lexicon first

Arcana consumes Lexicon state; it does not parse source code itself.

Verify that the target repository has a valid Lexicon snapshot:

```text
lexicon status --repo /path/to/repository
```

If it has not been initialized:

```text
lexicon init --repo /path/to/repository
```

## 2. Create Arcana state

Synchronize the current Lexicon snapshot:

```text
arcana sync \
  --lexicon /path/to/repository/.lexicon \
  --state /path/to/repository/.arcana
```

Arcana publishes generated state under:

```text
/path/to/repository/.arcana/
```

`.arcana/` contains immutable graph snapshots, manifests, locks, and optional vector state. Do not commit it.

## 3. Register automatic synchronization

On the first setup, normally run:

```text
arcana sync \
  --lexicon /path/to/repository/.lexicon \
  --state /path/to/repository/.arcana \
  --register
```

This writes an Arcana consumer definition into Lexicon. Later successful Lexicon scans can invoke the same one-shot Arcana synchronization automatically.

Check the registration with:

```text
lexicon consumer list --repo /path/to/repository
```

Registration is optional. Explicit `arcana sync` always remains valid.

## 4. Know which query surface to use

Use Arcana when the question is graph-shaped:

- what depends on this symbol;
- what can this change impact transitively;
- is there a path between two owners;
- what is the shortest call chain;
- what architectural communities exist;
- which references remain unresolved.

For a simple exact-name adjacency check, `arcana query` can inspect an explicit packed graph and catalogue.

For normal validated graph work, prefer `arcana protocol`. It opens the complete repository snapshot, validates the manifest and optional overlay, and serves deterministic `arcana.query.v1` requests.

## 5. Locate the current managed snapshot

`.arcana/CURRENT` contains the full Lexicon snapshot identity:

```text
sha256:<digest>
```

The matching Arcana snapshot directory is:

```text
.arcana/snapshots/<digest>/
```

That directory is the argument for `arcana protocol --snapshot`.

## 6. Run common protocol queries

Start with capabilities:

```json
{"id":"cap","op":"capabilities"}
```

Find nodes by name/path text:

```json
{"id":"find","op":"search_nodes","query":"Gateway","limit":20}
```

Resolve a symbol more narrowly:

```json
{"id":"resolve","op":"resolve_symbol","name":"Gateway","limit":20}
```

After obtaining a node ID, inspect outgoing calls:

```json
{"id":"calls","op":"neighbors","node_id":42,"direction":"outgoing","relation":"calls","limit":100}
```

Inspect transitive impact:

```json
{"id":"impact","op":"impact","node_id":42,"max_depth":8,"limit":500}
```

Summarize architecture within a subtree:

```json
{"id":"arch","op":"architecture_summary","path_prefix":"src","min_community_size":3,"limit":100}
```

Feed JSON Lines to:

```text
arcana protocol --snapshot /path/to/repository/.arcana/snapshots/<digest>
```

The process reads one JSON request per line and writes one JSON response per line until stdin closes.

## 7. Use direct query only for packed-base inspection

The human-readable command is:

```text
arcana query \
  --graph <snapshot>/graph.arcana \
  --catalogue <snapshot>/catalogue.tsv \
  --name ExactSymbolName \
  --relation calls
```

Important: `arcana query` opens the packed graph directly and does not apply an overlay. Use `protocol` when the managed snapshot may contain `overlay.arcana`.

## 8. Refresh after repository changes

Normally refresh Lexicon:

```text
lexicon scan --repo /path/to/repository
```

If Arcana is registered, synchronization follows successful publication. Otherwise run:

```text
arcana sync \
  --lexicon /path/to/repository/.lexicon \
  --state /path/to/repository/.arcana
```

Arcana chooses reuse, overlay, or rebuild internally. Those modes are diagnostics, not user-selected correctness levels.

## 9. Optional semantic vectors

Semantic vectors are explicitly optional and require a compatible external OpenAI-style embedding endpoint.

Build the index:

```text
arcana vectorize --state /path/to/repository/.arcana
```

Query it:

```text
arcana semantic-query \
  --state /path/to/repository/.arcana \
  --query "where is profile persistence handled?"
```

Semantic matches are entry points. Use exact graph operations for authoritative relationships, paths, and impact.

## Troubleshooting

If synchronization fails:

1. verify Lexicon first with `lexicon status` and `lexicon doctor`;
2. rerun explicit `arcana sync` and inspect its mode/warning output;
3. do not manually repair `.arcana/CURRENT` or snapshot manifests;
4. if a registered consumer fails, remember that the Lexicon snapshot remains valid and Arcana can be retried explicitly;
5. prefer `protocol` over direct `query` when overlays may exist.

## Related docs

- [Application and operations](APPLICATION.md)
- [Lexicon contract](LEXICON_CONTRACT.md)
- [Repository snapshots](repository-snapshots.md)
- [Vector index](vector-index.md)
- [Architecture](ARCHITECTURE.md)
- [Lexicon operator how-to](../../lexicon/docs/HOWTO.md)

## Notes

This guide describes the ordinary managed-repository path. Standalone import/update commands, exact protocol limits, vector storage, and publication invariants remain owned by the linked reference documents.
