# C-family Phase 2 oracle freeze

Parent index: [Development Documentation](INDEX.md)

**Date:** 2026-09-28  
**Branch:** `refactor/c-family-clang`  
**Baseline commit:** `7dcbcfea122c1c92f00b96e4253792341b10fe4a`  
**Adapter:** native Rust C-family `0.5.0`  
**Purpose:** Phase 2.1 of the authoritative frontend re-port

## Purpose

Freeze the semantic and performance state that exists immediately before the C/C++ production path moves from Tree-sitter plus Lexicon-owned compiler reconstruction to Clang/LibTooling.

The freeze has two layers:

1. a checked-in mixed C/C++ canonical-facts oracle for exact local regression detection;
2. pinned real-repository baselines for semantic cardinality, call-site outcomes, macro evidence, wall time, and process-tree RSS.

The July 2026 Go/Tree-sitter calibration remains historical semantic evidence. It is not treated as the current byte oracle because the Rust-native port has since changed some output details.

## Baseline repair discovered during the freeze

The first Codebase Memory baseline run exposed a pre-existing Rust-port panic in `macro_declarations::is_include_guard`.

The include-guard preview sliced a UTF-8 `&str` at byte 2048 without first moving to a character boundary. The pinned CBM corpus contains a box-drawing `─` crossing that boundary, causing:

```text
end byte index 2048 is not a char boundary
```

Phase 2.1 repairs only that invalid bounded preview. The preview still considers at most 2048 bytes and now floors the limit to a valid UTF-8 boundary. A focused unit test protects the boundary. No parser, resolution, identity, fact-policy, or corpus calibration rule changed.

## Frozen local oracle

Fixture:

`lexicon/testdata/c_family_phase2_oracle/`

The fixture deliberately covers:

- C and C++ in one repository;
- repository-local includes;
- function declarations and definitions;
- C function-pointer dispatch;
- function-like macro mediation and argument substitution;
- C++ inheritance;
- overloaded free functions;
- typed member calls;
- reads, writes, and direct argument flow;
- `compile_commands.json` forcing a `.c` source through C++ mode.

Canonical output:

| Measurement | Frozen value |
| --- | ---: |
| SHA-256 | `c1c1540b83d8a12cfa21f5962b8a5a16c31b90009471f4beadd13616abae9fe3` |
| JSONL bytes | **58,280** |
| Lines including header | **176** |
| Nodes | **65** |
| Edges | **106** |
| Unresolved | **4** |

Key edge counts are **8 calls**, **2 possible-calls**, **2 includes**, **1 extends**, **1 macro reference**, **5 passes-to**, **22 reads**, and **7 writes**.

`lexicon/tests/c_family_phase2_oracle.rs` requires byte-for-byte canonical equality.

This golden is an adjudication oracle, not a demand that Clang reproduce known Tree-sitter mistakes. During the cutover, any intentional semantic difference must be identified and the golden updated only after that difference is accepted.

## Pinned corpus matrix

The reusable runner is:

`scripts/c_family_phase2_baseline.py`

The frozen result is:

`lexicon/evaluation/performance/c-family-phase2-baseline-2026-09-28.json`

Every case ran twice. Cold and warm canonical hashes were required to match before a result was accepted.

| Case | Revision | Facts | Cold wall | Warm wall | Peak RSS | Canonical SHA-256 |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| Git | `9a0c4701` | 714,152 | 20.391 s | 22.125 s | 1.590 GB | `f13eed82337ffd9e97c2030666882d8012752d3aeb03dc4066792543ba282d7f` |
| Codebase Memory | `97ce23f9` | 608,129 | 21.047 s | 26.143 s | 1.406 GB | `1d6fecf663c01438d7722d9df4c85fcad9afef7f5f068063b3aa77013ca3c3b7` |
| LevelDB | `99b3c03b` | 54,624 | 2.761 s | 1.699 s | 131.7 MB | `f40ab82569bea55b2fb4727ba64275dc36b97d6e62415f65f8f514e0f0cd6c4b` |
| fmt | `407c905e` | 107,092 | 5.178 s | 3.797 s | 256.9 MB | `b85b8d3889fa8a2ca14e8545bdededc4bd67bc0ab3df04bea86a171eb955c3b1` |
| Catch2 `src/` | `191fa38c` | 38,858 | 3.638 s | 1.144 s | 92.2 MB | `9bd8fe596706c1ea090b06ee0e3c1b66b58b413b029f9f581aee81c68b31dc65` |
| nlohmann/json `include/nlohmann/` | `55f93686` | 23,189 | 1.170 s | 0.613 s | 63.2 MB | `57eef3523889711f5f19ccbc59374db82c2e3e7850e7fcc5045b48e2932142c9` |

Warm time is recorded as observed evidence rather than a guaranteed improvement. Git and CBM were slower on the second pass on this machine; no architectural conclusion is drawn from one two-sample run.

## Semantic baseline

### Git

The current Rust adapter observes **88,512** call sites.

Resolved categories preserve the accepted July calibration counts:

- **64,456** definite-only;
- **1,858** possible-only;
- **3** with both definite and possible evidence.

The current corpus also contains **22,195** unresolved-only sites.

The output contains **6,549 macro-mediated call edges**, **24,923 macro-expansion reference edges**, and maximum observed expansion depth **3**.

### Codebase Memory

The current scan covers **623 C-family files / 21.10 MB** and emits:

- **39,637** definite-only call sites;
- **1,338** possible-only;
- **1,998** definite-plus-possible;
- **39,380** unresolved-only;
- **10,625** macro-mediated call edges;
- **6,508** macro-expansion reference edges.

The measured peak of approximately **1.41 GB** is the largest memory target in this Phase 2 corpus other than Git.

### C++ calibration / validation / holdout

LevelDB retains the July semantic relation cardinalities exactly for the relations previously reported: **4,855 calls**, **6,834 possible-calls**, **13 extends**, **489 includes**, **1,784 passes-to**, **13,255 reads**, **359 references**, and **3,224 writes**.

fmt, Catch2, and nlohmann/json have modest Rust-port-era cardinality differences from the July Go-adapter report. Those differences are now explicit baseline evidence rather than hidden drift. Phase 2 calibration must adjudicate semantic quality using the pinned cases and targeted judgments; it must not attempt to restore July bytes blindly.

## Performance interpretation

The pre-Clang baseline is dominated by the large C workloads:

- Git: approximately **20.4 s / 1.59 GB** cold;
- Codebase Memory: approximately **21.0 s / 1.41 GB** cold.

The medium C++ cases complete in approximately **1.2–5.2 s** cold and remain below **260 MB** peak process-tree RSS.

These measurements include the current Rust adapter process and are suitable as the Phase 2 before-state. Phase 2.7 must rerun the same script against the Clang-backed production path.

A richer Clang analysis may perform more compiler work. Acceptance is therefore based on semantic depth plus explained wall/RSS behaviour, not on requiring every Clang number to be numerically below Tree-sitter.

## Verification surface

Phase 2.1 is protected by:

- `lexicon/tests/c_family_phase2_oracle.rs` — exact canonical fixture output;
- `macro_declarations::tests::bounded_prefix_never_splits_utf8` — CBM crash regression;
- existing C-family integration tests under `lexicon/tests/c_family_*.rs`;
- `scripts/c_family_phase2_baseline.py` — deterministic pinned-corpus baseline;
- the permanent registered-host C-family performance fixture in `scripts/lexicon_perf_baselines.json`.

## Phase 2.1 gate

Phase 2.1 is complete when:

- the local canonical oracle is checked in and passes;
- all six pinned corpus revisions are confirmed;
- cold/warm hashes are deterministic per corpus;
- fact/cardinality/call/macro metrics are preserved in a machine-readable artifact;
- cold wall and peak RSS are recorded;
- the UTF-8 baseline blocker is repaired and tested;
- existing C-family tests remain green;
- the worktree is committed before Phase 2.2 begins.

## Next

Phase 2.2 establishes the private Clang/LibTooling helper and runtime boundary without cutting facts production over yet.
