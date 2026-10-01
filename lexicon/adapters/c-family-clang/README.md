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

It reads one JSON request line from stdin and writes a framed stream to stdout.

The private protocol is v3 and the helper version is 0.7.0. There is no v2 compatibility reader. Protocol v3 exposes:

- `capabilities` — linked Clang version and compilation-database visibility;
- `structural` — translation-unit/build context plus source-language, declaration, include, macro, diagnostic, inheritance/override, call-target, overload, receiver-type, virtual-dispatch, compiler-bound argument/value-flow, pointer-binding, access, and macro-expansion observations.

The structural request contains `protocol_version`, `operation`, `repository_root`, `owned_files`, `context_files`, and `workers`. The ownership inventories are canonical, disjoint repository-relative paths. The removed `shards` and `merge_fan_in` fields are rejected. The helper creates primary parse units from exact real header compile commands, real source translation units, and one deterministic synthetic command for each source lacking a real command, in that order. It does not assign headers to source tasks or run a dependency-scanning planning pass.

Each worker lane reuses one `ClangTool` across its ordered translation units while each translation unit gets a fresh AST/Sema lifecycle. Owned paths encountered during parsing may produce observations. Context-only and external paths support semantic resolution but cannot publish ordinary file observations. An owned header is claimed by the first canonical translation unit that actually observes it; duplicate file observations are discarded. Owned headers not encountered by primary units receive one orphan fallback parse after primary execution. The ordered committer bounds completed-but-uncommitted results to a worker-sized window.

Changed-source requests parse the changed source units only. Changed-header requests use candidate context sources in real-before-synthetic order. Structural output is a framed stream: each claimed owned-file payload is emitted as its ordered claim commits, then the single metadata frame is emitted after translation-unit execution and file-frame emission finish. Rust copies each file payload directly into its final spool file, so there is no response-wide `files[]` object or decode → reserialize → decode loop. Context identities are compact target-resolution evidence and never ordinary file facts or graph-source nodes. Relationship sources must resolve to materialized owned declarations.

When `LEXICON_PERF=1`, helper metrics expose parse-unit counts, active Clang lanes, completed translation units, orphan fallback units, claimed owned files, discarded duplicate file observations, framed transport, helper RSS, frontend wall time, and the retained R7 C-family hot-path counters. These are instrumentation fields, not benchmark results. Compilation-database commands are used when available.

The protocol does not contain facts-v1 records or canonical Lexicon IDs.

## Ownership boundary

Clang owns C/C++ parsing, preprocessing/macro expansion, compiler declarations, type/receiver evidence, overload resolution, call binding, inheritance/override evidence, and compiler-bound value references.

Rust owns repository-relative discovery, canonical identities, local include/file ownership, compiler-reference correlation, definite/possible/unresolved policy, fixed-point callback propagation over compiler-bound identities, `passes-to`, `reads`, `writes`, facts-v1 validation, and deterministic output. When an owned observation references a declaration in an unchanged context file, the helper emits only the compact compiler/path/kind/qualified-name/signature identity needed for Rust to reproduce the existing canonical declaration ID; the context file itself remains unowned.

Rust does not parse macro replacement text or reconstruct C/C++ compiler semantics after the Phase 2.6 cutover.

## Code map

| Concern | Implementation |
| --- | --- |
| Helper entry point and protocol dispatch | `main.cpp`, `structural.cpp`, `structural_action.cpp` |
| Parse-unit planning and exact/synthetic compile commands | `structural_compilation.h`, `structural_compilation.cpp` |
| Ordered ownership claims, deterministic emission, and bounded commit window | `structural_commit.h`, `structural_commit.cpp` |
| Persistent worker lanes and orphan fallback | `structural_execution.h`, `structural_execution.cpp` |
| Translation-unit/declaration observations | `structural_frontend.cpp`, `structural_declarations.cpp`, `structural_declaration_support.cpp` |
| Calls and compiler relationships | `structural_calls.cpp`, `structural_relationships.cpp`, `structural_semantic_support.cpp`, `structural_semantics.cpp` |
| Pointer/value/access observations | `structural_access_flow.cpp`, `structural_value_flow.cpp`, `structural_value_flow_json.cpp` |
| Observation model and deterministic JSON | `structural_model.h`, `structural_model.cpp` |
| Rust production adapter and materialization | `../../src/adapters/c_family/clang_frontend.rs`, `clang_protocol.rs`, `clang_materialization.rs` |
| Rust graph/fact policy | `../../src/adapters/c_family/semantic_call_facts.rs`, `semantic_pointer_index.rs`, `semantic_dataflow_facts.rs` |
| Compilation-domain and ordered-commit unit tests | `structural_compilation_test.cpp`, `structural_commit_test.cpp` |
| Native ownership, determinism, incremental-context, and protocol regressions | `structural_ownership_test.py` |

## Calibration

Start calibration only after the complete Phase 7 integration gate passes. The first LevelDB invocation runs and records the multilang regression gate; later cases require that result and the preceding cold/warm case records. Use a frozen corpus tree containing `leveldb`, `nlohmann-json`, `fmt`, `catch2`, `codebase-memory-mcp`, and `git`; set `LEXICON_C_FAMILY_CORPUS` to its parent directory or pass `--corpus-source`. Build the release `adapter_eval` executable and native helper after Phase 7 passes.

From the repository root, the multilang gate is:

```powershell
$env:TEMP = Join-Path (Get-Location) '.hardcut-build\tmp'
$env:TMP = $env:TEMP
New-Item -ItemType Directory -Force $env:TEMP | Out-Null
python scripts/lexicon_perf_regression.py --tier multilang
```

Then run these harness cases in order, stopping at the first canonical-fact mismatch or incomplete run:

```powershell
python scripts/c_family_tu_calibration.py --case leveldb --adapter-eval lexicon/target/release/examples/adapter_eval.exe --clang-helper .hardcut-build/native/lexicon-c-family-clang.exe
python scripts/c_family_tu_calibration.py --case nlohmann-json --adapter-eval lexicon/target/release/examples/adapter_eval.exe --clang-helper .hardcut-build/native/lexicon-c-family-clang.exe
python scripts/c_family_tu_calibration.py --case fmt --adapter-eval lexicon/target/release/examples/adapter_eval.exe --clang-helper .hardcut-build/native/lexicon-c-family-clang.exe --check-concurrency
python scripts/c_family_tu_calibration.py --case catch2 --adapter-eval lexicon/target/release/examples/adapter_eval.exe --clang-helper .hardcut-build/native/lexicon-c-family-clang.exe
python scripts/c_family_tu_calibration.py --case codebase-memory --adapter-eval lexicon/target/release/examples/adapter_eval.exe --clang-helper .hardcut-build/native/lexicon-c-family-clang.exe --check-concurrency
python scripts/c_family_tu_calibration.py --case git --adapter-eval lexicon/target/release/examples/adapter_eval.exe --clang-helper .hardcut-build/native/lexicon-c-family-clang.exe
```

Make-based compilation-database preparation joins backslash-continued dry-run recipes before extracting compiler commands. Multi-source compiler/link recipes produce one syntax-only entry per source, preserving build include paths, defines, and language flags. Exact duplicate commands are removed while distinct build contexts remain. Focused capture regressions run with `python -m unittest discover -s scripts -p test_c_family_make_compdb.py`.

Each result stores cold and warm wall time, process-tree peak RSS, TU architecture counters, and the canonical facts hash. `--check-concurrency` adds worker-count 1, 2, and 4 runs and requires the same hash. Codebase Memory must complete without timeout, with cold wall at most 63 seconds and cold peak process-tree RSS at most 2.812 GB. Git calibration is a separate follow-up and is gated on that Codebase Memory result. Result records and build/cache data stay under `.hardcut-build` unless `--results-dir` is supplied.
