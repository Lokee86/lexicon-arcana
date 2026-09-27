# Go Adapter Phase 17 Cutover — 2026-09-27

Parent index: [Development Documentation](INDEX.md)

## Status

**Phase 17 — cutover: COMPLETE.**

Go analysis now has one production owner: the native Rust `GoAdapter`. The standalone Go facts-v1 adapter remains in-tree only as a parity oracle until Phase 19 cleanup.

## Cutover changes

- The Rust `AdapterHost` remains the authoritative Go registration and scan path.
- The legacy Go application runner now rejects `go` instead of launching either a packaged or source-tree standalone Go adapter.
- The root build workflow no longer builds `adapters/go` into a `lexicon-go` runtime executable.
- The standalone `adapters/go` implementation remains executable directly for differential/oracle maintenance.
- The private `adapters/go-semantic` helper remains the deliberate language-native semantic boundary used by the Rust adapter.
- A native end-to-end scan regression now proves both a full Go scan and a subsequent incremental Go scan use `ScanEngine` + native `GoAdapter`, replacing the changed file object while reusing an untouched package object.

## Verification

- Native Rust Go-adapter suite: **34/34 passed**.
- Full Rust Lexicon `cargo test`: green across unit, integration, and doc-test targets.
- Phase 15 live legacy/native differential tests remain green inside the native Go suite.
- Native execution-shape determinism remains green.
- Legacy Go application's adapter-runner tests: green, including explicit rejection of the retired Go runtime path.
- Full retained Go application `go test ./...`: green.
- Standalone legacy Go oracle suite: green when run directly from `adapters/go`.
- Root workflow smoke suite: **7/7 passed**, including a regression that the release workflow does not build the legacy Go runtime adapter.
- `git diff --check`: green before final documentation staging.

Phase 16's pinned Demon Docs, Space Rocks, Lexicon self-host, determinism, and Arcana acceptance evidence remains the semantic basis for this cutover.

## Phase 18 boundary

Phase 17 does **not** claim packaged native Go deployment is finished.

The Rust adapter still requires the private `lexicon-go-semantic` helper. Phase 18 owns its production build/install layout, discovery, version/fingerprint behaviour, platform handling, missing-helper diagnostics, and packaged-runtime tests. Until that is complete, removal of `lexicon-go` is intentionally not replaced by another standalone facts-v1 runtime fallback.

## End condition

The legacy Go facts-v1 adapter is no longer a runtime fallback or production build path. It is oracle/history only.

The next migration phase is **Phase 18 — packaging/runtime**.
