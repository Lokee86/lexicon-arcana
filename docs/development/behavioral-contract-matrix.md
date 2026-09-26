# Behavioral Contract Matrix

Parent index: [Development Documentation](INDEX.md)

## Purpose

Map active Lexicon + Arcana invariants to focused tests and release gates.

## Overview

The matrix protects independently usable component boundaries, immutable publication, graph compatibility, deterministic release composition, and documentation/architecture governance. Retired Grimoire discovery behavior is historical evidence rather than a compatibility target.

## Contracts

| Contract | Primary verification owner |
| --- | --- |
| Lexicon owns language parsing and normalized semantic facts | Lexicon adapter, contract, scan, and publication tests |
| Rust migration preserves facts-v1 identities, validation, ordering, and canonical JSONL against pinned Go oracles; the Go language adapter additionally preserves its frozen semantic fixture corpus byte-for-byte | `lexicon/tests/facts_contract.rs`, `lexicon/src/identity.rs` tests, `lexicon/evaluation/rust_migration/compare.py`, and `lexicon/adapters/go/oracle_test.go` |
| The private Go semantic-helper protocol is versioned, fail-closed, repository-relative, and independent of facts-v1 IDs and persistence state | `lexicon/adapters/go/semantic_protocol_roundtrip_test.go` and `semantic_protocol_validation_test.go` |
| Native Go adapter ownership is Rust-side; its private helper boundary validates the protocol handshake, bounds stderr, and never reintroduces a subprocess facts adapter | `lexicon/src/adapters/go/tests.rs`, `lexicon/src/adapters/helper.rs`, and `helper_capture.rs` |
| Go repository discovery, exclusions, module ownership, repository identity, and repository/directory/file facts are Rust-owned and preserve the frozen Go oracle | `lexicon/src/adapters/go/discovery_tests.rs`, `discovery_boundary_tests.rs`, and the frozen Go oracle goldens |
| Go structural AST extraction lives in the private semantic helper without Lexicon IDs/facts ownership; Rust materializes package/import/type/callable/closure declarations and preserves inactive build-tag variants | `lexicon/adapters/go-semantic/`, `lexicon/src/adapters/go/semantic_facts*.rs`, and seven-fixture repository/declaration parity in `semantic_parity_tests.rs` |
| Rust owns canonical Go identity strings, semantic-prefix → Lexicon-kind mapping, `_test` namespace canonicalization, and final node SHA generation; absolute checkout paths never enter identities | `lexicon/src/adapters/go/identities.rs`, `identities_tests.rs`, and seven-fixture node-ID parity in `semantic_parity_tests.rs` |
| The private Go helper owns typed package/type indexing with test-enabled `go/packages`, generic origins, alias unwrapping, completed interfaces, pointer/value method sets, Rust-inventory target filtering, and structured diagnostics | `lexicon/adapters/go-semantic/semantic_index*.go`, `semantic_targets.go`, `semantic_types.go`, helper tests, and unchanged legacy semantic/package tests |
| Go typed relationships are helper-reported semantic identities and Rust-owned Lexicon edges; `extends`, type/method `implements`, and `overrides` preserve legacy ownership, external/stdlib embedded-type contracts, endpoint validity, and no-implements-self-edge invariants | `lexicon/adapters/go-semantic/semantic_relationship*.go`, `lexicon/src/adapters/go/semantic_facts.rs`, `relationship_tests.rs`, and seven-fixture relationship parity in `semantic_parity_tests.rs` |
| Lexicon snapshots are immutable, content-addressed, and crash-safe | Go object-store/pending/recovery tests plus Rust `publication.rs` and `recovery.rs` parity tests |
| Fact-object binary encoding is deterministic, semantic-preserving, and backward-readable across v2, v1, and legacy JSON | Go `binary_codec_test.go` / `binary_golden_test.go` / `nodes_test.go`, Rust `storage_binary.rs` / `storage_compat.rs`, and Arcana `lexicon::binary_tests` |
| Lexicon consumers are bounded and cannot corrupt a valid publication | `lexicon/internal/consumer/runner_test.go` and scan/publication tests |
| Arcana consumes verified Lexicon state rather than duplicating language parsers | Arcana Lexicon-ingestion and repository tests |
| Arcana preserves the consumed Lexicon snapshot identity | repository manifest and snapshot tests |
| Arcana publishes complete graph generations before replacing active state | repository/snapshot publication-failure tests |
| Arcana overlays validate base identity and compact without changing graph meaning | overlay and compaction tests |
| Exact graph traversal remains independent of optional semantic vectors | protocol/traversal tests and vector-disabled tests |
| `arcana.query.v1` capabilities required by consumers are negotiated before a combined build is accepted | root workflow protocol verification |
| Lexicon and Arcana remain independently installable | workflow packaging/install smoke tests |
| Combined release bundles contain L+A executables, Lexicon adapters, the production L+A skill, installer, and legal metadata | `scripts/test_workflow.py` |
| The production L+A skill uses installed repository state and contains no benchmark-only environment contract | Pitlord repository policy and workflow smoke tests |
| Active release surfaces do not build, install, or publish Grimoire | Pitlord repository policy and workflow smoke tests |
| Root, Lexicon, and Arcana documentation trees pass without baselines | shared documentation policy and `scripts/check_docs.py` |
| Historical Grimoire benchmark artifacts remain evidence, not current product contracts | ADR 0006 and documentation ownership rules |

## Release gates

```bash
python scripts/workflow.py smoke
python scripts/workflow.py test
python .standards/docs_policy/check.py --repo .
python .standards/docs_policy/check.py --repo . --config docs-standard.lexicon.json
python .standards/docs_policy/check.py --repo . --config docs-standard.arcana.json
python scripts/check_docs.py
```

## Code map

| Matrix concern | Primary implementation or artifact | Protecting tests/gates |
| --- | --- | --- |
| Lexicon semantics/publication | `lexicon/adapters/`, `lexicon/internal/scan/`, `lexicon/internal/objectstore/` | Lexicon complete test matrix |
| Lexicon Rust migration parity | `lexicon/src/`, `lexicon/evaluation/rust_migration/` | Rust fmt/test/clippy, facts/storage/publication/recovery/materialization/dependency/topology/scan-planning/config/repository-state/adapter-host/scope/execution/scan-engine parity tests, Arcana Lexicon compatibility tests, Go planner/fingerprint oracles, Go state/config/adapter/scope/scan tests, and migration comparator |
| Arcana graph publication/traversal | `arcana/src/repository/`, `arcana/src/storage/`, `arcana/src/snapshot/`, `arcana/src/protocol/` | Arcana Cargo test suite |
| Documentation/change impact | `.standards/docs_policy/`, `scripts/check_docs.py` | documentation-standard workflow |
| Architecture invariants | `tools/pitlord/` | Pitlord validation/check |
| Release packaging | `scripts/workflow.py`, `scripts/install.py`, `.github/workflows/release.yml` | workflow smoke and full test gate |

## Related docs

- [Documentation coverage](documentation-coverage.md)
- [Release workflow](release-workflow.md)
- [Component architecture](../architecture/components.md)
- [Architecture verification](architecture-verification.md)

## Notes

Update this matrix when an active invariant, focused test location, component boundary, or release gate changes. Retired Grimoire contracts belong in historical reports/ADRs, not this matrix.