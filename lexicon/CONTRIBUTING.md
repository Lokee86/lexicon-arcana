# Contributing to Lexicon

Lexicon changes must preserve deterministic semantic evidence and explicit ownership boundaries across multiple language runtimes.

## Before changing code

If you are adding a language or substantially changing an adapter, read [docs/ADAPTER_AUTHORING.md](docs/ADAPTER_AUTHORING.md) first. It defines the current Rust-native authoring target and the additional compatibility work required when the adapter must run in the recommended Go Lexicon.

Identify the owning layer:

- native language extraction belongs in `src/adapters/<language>/`;
- Go-runtime compatibility/parity implementations belong in `adapters/<language>/`;
- native adapter registration belongs in `src/adapters/host.rs` and `src/languages/`;
- scan planning, storage, consumers, and watch behavior belong to their Rust library modules rather than adapters;
- Rust CLI presentation belongs in `../lexicon-cli/`;
- public format meaning belongs in `spec/`.

Do not add consumer-specific graph, ranking, or documentation policy to Lexicon facts or adapters.

## Change expectations

Keep changes bounded to the owning seam. Avoid unrelated cleanup. Prefer direct ownership and concrete data flow over wrapper layers that only redirect calls.

Semantic changes must preserve:

- stable identities;
- deterministic record and key ordering;
- source ownership;
- definite versus possible relationship meaning;
- explicit unresolved evidence;
- immutable object and snapshot behavior;
- complete-language fallback when scoped analysis cannot remain sound.

## Required verification

Run focused tests while developing, then the complete affected matrix before completion. The canonical commands are documented in [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md).

Cross-cutting changes should finish with:

```text
python evaluation/run_tests.py
```

Concurrency changes must also run:

```text
go test -race ./...
go -C adapters/go test -race ./...
```

Semantic output changes require facts-v1 validation, a semantic report, repeat-run comparison, and the relevant corpus cases. Update `evaluation/validation/baseline.json` only through a complete passing validation run.

## Contract changes

Do not silently change v1 contract meaning. Changes to stable IDs, ownership, sorting, relation semantics, object bytes, hash domains, manifests, or runtime-evidence reconciliation require explicit compatibility analysis and coordinated reader/writer updates.

See [spec/README.md](spec/README.md).

## Documentation requirements

Update documentation in the same change as implementation:

- root entry-point changes: `README.md`;
- CLI or operational changes: `docs/APPLICATION.md`;
- architecture, ownership, concurrency, or storage changes: `docs/ARCHITECTURE.md`;
- capability or limitation changes: `docs/STATUS.md`;
- language behavior: the owning adapter README;
- normative format behavior: `spec/`;
- test or release workflow: `docs/DEVELOPMENT.md` or `docs/RELEASE_PACKAGING.md`.

Every new document must be linked from its owning folder index. Dated validation results must state their date and must not be presented as timeless performance guarantees.

## Review checklist

Before submitting a change, confirm:

- the owning seam is clear;
- no unsupported relationship is guessed;
- new behavior has positive and negative tests;
- output remains deterministic;
- scoped analysis retains a safe full fallback;
- generated files and caches are not committed;
- relevant documentation is current;
- focused and complete verification passed.
