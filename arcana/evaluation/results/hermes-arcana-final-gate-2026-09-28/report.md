# Hermes Arcana final memory gate — 2026-09-28

## Result

The Arcana Lexicon ingestion memory refactor passes its final integration gate.

The frozen Hermes rebuild peaks at **617,877,504 B / 589.25 MiB**, below the
required **600 MiB** process-tree RSS limit by **10.75 MiB**.

Repository and graph outputs remain byte-identical to the frozen oracle.

## Tested revision

- Worktree: `C:\!bin\workspace\lexicon-arcana-heap`
- Branch: `perf/arcana-heap-refactor`
- Runtime revision:
  `04a109dabaabd12c6b8e5a346e8b72183303bede`
  (`Remove nested ownership build structures`)
- Frozen Hermes Lexicon snapshot:
  `sha256:ee46a8a475c443f908e0eff6db9efca62e3871fe1f541e62251ebd9f5ed2c621`
- Fresh Arcana state directory
- Release binary built from the dedicated heap-refactor worktree
- No Hermes source rescan or Lexicon rebuild
- RSS sampled every 20 ms
- No child process was observed
- The harness refused to start while any `cargo.exe`, `rustc.exe`, or
  `lexicon.exe` process was active. An unrelated Warlock Rust test was allowed
  to finish before this accepted run began.

## Verification before measurement

The repository's verification gates passed before the RSS run:

- `lexicon`: `go test ./...`
- Arcana formatting: `cargo fmt --check`
- Arcana compile surface: `cargo check --all-targets --locked`
- compact snapshot semantic seam: **PASS**
- compact-vs-rich repository byte seam: **PASS**
- compact-vs-rich graph seam: **PASS**
- Arcana full package tests: `cargo test --all-targets --locked`
  - library: **190 passed, 0 failed, 7 ignored**
  - CLI binary: **17 passed, 0 failed**
- release build: **PASS**

## Frozen artifact oracle

| Measurement | Final gate | Oracle status |
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

Both artifacts are byte-identical to the frozen oracle.

## Memory result

- Final process-tree RSS limit: **600 MiB**
- Global peak process-tree RSS:
  **617,877,504 B / 589.25 MiB**
- Margin below required limit: **10.75 MiB**
- Peak timestamp: **233,002.362 ms**
- Wall time: **233.983 s**
- Final memory gate: **PASS**

Per-phase sampled peaks:

| Phase | Peak RSS |
| --- | ---: |
| Compact node pass | **433.02 MiB** |
| Compact relation pass | **514.77 MiB** |
| Compact build finish | **530.96 MiB** |
| Compact store write | **521.42 MiB** |
| Compact store checksum | **493.77 MiB** |
| Compact graph compile | **519.99 MiB** |
| Post-compile publication | **589.25 MiB** |

The ownership refactor moved compact build finalization from the Stage C peak of
**612.59 MiB** down to **530.96 MiB**, a reduction of **81.63 MiB** in the
phase it directly targeted.

The global Stage C peak was **612.59 MiB**. The final global peak is
**589.25 MiB**, a further **23.34 MiB** reduction and enough to clear the final
acceptance threshold.

Against the original measured baseline of **937.66 MiB**, the final result is
lower by **348.41 MiB**, approximately **37.2%**.

## Remaining peak explanation

The final global peak no longer occurs in Lexicon ingestion, relation staging,
string canonicalization, ownership construction, repository writing, checksum,
or graph compilation.

The **589.25 MiB** peak occurs after the profiled graph-compile phase, while
`write_compiled_compact_owned` still owns both the completed
`CompactRepositoryBuild` and compiled graph and proceeds through graph
serialization/publication and import-summary work.

This explains the remaining memory above the core compact-build phases rather
than leaving it unattributed:

- compact build finalization: **530.96 MiB**
- graph compilation: **519.99 MiB**
- final publication overlap: **589.25 MiB**

The core build therefore already sits in the requested approximately
**500–550 MiB** region. Further reduction toward that range for the complete
process can target post-compile publication lifetime independently, without a
persistent-format change.

## Architectural completion

The accepted build satisfies the memory-refactor completion properties:

- managed Lexicon sync remains compact-only;
- staging is capacity-aware;
- consuming finalization does not retain drained staging allocations;
- old node-signature and identity-resolution maps are gone;
- sorted node staging owns duplicate/collision validation;
- strings are arena/blob-backed and transient interning lookup is dropped;
- compact ownership uses dense node ownership plus count/prefix/fill;
- contributions are allocated directly in one exact flat array;
- node-wide owner trees and nested contribution vectors are gone;
- final node indexes reserve directly from known node count;
- no migration-only compatibility builder, node-map fallback, or dual compact
  string/ownership architecture remains;
- persistent repository and graph formats are unchanged.

## Gate decision

- formatting/check/full tests: **PASS**
- compact snapshot semantic parity: **PASS**
- repository byte parity: **PASS**
- graph parity: **PASS**
- frozen counts/sizes/hashes: **PASS**
- process-tree RSS <= 600 MiB: **PASS — 589.25 MiB**
- remaining peak measured and attributed: **PASS**
- final integration gate: **PASS**

## Evidence

- `final-gate.json` — accepted gate summary, hashes, counts, timings, and RSS.
- `rss-trace.csv` — 20 ms process-tree RSS trace.
- `run-final-gate.ps1` — reproducible frozen-snapshot harness with quiet-machine
  preflight and hard final-gate assertions.
