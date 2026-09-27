# Go Adapter Phase 18 Packaging and Runtime — 2026-09-27

Parent index: [Development Documentation](INDEX.md)

## Status

**Phase 18 — packaging/runtime: COMPLETE.**

The native Rust `GoAdapter` now has a production packaging boundary for its private `lexicon-go-semantic` helper. Installed Lexicon discovers and runs that helper from the packaged adapter tree without a source checkout or repository-local helper build.

## Packaged layout

The root workflow builds and installs:

```text
bin/
  lexicon[.exe]
adapters/
  go-semantic/
    VERSION
    lexicon-go-semantic[.exe]
```

The retired standalone facts-v1 `lexicon-go` adapter is not built or installed, and its oracle source tree is excluded from release/install payloads.

The helper is compiled with the same bounded release workflow used for the other packaged adapter assets, and its `--version` output is verified before the build proceeds.

## Discovery

The native helper runner resolves the Go semantic helper in deterministic order:

1. explicit `LEXICON_GO_SEMANTIC_HELPER`;
2. `<adapter-root>/go-semantic/lexicon-go-semantic[.exe]`;
3. `<adapter-root>/lexicon-go-semantic[.exe]`;
4. executable adjacency.

For an installed Lexicon CLI, adapter-root discovery already checks the executable-adjacent `adapters/` directory, so the normal installed layout requires no explicit `--adapters` or helper override.

Missing-helper errors now list every checked path and point to `LEXICON_GO_SEMANTIC_HELPER`. `lexicon doctor` reports the native Go helper as `runtime helper: go` rather than checking for the retired `adapters/go` runtime directory.

## Version and fingerprint behaviour

Helper version `0.13.0` has one checked-in authority: `lexicon/adapters/go-semantic/VERSION`.

- The Go helper embeds that file and reports `lexicon-go-semantic 0.13.0` through `--version`.
- Rust includes the same file and passes `--helper-version 0.13.0` on every semantic-helper invocation.
- A mismatched/stale helper exits before reading or analyzing a request.
- The native Go adapter fingerprint continues to include the helper version, so helper-version changes invalidate the adapter fingerprint.
- The release workflow executes the built helper's `--version` command and rejects mismatched packaging.

The Lexicon CLI lockfile was regenerated to match the current Rust library dependency graph because the production `--locked` build correctly refused the stale lockfile.

## Runtime dependency

Packaging removes the need to compile the private helper from a source checkout at use time, but it does **not** remove Go tooling from the current semantic runtime.

The private helper uses `golang.org/x/tools/go/packages` for typed package loading, and that API delegates to the installed `go` command. Therefore an installed Go toolchain remains an explicit runtime prerequisite for typed Go analysis.

If `go` is absent from `PATH`, the helper fails closed before typed analysis with an actionable diagnostic identifying `go/packages`, `PATH`, and the installed-Go requirement. It does not silently emit a structurally reduced graph.

This replaces the earlier over-strong Phase 18 end-condition wording. The proven boundary is: **users need no Go source checkout or repository-local helper build artifact; the packaged helper is installed and discovered automatically, while the Go toolchain remains an explicit runtime dependency.**

## Verification

- Go semantic-helper suite: green, including shared-version and missing-Go diagnostics.
- Native Rust Go-adapter suite: **34/34 passed**, including live legacy/native differential parity and execution-shape determinism.
- Doctor runtime-helper tests: **5/5 passed**.
- Packaged adapter-root integration: passed using an actual compiled `lexicon-go-semantic` binary.
- Root workflow smoke suite: **8/8 passed**, including packaged-helper inclusion/install and helper-version-verifier regressions.
- Direct production adapter packaging verified the Go runtime payload contains exactly `VERSION` + `lexicon-go-semantic.exe` on Windows and excludes the legacy `adapters/go` oracle tree.
- Direct helper version check: `lexicon-go-semantic 0.13.0`.
- Stale helper-version invocation: rejected before semantic work.
- Real locked Lexicon build:
  - Rust `lexicon-cli` release build passed after lockfile reconciliation;
  - packaged `lexicon-go-semantic.exe` build and version verification passed;
  - remaining Go/C-family/GDScript/Kotlin/generic adapter builds passed;
  - the full cross-language build later stopped at the unrelated existing JDK 21+ prerequisite for the Java adapter.
- Installed-tree Go-present smoke: an installed `lexicon.exe init --languages go` succeeded without `--adapters` against the cleaned runtime-only adapter payload, proving executable-adjacent discovery and packaged-helper execution with no legacy `adapters/go` tree.
- Installed-tree Go-absent smoke: the packaged helper failed as expected with the explicit installed-Go diagnostic.

## Phase 19 boundary

Phase 19 owns final migration cleanup and documentation consolidation: removing the retired legacy Go runtime implementation where it is no longer needed, retaining useful oracle/parity evidence, removing migration-only scaffolding, and documenting the final stable Rust/helper ownership boundary.
