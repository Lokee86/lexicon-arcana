# Hermes Arcana Stage B sorted-node-staging gate — 2026-09-28

## Result

Stage B's structural, semantic, and repository-wide verification gates pass. The
planned **650 MiB global process-tree RSS target does not**.

The exact committed Hermes run peaks at **727,298,048 B / 693.61 MiB**, which is
**43.61 MiB above** the Stage B target.

The Stage B work did remove the diagnosed node-wide identity-map peak:

- Stage A global peak: **813.16 MiB**
- Stage B global peak: **693.61 MiB**
- reduction: **119.55 MiB**
- Stage B node-pass peak: **444.41 MiB**
- Stage B relation-pass peak: **604.16 MiB**

The remaining peak occurs after the profiled graph-compile phase while the final
repository/graph publication state is live. It is therefore not the deleted
`NodeSignature`, `NodeKey -> identity`, or `identity -> NodeKey` state.

## Tested revision

- Worktree: `C:\!bin\workspace\lexicon-arcana-heap`
- Branch: `perf/arcana-heap-refactor`
- Revision:
  `ff256a0b90cf94baf095fc893b81a57b72f1c451`
  (`Delete obsolete compact node identity maps`)
- Frozen Hermes Lexicon snapshot:
  `sha256:ee46a8a475c443f908e0eff6db9efca62e3871fe1f541e62251ebd9f5ed2c621`
- Fresh Arcana state directory
- Release binary built from the dedicated heap-refactor worktree
- No Hermes source rescan or Lexicon rebuild
- RSS sampled every 20 ms
- No child process was observed

An initial Stage B harness invocation was discarded because the copied script
still pointed to the original integration checkout. That run was cancelled, its
two generated untracked output files were removed from the shared worktree, and
the harness was corrected before the accepted measurement below. The accepted
measurement's process executable is the dedicated worktree's
`arcana\target\release\arcana.exe`.

## Repository-wide verification

Focused Stage B checks passed:

- canonical node staging sort/deduplication;
- conflicting-definition detection;
- NodeKey identity-collision detection;
- canonical relationship identity resolution;
- compact snapshot legacy/v2 parity.

The repository's documented verification passed:

- `lexicon`: `go test ./...`
- `arcana`: `cargo test --all-targets --locked`

Arcana's complete test run included:

- library: **182 passed, 0 failed, 7 ignored**
- CLI binary: **17 passed, 0 failed**
- doc tests: **0 failed**

## Structural hard-cut checks

The compact ingestion path no longer retains any of the three old node-wide
identity structures:

- no `BTreeMap<LexiconIdentity, NodeSignature>`;
- no `HashMap<NodeKey, LexiconIdentity>`;
- no `HashMap<LexiconIdentity, NodeKey>`.

`NodeSignature` and its rich boxed attribute/span representation are deleted
from the compact path.

Canonical membership is the sorted staged-node vector. Relationship identity
references derive `identity.node_key()` and validate membership against that
sorted canonical table.

The separate rich-materialization path still has its own `external_ids`
machinery; that path is outside the compact-only Stage B ownership cut.

## Artifact oracle

The dedicated-worktree Stage B build reproduced the frozen oracle exactly.

| Measurement | Stage B | Oracle status |
| --- | ---: | --- |
| Nodes | **1,125,126** | exact |
| Graph-visible edges | **2,367,423** | exact |
| Unresolved | **699,349** | exact |
| `repository.arcana` | **489,778,032 B** | exact |
| `graph.arcana` | **46,411,248 B** | exact |

SHA-256:

- `repository.arcana`:
  `10cb311318a28703e3c9a510cae1b177e24f123c2d604ffafc194c3e55de0281`
- `graph.arcana`:
  `ee64b0367905d5e39c64d75c1269b429fc3791b5a6287cce0576e32f22e8fd5d`

Both are byte-identical to the frozen oracle.

## Memory result

- Global peak process-tree RSS:
  **727,298,048 B / 693.61 MiB**
- Stage B target: **650 MiB**
- target delta: **+43.61 MiB**
- peak timestamp: **67,559.400 ms**
- wall time: **68.629 s**

Per-phase sampled peaks:

| Phase | Peak RSS |
| --- | ---: |
| Compact node pass | **444.41 MiB** |
| Compact relation pass | **604.16 MiB** |
| Compact build finish | **661.07 MiB** |
| Compact store write | **673.95 MiB** |
| Compact store checksum | **596.30 MiB** |
| Compact graph compile | **618.89 MiB** |

The absolute peak occurs after the profiled graph compile has completed, during
the remaining final publication lifetime.

RSS at the major phase boundaries was:

| Boundary | RSS |
| --- | ---: |
| Node pass complete | **338.51 MiB** |
| Relation pass complete | **550.78 MiB** |
| Build finish complete | **536.35 MiB** |
| Store write complete | **596.30 MiB** |
| Store checksum complete | **596.31 MiB** |
| Graph compile complete | **618.89 MiB** |

## Stage A comparison

The Stage A committed run on the same frozen Hermes snapshot recorded:

- **813.16 MiB** global peak RSS;
- **103,505.327 ms** compact node pass.

Stage B records:

- **693.61 MiB** global peak RSS;
- **45,733.681 ms** compact node pass.

This is a **119.55 MiB** reduction in global peak RSS while preserving exact
output bytes. Timing is diagnostic only and is not an acceptance criterion.

## Gate diagnosis

Stage B accomplished its intended ownership cut:

- duplicate/collision semantics are owned by sorted staging;
- relation resolution is owned by canonical staged-node membership;
- all three old global node maps are gone.

The remaining RSS excess is downstream of that cut. The node pass itself is now
well below 650 MiB, and relation ingestion remains below 650 MiB. The global
overshoot appears only as final compact repository/graph state overlaps with
publication work.

The next planned stages directly target structures still live in that region:

- Stage C replaces the individual-string/BTreeMap representation with the
  arena-backed string subsystem and drops the transient lookup after freeze;
- Stage D replaces the remaining ownership trees and nested contribution
  capacities with dense/flat exact-allocation structures.

No compatibility shim, fallback, alternate identity map, or Stage B-specific
memory workaround was added to force this gate green.

## Gate decision

- sorted staging is canonical: **PASS**
- duplicate collapse/conflict semantics: **PASS**
- relationship resolution from canonical staging: **PASS**
- old compact node maps deleted: **PASS**
- repository-wide verification: **PASS**
- artifact counts/sizes/hashes: **PASS**
- global RSS <= 650 MiB: **MISS** at **693.61 MiB**

Step 10 therefore closes with a documented Stage B threshold miss rather than a
false pass. The evidence supports proceeding to the already-planned Stage C/D
memory cuts rather than reintroducing temporary Stage B machinery.

## Evidence

- `stage-b.json` — exact accepted run summary, oracle hashes, timings, and RSS.
- `rss-trace.csv` — 20 ms RSS samples.
- `run-stage-b.ps1` — frozen-snapshot gate harness pinned to the dedicated
  heap-refactor worktree.

## Documentation impact

- Inspected: root verification contract, Arcana component documentation,
  Stage A gate evidence.
- Updated: this Stage B gate evidence.
- Not affected: public CLI, persistent formats, protocol identifiers, operator
  workflow, and Lexicon/Arcana ownership contracts.
- Compliance check: documented repository verification commands passed.
- Known documentation gaps: none introduced by Stage B.
