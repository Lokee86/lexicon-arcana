# Go Adapter Phase 19 Final Cleanup — 2026-09-27

Parent index: [Development Documentation](INDEX.md)

## Status

**Phase 19 — final cleanup: COMPLETE.**

The Go-language adapter migration is complete. Go analysis ownership is Rust + the private Go semantic helper; the retired standalone Go facts adapter is removed from the active source tree. This does not change the broader Lexicon migration status or operator-runtime recommendation.

## Cleanup

Phase 19:

- removed the retired `lexicon/adapters/go/` standalone implementation;
- removed the now-dead release-workflow shim that stripped that legacy directory after packaging;
- removed the live legacy-adapter differential build path;
- retained the seven fixture repositories and canonical facts as immutable frozen-oracle evidence under `lexicon/testdata/go_oracle/`;
- converted full-record parity to native-vs-frozen-golden comparison, including deterministic execution-shape checks;
- retained strict helper protocol coverage in the production `go-semantic` module;
- removed the migration-only Go-adapter real-repository comparator while retaining the broader Go-runtime compatibility guide needed by unfinished Lexicon migration work;
- preserved the broader Lexicon operator-runtime recommendation instead of treating Go-adapter completion as whole-application migration completion;
- consolidated the final Go-language ownership contract in `lexicon/docs/GO_ADAPTER.md`.

## Final ownership

Rust owns the Go adapter, fact identities/materialization, repository discovery, incremental integration, validation, storage interaction, and helper lifecycle.

The private Go helper owns the language-native parser/type/SSA/VTA/dataflow work required to produce semantic observations. It is packaged as a private runtime asset, not as a standalone facts adapter.

There is no second Go facts implementation and no production fallback to the retired runtime.

## Retained compatibility evidence

The compatibility boundary is now immutable evidence rather than executable duplicate code:

- frozen fixture repositories;
- frozen canonical facts;
- exact full-record parity tests;
- deterministic execution-shape tests;
- Phase 16 pinned real-repository calibration results;
- historical migration commits and reports.

The pinned pre-Rust revision remains historical evidence for this completed Go-language adapter migration; it may still serve the broader Lexicon migration and operator guidance until those independent migrations finish.

## Verification gate

All Phase 19 gates passed:

- Go semantic-helper `go test ./...`: green after moving all fixture consumers to `testdata/go_oracle/repositories/`;
- native Rust Go-adapter suite: **34/34 passed**, including exact full-record parity against frozen canonical facts and serial/parallel execution-shape determinism;
- full Rust Lexicon `cargo test`: green across unit, integration, packaged-helper, storage, scan, adapter, and doc-test targets;
- locked `lexicon-cli` tests: green across unit, command, lookup, and doc-test targets;
- retained root Go application `go test ./...`: green after removal of the nested standalone Go-adapter module;
- packaged-runtime/doctor gate: **1/1 + 5/5 passed**;
- workflow packaging/install smoke: **8/8 passed**;
- documentation validation: **192 Markdown files passed**, with all configured required paths present;
- `cargo fmt -- --check` and `git diff --check`: green;
- fresh native-Go → Arcana acceptance on `basic_calls`: imported **32 nodes / 44 edges / 1 unresolved**, protocol `stats` passed, and exact `resolve_symbol`/`search_nodes` resolved `caller` in `main.go` with identity `sha256:4d74db46f55aa705188c325728b3980b01f0fb85fcc8beeb305915c1a45dc389`.

## End condition

The Go migration has one production implementation boundary, one private language-native helper, one frozen compatibility oracle, and no live legacy adapter path.

Further work on Go is ordinary adapter maintenance or optimization, not migration.
