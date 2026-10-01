# Lexicon Incremental Scan Phase 1 — Git-backed Source Mirroring

Parent index: [Development Documentation](INDEX.md)

## Purpose

Restore inexpensive and correct source change detection on ordinary Git-backed working trees, without changing Lexicon analysis scope, semantic facts, snapshot publication or Arcana ingestion.

## Overview

The Phase 0 mirror fast path tried to hash every source file through a separate Git subprocess, failed on the controlled Windows fixtures, then byte-compared the whole tree. It also abandoned every skip when any private-mirror file was dirty.

The replacement reads Git metadata, not every file body. The prior **published private Git HEAD tree** is the generation-bound authoritative baseline; the source Git HEAD tree supplies candidate content object IDs. A file is eligible for skipping only when both blob IDs match, both source and private worktrees report it clean, the source index entry is ordinary (not assume-unchanged or skip-worktree), working-tree and indexed line endings match, no checkout-transforming filter/ident/encoding attributes apply, and the destination is still a regular file.

Everything else receives the existing byte-exact `copy_one` fallback. There is no separate mutable baseline, whole-repository `hash-object` call, file timestamp cache, compatibility planner, or hidden source content trust.

## Implementation

- `repository/mirror_index.rs` obtains the published private HEAD tree, private changed-path set, source Git root/prefix, source HEAD tree and source changed-path set. It compares immutable blob IDs only for relevant desired paths and rejects dirty, absent or ambiguous paths independently. New HEADs, branch switches, staged/unstaged edits and source-tree renames need no separately maintained epoch metadata; the private HEAD already records actual last-published mirrored content.
- `repository/mirror_index_worktree.rs` excludes Git's assume-unchanged and skip-worktree flags, CRLF/other EOL conversions, and `filter`, `ident` and `working-tree-encoding` checkout transformations. Attribute input is streamed with a concurrent writer to avoid blocking on full stdout pipes for large repositories.
- `mirror.rs` retains the existing deterministic inventory, verified candidate comparison, removal of now-missing/ignored files and opt-in metrics. `scan.source_index_metadata` reports eligible paths and dirty-path counts; `scan.mirror_index_fallback` emits numeric reasons for unavailable Git proof.
- Without source Git, a published private commit, or trustworthy Git metadata, synchronization continues to compare source bytes against the mirror; it never silently assumes a file is unchanged.

The content-ID proof does not use source HEAD alone. A file previously scanned from a dirty source has different content in the private HEAD, so reverting it, modifying it repeatedly, committing a different source HEAD or interrupting a scan does not invalidate the comparison.

## Controlled production-path evidence

The same development-profile Rust CLI ran on fresh disposable synthetic Python checkouts. Git-backed source trees were explicitly enabled in the benchmark harness; this is not a controlled wall-time A/B comparison with the Phase 0 non-Git fixture, so the primary performance gate is *work cardinality*, not elapsed-time percentage.

| Scenario | Source files | Indexed skips | Mirror source files compared/copied | Total wall |
| --- | ---: | ---: | ---: | ---: |
| Git-backed unchanged | 1,001 | 1,001 | 0 | 1.172 s |
| First one-file edit | 1,001 | 1,000 | 1 | 2.281 s |
| Second edit to same file | 1,001 | 1,000 | 1 | 2.016 s |
| Private mirror dirty, source unchanged | 1,001 | 1,000 | 1 repaired | 1.250 s |

The initial 1,001-file full initialization copied all files as expected because the private HEAD did not yet exist. Both one-file edits still loaded **all 1,002 dependency fact objects** during planning, a separate Phase 2–4 defect that this source-only repair intentionally does not address.

Final instrumented evidence: `lexicon/evaluation/performance/incremental-phase1-git1000-final-2026-10-01.json`. Full mutation-scenario evidence for the 61-file Git fixture: `incremental-phase1-git60-final-2026-10-01.json`.

## Verification

- Eight targeted Git-mirror tests cover clean and dirty private/source trees, repeated edits and reversions, staged/unstaged files, Git assume-unchanged/skip-worktree markers, different committed source HEADs, additions, removals, renames, changed Lexicon ignore rules, nested repositories, Unicode names, non-Git fallback, checkout CRLF conversion and custom filter attributes.
- Five existing focused integration suites verify source mirroring, repository policy, scan planning, dependency topology and scan engine behaviour.
- Production CLI runs test unchanged, first/second edits, dirty private mirror, addition, deletion and rename on disposable Git-backed sources. Metrics must show zero unrelated source-byte comparisons for ordinary one-file edits and preserved snapshot IDs for unchanged/repair-only runs.
- The bounded pinned-Hermes cold diagnostic from Phase 0 remains incomplete. This phase did **not** rerun a full Hermes scan or claim a current Hermes end-to-end time; those measurements belong to Phase 5 after the dependency index is repaired.

## Limits and failure behaviour

Metadata discovery, Git processes, Git staging and mirror pruning still have repository-size overhead; Phase 1 guarantees elimination of unnecessary unchanged-source **content reads** on verifiable Git paths, not absolute O(changes) wall time. Files ignored by source Git but retained by Lexicon, filter-transformed files, non-Git trees and any failed/unverifiable Git probe retain byte comparison. Git's working-tree cleanliness ultimately follows Git's own stat/racy-file model, while explicit assume-unchanged and skip-worktree flags are excluded. Source changes racing the synchronization transaction still require the existing safe publication/recovery flow.

## Code map

| Boundary | Owner and evidence |
| --- | --- |
| Generation-bound proof and dirty-file gating | `lexicon/src/repository/mirror_index.rs` |
| EOL, index flags and checkout attributes | `lexicon/src/repository/mirror_index_worktree.rs` |
| Copy and deletion fallback | `lexicon/src/repository/mirror.rs`, `mirror_copy.rs` |
| Regression fixtures | `lexicon/src/repository/mirror_index_tests.rs`, `lexicon/tests/source_mirror.rs` |
| CLI benchmark harness and evidence | `lexicon/evaluation/performance/incremental_phase0.py`, `incremental-phase1-git*-final-2026-10-01.json` |

## Related docs

- [Incremental scan repair plan](../planning/lexicon-incremental-scan-performance.md)
- [Phase 0 baseline](lexicon-incremental-phase0-2026-10-01.md)
- [Testing and benchmarks](testing-and-benchmarks.md)

## Notes

Do not broaden this source-mirror repair into dependency index or Arcana work. Phase 2 owns persistent dependency topology; Phase 5 owns complete pinned-Hermes before/after acceptance.
