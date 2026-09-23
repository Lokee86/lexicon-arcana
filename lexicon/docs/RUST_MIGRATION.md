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

The first two migration slices are implemented:

- the Go parity oracle is pinned and representative contract, snapshot, incremental, recovery, and full-scan migration vectors are recorded;
- existing Go binary v1/v2 goldens and publication/export/scan tests remain authoritative instead of being duplicated;
- the Rust `lexicon` crate exists as the future library boundary;
- Rust owns facts-v1 header/record types, canonical JSONL parsing and emission, record ordering, path/span checks, incremental ownership checks, SHA-256 identity validation, stable node identity generation, and source-content identities;
- the Rust facts fixture round-trips byte-identically.

No Rust object storage, snapshot publication, scan planning, adapter execution, interstack synthesis, CLI replacement, or Warlock integration is implemented yet.

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

The next planned slice is immutable storage: binary v2 write/read parity, binary v1 and legacy JSON reads, object identities, snapshot manifest hashing, and node-only reads. It must consume the existing Go golden bytes rather than introduce a new storage contract.

## Related docs

- [Lexicon architecture](ARCHITECTURE.md)
- [Development and verification](DEVELOPMENT.md)
- [Current status](STATUS.md)
- [Lexicon contracts](../spec/README.md)

## Notes

The migration oracle may grow as later slices become testable, but changes to facts, object, snapshot, or publication semantics require the normal contract-change process rather than an oracle update alone.
