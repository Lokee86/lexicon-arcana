# C-family Phase 2.7 calibration and performance gate

Parent index: [Development Documentation](INDEX.md)

**Date:** 2026-09-29

**Branch:** `refactor/c-family-clang`

**Starting checkpoint:** `610f840` — `Cut C-family production over to Clang`

**Status:** calibration in progress; evidence below is durable and the remaining pinned-corpus matrix is appended as each case completes.

## Purpose

Phase 2.7 calibrates the Clang-backed production C/C++ adapter against the frozen Phase 2.1 oracle. It does not restore Tree-sitter approximations merely to preserve old counts, and it does not begin Phase 2.8 deletion work.

The gate requires:

- the six pinned C/C++ corpus revisions from Phase 2.1;
- deterministic canonical facts;
- semantic-delta adjudication rather than byte parity;
- frontend, IPC, Rust materialization, graph-extension, canonicalization, wall-time, and process-tree RSS evidence;
- no unexplained material performance regression.

## Reproducible native toolchain

The Windows host does not provide LLVM/Clang development packages, so native acceptance is run in the repo-defined calibration image:

- Rust **1.90.0**;
- Clang/LLVM **18.1.3**;
- CMake **3.28.3**;
- Python psutil **5.9.8**;
- Ubuntu 24.04 runtime with pinned LLVM/Clang development packages.

The image definition lives at `scripts/c_family_phase2_calibration.Dockerfile`. This keeps the helper build isolated from host packages while exercising the real LibTooling implementation.

## Calibration-exposed runtime fixes

The first native Clang 18 build exposed several issues that synthetic protocol fixtures could not:

- the helper CMake project must enable both C and C++ because LLVM's exported CMake configuration performs C feature probes;
- Clang 18 removed the historical `bool ModuleImported` parameter from `PPCallbacks::InclusionDirective`;
- LLVM 18 requires explicit `llvm::json::Object{...}` construction for returned JSON objects;
- declaration code must include `ASTContext.h` directly rather than rely on a transitive include;
- compiler IDs for template instantiations must collapse to their template-instantiation pattern;
- macro-spanned source ranges need expansion-location fallback when spelling endpoints are cross-file or reversed;
- compilation-database lookup must handle absolute, repository-relative, and command-directory-relative filenames;
- materialization must resolve compiler-owned source identities across files for macro-spanned calls, relationships, and accesses;
- implicit lambda call operators are not Lexicon source-written callable nodes, so semantic observations inside a lambda must climb to the nearest materialized enclosing function;
- token-pasted function names may live in Clang `<scratch space>` even though the declaration is source-written, so source ownership falls back from spelling to expansion location when spelling has no repository owner;
- call/access emission now enforces a stronger invariant: if Clang emits semantic evidence from a source-written callable, that callable declaration is explicitly materialized before the semantic observation is recorded. This covers instantiated and macro-generated template bodies that RecursiveASTVisitor does not always expose through the same declaration visitation path;
- macro-derived declaration ranges can have a valid source-written start while Clang supplies no usable end position. The helper now preserves the valid anchor and collapses only that invalid end to the start instead of publishing a zero-coordinate span. The fmt corpus exposed exactly two such fields in `test/gtest-extra.h`.
- compiler-observed function-pointer assignments can target fields declared outside the repository (for example `sigaction.sa_handler`). Those observations are valid compiler evidence but cannot become repository-owned pointer nodes, so Rust excludes external pointer bindings from the repository pointer index instead of treating them as missing materialization.
- whole-repository compiler responses can exceed hundreds of MiB. The frontend runner now spools bounded helper stdout to a temporary file, waits for the helper to exit and release Clang memory, then streams JSON decoding from disk. This avoids retaining the helper's AST/Sema working set and the full serialized frame in memory at the same time; the hard response ceiling remains bounded at 1 GiB.
- Git then exposed a second capacity issue before serialization: one helper process retained repository-wide Clang observations across hundreds of translation units. The production `ClangFrontend` now bounds helper lifetime instead: source files are sent in deterministic batches of at most **128 files**, each helper exits before the next batch, exact duplicate observations are merged in Rust, and requested headers are analyzed afterward only when no source batch already observed them, preserving the existing shallowest-orphan-header policy. The native worker ceiling remains eight.

The calibration path also now records stage metrics and supports bounded parallel translation-unit execution. The default frontend worker ceiling is eight and can be overridden with `LEXICON_CLANG_JOBS`.

Final corpus gates run from writable container-local snapshots of the pinned repositories. This permits each project’s real compilation database and generated build headers to live beside the source without modifying the frozen host corpus. Repository revisions are preserved in each snapshot, and all semantic analysis still uses the same production `adapter_eval` plus the clean-built native helper.

For Makefile projects, `scripts/c_family_make_compdb.py` extracts compiler invocations from a forced dry run. It handles both ordinary one-source compile rules and Git-style shell presentation wrappers such as `echo ...; clang ...`. The resulting build-context inventories contain **566** Git commands and **167** Codebase Memory commands. CMake produces **39** LevelDB, **2** fmt library, and **107** Catch2 commands; discovered source/header files without an entry continue through the production synthetic-command policy.

## Current focused verification

The current C-family unit surface passes **22/22** tests, including cross-file macro-source identity, checkout-path relocation determinism, full/incremental record equivalence, external function-pointer handling, and deterministic duplicate call-edge merging. The generic frontend-runner surface passes **5/5** tests, including preservation of first-frame protocol semantics on the spooled response path. Host-side formatting and `git diff --check` are green.

A clean Clang 18 rebuild also passes two native end-to-end source-identity regressions:

- a call inside a lambda nested in a source-written function is attributed to the enclosing materialized callable rather than an implicit lambda `operator()`;
- a macro-generated function template materializes its callable source identity before emitting semantic call/access observations.

The helper now enforces this invariant directly: semantic call/access/value-flow source emission first ensures the source-written callable declaration exists in the observation state. This removes dependence on incidental `RecursiveASTVisitor` declaration visitation order for template instantiations and macro-generated bodies.

## Git no-build-context performance finding

Running the unchanged Phase 2.1 calibration runner against the pinned Git corpus with no compilation database hit the **600 s** per-case cap.

- frozen Phase 2.1 cold wall: **20.391 s**;
- Clang-backed run: **>600 s**;
- repository discovery: **0.712 s**;
- process-tree peak observed before timeout: **258,396,160 bytes**.

The regression is therefore not repository discovery or Rust fact materialization. It is inside compiler frontend work under synthetic build context. This timeout is retained as real performance evidence; semantic calibration for projects that expose build context uses their actual compilation database rather than treating synthetic commands as authoritative build information.

## LevelDB semantic calibration

Pinned revision: `99b3c03b3284f5886f9ef9a4ef703d57373e61be`.

The diagnostic calibration used LevelDB's CMake-generated `compile_commands.json`, the production adapter, the native Clang helper, and eight frontend workers.

| Metric | Phase 2.1 Tree-sitter | Phase 2.7 Clang | Delta |
|---|---:|---:|---:|
| facts | 54,624 | 54,358 | -266 |
| nodes | 9,201 | 9,470 | +269 |
| edges | 40,025 | 40,987 | +962 |
| unresolved | 5,398 | 3,901 | -1,497 |
| calls | 4,855 | 5,195 | +340 |
| possible-calls | 6,834 | 5,073 | -1,761 |
| unresolved calls | 5,022 | 3,594 | -1,428 |
| extends | 13 | 72 | +59 |
| overrides | 0 | 391 | +391 |
| passes-to | 1,784 | 2,763 | +979 |
| reads | 13,255 | 14,017 | +762 |
| writes | 3,224 | 3,500 | +276 |

### Adjudication

These changes are consistent with the ownership cut rather than evidence that the retired resolver should be restored:

- Clang contributes substantially more definite inheritance/override evidence.
- Definite calls increase while possible and unresolved calls fall.
- The old `ambiguous-target` reconstruction disappears; compiler-bound indirect calls instead preserve the Phase 2.5-required `dynamic-target` unresolved state alongside repository-bounded possible targets.
- Compiler-bound arguments and accesses produce more `passes-to`, `reads`, and `writes` edges.
- The old `references(role=macro-expansion)` graph reconstruction is intentionally retired. Phase 2.5 defines the replacement contract as `clang-macro-expansion` evidence on compiler-observed call edges. LevelDB contains **100** such Clang expansion call edges.

The final deterministic LevelDB canonical SHA-256 is `f52a4ff5c291f3ac6fb6b9f7213d9221453175f7ac352673dd6ded766dd16dda`. Cold and warm runs produced byte-identical canonical facts.

## LevelDB performance profile

The final cold run is **97.837 s** versus **2.761 s** before; the final warm run is **93.075 s** versus **1.699 s**. Peak process-tree RSS is **1.529 GB** cold and **1.533 GB** warm, versus about **132 MB** and **126 MB** before.

The final cold run records:

| Stage | Time |
|---|---:|
| repository discovery | 0.011 s |
| helper startup | 0.000 s |
| compilation database load | 0.002 s |
| Clang frontend work | 92.541 s wall |
| aggregate semantic visitor CPU | 616.908 s |
| observation emission | 2.841 s |
| helper IPC total | 96.218 s |
| response decode | 0.184 s |
| Rust materialization | 0.307 s |
| Lexicon graph extensions | 0.213 s |
| canonicalization | 0.010 s |
| facts-v1 validation | 0.047 s |

The material regression is fully localized: Rust-side graph work remains sub-second while Clang AST/Sema traversal dominates. The helper response is approximately **21.9 MB** and the final facts output approximately **23.0 MB**, so serialization is secondary but visible.

This is a measured compiler-semantic cost, not an unexplained adapter regression. Phase 2.7 added bounded compiler-native concurrency, but deeper frontend performance work remains Phase 11 scope unless later corpus evidence reveals a correctness-coupled optimization.

## fmt correctness canary

After the callable-source invariant fix, the pinned fmt corpus reaches full validation successfully under a clean Clang 18 helper build:

- **70** discovered files;
- **48** translation units;
- **78,634** canonical facts;
- Clang frontend work: **107.840 s** wall;
- aggregate semantic visitor CPU: **594.908 s**;
- helper IPC: **127.342 s**;
- Rust materialization: **1.134 s**;
- graph extensions: **0.290 s**;
- canonicalization: **0.031 s**;
- validation: **0.102 s**.

The earlier `CmpHelperNE` source-identity failure is therefore closed. The canary's final JSONL write was deliberately excluded from acceptance because it targeted the Windows bind mount directly; the real calibration runner writes facts to container-local temporary storage and only persists the compact summary.

## fmt final gate

Pinned revision: `407c905e45ad75fc29bf0f9bb7c5c2fd3475976f`.

The repaired production path completes both cold and warm runs with identical canonical facts.

| Metric | Phase 2.1 Tree-sitter | Phase 2.7 Clang | Delta |
|---|---:|---:|---:|
| facts | 107,092 | 78,636 | -28,456 |
| nodes | 17,024 | 17,710 | +686 |
| edges | 79,319 | 51,214 | -28,105 |
| unresolved | 10,749 | 9,712 | -1,037 |
| call sites | 16,948 | 14,229 | -2,719 |
| calls | 7,115 | 5,586 | -1,529 |
| possible-calls | 19,782 | 293 | -19,489 |
| unresolved calls | 10,023 | 8,988 | -1,035 |
| reads | 17,887 | 19,143 | +1,256 |
| writes | 4,174 | 4,512 | +338 |

The dominant semantic shift is the removal of heuristic candidate fanout. `ambiguous-target` falls from **3,936** to zero while `dynamic-target` becomes **5,536** explicit unresolved sites. Legacy macro-reference reconstruction also disappears; **680** call edges carry `clang-macro-expansion` provenance. This is consistent with the Phase 2.4/2.5 ownership policy: compiler evidence is retained, while guessed repository candidates are not recreated merely for parity.

Cold wall time is **140.118 s** versus **5.178 s** before; warm wall is **104.925 s** versus **3.797 s**. Peak process-tree RSS is **3.037 GB** cold and **3.035 GB** warm, versus about **257 MB** before. The cold stage profile localizes the cost to the Clang frontend and response path: **96.857 s** frontend work / **538.018 s** aggregate visitor CPU, **22.733 s** observation emission, and **131.096 s** helper IPC, versus **3.955 s** Rust materialization, **0.712 s** graph extension, **0.013 s** canonicalization, and **0.073 s** validation. The final deterministic canonical fact hash is `545a99316b8cb50e096c626560ec3ec280bd7ebe960fca8cbbc681c74d03dc0d`.

## nlohmann/json semantic calibration

Pinned revision: `55f93686c01528224f448c19128836e7df245f72`.

The production adapter now completes the pinned header-only corpus after the lambda-source identity repair. Orphan-header batching reduced 46 requested headers to six direct synthesized translation units.

| Metric | Phase 2.1 Tree-sitter | Phase 2.7 Clang | Delta |
|---|---:|---:|---:|
| facts | 23,189 | 20,391 | -2,798 |
| nodes | 3,485 | 3,742 | +257 |
| edges | 17,234 | 12,752 | -4,482 |
| unresolved | 2,470 | 3,897 | +1,427 |
| call sites | 4,076 | 4,317 | +241 |
| calls | 1,876 | 686 | -1,190 |
| possible-calls | 3,556 | 56 | -3,500 |
| unresolved calls | 2,200 | 3,640 | +1,440 |
| passes-to | 504 | 330 | -174 |
| reads | 5,641 | 6,349 | +708 |
| writes | 841 | 1,047 | +206 |

The call-site count increases even though edge counts fall. The old resolver expanded uncertain template/dependent calls into large repository-local candidate sets; Clang instead leaves those dependent/dynamic sites unresolved until it has a compiler-proven target. The possible-target p90 therefore falls from **7** to **1**, and `dynamic-target` becomes the dominant explicit unresolved reason. This is an uncertainty-policy shift from guessed fanout to compiler-bounded evidence, not a missing traversal: the final adapter observes **4,317** call sites versus **4,076** before.

Legacy macro-reference reconstruction is again absent; **181** call edges carry `clang-macro-expansion` provenance instead.

The final deterministic canonical SHA-256 is `29c1214e0ebe01b22ff6938e48e71b7b90b3bc427da79c066f2718ccb501f0c2`.

### nlohmann/json performance profile

The final cold run is **22.990 s** versus **1.170 s** before; the final warm run is **18.043 s** versus **0.613 s**. Peak process-tree RSS is **570.7 MB** cold and **568.6 MB** warm, versus about **63.2 MB** and **61.6 MB** before.

| Stage | Time |
|---|---:|
| repository discovery | 0.012 s |
| helper startup | 0.025 s |
| compilation database load | 0.002 s |
| Clang frontend work | 21.260 s wall |
| aggregate semantic visitor CPU | 50.611 s |
| observation emission | 0.675 s |
| helper IPC total | 22.538 s |
| response decode | 0.069 s |
| Rust materialization | 0.060 s |
| Lexicon graph extensions | 0.017 s |
| canonicalization | 0.005 s |
| facts-v1 validation | 0.023 s |

Orphan-header batching is material here: the earlier diagnostic profile took **135.606 s** in frontend work, while the final batched run takes **21.260 s**. Rust graph construction remains negligible relative to compiler frontend work.

## Catch2 final gate

Pinned revision: `191fa38c9b1596cd2576ab531d4ab4d5e8e05190`.

Catch2 exposed two calibration-only correctness issues before its final gate: an external `sigaction.sa_handler` function-pointer binding that cannot own a repository node, and duplicate header call observations whose contextual metadata varied across translation units. External pointer bindings are now excluded from the repository pointer index. Duplicate call evidence is canonicalized twice at the ownership boundary: the helper uses the compiler-qualified receiver type and deterministic call preference, while Rust merges final duplicate call-edge metadata rather than accepting arrival order.

The final cold/warm run is deterministic.

| Metric | Phase 2.1 Tree-sitter | Phase 2.7 Clang | Delta |
|---|---:|---:|---:|
| facts | 38,858 | 42,366 | +3,508 |
| nodes | 9,848 | 10,092 | +244 |
| edges | 26,325 | 28,575 | +2,250 |
| unresolved | 2,685 | 3,699 | +1,014 |
| call sites | 4,391 | 7,071 | +2,680 |
| calls | 2,191 | 3,889 | +1,698 |
| possible-calls | 3,340 | 1,271 | -2,069 |
| unresolved calls | 2,200 | 3,263 | +1,063 |
| extends | 87 | 91 | +4 |
| overrides | 0 | 569 | +569 |
| passes-to | 517 | 1,178 | +661 |
| reads | 7,399 | 8,921 | +1,522 |
| writes | 1,360 | 1,484 | +124 |

### Adjudication

The semantic delta is consistent with the authoritative-Clang contract:

- compiler-selected definite calls increase substantially while heuristic possible-call fanout falls;
- compiler-native override evidence adds **569** repository override edges;
- unresolved calls increase because dynamic, external, and missing targets remain explicit instead of being expanded into guessed local candidates;
- compiler-bound arguments and accesses increase `passes-to`, `reads`, and `writes`;
- the old macro-reference reconstruction is retired; **384** call edges instead carry `clang-macro-expansion` evidence.

A dedicated regression now proves duplicate canonical call edges produce the same merged payload regardless of observation order. The final deterministic canonical SHA-256 is `471b52c24dab6e5d9042be758223b3b662c736288683c0e276f84dcceca1cfc5`.

### Catch2 performance profile

Cold wall time is **150.694 s** versus **3.638 s** before; warm wall is **159.792 s** versus **1.144 s**. Peak process-tree RSS is **1.255 GB** cold and **1.285 GB** warm, versus about **92.2 MB** and **93.3 MB** before.

The final cold run records:

| Stage | Time |
|---|---:|
| repository discovery | 0.004 s |
| helper startup | 0.005 s |
| compilation database load | 0.007 s |
| Clang frontend work | 147.359 s wall |
| aggregate semantic visitor CPU | 969.987 s |
| observation emission | 0.902 s |
| helper IPC total | 149.730 s |
| response decode | 0.169 s |
| Rust materialization | 0.257 s |
| Lexicon graph extensions | 0.050 s |
| canonicalization | 0.004 s |
| facts-v1 validation | 0.024 s |

As with the other compiled corpora, the material cost is concentrated in Clang AST/Sema work. Rust materialization, graph extension, canonicalization, and validation remain small relative to frontend time.

## Reduced correctness reproductions

Two native reduced cases protect the interpretation of real-corpus failures during calibration:

- a templated lambda body now attributes inner calls and reads to its nearest source-written enclosing function rather than to Clang's implicit lambda `operator()`;
- a token-pasted function-template declaration such as `CmpHelper##NE` now gets repository ownership from its macro expansion site when its name's spelling location is Clang `<scratch space>`.

Both cases produce matching declaration/source compiler IDs through the helper; the lambda case also materializes end-to-end through the production Rust adapter without a missing-source error.

## Remaining matrix

The remaining final cold/warm gates are:

- Git `9a0c4701dcd5725c4184599322b52933ff5005ca`;
- Codebase Memory `97ce23f9827177fff3858831156e9795c6832b18`.

LevelDB, fmt, Catch2, and nlohmann/json have completed deterministic cold/warm gates and are recorded above.

The final gate records each case's semantic summary, deterministic hash, cold/warm wall, peak RSS, and any required adjudication before Phase 2.8 begins.

## Whole-system acceptance evidence

The non-corpus Phase 2.7 gates are also exercised:

- C-family focused unit surface: **22/22**;
- shared frontend runner / Go adapter regression surface: **38/38** Go tests plus **5/5** generic frontend-runner tests;
- Doctor integration: **7/7**;
- scan engine full/no-op/incremental transaction coverage: **2/2**;
- scan execution publication/ownership coverage: **3/3**;
- root workflow smoke, including helper packaging/version verification: **9/9**;
- documentation policy validation: **204 Markdown files**;
- real installed-tree helper execution without a source checkout: **9 canonical facts** from a package-local helper;
- real native relocation check: byte-identical facts from the same checkout name under two different absolute parent paths;
- multi-language matrix: all non-C-family frozen hashes remain exact; the C-family fixture remains **8 facts / 4 nodes / 4 edges / 0 unresolved** and its expected hash is updated to the adjudicated Clang-backed representation.

The C-family multilang fixture completed in **243 ms**, below its **5 s** ceiling. No other language baseline changed.
