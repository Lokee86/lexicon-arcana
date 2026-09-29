# Arcana Lexicon ingestion heap baseline — 2026-09-28

Parent index: [Development](INDEX.md)

This document freezes the measured baseline used by the Arcana Lexicon ingestion
memory refactor. It is historical evidence for comparison at later integration
gates; it is not a fresh benchmark run.

No new Hermes rebuild was performed to create this document.

## Measurement identity

- Refactor baseline branch: `perf/arcana-compact-lexicon-sync`
- Baseline implementation commit: `cbbb5c34de6f00f10d9279c71d9ca02ed4b52d3d`
  (`cbbb5c3`, `Buffer compact repository writes`)
- Frozen Lexicon snapshot:
  `sha256:ee46a8a475c443f908e0eff6db9efca62e3871fe1f541e62251ebd9f5ed2c621`
- Snapshot live Lexicon objects: **209,122,429 B**
- Nodes: **1,125,126**
- Pre-dedup edges: **2,426,336**
- Unresolved references: **699,349**
- Final Arcana strings: **1,858,705**

The current integration branch may advance beyond `cbbb5c3`; later measurements
must compare against this frozen baseline rather than silently redefining it.

## Frozen heap measurements

| Measurement | Frozen value |
| --- | ---: |
| Process-tree peak RSS | approximately **974 MB** |
| Peak live requested heap | **1,148,164,251 B** |
| Snapshot-to-live-heap expansion | approximately **5.49x** |
| Node-pass live heap | approximately **833.7 MB** |
| Completed compact repository logical storage | approximately **563.3 MB** |

The diagnosed peak was accounted for to better than 1%.

## Frozen allocation accounting

### Temporary record staging

| Allocation | Capacity |
| --- | ---: |
| Temp nodes | **201.3 MB** |
| Temp edges | **201.3 MB** |
| Temp unresolved | **67.1 MB** |
| **Combined** | **469.8 MB** |

The staging vectors were geometrically overallocated. After records were drained
into final vectors, the empty source vectors retained their backing allocations
through later index and ownership construction.

### Node-pass lookup and validation state

The node pass simultaneously retained:

- `BTreeMap<LexiconIdentity, NodeSignature>`
- `HashMap<NodeKey, LexiconIdentity>`
- `HashMap<LexiconIdentity, NodeKey>`

Clearing two of these structures after the node pass reduced live heap by
approximately **380 MB**.

Relationship resolution also retained a million-entry `external_ids` identity
map. Dropping it reduced live heap by approximately **86 MB**.

### String ownership

The final string population was approximately **1.86 million** strings represented
as individually allocated `String` values in a `BTreeMap`.

### Ownership construction

The baseline ownership path simultaneously constructed:

- a million-entry node-owner `BTreeMap`;
- a path-to-`Vec<Contribution>` tree;
- approximately **96 MB** of nested contribution vector capacity;
- another approximately **67 MB** final contribution vector while flattening.

Several final node indexes also relied on geometric vector growth despite their
final lengths being known.

## Frozen artifact oracle

Later integration gates must preserve the existing canonical artifacts exactly.

| Measurement | Oracle |
| --- | ---: |
| Nodes | **1,125,126** |
| Graph-visible edges | **2,367,423** |
| Unresolved | **699,349** |
| `repository.arcana` | **489,778,032 B** |
| `graph.arcana` | **46,411,248 B** |

Required SHA-256:

- `repository.arcana`:
  `10cb311318a28703e3c9a510cae1b177e24f123c2d604ffafc194c3e55de0281`
- `graph.arcana`:
  `ee64b0367905d5e39c64d75c1269b429fc3791b5a6287cce0576e32f22e8fd5d`

The existing compact managed-sync proof remains the artifact-parity evidence for
this snapshot. Future memory gates should use a fresh Arcana state directory and
process-tree RSS sampling without concurrent Lexicon, Cargo, or Rust workloads.

## Refactor comparison gates

These are comparison targets, not claims about the frozen baseline:

- Stage A: **<= 750 MiB** process-tree peak RSS
- Stage B: **<= 650 MiB** process-tree peak RSS
- Final: **<= 600 MiB** process-tree peak RSS

The final target is required. The design should retain a credible path toward
approximately **500-550 MiB** without changing persistent formats.

## Baseline rule

Do not rerun or replace this baseline merely because implementation work advances.
New measurements are gate results and should be recorded separately with their
commit, fixture identity, state-directory conditions, and artifact hashes.
