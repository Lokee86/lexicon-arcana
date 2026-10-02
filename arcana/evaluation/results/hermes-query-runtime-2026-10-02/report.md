# Hermes query runtime repair evidence

## Purpose

Record the immutable-snapshot baseline and query/runtime repair verification performed on 2026-10-02 UTC (2026-10-01 local time). These are measurements under the recorded conditions, not general latency guarantees.

## Fixture and method

The existing binary-format Hermes generation is `sha256:ee46a8a475c443f908e0eff6db9efca62e3871fe1f541e62251ebd9f5ed2c621`: 1,125,126 nodes, 2,367,423 visible edges, and 699,349 unresolved references. No Lexicon scan or Arcana rebuild was performed. The executable was rebuilt in release mode before preserving the pre-cutover baseline. Python subprocess JSONL measurements used psutil sampling at 5–10 ms. Filesystem cache was warm/uncontrolled; each cold-start measurement used a new process. Windows host load was uncontrolled. Toolchain: rustc 1.97.1; Intel Core i9-11900H; 15.77 GiB physical memory. Baseline source: `32322191259da0d8e7060d4b8ca4b71016ee82ef`; Workspace base: `7e7935906da7b8b1c31d2b512c8ed455438055d8`.

## Results

| Measurement | Pre-cutover | Repaired query path |
| --- | ---: | ---: |
| Fresh process capabilities, warm filesystem | 9.74 s | 1.39 s |
| Statistics after capabilities | 0.38 s | 0.35 s |
| Basic three-query process peak RSS | 1,902,448,640 bytes | 71,495,680 bytes |
| Exact lookup p95, 100 repeated nonempty lookups | Not measured | 0.000233 s |
| Full-text search, `run_agent` | Not measured | 0.57 s |
| Broad substring search, `a` | Not measured | 0.54 s |
| Unfiltered unresolved query, limit 10 | Not measured | 0.13 s |
| Scoped architecture summary, `hermes_cli` | Not measured | 2.00 s |
| Same-generation diff including comparison open | Not measured | 1.25 s |
| Whole representative suite peak RSS | Not measured | 141,590,528 bytes |

All representative requests completed within the unchanged 30-second query deadline. A separate real-process Workspace manager probe served 20 concurrent requests with one child and one handshake in 1.55 seconds including startup, then left zero sessions after shutdown.

## Correctness and ownership

The full Arcana and Workspace test suites cover the existing protocol contract and the new query/session owners. A manual exact-JSON parity gate compares every operation against the pre-cutover executable, including duplicate facts, zero limits, pagination, unknown nodes, invalid kinds, and two-generation differences. Source audits verify that interactive queries cannot call the rich snapshot reader, materialize all facts, compile the repository, or rebuild a global catalogue/unresolved map. Explicit rich-data reconstruction/audit remains available.

The first cutover experiment exposed a 107-second statistics scan despite a 1.45-second startup and 76 MB RSS. That experiment was rejected. The final statistics path scans compact kind/reason codes, while text search ranks compact records and decodes only retained responses. The raw first-cutover result is retained separately so the failed attempt is not confused with final acceptance.

## Verification limits

True cold filesystem-cache measurements were not performed: no system cache purge or machine-wide memory pressure was introduced. Changed-generation large-snapshot diff has correctness fixture coverage but no large-repository latency measurement here. Strict Clippy on the installed Rust 1.97 toolchain reports inherited lint failures in untouched build/rewrite and Lexicon test modules; the runtime changes introduce none of the reported failures. Baseline stage-level materialization/compiler timings were not separately instrumented; the baseline captures total startup and RSS. The repaired owner supports opt-in `ARCANA_QUERY_TRACE` stage diagnostics.

The active Hermes `.arcana` directory still contains an obsolete v1 repository manifest. It was not silently replaced with another generation. Workspace needs a service reload to load the new manager; this evidence exercises the new code directly rather than claiming the already-running MCP process was hot-reloaded.

## Raw artifacts

- [Pre-cutover baseline](baseline.json)
- [Initial rejected statistics experiment](initial-cutover.json)
- [Optimized statistics sample](optimized-stats.json)
- [Initial optimized query suite](query-suite.json)
- [Final query suite with stage diagnostics](final-query-suite.json)
- [Real Workspace session probe](workspace-session-probe.json)

## Related documentation

- [Repository snapshots](../../../docs/repository-snapshots.md)
- [Query runtime verification](../../../docs/DEVELOPMENT.md)

## Verification closeout

- Arcana: 216 library tests and 20 CLI tests passed; 8 manual benchmarks/parity tests are ignored by default. The exact-JSON parity test was explicitly run and passed.
- Workspace: 141 tests passed, 2 environment-dependent tests skipped.
- Release build and formatting check passed.
- Arcana and Lexicon documentation profiles passed; local-link validation passed for 212 Markdown files.
- Shared-root documentation policy retains 24 inherited missing-section findings in untouched documents.
- Strict Clippy retains 13 inherited lint findings in untouched modules on this toolchain; the source diff confirms those files were not modified.
- Final owner diagnostics measured approximately 1 ms manifest validation, 179 ms graph open, 1,140 ms store integrity validation, and 1,361 ms total startup with zero fact materializations/compiler invocations.
- Source audit finds no rich snapshot restoration, catalogue owner, global unresolved source map, fact materialization, or repository compilation in production protocol modules. Workspace's query tool contains no per-request spawn or stdin EOF launch path.

Documentation impact:
- Inspected: repository snapshot, protocol, integration, process lifecycle, coverage, behavioral matrix, and maintainer guidance.
- Updated: Arcana snapshot/architecture/development guidance; Workspace integration/execution/maintainer guidance; both coverage and behavioral matrices; retained performance evidence.
- Not affected: Lexicon adapters, snapshot format, semantic relationships, authentication, and unrelated rich-data consumers.
- Compliance check: component profiles and link validation passed; shared-root policy remains blocked by inherited findings.
- Known documentation gaps: the 24 inherited shared-root policy findings; no changes here claim to close them.
