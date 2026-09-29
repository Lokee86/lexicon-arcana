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

The calibration path also now records stage metrics and supports bounded parallel translation-unit execution. The default frontend worker ceiling is eight and can be overridden with `LEXICON_CLANG_JOBS`.

Final corpus gates run from writable container-local snapshots of the pinned repositories. This permits each project’s real compilation database and generated build headers to live beside the source without modifying the frozen host corpus. Repository revisions are preserved in each snapshot, and all semantic analysis still uses the same production `adapter_eval` plus the clean-built native helper.

For Makefile projects, `scripts/c_family_make_compdb.py` extracts compiler invocations from a forced dry run. It handles both ordinary one-source compile rules and Git-style shell presentation wrappers such as `echo ...; clang ...`. The resulting build-context inventories contain **566** Git commands and **167** Codebase Memory commands. CMake produces **39** LevelDB, **2** fmt library, and **107** Catch2 commands; discovered source/header files without an entry continue through the production synthetic-command policy.

## Current focused verification

The current C-family unit surface passes **19/19** tests, including cross-file macro-source identity, checkout-path relocation determinism, and full/incremental record-equivalence coverage. Host-side `cargo fmt --check`, Python byte-compilation of the calibration scripts, and `git diff --check` are green.

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
| facts | 54,624 | 54,160 | -464 |
| nodes | 9,201 | 9,470 | +269 |
| edges | 40,025 | 40,974 | +949 |
| unresolved | 5,398 | 3,716 | -1,682 |
| calls | 4,855 | 5,193 | +338 |
| possible-calls | 6,834 | 5,073 | -1,761 |
| unresolved calls | 5,022 | 3,409 | -1,613 |
| extends | 13 | 72 | +59 |
| overrides | 0 | 391 | +391 |
| passes-to | 1,784 | 2,761 | +977 |
| reads | 13,255 | 13,995 | +740 |
| writes | 3,224 | 3,513 | +289 |

### Adjudication

These changes are consistent with the ownership cut rather than evidence that the retired resolver should be restored:

- Clang contributes substantially more definite inheritance/override evidence.
- Definite calls increase while possible and unresolved calls fall.
- The old `ambiguous-target` reconstruction disappears; compiler-bound indirect calls instead preserve the Phase 2.5-required `dynamic-target` unresolved state alongside repository-bounded possible targets.
- Compiler-bound arguments and accesses produce more `passes-to`, `reads`, and `writes` edges.
- The old `references(role=macro-expansion)` graph reconstruction is intentionally retired. Phase 2.5 defines the replacement contract as `clang-macro-expansion` evidence on compiler-observed call edges. LevelDB contains **100** such Clang expansion call edges.

The resulting LevelDB canonical SHA-256 is `c650a79ecb23f6b1a65ba0c1e584be2bc6e8dfa863c782bfb8b4a13fb40fad7f`.

## LevelDB performance profile

The old cold baseline was **2.761 s**. With real compilation context, the current production adapter records:

| Stage | Time |
|---|---:|
| repository discovery | 0.008 s |
| helper startup | 0.011 s |
| compilation database load | 0.017 s |
| Clang frontend work | 54.324 s wall |
| aggregate semantic visitor CPU | 320.892 s |
| observation emission | 3.522 s |
| helper IPC total | 59.553 s |
| response decode | 0.288 s |
| Rust materialization | 0.427 s |
| Lexicon graph extensions | 0.223 s |
| canonicalization | 0.016 s |
| facts-v1 validation | 0.057 s |

The material regression is fully localized: Rust-side graph work is sub-second; Clang AST/Sema traversal dominates. The observation payload is approximately **21.7 MB** and the final facts output approximately **22.9 MB**, so serialization is secondary but visible.

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
| facts | 107,092 | 78,633 | -28,459 |
| nodes | 17,024 | 17,710 | +686 |
| edges | 79,319 | 51,211 | -28,108 |
| unresolved | 10,749 | 9,712 | -1,037 |
| call sites | 16,948 | 14,229 | -2,719 |
| calls | 7,115 | 5,586 | -1,529 |
| possible-calls | 19,782 | 292 | -19,490 |
| unresolved calls | 10,023 | 8,988 | -1,035 |
| reads | 17,887 | 19,143 | +1,256 |
| writes | 4,174 | 4,512 | +338 |

The dominant semantic shift is the removal of heuristic candidate fanout. `ambiguous-target` falls from **3,936** to zero while `dynamic-target` becomes **5,536** explicit unresolved sites. Legacy macro-reference reconstruction also disappears; **680** call edges carry `clang-macro-expansion` provenance. This is consistent with the Phase 2.4/2.5 ownership policy: compiler evidence is retained, while guessed repository candidates are not recreated merely for parity.

Cold wall time is **100.590 s** versus **5.178 s** before; warm wall is **87.704 s** versus **3.797 s**. Peak process-tree RSS is **3.095 GB** cold and **3.032 GB** warm, versus about **257 MB** before. The cold stage profile localizes the cost to the Clang frontend: **90.997 s** frontend work / **539.358 s** aggregate visitor CPU, versus **0.704 s** Rust materialization, **0.376 s** graph extension, **0.015 s** canonicalization, and **0.060 s** validation. The canonical fact hash is `2ac08dbcdece0bdc0251e3dde62ffe94a5eb4d535cd9983d4936cacc0935ef54`.

## nlohmann/json semantic calibration

Pinned revision: `55f93686c01528224f448c19128836e7df245f72`.

The production adapter now completes the pinned header-only corpus after the lambda-source identity repair. Orphan-header batching reduced 46 requested headers to six direct synthesized translation units.

| Metric | Phase 2.1 Tree-sitter | Phase 2.7 Clang | Delta |
|---|---:|---:|---:|
| facts | 23,189 | 20,216 | -2,973 |
| nodes | 3,485 | 3,690 | +205 |
| edges | 17,234 | 12,700 | -4,534 |
| unresolved | 2,470 | 3,826 | +1,356 |
| call sites | 4,076 | 4,248 | +172 |
| calls | 1,876 | 686 | -1,190 |
| possible-calls | 3,556 | 56 | -3,500 |
| unresolved calls | 2,200 | 3,571 | +1,371 |
| passes-to | 504 | 330 | -174 |
| reads | 5,641 | 6,349 | +708 |
| writes | 841 | 1,047 | +206 |

The call-site count increases slightly even though edge counts fall. The old resolver expanded uncertain template/dependent calls into large repository-local candidate sets; Clang instead leaves those dependent/dynamic sites unresolved until it has a compiler-proven target. The possible-target p90 therefore falls from **7** to **1**, and `dynamic-target` becomes the dominant explicit unresolved reason. This is an uncertainty-policy shift from guessed fanout to compiler-bounded evidence, not a missing traversal: the adapter observes **4,248** call sites versus **4,076** before.

Legacy macro-reference reconstruction is again absent; **181** call edges carry `clang-macro-expansion` provenance instead.

The current canonical SHA-256 is `0bb5f82a4727089f26b71a53f22e7706de2cb2b3eb1f9680908e3cda6a27c3d5`.

### nlohmann/json performance profile

| Stage | Time |
|---|---:|
| repository discovery | 0.035 s |
| helper startup | 0.011 s |
| compilation database load | 0.009 s |
| Clang frontend work | 135.606 s wall |
| aggregate semantic visitor CPU | 293.666 s |
| observation emission | 1.010 s |
| helper IPC total | 137.709 s |
| response decode | 0.089 s |
| Rust materialization | 0.889 s |
| Lexicon graph extensions | 0.200 s |
| canonicalization | 0.006 s |
| facts-v1 validation | 0.017 s |

As with LevelDB, the material cost is the compiler frontend rather than Rust graph construction.

## Reduced correctness reproductions

Two native reduced cases protect the interpretation of real-corpus failures during calibration:

- a templated lambda body now attributes inner calls and reads to its nearest source-written enclosing function rather than to Clang's implicit lambda `operator()`;
- a token-pasted function-template declaration such as `CmpHelper##NE` now gets repository ownership from its macro expansion site when its name's spelling location is Clang `<scratch space>`.

Both cases produce matching declaration/source compiler IDs through the helper; the lambda case also materializes end-to-end through the production Rust adapter without a missing-source error.

## Remaining matrix

The remaining final cold/warm gates are:

- Git `9a0c4701dcd5725c4184599322b52933ff5005ca`;
- Codebase Memory `97ce23f9827177fff3858831156e9795c6832b18`;
- LevelDB `99b3c03b3284f5886f9ef9a4ef703d57373e61be` (final deterministic rerun; diagnostic calibration above is already complete);
- Catch2 `191fa38c9b1596cd2576ab531d4ab4d5e8e05190`;
- nlohmann/json `55f93686c01528224f448c19128836e7df245f72` (final deterministic rerun; diagnostic calibration above is already complete).

The final gate records each case's semantic summary, deterministic hash, cold/warm wall, peak RSS, and any required adjudication before Phase 2.8 begins.

## Whole-system acceptance evidence

The non-corpus Phase 2.7 gates are also exercised:

- C-family focused unit surface: **19/19**;
- Doctor integration: **7/7**;
- scan engine full/no-op/incremental transaction coverage: **2/2**;
- scan execution publication/ownership coverage: **3/3**;
- root workflow smoke, including helper packaging/version verification: **9/9**;
- documentation policy validation: **204 Markdown files**;
- real installed-tree helper execution without a source checkout: **9 canonical facts** from a package-local helper;
- real native relocation check: byte-identical facts from the same checkout name under two different absolute parent paths;
- multi-language matrix: all non-C-family frozen hashes remain exact; the C-family fixture remains **8 facts / 4 nodes / 4 edges / 0 unresolved** and its expected hash is updated to the adjudicated Clang-backed representation.

The C-family multilang fixture completed in **243 ms**, below its **5 s** ceiling. No other language baseline changed.
