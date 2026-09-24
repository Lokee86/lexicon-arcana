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

The migration foundation and Interstack synthesis slice are implemented:

- the Go parity oracle is pinned and representative contract, snapshot, incremental, recovery, and full-scan migration vectors are recorded;
- existing Go binary v1/v2 goldens and publication/export/scan tests remain authoritative instead of being duplicated;
- the Rust `lexicon` crate exists as the future library boundary;
- Rust owns facts-v1 header/record types, canonical JSONL parsing and emission, record ordering, path/span checks, incremental ownership checks, SHA-256 identity validation, stable node identity generation, and source-content identities;
- the Rust facts fixture round-trips byte-identically;
- Rust immutable storage now writes binary v2 byte-identically to the Go golden, reads binary v2, binary v1, and legacy JSON objects, preserves object-ID and snapshot-ID hash domains, preserves Go `nil` versus empty snapshot slices, and supports node-only reads without materializing relationship sections;
- the Rust storage tests consume the Go binary golden constants directly, and Arcana's independent Lexicon compatibility tests remain green;
- Rust publication now preserves the existing `objects/`, `snapshots/`, `CURRENT`, `PENDING`, and `LOCK` layout, atomic replacement and immutable-write behavior, content verification, pending-publication bytes, single-writer locking, and the existing discard-versus-republish recovery decisions;
- Rust now materializes deterministic full, shared, and incremental language entries from typed facts and explicit source bytes, preserves unchanged object/shared-fact reuse, provides sorted manifest language mutation, computes dependency/context closure and direct-change full-analysis triggers, and detects unsafe new relationship topology before scoped replacement. The facts validator also accepts the existing synthetic repository-root node convention `path: "."` used by current Go adapters;
- Rust now owns the deterministic language registry and scan-plan composition used after source changes are known: enabled-language pruning, source/snapshot drift, optional adapter-fingerprint drift, modified-source scoped planning, structural-change full fallback, deterministic result assembly, and the `PENDING` → external state commit → snapshot publication handoff. A temporary Go oracle test confirmed the Rust plan decisions for modified source, modified config, added source, renamed source, and explicit drift;
- Rust now owns configuration normalization and analysis identity, adapter-root discovery, permanent repository exclusions and `.lexiconignore` semantics, repository-local state-directory preparation, full/scoped source mirroring, normalized source-change records, and the existing one-replaceable-commit Git-backed private state lifecycle. Focused parity tests cover Go's ignored-parent semantics, CRLF-preserving `.gitignore` updates, mirror reconciliation, rename detection, and one reachable private-state commit;
- Rust now owns the adapter host contract: typed adapter requests, the production language capability registry, deterministic adapter fingerprints, packaged/development process command selection, partitioning arguments, Python streaming output, TypeScript build preparation, and a `NativeAdapter` seam for in-process frontends. Existing language frontends remain unchanged. A Go-derived fingerprint vector and command/registry tests pin the compatibility boundary;
- Rust now owns scoped-analysis repository construction, Go-package/Rust-crate semantic-unit expansion, logical shard/worker/merge planning, weighted execution budgeting, concurrent plan execution, scoped-to-full retry, topology fallback, materialization, deterministic manifest merge, and the base repository scan transaction from mirror/diff/plan through `PENDING`, private-state commit, and snapshot publication. Repository-level tests cover initial full scan, stable no-op reuse, incremental source update, and scoped failure fallback.
- Rust now owns Interstack synthesis as a derived shared language over the ordinary manifest. The Rust resolver ports the Go HTTP, packet/message, configuration, process/CLI, `arcana.query.v1` protocol, and filesystem-state boundary detection; preserves `@interstack` identities and unresolved references; uses the Go `0.2.0` fingerprint contract; refreshes after ordinary analyses; removes Interstack when no ordinary languages remain; and lets Interstack drift force an otherwise no-op scan. Focused parity tests cover cross-language HTTP/message/config linking, Go handler-provider binding, nested Rails namespaces, parser-token rejection, process/CLI/protocol/state boundaries, shared-language materialization, and scan-engine drift refresh. Interstack facts explicitly permit relationship sources owned by ordinary language objects while normal facts continue rejecting unknown sources.
- Rust now owns legacy `repo/library/<language>.jsonl` migration during scan recovery. A dedicated legacy parser preserves the Go reader's permissive historical semantics without weakening modern fact validation; committed legacy libraries can seed a missing snapshot or recover a snapshot/private-state mismatch, corrupt legacy data falls back to source rebuild when no current snapshot exists, and the legacy directory is removed and committed away after migration. Focused tests cover all three paths, including the Go fixture's non-SHA legacy node identity.
- Rust now exposes a public `Lexicon` library handle with `open`, host-injected open, initialization with preserved or explicit language selection, host-injected initialization for future in-process integrations, repository/state/adapter accessors, and bounded scan entry points. Initialization retains Go's full-reanalysis behavior rather than degrading into an ordinary incremental/no-op scan, and the constructor owns configuration, private-state setup, recovery, mirror population, Interstack synthesis, and initial publication behind the existing writer lock.
- Rust storage now reconstructs complete deterministic facts-v1 JSONL exports from `CURRENT` or an explicit snapshot, verifies every referenced object before atomically replacing any destination, and preserves legacy stored-record tolerance without weakening normal fact validation. Rust also owns retention/pin-aware GC planning, dry-run/live execution, plan validation, `CURRENT` race rejection, and a lock-owning `Lexicon::garbage_collect` façade.

No Rust status/doctor diagnostics, post-publication consumer execution, CLI replacement, watch surface, or Warlock integration is implemented yet.

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

The next planned slice is status/doctor diagnostics, followed by post-publication consumers needed before Warlock can switch to the Rust library. CLI/watch remain thin hosts after the in-process library surface is complete. The replacement `lexicon` executable should live in a separate binary crate that depends on the library. In addition to the existing operational commands, its planned direct lookup surface includes `find`, `show`, `refs`, and `calls` so Lexicon can answer bounded semantic code-location/reference questions without requiring Arcana; multi-hop graph analysis remains Arcana's responsibility.

## Related docs

- [Lexicon architecture](ARCHITECTURE.md)
- [Development and verification](DEVELOPMENT.md)
- [Current status](STATUS.md)
- [Lexicon contracts](../spec/README.md)

## Notes

The migration oracle may grow as later slices become testable, but changes to facts, object, snapshot, or publication semantics require the normal contract-change process rather than an oracle update alone.
