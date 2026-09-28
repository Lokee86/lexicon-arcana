# Hermes Arcana compact managed-sync proof — 2026-09-27

Parent evidence: [Hermes Arcana storage and sync performance](../hermes-arcana-storage-performance-2026-09-27/report.md)

## Scope

This is the Phase 9 production proof for the compact managed-sync pipeline. The run used the repository-built release binary from commit `8bc45d3` on branch `perf/arcana-compact-lexicon-sync`.

Arcana consumed the already-published Hermes Lexicon snapshot directly. No Hermes source scan or Lexicon rebuild was performed.

Frozen Lexicon snapshot:

`sha256:ee46a8a475c443f908e0eff6db9efca62e3871fe1f541e62251ebd9f5ed2c621`

The run used a fresh Arcana state directory, so managed sync selected `mode=rebuild`. Peak RSS was sampled across the Arcana process tree every 20 ms.

## Production result

| Measurement | Result |
| --- | ---: |
| Sync mode | rebuild |
| Exit status | 0 |
| Wall time | **200.052 s** |
| Peak process-tree RSS | **983,207,936 B** |
| Peak RSS | **937.66 MiB** |
| Nodes | **1,125,126** |
| Visible edges | **2,367,423** |
| Unresolved | **699,349** |
| Compatibility warnings | **0** |
| Compact graph compile | **2,051.814 ms** |
| `graph.arcana` | **46,411,248 B** |
| `repository.arcana` | **489,778,032 B** |
| Published generation total | **536,190,125 B** |

Raw evidence: [phase9.json](phase9.json).

## Phase 8 oracle parity

The two large artifacts are byte-identical to the frozen Phase 8 oracle.

| Artifact | Phase 9 SHA-256 | Oracle match |
| --- | --- | --- |
| `repository.arcana` | `10cb311318a28703e3c9a510cae1b177e24f123c2d604ffafc194c3e55de0281` | **byte-identical** |
| `graph.arcana` | `ee64b0367905d5e39c64d75c1269b429fc3791b5a6287cce0576e32f22e8fd5d` | **byte-identical** |

Repository identity/checksum remains `342a2379cc36c43c`; graph snapshot identity remains `c8ff2f646b932c40`.

The `graph.manifest` checksum is not expected to match an earlier generation because the manifest contains the publication timestamp. The graph payload itself is byte-identical.

## Memory comparison

The original Phase 8 rebuild baseline was **1,592,414,208 B peak RSS**. Phase 9 is **983,207,936 B**, a reduction of:

- **609,206,272 B**
- **38.26%**
- about **580.98 MiB**

The compact managed-sync pipeline therefore achieves the Phase 9 sub-1-GB rebuild target while preserving exact repository and graph bytes.

For reference, the earlier Phase 2 compact-identity run peaked at **1,369,616,384 B**. Phase 9 is another **386,408,448 B / 28.21%** below that intermediate result.

## Timing

The clean Phase 9 wall time is **200.052 s**. The original Phase 8 rebuild baseline was **156.946 s**.

That is a **43.106 s / 27.47% slowdown** despite the substantial memory reduction. Earlier intermediate timing runs were contaminated by concurrent Lexicon scans; this Phase 9 run was not. No other Arcana/Lexicon/Cargo/Rust compilation job was active when the proof began, and the resident `cargo-reclaim` process showed zero CPU use over a two-second preflight sample.

The memory restoration therefore passes, but rebuild throughput still has a measurable regression that should be treated as separate follow-up performance work rather than obscured by the successful RSS result.

## Conclusion

**Phase 9 passes the compact-sync proof.**

- Production managed sync runs through the compact path.
- The frozen Hermes repository completes successfully without a rescan.
- Peak process-tree RSS is below 1 GB.
- Node, edge, and unresolved counts exactly match the oracle.
- `repository.arcana` and `graph.arcana` are byte-identical to the oracle.
- Published storage size is unchanged.
- The remaining known issue is rebuild wall time: the compact pipeline is currently about 27.5% slower than the original Phase 8 rebuild measurement.
