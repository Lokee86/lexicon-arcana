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

The memory restoration therefore passes. The apparent rebuild-throughput regression was investigated immediately afterward.

## Throughput regression follow-up

The follow-up added phase timing around compact ingestion/build/write and compared the repository writer with and without buffering on the same frozen Hermes snapshot.

The regression source was the repository-store sink writing encoded records directly to `File`. The store contains millions of small encoded records, so the unbuffered path paid a large syscall/I/O overhead even though the final file is only 489.8 MB.

A controlled warm-cache A/B isolated the writer stage:

| Variant | Repository store write |
| --- | ---: |
| Original unbuffered writer | **44.674 s** |
| 1 MiB `BufWriter` | **1.758–2.217 s** |

Buffering removes about **42.9 s / 96.1%** of the repository-write stage, a **25.4×** speedup for that stage. The explicit full-store artifact checksum is only about **0.68–1.05 s**, so duplicate checksum work was not the regression source and remains unchanged.

End-to-end wall time varied substantially with Windows filesystem cache state, especially in the node pass. Observed buffered production runs were:

- **105.553 s** with a relatively cold node pass of **80.743 s**.
- **40.236 s** with a warm node pass of **14.143 s**.

The warm trace broke down as:

| Phase | Time |
| --- | ---: |
| compact node pass | **14.143 s** |
| compact relation pass | **14.793 s** |
| compact build finish | **3.994 s** |
| repository store write | **1.758 s** |
| repository checksum | **0.680 s** |
| compact graph compile | **2.006 s** |
| total managed sync | **40.236 s** |

Because the node-pass time is cache-sensitive, the end-to-end numbers should not be used as a controlled before/after speedup. The writer-stage A/B is the causal measurement.

The warm buffered RSS trace peaked at **990,957,568 B**, during compact-build finalization. The writer stage itself peaked around **633 MB** and therefore is not the remaining memory ceiling.

After the writer fix, both generated artifacts remain byte-identical to the Phase 8 oracle:

- `repository.arcana`: `10cb311318a28703e3c9a510cae1b177e24f123c2d604ffafc194c3e55de0281`
- `graph.arcana`: `ee64b0367905d5e39c64d75c1269b429fc3791b5a6287cce0576e32f22e8fd5d`

Raw follow-up evidence: [throughput-regression.json](throughput-regression.json).

## Conclusion

**Phase 9 passes the compact-sync proof, and the subsequent throughput regression has been identified and repaired.**

- Production managed sync runs through the compact path.
- The frozen Hermes repository completes successfully without a rescan.
- Peak process-tree RSS remains below 1 GB.
- Node, edge, and unresolved counts exactly match the oracle.
- `repository.arcana` and `graph.arcana` remain byte-identical to the oracle.
- Published storage size is unchanged.
- The measured throughput regression came from unbuffered repository-store writes; buffering reduces that stage from **44.674 s** to **1.758–2.217 s**.
- The dominant remaining runtime is now Lexicon ingestion, with node-pass timing strongly affected by filesystem cache state.
