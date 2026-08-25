# Arcana Maintainer Map

Parent index: [Arcana Documentation](README.md)

## Purpose

This document routes common Arcana changes to their canonical documentation and implementation boundary. It is a navigation aid, not a file-by-file source inventory.

## Overview

Use this page to select the owning Arcana document. Continue in that document's focused code map for exact implementation and verification paths.

## Change routing

| Change area | Canonical documentation | Primary implementation boundary |
| --- | --- | --- |
| Commands, flags, diagnostics, and exit behavior | [Application](APPLICATION.md) | `src/cli.rs`, `src/cli_*.rs`, `src/main.rs` |
| Lexicon snapshot ingestion and compatibility | [Lexicon contract](LEXICON_CONTRACT.md) | `src/lexicon/`, `src/cli_sync.rs` |
| Repository facts, identities, and dense compilation | [Architecture](ARCHITECTURE.md) | `src/repository/` |
| Packed graph format and validation | [Architecture](ARCHITECTURE.md) | sibling `arcana-graph/src/storage/`; `src/storage.rs` compatibility export |
| Graph topology snapshots, overlays, and compaction | [Repository snapshots](repository-snapshots.md) | sibling `arcana-graph/src/snapshot/`; `src/snapshot.rs` compatibility export |
| Repository snapshot binding | [Repository snapshots](repository-snapshots.md) | repository snapshot modules under `src/repository/` |
| Deterministic graph protocol and traversal | [Application](APPLICATION.md), [Architecture](ARCHITECTURE.md) | `src/protocol/` adapters over `arcana-graph/src/traversal.rs` |
| Optional semantic graph vectors | [Vector index](vector-index.md) | `src/vector/`, `src/cli_vectors.rs` |
| Synthetic graphs and performance evidence | [Development](DEVELOPMENT.md) | `src/synthetic/`, `src/benchmark/` |
| Build and verification | [Development](DEVELOPMENT.md) | `Cargo.toml`, module tests, integration fixtures |

## Boundaries

- Arcana consumes normalized facts; it does not parse source languages.
- Packed graph traversal remains authoritative; semantic vectors provide entry points only.
- Grimoire owns the provider-neutral discovery response and evidence-lane assembly.
- Focused implementation paths and tests belong in each subject document's `## Code map` section.

## Related docs

- [Architecture](ARCHITECTURE.md)
- [Application](APPLICATION.md)
- [Development](DEVELOPMENT.md)
- [Status](STATUS.md)

## Notes

Use this map to select the owning document, then use that document's code map for exact implementation paths.
