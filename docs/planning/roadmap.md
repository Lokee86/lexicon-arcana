# Roadmap

Parent index: [Planning](INDEX.md)

## Purpose

Own the remaining Lexicon + Arcana transition and product work after Grimoire retirement.

## Overview

Grimoire's discovery runtime, MCP surface, repository state, installed skill, and active release ownership are retired. Remaining work is about finishing the L+A product transition, improving measured repository-analysis value, and integrating the surviving components cleanly with downstream consumers such as Warlock.

## Current status

Grimoire retirement is complete. Active build/test/install/release targets Lexicon + Arcana only, the retired runtime/MCP/discovery source is absent from the active tree, the canonical repository is now `Lokee86/lexicon-arcana`, and historical benchmark evidence and ADRs remain preserved.

## Expected ownership

- Lexicon owns polyglot semantic extraction, normalized facts, immutable semantic snapshots, and publication lifecycle.
- Arcana owns verified Lexicon ingestion, repository/call graph storage, graph queries, snapshots, overlays, and optional semantic graph entry points.
- Warlock/consumers own agent/task/context orchestration.
- Ordinary source/Git/search tools remain direct evidence surfaces.
- No surviving component owns Grimoire's retired umbrella discovery layer.

## Planned behavior

The product family should ship and operate as two deterministic analysis tools with a small shared distribution surface, direct consumer boundaries, preserved benchmark evidence, and no hidden dependency on retired Grimoire code or state.

## Implementation sequence

1. **Complete:** remove the physical Grimoire runtime and product-specific dependencies while preserving historical evidence.
2. **Complete:** remove or reclassify stale Grimoire-only documentation, governance, and evaluation entry points.
3. **Complete for current analysis tooling:** update Warlock and Reliquary to consume direct Lexicon/Arcana boundaries; broader Warlock runtime integration remains separately planned.
4. **Complete:** rename the canonical repository identity to `Lokee86/lexicon-arcana`.
5. **Complete — Lexicon durable-store compaction:** binary v2 preserves the full semantic fact set while reducing the current Hermes fact-object corpus from **220,393,540 bytes to 125,836,351 bytes (42.9%)**.
6. **Complete — Python adapter memory:** compact Python semantic-analysis ownership and durable in-memory records while preserving emitted-fact and snapshot semantics.
7. Continue judged repository-analysis experiments on larger and more varied corpora.

## Near-term priorities

- Keep active build, release, CI, policy, and documentation surfaces free of dependencies on retired Grimoire runtime/state.
- Wire the future Warlock runtime integration to direct Lexicon/Arcana surfaces and installed-state conventions rather than recreating an umbrella layer.
- Preserve the clean distinction between the installed production skill and frozen benchmark prompt experiments.
- Add stable machine-readable diagnostics/exit classes where current component behavior remains pre-release.
- Expand judged corpora across languages, repository sizes, and task classes.

## Lexicon work

- Continue semantic-fact coverage where judged consumers need richer language semantics.
- Improve adapter correctness and unresolved-evidence quality before adding speculative cross-language inference.
- Measure initialization and incremental scan cost on substantially larger repositories.
- Compact the durable CAS representation before considering semantic-fact pruning.
- Keep Python semantic-analysis memory bounded around compact retained state; the current Hermes benchmark is 3.612 GiB peak RSS at full semantic coverage.
- Keep immutable publication and bounded external-consumer behavior as hard contracts.

### Lexicon performance/storage sequence

#### 1. Durable CAS format compaction — complete

**Owner:** Lexicon object store.

Hermes baseline from the current binary object format:

- durable fact objects: about **220.4 MB**;
- per-object string tables: about **158.5 MB / 71.9%**;
- encoded node, edge, and unresolved sections combined: about **61.8 MB**.

Implementation plan:

- encode SHA-256 identities as binary digests rather than textual `sha256:` strings;
- replace local semantic-node string references with compact object-local ordinals and a bounded external-reference table;
- encode bounded node kinds and edge relations as compact enum/varint values;
- factor repeated owner/path/qualified-name identity where object structure already supplies it;
- reduce cross-record and cross-object string duplication, then evaluate general compression only after structural encoding is compact;
- benchmark object build, load, traversal, and total durable bytes against the existing format.

Acceptance gate: **met**. Binary v2 keeps binary v1 and legacy JSON read compatibility, preserves the JSON-level semantic fact set, and uses only representation changes: raw SHA-256 identities, local node ordinals, bounded external references, stable kind/relation codes with lossless fallback, exact repeated-field factoring, and deterministic front-coded string tables.

Measured results:
- 200-node synthetic fixture: **29,969 bytes v1 → 17,932 bytes v2 (40.2% smaller)**;
- current Hermes snapshot, 6,697 referenced objects: **220,393,540 bytes → 125,836,351 bytes (42.9% smaller)**;
- 500-node codec benchmark on the current development machine (100 iterations): typed v2 encode about **0.62 ms/op**, full v2 decode about **1.40 ms/op**, node-only decode about **0.56 ms/op**;
- no semantic-fact pruning or snapshot-contract change.

#### 2. Python semantic-analysis memory — complete

**Owner:** Lexicon Python adapter.

The monolithic JSONL handoff is already removed by `a4a88ba` (`Stream Python analysis into Lexicon`). The validated Hermes cold run completed in about **6m14.6s** with no temporary JSONL, but Python semantic analysis still reached roughly **5–6 GB** peak working memory.

Implementation plan:

- profile retained bytes across nodes, edges, unresolved facts, resolution indexes, merge intermediates, duplicated strings, and section sorting;
- compact high-cardinality semantic records and indexes rather than reducing fact coverage;
- release shard/intermediate state as soon as global resolution no longer requires it;
- avoid full-size sorting/materialization copies where deterministic ordering can be produced more cheaply;
- remeasure peak memory, cold-scan wall time, and emitted-fact/snapshot equivalence on Hermes and larger corpora.

Acceptance gate: **met**. The adapter preserves emitted facts and deterministic publication while reducing retained analysis state and durable-record overhead without pruning semantic coverage.

Implemented changes:
- bound/release source-byte caches and remove per-file line-string copies;
- release source bytes, source text, full file AST roots, and merged shard containers when later phases no longer need them;
- retain only function/class AST fragments needed by repository-wide resolution;
- remove duplicate repository-wide indexes and keep dataflow deduplication file-local;
- store durable nodes, edges, unresolved facts, and spans as compact slotted records/tuples, materializing JSON dictionaries only at emission.

Measured on Hermes (**6,694 Python files / 84.3 MB**) with **16 active workers / 256 logical shards / merge fan-in 8**:
- measured baseline: **6.10 GiB peak RSS, 285.5 s**;
- final result: **3.612 GiB peak RSS, 134.574 s**;
- about **41% lower peak memory** and **53% lower wall time**;
- the optimized and frozen baseline adapters emit the exact same **1,058,412,847-byte / 2,972,095-line** Hermes JSONL stream with SHA-256 `6f3744688d376e5473bbe6e5f333d63afebc2f0c9a1b95630840ae8eacd6a3a4`;
- existing Python adapter and Go scan integration tests preserve emitted semantic behavior.

#### 3. Cross-adapter scan sanity audit — complete for locally executable adapters

**Owner:** Lexicon language adapters.

A September 21, 2026 local scaling pass checked the remaining adapter implementations after the Python scan fixes. The goal was to distinguish compiler/runtime cost from avoidable scan, merge, output-materialization, or resolution pathologies.

Representative isolated results on the current development machine:

- C-family / LevelDB: **1.82 s** for 133 source/header files and a **19.6 MB** fact stream;
- GDScript / Space Rocks client: **6.60 s** warm for 537 files and a **46.3 MB** fact stream;
- Kotlin / Detekt: **2.58 s** warm for 1,157 files and a **67.6 MB** fact stream;
- Ruby / Space Rocks API: **3.14 s** warm for 121 files;
- TypeScript/Svelte / Lexicanter: approximately **11.5 s** for the current working implementation and a roughly **42 MB** fact stream;
- Go / the Lexicon tree: **21.1 s** for 349 indexed files, of which **11.28 s** was package loading and **6.03 s** was SSA/VTA; shard-local typed resolution was **0.44 s**;
- Rust / Arcana: **8.17 s** for 127 Rust files;
- generic fallback: **0.50 s** for a synthetic 1,000-file PowerShell corpus.

No locally executable adapter reproduced Python's former multi-minute scan/merge pathology. C# was the material outlier. Profiling on Dapper showed the cost inside repeated Roslyn semantic passes rather than discovery: declarations, calls, and dataflow dominated. Replacing full canonical-JSON serialization as the edge/unresolved deduplication key reduced one profiled Dapper files-mode run from **18.6 s to 14.9 s** while preserving byte-identical output; the larger Polly files-mode corpus remained about **27 s**, showing that Roslyn semantic work is still the main cost.

Java was not timed in this pass because the current host has no JDK available to the adapter. Its implementation uses one compiler-backed batch and streams compiler evidence rather than spawning per-file compiler work, but it still needs a measured current-host baseline when a JDK/runtime is available.

Follow-up:

- profile and optimize C# semantic passes before adding generic file sharding; preserve Roslyn project/type semantics and deterministic output;
- keep large-output materialization under review across non-streaming adapters and add streaming only where measurements justify the complexity;
- add a current Java corpus timing when a JDK or packaged Java runtime is available;
- retain adapter-specific timing/output-size checks when substantially larger corpora are added.

## Arcana work

- Continue graph correctness, compatibility, overlay, compaction, and query-protocol validation.
- Evaluate community/summary/semantic entry points only where they reduce real investigation cost.
- Keep exact graph traversal authoritative and semantic vectors optional.
- Measure storage and traversal behavior at larger graph scales.

## Agent and consumer work

- Maintain the shipped L+A production skill as bounded direct-component guidance; keep the benchmark skill frozen to its experiment conditions.
- Keep source inspection as implementation authority.
- Use Lexicon for semantic ownership/relationships and Arcana for bounded graph questions.
- Put stopping, task decomposition, context routing, and probabilistic continuation in Warlock rather than either analysis component.
- Benchmark Plain versus L+A as the normal repository-agent comparison; retain Grimoire runs only as historical baselines.

## Distribution and compatibility

- Use `Lokee86/lexicon-arcana` as the canonical repository identity and Lexicon + Arcana naming for the combined bundle.
- Define canonical install/discovery locations for Lexicon executable, adapters, and Arcana executable.
- Decide whether former component repositories become mirrors or redirects.
- Define stable migration policy for `.lexicon/` and `.arcana/` before a stable release.
- Keep `.grimoire/` historical/migration-only; do not invent new state there.

## Release gates

Establish measured gates for:

- Lexicon adapter and publication correctness;
- Arcana graph correctness and Lexicon compatibility;
- `arcana.query.v1` compatibility;
- independent Lexicon and Arcana installation;
- combined L+A bundle integrity;
- end-to-end agent correctness, grounding, and investigation efficiency;
- preparation latency and memory on representative repository scales;
- documentation and architecture-policy compliance.

## Acceptance criteria

A roadmap item requires a named owner, implementation plan, focused tests/evaluation, documentation impact, and a clear current-behavior owner before it is complete.

Grimoire retirement is complete:

- no active build/release/CI surface requires or publishes Grimoire;
- retired runtime/MCP/discovery source is absent from the active tree;
- current documentation and policy describe only L+A ownership;
- Lexicon and Arcana build/test independently and through the combined workflow;
- downstream current products consume direct component boundaries;
- historical benchmark artifacts/ADRs remain understandable and intact.

## Open decisions

Open decisions are the downstream Warlock runtime integration shape and which larger-repository tasks justify additional Lexicon/Arcana capabilities.

## Related docs

- [System overview](../architecture/system-overview.md)
- [Current limitations](../limits/current-limitations.md)
- [Agent benchmark findings](../development/agent-benchmark-findings.md)
- [ADR 0006](../decisions/0006-retire-grimoire-lead-with-lexicon-arcana.md)

## Notes

A future capability must have an explicit Lexicon, Arcana, shared-release, or downstream-consumer owner. Do not restore retired Grimoire responsibilities as an implicit coordination layer.