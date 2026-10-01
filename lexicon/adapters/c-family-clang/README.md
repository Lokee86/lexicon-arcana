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

The structural request contains `protocol_version`, `operation`, `repository_root`, `owned_files`, `context_files`, and `workers`. The ownership inventories are canonical, disjoint repository-relative paths. The removed `shards` and `merge_fan_in` fields are rejected. The helper plans exact real header compile commands, real source translation units, and deterministic synthetic candidates for sources lacking a real command. Real units finish before synthetic execution. A synthetic source is omitted only when its canonical real-TU claim contains declarations or macros, the real TU completed without error/fatal diagnostics, and its language matches the synthetic source language. Merely entering a file, an inactive include, an empty file entry, or a recovery AST does not qualify. Sources with explicit compile commands always run. It does not assign headers to source tasks or run a dependency-scanning planning pass.

Within each execution phase, each worker lane reuses one `ClangTool` across its ordered translation units while each translation unit gets a fresh AST/Sema lifecycle. Owned paths encountered during parsing may produce observations. Context-only and external paths support semantic resolution but cannot publish ordinary file observations. An owned file is claimed by the first canonical translation unit that actually observes it; claims persist across the real and synthetic phases, and duplicate file observations are discarded. Owned headers not encountered by primary units receive one orphan fallback parse after primary execution. Before a phase starts, its immutable prior claims suppress duplicate file-observation construction; global ownership remains available for semantic target resolution. Claims arriving within a phase never change traversal, so worker timing cannot change output. AST traversal skips external or previously claimed declaration subtrees only when their source ranges contain no owned include or macro expansion. The same conservative top-level roots are supplied to Clang's `ASTContext::setTraversalScope()`, so `getParents()` does not rebuild a parent map over excluded dependency subtrees. Compiler declarations and type resolution remain accessible. Ambiguous ranges and loaded/PCH source locations retain traversal and the full parent-map scope.

The ordered committer bounds queued results by both a worker-sized count window and a 64 MiB retained-allocation estimate, including observation vector/string capacities and container overhead. Encoding and emission happen outside the committer mutex through one ordered emitter. An oversized future result waits in its producer lane until it reaches the frontier, where it drains without queue admission. The estimate is not an RSS cap: active Clang ASTs, one blocked result per producer, in-flight encoding, accumulated metadata, and allocator retention remain outside the pending queue budget.

Changed-source requests parse the changed source units only. Changed-header requests use candidate context sources in real-before-synthetic order. Structural output is a framed stream: each claimed owned-file payload is emitted as its ordered claim commits, then the single metadata frame is emitted after translation-unit execution and file-frame emission finish. Rust copies each file payload directly into its final spool file, so there is no response-wide `files[]` object or decode → reserialize → decode loop. Context identities are compact target-resolution evidence and never ordinary file facts or graph-source nodes. Relationship sources must resolve to materialized owned declarations.

When `LEXICON_PERF=1`, helper metrics expose parse-unit counts, active Clang lanes, completed translation units, orphan fallback units, claimed owned files, discarded duplicate file observations, framed transport, helper RSS, frontend wall time, and the retained R7 C-family hot-path counters. `synthetic_source_candidates` counts planned source fallbacks, `skipped_covered_source_units` counts candidates covered by valid real observations, and `primary_synthetic_parse_units` counts those actually scheduled. Skipped sources have no synthetic TU metadata entry; their emitted file retains the real TU context. `peak_pending_estimated_bytes` records the pending queue estimate high-water mark; `pending_byte_limit` records its admission budget. Traversal metrics report pruned and traversed declarations. Opt-in `c-family.clang.tu.*` metrics sample before parsing, at AST-consumer entry, after semantic traversal, after compiler teardown, and after committer submission. They report current/peak process RSS, AST arena/side-table allocations, and estimated observation retention; supported glibc versions also report live/free allocator chunks and mapped allocation bytes. Unsupported allocator/current-RSS measurements return zero. These counters describe the process, not an individual lane's exclusive memory. Rank is local to its real, synthetic, or orphan execution phase; `after_submit` may mean queue admission rather than completed emission. These are instrumentation fields, not benchmark results. Compilation-database commands are used when available.

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
| Ordered ownership claims, unlocked emission, and count/byte admission | `structural_commit.h`, `structural_commit.cpp`, `structural_memory.h`, `structural_memory.cpp` |
| Persistent worker lanes, orphan fallback and thresholded freed-page reclamation | `structural_execution.h`, `structural_execution.cpp`, `structural_heap.h`, `structural_heap.cpp`, `structural_heap_test.py` |
| Translation-unit/declaration observations and conservative AST pruning | `structural_frontend.cpp`, `structural_traversal_scope.h`, `structural_traversal_scope.cpp`, `structural_declarations.cpp`, `structural_declaration_support.cpp` |
| Calls and compiler relationships | `structural_calls.cpp`, `structural_relationships.cpp`, `structural_semantic_support.cpp`, `structural_semantics.cpp` |
| Pointer/value/access observations and owned binding-source regression | `structural_access_flow.cpp`, `structural_value_flow.cpp`, `structural_value_flow_json.cpp`, `structural_pointer_ownership_test.py` |
| Observation model and deterministic JSON | `structural_model.h`, `structural_model.cpp` |
| Opt-in TU lifetime, RSS, AST allocation, and allocator-retention profiling | `structural_profile.h`, `structural_profile.cpp`, `structural_profile_test.py` |
| Rust production adapter and materialization | `../../src/adapters/c_family/clang_frontend.rs`, `clang_protocol.rs`, `clang_materialization.rs` |
| Rust graph/fact policy | `../../src/adapters/c_family/semantic_call_facts.rs`, `semantic_pointer_index.rs`, `semantic_dataflow_facts.rs` |
| Compilation-domain, ordered-commit, byte backpressure/unlocked encoding, and valid source-coverage unit tests | `structural_compilation_test.cpp`, `structural_commit_test.cpp`, `structural_commit_buffer_test.cpp`, `structural_coverage_test.cpp` |
| Native ownership, determinism, incremental-context, protocol, staged source coverage, and owned include/macro traversal regressions | `structural_ownership_test.py`, `structural_source_coverage_test.py`, `structural_observation_work_test.py` |

## Focused verification

Run the helper's configured CTest target to exercise compilation planning, ordered claims, byte admission, unlocked encoding, ownership, source coverage, and conservative traversal. The observation-work fixture uses four real and four synthetic units; workers 1, 2, and 4 must preserve observations, including an owned include nested in an external namespace and an owned macro expanded inside an external function. Pass a third helper path to `structural_observation_work_test.py` to compare against a saved baseline.

Step-four verification on 2026-10-01 also compared an eight-TU fixture containing a 250,000-element excluded vendor array and 300 owned inline functions. The pre-change helper and current helper emitted byte-identical native observations and 1,608 canonical JSONL records; current worker counts 1, 2, and 4 preserved the canonical SHA-256. Three alternating two-worker samples gave these medians:

| Metric | Before | After |
| --- | ---: | ---: |
| Semantic traversal | 510.056 ms | 452.502 ms |
| Helper invocation wall time | 1.184 s | 1.210 s |
| Peak helper RSS | 220.434 MB | 220.549 MB |
| Constructed duplicate file results discarded | 7 | 3 |

This fixture demonstrates reduced observation construction and semantic traversal, without a total-wall or RSS improvement. Those step-four bounds did not cover Clang parsing, whole-AST parent-map construction, metadata, or allocator retention. These are small-fixture results; no additional full Codebase Memory scan was run, and its performance gate remains unverified.

### Grammar workload and parent-map scope

Step-five source inspection found 159 CBM grammar shims with 261 distinct directly included parser/scanner files totalling 1,250,621,120 bytes. Each direct file occurs in one shim. The generated grammars have conflicting static symbols and remain separate TUs. Shared runtime headers exist, but a common parser/scanner precompiled context cannot amortize these distinct inputs within a cold scan.

The bounded `scripts/c_family_grammar_probe.py` harness parses only JSON, C and Python grammar shims, both unchanged and with a small owned read/write function: six TUs total. Three alternating one-worker samples before/after limiting Clang's parent-map scope gave these medians:

| Metric | Before scope | After scope |
| --- | ---: | ---: |
| Helper invocation wall | 2.954 s | 2.146 s |
| Semantic traversal total | 761.740 ms | 122.611 ms |
| Peak helper RSS | 229.638 MB | 145.388 MB |
| RSS after final submission | 204.149 MB | 141.754 MB |

In the initial C probe, parent queries cost 457 ms and traversal added about 84 MB RSS despite retaining only about 6 KB of observations. This isolates a whole-AST parent-map cost on the fixture; it does not attribute CBM's complete 2.8 GB peak. AST parsing still allocates substantial memory, and teardown does not promise allocator pages return to the OS. A subsequent single-worker allocator sample for the C shim reported 61.99 MB of live heap chunks while parsed; after teardown, live chunks fell to 0.676 MB while free retained chunks rose to 62.05 MB and RSS remained 130.96 MB. This distinguishes freed allocator retention from live AST or queued observations on the fixture. It does not justify a production trimming policy by itself.

`scripts/c_family_pch_probe.py` compares source parsing with an explicit Clang PCH containing only guarded external context. It separately records PCH creation and checks semantic file payloads, context identities and diagnostics; compilation arguments intentionally differ. The external PCH regression requires a matching Clang executable and runs when CMake finds one. Loaded/PCH ASTs retain full traversal until nested ownership can be indexed safely. In the final C-parser PCH fixture, median warm wall was 0.584 s from source versus 0.581 s with PCH, while peak helper RSS increased from 141.160 MB to 190.398 MB. PCH creation added 0.747 s and produced a 10.289 MB artifact. Automatic PCH creation or dependency caching is not enabled: the cold creation cost is paid per distinct grammar, and safe cache reuse must preserve preprocessing observations, diagnostics, dependency/configuration invalidation and owned-source semantics.

Seven native CTest suites and 25 baseline fixture comparisons protect the change. The 64-record grammar fixture and 1,608-record observation fixture also preserve byte-identical canonical Rust output before/after and at workers 1, 2, and 4. These measurements use small fixtures and do not establish the Codebase Memory performance gate.

### Dependency pointer sources and heap reclamation

Pointer-binding assignments are emitted only when the pointer declaration belongs to the owned inventory. Dependency fields and variables remain available as compact context identities for accesses and indirect calls; they cannot become graph sources. Rust continues to reject malformed bindings whose sources lack materialized declarations. The native pointer-ownership fixture reproduces the former failure and protects the retained owned binding, dependency writes and indirect call.

On glibc 2.33 or newer, worker lanes request heap reclamation after compiler teardown and observation handoff. A nonblocking process-wide lock prevents simultaneous trims. Reclamation requires at least 64 MiB of free allocator chunks and at least 64 MiB resident growth since the last attempt. Other platforms retain their allocator behaviour. This releases freed pages rather than limiting live ASTs, queue results or process RSS. It does not change ownership, parse order or semantic output. The large-AST regression exercises reclamation and worker determinism; opt-in `c-family.clang.heap_reclaim` reports elapsed time and free heap before/after.

The first full CBM validation after step five took 118.560 seconds until Rust materialization failed, with 2.963 GB peak process-tree RSS. It planned 455 real and 38 synthetic sources and returned observations for all 623 files over 469 TUs. The rejected source was the dependency field `TSParseOptions::progress_callback`. No canonical fact stream was produced. Raw post-TU telemetry showed approximately 2.22 GB free allocator chunks with approximately 20 MB live chunks, establishing allocator retention on the full corpus. The 63-second/2.812-GB gate failed; complete semantic coverage was not verified.

The reclamation fixture's three alternating samples retained identical native observations. Median final submission RSS fell from 141.578 MB to 123.945 MB with the resident-growth guard, while peak RSS remained approximately 145.5 MB and wall remained approximately 1.75 seconds. Reclamation reduces retention rather than the live parsing peak.

Post-fix CBM integration at four workers completed in 117.357 seconds with 3.368 GB peak process-tree RSS, 469 TUs, all 623 files and 869,893 canonical facts. The pointer failure is resolved; the performance gate still fails. At a later post-TU sample, RSS was approximately 240 MB despite approximately 2.786 GB free virtual allocator chunks, demonstrating released resident pages. Four-lane peak live work still exceeds the memory budget. The corrected 461-command capture changes build contexts and therefore canonical output from the old 167-command baseline; file coverage matches, but the complete symbol/relationship delta has not been adjudicated. Nine native suites, the pointer fixture's canonical worker hashes and bounded grammar comparisons protect these fixes, without establishing full-corpus parity.

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

Make-based compilation-database preparation joins backslash-continued dry-run recipes before extracting compiler commands. Multi-source compiler/link recipes produce one syntax-only entry per source, preserving build include paths, defines, and language flags. Exact duplicate commands are removed while distinct commands within a selected target remain. Repeat `--fallback-target` to capture sources absent from earlier targets; the primary `--target` owns shared sources, followed by fallbacks in argument order. A failed or empty target aborts capture before the output database is replaced.

Codebase Memory preparation captures `cbm`, then `build/c/test-runner` and `build/c/test-repro-runner` as fallbacks. Production flags remain authoritative for shared production sources; test-only and repro-only sources receive their actual target's includes, test API defines, and sanitizer flags. This adds build context without excluding repository files or changing parse-unit planning. Focused capture regressions run with `python -m unittest discover -s scripts -p 'test_c_family_make_compdb*.py'`.

Each result stores cold and warm wall time, process-tree peak RSS, TU architecture counters, and the canonical facts hash. `--check-concurrency` adds worker-count 1, 2, and 4 runs and requires the same hash. Codebase Memory must complete without timeout, with cold wall at most 63 seconds and cold peak process-tree RSS at most 2.812 GB. Git calibration is a separate follow-up and is gated on that Codebase Memory result. Result records and build/cache data stay under `.hardcut-build` unless `--results-dir` is supplied.
