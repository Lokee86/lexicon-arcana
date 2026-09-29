# Go adapter

Parent index: [Lexicon Documentation](README.md)

## Purpose

Define the production Go-analysis ownership boundary, runtime requirements, deterministic semantic contract, and retained oracle coverage.

## Overview

Production Go analysis is owned by the native Rust `GoAdapter` under `src/adapters/go/`.

The adapter keeps repository discovery, canonical Lexicon identities, facts materialization, incremental ownership, validation, and scan orchestration in Rust. Language-native typed analysis runs in the private `adapters/go-semantic/` helper and returns semantic observations rather than facts-v1 records.

The retired standalone Go facts adapter has been removed. There is no legacy Go runtime fallback.

This completion applies to the **Go-language adapter migration**. It does not declare the broader Lexicon Go-to-Rust application migration complete or change the current operator-runtime recommendation recorded in [Status](STATUS.md).

## Runtime boundary

Installed Lexicon packages:

```text
adapters/
  go-semantic/
    VERSION
    lexicon-go-semantic[.exe]
```

Helper discovery is deterministic:

1. `LEXICON_GO_SEMANTIC_HELPER`;
2. `<adapter-root>/go-semantic/lexicon-go-semantic[.exe]`;
3. `<adapter-root>/lexicon-go-semantic[.exe]`;
4. executable adjacency.

Rust and the helper consume the same checked-in `VERSION` file. Rust supplies both protocol and helper versions on each invocation. A stale or incompatible helper fails before semantic analysis.

The helper is prebuilt in release packages, but typed analysis uses `golang.org/x/tools/go/packages`, which requires an installed `go` executable. Missing Go tooling is a hard runtime error; Lexicon does not silently degrade to structurally incomplete Go output.

## Ownership

Rust owns:

- repository/file/module discovery and exclusions;
- module ownership;
- repository, directory, file, and dependency facts;
- canonical semantic identity to Lexicon node-ID mapping;
- facts-v1 node, edge, unresolved, span, attribute, and owner materialization;
- incremental scope and publication integration;
- adapter fingerprinting and helper execution.

The private Go helper owns:

- Go parsing and declaration extraction;
- `go/packages` / `go/types` indexing;
- typed relationships;
- direct call observations;
- SSA/VTA higher-order and interface dispatch evidence;
- closure capture evidence;
- typed dataflow;
- deterministic semantic parallel reduction.

The helper does not own Lexicon SHA IDs, facts-v1 encoding, snapshot state, object storage, repository crawling, or publication.

## Semantic contract

The adapter models multi-module Go repositories, including:

- packages, imports, types, functions, methods, tests, interface methods, closures, parameters, variables, fields, and constants;
- `extends`, `implements`, `overrides`, and `references`;
- definite `calls`, `possible-calls`, conversions, builtins, and explicit unresolved call evidence;
- SSA/VTA reconciliation for function values, callbacks, method values, closures, and interface invokes;
- conservative `reads` and `writes`;
- `go.mod` dependencies and repository-local package dependencies;
- deterministic full and incremental analysis.

Reflection, unsafe runtime mutation, unscanned external implementations, and other unsupported dynamic forms remain outside the adapter's authority and are unresolved or external rather than guessed.

## Frozen oracle

The completed migration retains immutable compatibility evidence under:

```text
testdata/go_oracle/
  repositories/
  golden/
```

The seven fixture repositories cover calls, relationships, higher-order flow, dataflow, build tags, multi-module ownership, and deterministic parallel execution.

Native tests compare the complete semantic header and canonical records against the frozen goldens. The parallel fixture is also checked across multiple worker/shard/fan-in shapes.

The retired implementation is intentionally not retained. The immutable goldens are the compatibility oracle.

Phase 16 additionally calibrated the native adapter against pinned Demon Docs, Space Rocks, and Lexicon self-hosting repositories before cutover.

## Incremental behavior

Go's minimum safe incremental unit is a package. Lexicon expands a changed-file scope to the required package and dependency context, runs native Go analysis, restricts emitted ownership, and falls back to complete-language analysis when topology or prior state makes narrowing unsafe.

Incremental output must remain semantically equivalent to the corresponding full result for replaced owners, while untouched immutable objects remain reusable.

## Code map

| Concern | Primary implementation | Verification |
| --- | --- | --- |
| Rust adapter shell | `src/adapters/go/mod.rs` | `src/adapters/go/tests.rs` |
| Repository discovery/module ownership | `src/adapters/go/discovery.rs`, `module_ownership.rs` | discovery tests + frozen oracle |
| Identity and facts materialization | `src/adapters/go/identities.rs`, `facts.rs`, `semantic_*_facts.rs` | identity tests + seam reconciliation + frozen oracle |
| Private protocol | `src/adapters/go/protocol*.rs`, `adapters/go-semantic/protocol.go` | Rust protocol decoding + helper protocol tests |
| Typed Go semantics | `adapters/go-semantic/semantic_*.go` | helper Go tests |
| Runtime helper execution | `src/adapters/frontend/runner.rs`, `src/adapters/go/mod.rs`, `adapters/go-semantic/runtime.go` | frontend/helper/runtime/doctor tests |
| Packaging | `../scripts/workflow.py` | `../scripts/test_workflow.py`, `tests/go_packaged_runtime.rs` |
| Frozen semantic oracle | `testdata/go_oracle/` | `src/adapters/go/oracle_parity_tests.rs` |

## Tests

The permanent Go acceptance surface includes:

- private-helper Go tests;
- native Rust Go-adapter tests;
- complete native-vs-frozen-oracle parity;
- deterministic serial/parallel execution checks;
- incremental package-scope checks;
- packaged-helper integration;
- installed helper discovery and diagnostics through `doctor`.

## Related docs

- [Application](APPLICATION.md)
- [Adapter authoring](ADAPTER_AUTHORING.md)
- [Dependency semantics](DEPENDENCY_SEMANTICS.md)
- [Current status](STATUS.md)
- [Phase 16 calibration](../../docs/development/go-adapter-phase16-calibration-2026-09-27.md)
- [Phase 17 cutover](../../docs/development/go-adapter-phase17-cutover-2026-09-27.md)
- [Phase 18 packaging/runtime](../../docs/development/go-adapter-phase18-packaging-runtime-2026-09-27.md)

## Notes

The pre-Rust Go implementation remains available in repository history and its pinned migration reference, but it is not an active runtime, source dependency, release asset, or test executable.
