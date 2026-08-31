# Lexicon ingestion boundary

Parent index: [Arcana Documentation](README.md)

## Purpose

This document defines Arcana's exact ingestion boundary for immutable Lexicon snapshots, identities, normalized semantics, compatibility warnings, and synchronization decisions.

## Overview

Arcana verifies Lexicon state and converts supported normalized facts into repository graph input without invoking adapters or inventing missing language semantics.

Arcana consumes Lexicon snapshot contract v1 and compacts its durable identities into a packed repository graph. Binary v1 fact objects are the normal snapshot transport: Arcana verifies their exact content hashes, decodes their node, edge, and unresolved sections into typed records, and does not reconstruct JSONL. Legacy canonical JSON fact objects and the complete JSONL importer remain available for migration and diagnostics.

## Identity boundary

Lexicon owns cross-tool SHA-256 node identities. Arcana stores each full Lexicon identity in the catalogue, hashes it into an internal 64-bit `NodeKey`, checks for compaction collisions during import, and assigns dense packed `NodeId` values during compilation. Dense IDs are snapshot-local and must never escape as durable cross-tool identities. Lexicon file content IDs are compacted for Arcana's internal change detection.

Arcana continues to read its legacy TSV facts during migration, but no language adapter is owned by this repository.

## Preserved semantics

Arcana accepts the common Lexicon node and relation vocabulary, including:

- interfaces, traits, constructors, and parameters;
- definite `calls` and conservative `possible-calls` as separate relations;
- conversions, implementations, inheritance, trait use, overrides, reads, writes, and annotations;
- interstack `http-endpoint`, `message-channel`, and `config-key` nodes;
- `calls-endpoint`, `handled-by`, `publishes`, `consumes`, and `reads-config` relationships;
- unresolved references with source spans and candidate metadata.

Source spans are preserved in the catalogue and unresolved-reference store. Explicit file ownership drives Arcana's file-scoped replacement model. Binary record attributes are length-prefixed, so Arcana can skip arbitrary Lexicon `attributes` without parsing or persisting them; adding a provenance sidecar later will not require changing graph identity.

## Forward compatibility and warnings

Arcana does not reject an otherwise valid Lexicon snapshot solely because a newer adapter emits an unrecognized semantic label:

- known unresolved-reason labels remain typed, including the C-family macro reasons `unsupported-macro-expansion`, `macro-argument-mismatch`, `macro-expansion-cycle`, and `macro-expansion-depth`, plus Java compiler signals `compiler-analysis-failed` and `compiler-identity-mismatch`;
- unknown unresolved-reason labels are preserved verbatim and remain queryable and visible in statistics;
- unknown node kinds are conservatively represented as `symbol` so their identities and recognized relationships remain available;
- edge and unresolved-reference records with unknown relation labels are skipped because Arcana cannot safely invent graph semantics for them.

Every degradation is deduplicated and reported as an `arcana sync WARNING`. The warnings are also written to the immutable Arcana snapshot as `compatibility.warnings`, and Grimoire repository status promotes them into its top-level warnings. Empty or structurally malformed required fields remain hard errors.

## Snapshot synchronization

`arcana sync` resolves Lexicon's atomic `CURRENT` pointer, verifies the content-addressed snapshot manifest and every referenced fact object, and compares file object identities with the Lexicon snapshot consumed by the previous Arcana state. Added, changed, and removed file-object paths become Arcana's file-scoped replacement set. Any language-level shared-object change conservatively forces a packed rebuild, including when file objects changed in the same snapshot.

Arcana stores immutable graph states under `.arcana/snapshots/<lexicon-snapshot-digest>/`. All sync writers share `.arcana/LOCK`, and `.arcana/CURRENT` is replaced atomically only after the new state verifies. The repository manifest identifies Lexicon as the adapter and records the consumed Lexicon snapshot ID as its adapter version, while a `lexicon.snapshot` sidecar makes the relationship explicit.

When node identities remain unchanged, Arcana emits one cumulative overlay against the packed base. Node additions or removals, unusable prior state, unsupported incremental ownership, or any incremental planning failure fall back to a complete packed rebuild. This choice is internal; callers invoke the same `sync` operation in every case.

`arcana sync --register` writes a versioned command definition under `.lexicon/consumers/`. Lexicon invokes that one-shot command after every successful manual or daemon-triggered scan. The event only reduces latency: immutable snapshots remain the durable handoff, so Arcana can also catch up later through an explicit `arcana sync`.

Scoped Lexicon `mode=incremental` JSONL streams remain invalid as complete import input. Arcana derives incremental scope from verified snapshot manifests rather than accepting a partial stream without its surrounding snapshot state.

## Code map

| Contract concern | Primary implementation | Related tests |
| --- | --- | --- |
| Snapshot and object loading | `src/lexicon/snapshot.rs`, `format.rs`, `binary.rs`, `object.rs` | Lexicon module tests and binary/format tests |
| Record and identity conversion | `src/lexicon/records.rs`, `src/repository/model.rs` | record conversion and compiler tests |
| Compatibility warnings | `src/lexicon/mod.rs`, `records.rs`, `src/cli_sync.rs` | Lexicon and sync tests; installed release-consumer warning smoke |
| Snapshot comparison and update choice | `src/lexicon/mod.rs`, `src/cli_sync.rs`, `src/repository/incremental.rs` | sync and incremental tests |
| Normative producer contract | `lexicon/spec/` | owned and verified by Lexicon; consumed here |

Arcana reads immutable Lexicon state. It does not invoke adapters or reinterpret source-language semantics.

## Related docs

- [Arcana architecture](ARCHITECTURE.md)
- [Repository snapshots](repository-snapshots.md)
- [Lexicon architecture](../../lexicon/docs/ARCHITECTURE.md)
- [Root analysis stack](../../docs/architecture/analysis-stack.md)

## Notes

Unknown compatible vocabulary is degraded only according to the documented warning policy; corrupt or unsafe input fails.
