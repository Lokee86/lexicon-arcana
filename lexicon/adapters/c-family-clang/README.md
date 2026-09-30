# C-family Clang frontend

This directory owns Lexicon's private C/C++ compiler frontend process. Clang/LibTooling is the sole production C/C++ syntax and compiler-semantic authority.

The helper is a process boundary: Clang/LLVM types do not cross into the Rust adapter. It emits versioned compiler observations while Rust owns repository discovery, canonical Lexicon identities, source ownership, fact policy, graph-specific value flow, deterministic canonicalization, and publication.

There is no Tree-sitter C/C++ production path and no Clang-to-Tree-sitter fallback.

## Requirements

Builds require a Clang/LLVM development installation that exports both `LLVMConfig.cmake` and `ClangConfig.cmake`. A compiler executable alone is not sufficient.

## Build

Use the repository build helper:

```text
python scripts/build_c_family_clang.py --output build/c-family-clang
```

If LLVM/Clang CMake packages are outside the normal CMake prefix path, pass `--llvm-dir` and `--clang-dir`.

Release packaging builds and version-verifies this helper. Installed C-family analysis fails closed when the helper is missing, stale, or cannot execute.

## Runtime contract

The helper accepts:

- `--version`
- `--protocol-version <n>`
- `--helper-version <version>`

It reads one JSON request line from stdin and writes one JSON response line to stdout.

Protocol v2 exposes:

- `capabilities` — linked Clang version and compilation-database visibility;
- `structural` — translation-unit/build context plus source-language, declaration, include, macro, diagnostic, inheritance/override, call-target, overload, receiver-type, virtual-dispatch, compiler-bound argument/value-flow, pointer-binding, access, and macro-expansion observations.

The structural request receives the repository root, explicit `owned_files` and `context_files` inventories, and Lexicon's `workers`, `shards`, and `merge_fan_in` execution policy. The two ownership inventories must be canonical, disjoint repository-relative paths. Protocol v2 has no legacy `files` field. Compilation-database commands are used when available. Owned headers are assigned one deterministic real translation-unit context when possible and use direct synthetic analysis only when no usable real context exists. Context files may be parsed for compiler semantics but cannot emit ordinary file observations.

The protocol does not contain facts-v1 records or canonical Lexicon IDs.

## Ownership boundary

Clang owns C/C++ parsing, preprocessing/macro expansion, compiler declarations, type/receiver evidence, overload resolution, call binding, inheritance/override evidence, and compiler-bound value references.

Rust owns repository-relative discovery, canonical identities, local include/file ownership, compiler-reference correlation, definite/possible/unresolved policy, fixed-point callback propagation over compiler-bound identities, `passes-to`, `reads`, `writes`, facts-v1 validation, and deterministic output. When an owned observation references a declaration in an unchanged context file, the helper emits only the compact compiler/path/kind/qualified-name/signature identity needed for Rust to reproduce the existing canonical declaration ID; the context file itself remains unowned.

Rust does not parse macro replacement text or reconstruct C/C++ compiler semantics after the Phase 2.6 cutover.

## Code map

| Concern | Implementation |
| --- | --- |
| Helper entry point and protocol dispatch | `main.cpp`, `structural.cpp`, `structural_action.cpp` |
| Deterministic compiler-context planning | `structural_plan.h`, `structural_plan.cpp` |
| Translation-unit/declaration observations | `structural_frontend.cpp`, `structural_declarations.cpp`, `structural_declaration_support.cpp` |
| Calls and compiler relationships | `structural_calls.cpp`, `structural_relationships.cpp`, `structural_semantic_support.cpp`, `structural_semantics.cpp` |
| Pointer/value/access observations | `structural_access_flow.cpp`, `structural_value_flow.cpp`, `structural_value_flow_json.cpp` |
| Observation model and deterministic JSON | `structural_model.h`, `structural_model.cpp` |
| Rust production adapter and materialization | `../../src/adapters/c_family/clang_frontend.rs`, `clang_protocol.rs`, `clang_materialization.rs` |
| Rust graph/fact policy | `../../src/adapters/c_family/semantic_call_facts.rs`, `semantic_pointer_index.rs`, `semantic_dataflow_facts.rs` |
