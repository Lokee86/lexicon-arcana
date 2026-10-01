# Hermes Arcana Stage C string-arena gate — 2026-09-28

## Result

Stage C passes its string-system and Hermes parity gates.

The arena-backed compact string path preserves exact repository semantics and
persistent bytes while reducing the frozen Hermes process-tree peak to
**642,342,912 B / 612.59 MiB**.

This is **81.02 MiB lower** than Stage B's 693.61 MiB peak and leaves
**12.59 MiB** between the current peak and the final <=600 MiB acceptance target.

Stage C has no separately frozen RSS threshold. Memory is recorded here as an
observation for the final gate rather than inventing an intermediate acceptance
criterion.

## Tested revision

- Worktree: `C:\!bin\workspace\lexicon-arcana-heap`
- Branch: `perf/arcana-heap-refactor`
- Runtime revision:
  `2f6140a8c275dae077b4328e3fd346da386ab5f9`
  (`Cut compact ingestion over to string arena`)
- Frozen Hermes Lexicon snapshot:
  `sha256:ee46a8a475c443f908e0eff6db9efca62e3871fe1f541e62251ebd9f5ed2c621`
- Fresh Arcana state directory
- Release binary built from the dedicated heap-refactor worktree
- No Hermes source rescan or Lexicon rebuild
- RSS sampled every 20 ms
- No child process was observed

The Step 13 test additions are test-only and were uncommitted during the runtime
measurement; the release binary therefore corresponds exactly to the runtime
revision above.

## String gate

The canonical compact string path now proves:

- duplicate interning stores one byte sequence and returns one stable temporary
  handle;
- lexical freeze is deterministic across insertion order;
- unused temporary handles are omitted and remapped to the absent sentinel;
- empty strings remain distinct from absent strings;
- NUL, newline, tab, and multibyte UTF-8 strings survive freeze and binary
  round-trip unchanged;
- the binary string-table record/index bytes are checked against an explicit
  expected byte sequence;
- invalid UTF-8 in a persisted string table is rejected;
- compact-vs-rich repository writing remains byte-identical.

The transient interning hash table is consumed and dropped inside
`StagedStringArena::freeze` before lexical ordering and final table materialization.

The final `CompactStringTable` remains blob-backed: one UTF-8 byte blob plus
offsets, with no per-string `String` allocation.

## Verification

Targeted string checks passed:

- arena deduplication;
- lexical ordering and remapping;
- insertion-order determinism;
- control-character and UTF-8 preservation;
- exact encoded-byte fixture;
- invalid UTF-8 rejection;
- compact-vs-facts writer byte parity.

Full Arcana package verification passed:

- library: **187 passed, 0 failed, 7 ignored**
- CLI binary: **17 passed, 0 failed**
- `cargo test --all-targets --locked`: **PASS**
- release build: **PASS**

## Artifact oracle

The frozen Hermes artifacts remain exact.

| Measurement | Stage C | Oracle status |
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

## Memory observation

- Global peak process-tree RSS:
  **642,342,912 B / 612.59 MiB**
- Final acceptance target:
  **<=600 MiB**
- Current delta to final target:
  **+12.59 MiB**
- Peak timestamp:
  **183,998.526 ms**
- Wall time:
  **194.546 s**

Per-phase sampled peaks:

| Phase | Stage C peak RSS |
| --- | ---: |
| Compact node pass | **413.44 MiB** |
| Compact relation pass | **493.52 MiB** |
| Compact build finish | **612.59 MiB** |
| Compact store write | **578.89 MiB** |
| Compact store checksum | **494.76 MiB** |
| Compact graph compile | **520.57 MiB** |

The absolute peak now occurs during compact build finalization.

## Stage B comparison

| Phase | Stage B | Stage C | Change |
| --- | ---: | ---: | ---: |
| Global peak | 693.61 MiB | **612.59 MiB** | **-81.02 MiB** |
| Node pass | 444.41 MiB | **413.44 MiB** | **-30.97 MiB** |
| Relation pass | 604.16 MiB | **493.52 MiB** | **-110.64 MiB** |
| Build finish | 661.07 MiB | **612.59 MiB** | **-48.48 MiB** |
| Store write | 673.95 MiB | **578.89 MiB** | **-95.06 MiB** |
| Checksum | 596.30 MiB | **494.76 MiB** | **-101.54 MiB** |
| Graph compile | 618.89 MiB | **520.57 MiB** | **-98.32 MiB** |

Wall timing is diagnostic only and is not compared as an acceptance result.
This run was visibly host/I/O constrained; RSS and artifact parity are the gate
signals.

## Gate diagnosis

Stage C achieved its intended ownership cut:

- compact ingestion uses only the arena-backed interner;
- the old compact `BTreeMap<String, TempStringId>` path is gone;
- there is no dual compact string representation;
- lexical freeze produces stable final IDs and drops transient lookup state;
- the persistent repository format and byte output are unchanged.

The remaining global peak is no longer in relationship ingestion or publication.
It is concentrated in compact build finalization at 612.59 MiB.

That aligns with Stage D's remaining planned work:

- replace node-owner tree state with dense `owner_by_node`;
- build contributions directly into one exact flat array;
- delete the old ownership trees/nested capacities and final flattening overlap.

Stage D needs only **12.59 MiB** of additional peak reduction to satisfy the
final <=600 MiB gate on this frozen corpus.

## Gate decision

- arena interning canonical: **PASS**
- deterministic lexical freeze/remap: **PASS**
- control/UTF-8 preservation: **PASS**
- invalid UTF-8 rejection: **PASS**
- exact binary string bytes: **PASS**
- no old compact BTreeMap interner: **PASS**
- full Arcana verification: **PASS**
- Hermes artifact counts/sizes/hashes: **PASS**
- Stage C RSS observation: **612.59 MiB**
- final <=600 MiB target: **not yet evaluated as final gate; current observation is 12.59 MiB above it**

Stage C is complete. Proceed to the planned Stage D ownership/contribution cut.

## Evidence

- `stage-c.json` — accepted Hermes run summary, hashes, timings, and RSS.
- `rss-trace.csv` — 20 ms RSS samples.
- `run-stage-c.ps1` — frozen-snapshot Stage C harness.
