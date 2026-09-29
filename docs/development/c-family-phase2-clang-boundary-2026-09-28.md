# C-family Phase 2.2 Clang runtime boundary

Parent index: [Development Documentation](INDEX.md)

**Date:** 2026-09-28

**Branch:** `refactor/c-family-clang`

**Starting checkpoint:** `b3d0c1281c2a53bf8cd61b467752ee06d48b4e52` — `Freeze C-family Phase 2 oracle`

**Helper protocol:** `1`

**Helper version:** `0.1.0`

## Purpose

Phase 2.2 establishes the permanent private process boundary for the C/C++ authoritative frontend before any production fact ownership moves to Clang.

The architectural direction is:

```text
Clang/LibTooling
      |
      | private versioned JSON observations
      v
FrontendRunner
      |
      v
Rust C-family adapter
      |
      v
Analysis / canonical Lexicon facts
```

In Phase 2.2, only the boundary exists. The existing native Rust/Tree-sitter C-family path remains the sole producer of production facts. There is no dual-analysis path, fallback, shadow facts producer, or facts-shaped Clang protocol.

## Private helper

The helper source lives at:

`lexicon/adapters/c-family-clang/`

Its runtime executable is:

`lexicon-c-family-clang[.exe]`

and its packaged runtime directory is:

`adapters/c-family-clang/`

The helper links Clang/LibTooling rather than shelling out to a compiler. The first protocol operation is intentionally narrow: `capabilities`.

A capabilities request contains:

- protocol version;
- operation name;
- absolute repository root.

The response contains:

- protocol version;
- helper version;
- linked Clang version;
- advertised compiler capabilities;
- whether a compilation database was discovered;
- the compilation-database diagnostic when discovery fails.

The helper currently advertises the permanent frontend evidence domains that later Phase 2 work will populate: AST, compilation database, preprocessor, and source manager.

## Rust boundary

`lexicon/src/adapters/c_family/clang_frontend.rs` owns helper discovery and execution policy.

It reuses the existing deep `FrontendRunner` process seam. C-family does not introduce a second process-management abstraction.

Runtime discovery checks, in order, the existing `FrontendRunner` locations including the explicit:

`LEXICON_C_FAMILY_CLANG_HELPER`

override.

`lexicon/src/adapters/c_family/clang_protocol.rs` owns the private wire contract and embeds the required helper version from the packaged `VERSION` file.

Protocol and helper versions are checked independently. A stale helper cannot silently satisfy a current Rust caller.

## Ownership in Phase 2.2

### Clang helper owns

- proof that the linked Clang runtime is available;
- Clang version reporting;
- compilation-database discovery;
- declaration of compiler evidence capabilities.

### Rust owns

- helper discovery and lifecycle;
- protocol validation;
- helper-version validation;
- all existing C-family parsing and semantic processing;
- Lexicon identity policy;
- certainty and unresolved policy;
- fact materialization;
- canonicalization and validation.

No production `CFamilyAdapter::analyze` call reaches the Clang helper yet.

## Diagnostics

Lexicon Doctor now treats the packaged Clang helper as part of a usable C-family runtime.

For a snapshot containing C-family data it reports:

`runtime helper: c-family`

and identifies the missing helper plus the `LEXICON_C_FAMILY_CLANG_HELPER` override when runtime discovery fails.

This check verifies runtime presence only. The capabilities operation is the protocol-level proof surface used by focused tests and subsequent migration phases.

## Packaging

The Lexicon build workflow now:

1. builds the private Go semantic helper as before;
2. builds the C-family Clang helper through `scripts/build_c_family_clang.py`;
3. verifies the helper's `--version` output;
4. packages only the runtime binary and `VERSION` beneath `adapters/c-family-clang/`;
5. no longer builds the historical external Go `lexicon-c-family` executable into the runtime package.

The historical Go C-family source remains repository evidence. It is not a packaged fallback.

The helper build script accepts optional `--llvm-dir` and `--clang-dir`, corresponding to the LLVM and Clang CMake package locations.

## External build dependency

Building the native helper requires LLVM and Clang **development** CMake packages, including `LLVMConfig.cmake` and `ClangConfig.cmake`.

On the current Windows development host, CMake and a C++ compiler are available, but no LLVM/Clang development package is installed or discoverable. A real configure attempt reaches compiler detection successfully and then stops at `find_package(LLVM REQUIRED CONFIG)` because `LLVMConfig.cmake` is absent.

Therefore:

- the Rust boundary, protocol, diagnostics, packaging logic, and build-driver logic are exercised;
- CMake dependency discovery fails at the expected external prerequisite;
- an actual linked `lexicon-c-family-clang` binary cannot be compiled on this host yet.

No system package was installed as part of Phase 2.2.

## Verification

Phase 2.2 protects the boundary with:

- C-family Clang protocol tests for successful capability negotiation and stale-helper rejection;
- all C-family unit tests;
- all existing C-family integration tests;
- the exact Phase 2.1 canonical-facts oracle;
- Doctor runtime-helper presence/missing diagnostics;
- workflow packaging and version-verification tests;
- Python syntax verification for the standalone helper build driver;
- CMake configuration through compiler detection and explicit LLVM dependency failure;
- the permanent multilang performance regression gate.

The key non-regression is that Phase 2.2 introduces **zero production C-family fact changes**.

## Gate

Phase 2.2 is complete in repository architecture when:

- the private helper protocol and runtime seam exist;
- helper discovery/version rejection is tested;
- Doctor diagnoses the runtime;
- release packaging owns the helper and removes the legacy packaged C-family sidecar;
- the Phase 2.1 facts oracle remains exact;
- the C-family and multilang regression surfaces remain green.

Native helper compilation remains an environment-dependent verification until an LLVM/Clang development SDK is available.

## Next

Phase 2.3 moves structural compiler evidence behind this boundary: translation-unit/build context, parsing, declarations, namespaces and types, includes, macros, and source-language context become Clang-authoritative observations. Rust continues to own Lexicon identities and materializes equivalent structural facts.
