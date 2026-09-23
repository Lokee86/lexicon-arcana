# Lexicon Rust migration

Parent index: [Lexicon Documentation](README.md)

## Purpose

Define the active parity-first migration boundary from the Go Lexicon application to the Rust library that will become Warlock's first-class in-process integration surface.

## Overview

The migration keeps the existing Go implementation as the behavioral reference until each Rust slice reaches its applicable parity gate. Versioned contracts under `spec/` remain authoritative; migration fixtures are verification evidence, not new product contracts.

## Current migration boundary

The existing Go implementation remains the executable application and behavioral authority for all functionality not yet ported.

The reference implementation is pinned at commit
`758af9daf6e71fc0a7ebb837875efe366f6403fd`. Migration fixtures and the differential harness live under
`evaluation/rust_migration/`.

## Completed foundation

The first four migration slices are implemented:

- the Go parity oracle is pinned and representative contract, snapshot, incremental, recovery, and full-scan migration vectors are recorded;
- existing Go binary v1/v2 goldens and publication/export/scan tests remain authoritative instead of being duplicated;
- the Rust `lexicon` crate exists as the future library boundary;
- Rust owns facts-v1 header/record types, canonical JSONL parsing and emission, record ordering, path/span checks, incremental ownership checks, SHA-256 identity validation, stable node identity generation, and source-content identities;
- the Rust facts fixture round-trips byte-identically;
- Rust immutable storage now writes binary v2 byte-identically to the Go golden, reads binary v2, binary v1, and legacy JSON objects, preserves object-ID and snapshot-ID hash domains, preserves Go `nil` versus empty snapshot slices, and supports node-only reads without materializing relationship sections;
- the Rust storage tests consume the Go binary golden constants directly, and Arcana's independent Lexicon compatibility tests remain green;
- Rust publication now preserves the existing `objects/`, `snapshots/`, `CURRENT`, `PENDING`, and `LOCK` layout, atomic replacement and immutable-write behavior, content verification, pending-publication bytes, single-writer locking, and the existing discard-versus-republish recovery decisions.

No Rust scan planning, language-materialization/update pipeline, adapter execution, interstack synthesis, CLI replacement, or Warlock integration is implemented yet.

## Parity rule

Migration work preserves the versioned contracts under `spec/`. A behavior mismatch is fixed against the Go reference before it is accepted in Rust unless a separate compatibility migration is explicitly approved.

Durable byte contracts require byte equality. Behavioral slices use the narrowest deterministic observable comparison available. The Go implementation is not removed until the Rust library and Warlock integration satisfy the applicable parity gates.

## Verification

Run the Rust foundation checks from `lexicon/`:

```text
cargo fmt -- --check
cargo test
cargo clippy --all-targets -- -D warnings
```

Run the migration comparator against a candidate fixture directory with:

```text
python evaluation/rust_migration/compare.py PATH
```

The existing Go tests remain required while they own untranslated behavior.

## Next slice

The next planned slice is language materialization and manifest mutation: deterministic owner grouping, per-file/shared object creation, full and incremental language replacement, manifest lookup/update, and the dependency/topology evidence required by scan planning. Adapter execution and repository orchestration remain outside that slice.

## Related docs

- [Lexicon architecture](ARCHITECTURE.md)
- [Development and verification](DEVELOPMENT.md)
- [Current status](STATUS.md)
- [Lexicon contracts](../spec/README.md)

## Notes

The migration oracle may grow as later slices become testable, but changes to facts, object, snapshot, or publication semantics require the normal contract-change process rather than an oracle update alone.