# Hermes Arcana Stage A allocation-topology gate — 2026-09-28

## Result

Stage A's structural and semantic gates pass. The provisional **750 MiB global
process-tree RSS target does not**.

The remaining global peak is isolated to the node pass, whose node-wide validation
and identity maps are the explicit target of Stage B. Pulling those removals into
Stage A would collapse the stage boundary instead of fixing a Stage A allocation
problem.

## Tested revision

- Branch: `integration/lexicon-arcana-next`
- Arcana revision: `d91a6c6546a612d9906d1d58853b4a598d0e6a3a`
  (`Refine compact capacity planning at Stage A gate`)
- Frozen Hermes Lexicon snapshot:
  `sha256:ee46a8a475c443f908e0eff6db9efca62e3871fe1f541e62251ebd9f5ed2c621`
- Fresh Arcana state directory
- Release binary
- No Hermes source scan or Lexicon rebuild
- RSS sampled every 20 ms
- No child process was observed

## Verification

Targeted compact verification passed:

- repository-store compact build/writer/index tests;
- compact snapshot direct-v2 and legacy tests;
- compact-versus-rich byte parity;
- compact-versus-rich graph parity.

Full Arcana test suite passed:

- library: **176 passed, 0 failed, 7 ignored**;
- CLI binary: **17 passed, 0 failed**;
- doc tests: **0 failed**.

## Artifact oracle

The committed Stage A build reproduced the frozen oracle exactly.

| Measurement | Stage A | Oracle status |
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

- Global peak process-tree RSS: **852,656,128 B / 813.16 MiB**
- Provisional Stage A target: **750 MiB**
- Target delta: **+63.16 MiB**
- Peak timestamp: **103,448.848 ms**
- Compact node-pass duration: **103,505.327 ms**

The global peak therefore occurs inside the node pass, immediately before that
phase completes.

After the nominal node-pass boundary, the highest sampled RSS is
**784,568,320 B / 748.22 MiB**, below the Stage A target.

This distinction matters: Stage A's allocation-topology work has removed the
later staging/finalization peak. The remaining >750 MiB global peak is the node
pass itself.

## Stage A correction discovered at the gate

The first Step 2 implementation reserved node capacity exactly for every object
and allocated the complete edge/unresolved staging vectors at
`finish_node_pass()`.

That had two undesirable effects:

1. repeated exact node-vector growth caused severe node-pass reallocation cost;
2. allocating all relation staging at the node/relation boundary created a
   transient post-node RSS spike.

The gate correction keeps the builder as the capacity-policy owner while changing
the timing:

- node counts accumulate to a cumulative required capacity;
- node staging grows with a small bounded headroom instead of geometric growth;
- edge/unresolved totals are still planned during the node pass;
- relation staging is reserved cumulatively as each relation object begins;
- relation counts are checked against the node-pass plan.

This preserves the no-third-traversal design and avoids front-loading all relation
storage at the phase boundary.

## Phase timings

The exact committed run reported:

| Phase | Time |
| --- | ---: |
| Compact node pass | **103,505.327 ms** |
| Compact relation pass | **17,298.359 ms** |
| Compact build finish | **2,985.333 ms** |
| Compact store write | **3,629.572 ms** |
| Compact store checksum | **698.845 ms** |
| Compact graph compile | **1,380.839 ms** |

Wall time is not treated as a Stage A acceptance signal because these runs are
filesystem/cache and host-load sensitive.

## Gate decision

Stage A is complete with one explicit carry-forward:

- semantic parity: **PASS**
- byte parity: **PASS**
- graph parity: **PASS**
- full Arcana tests: **PASS**
- staging/finalization topology: **PASS**
- post-node-pass RSS <= 750 MiB: **PASS** at **748.22 MiB**
- global RSS <= 750 MiB: **MISS** at **813.16 MiB**

The global miss is diagnosed rather than waived: the peak is in the node pass,
where the three node-wide identity/validation structures still exist. Removing
those structures is Stage B's defined work. Stage B should proceed without adding
Stage A compatibility or temporary memory machinery.

## Evidence

- `stage-a.json` — exact committed-run summary, counts, hashes, timings, and RSS.
- `rss-trace.csv` — 20 ms RSS samples for the committed run.
- `run-stage-a.ps1` — frozen-snapshot gate harness.
