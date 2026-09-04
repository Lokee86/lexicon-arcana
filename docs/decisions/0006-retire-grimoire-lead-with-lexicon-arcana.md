# ADR 0006: Retire Grimoire and lead with Lexicon + Arcana

Parent index: [Architecture decisions](INDEX.md)

## Purpose

Record the decision to retire Grimoire as a product and repository-discovery layer, make Lexicon and Arcana the lead repository-analysis products, and avoid transferring Grimoire's retired responsibilities into either component without separate evidence and ownership decisions.

## Status

Accepted — 2026-09-03.

This ADR supersedes ADR 0002 as the active product interaction model and supersedes the Grimoire-product portions of ADRs 0001 and 0004. Their Lexicon/Arcana independence and immutable process-boundary rationale remain valid.

## Context

Grimoire was built as a downstream discovery layer over ordinary source retrieval, Lexicon language facts, Arcana graph relationships, documentation retrieval, stable handles, investigation sessions, and provider-state coordination.

Agent benchmark work showed that prepared Lexicon + Arcana evidence already provides a material benefit over plain repository exploration without requiring Grimoire's discovery layer. On the current Detekt control, the completion-bounded Lexicon + Arcana condition reduced started inference/tool items from 25 to 19, wall time from 470.2 seconds to 300.6 seconds, total input from 1.906M to 1.472M tokens, and fresh input from 195k to 113k while preserving the accepted answer quality and grounding. The remaining inefficiency was open-ended agent exploration after sufficient evidence was available, not absence of another repository-discovery wrapper.

Lexicon and Arcana already have independent executables, state, tests, documentation, and process contracts. Lexicon does not depend on Grimoire. Arcana's deterministic graph path consumes Lexicon snapshots directly and does not depend on Grimoire.

## Decision

Retire Grimoire as an active product, discovery API, MCP surface, and orchestration layer.

The lead analysis stack is:

```text
repository source
    -> Lexicon
       deterministic polyglot semantic facts
       immutable .lexicon snapshots
    -> Arcana
       deterministic repository/call graph
       immutable .arcana snapshots
    -> humans, agents, Warlock, Pitlord, and other consumers
```

Ordinary development tools remain first-class alongside that stack:

```text
source files, Git, ripgrep, IDE/file reads
```

Lexicon and Arcana are complementary products with separate ownership. Do not create a replacement umbrella discovery layer merely to preserve Grimoire's old shape.

## Ownership consequences

### Lexicon owns

- language adapters and semantic extraction;
- normalized symbols, calls, relationships, dataflow, dependencies, and unresolved evidence;
- immutable semantic objects and snapshots;
- incremental repository analysis and publication.

### Arcana owns

- verified Lexicon snapshot ingestion;
- packed repository and call graphs;
- graph storage, overlays, compaction, traversal, impact, paths, call chains, architecture summaries, and unresolved-reference queries;
- optional semantic graph entry points through a generic external embedding endpoint.

### Warlock owns

- agent/task orchestration;
- context policy and task decomposition;
- lifecycle and tool exposure;
- any higher-level probabilistic workflow that consumes repository analysis.

### Ordinary development tools own

Literal source search, direct source inspection, and Git/history workflows remain ordinary tool responsibilities. They are not reimplemented in Lexicon merely because Grimoire previously wrapped them.

### Retired Grimoire ownership

The following are removed from the active product architecture rather than reassigned by default:

- heterogeneous source/document/symbol/graph discovery lanes;
- BM25 source indexing and repository retrieval policy;
- Grimoire documentation indexing;
- stable Grimoire handles and investigation sessions;
- `grimoire.discovery.v1` and Grimoire MCP;
- Grimoire repository-state orchestration and provider routing;
- Grimoire-specific embedding/runtime and Lodestone integration where no surviving owner requires them.

A future product may reintroduce one of these capabilities only with a new owner and evidence for the need. Retirement does not authorize silently moving them into Lexicon or Arcana.

## State consequences

Active generated repository-analysis state is:

- `.lexicon/` — Lexicon-owned semantic analysis state;
- `.arcana/` — Arcana-owned graph state and optional graph-vector state.

`.grimoire/` is retired generated state. Existing directories and historical artifacts may remain temporarily for migration, benchmark reproducibility, or cleanup, but they are not current product state.

Arcana remains bound to the exact Lexicon snapshot it consumed. Neither component mutates the other's private state.

## Product and release consequences

- `lexicon` and `arcana` become the normal public executables.
- The combined release, if retained, is a Lexicon + Arcana bundle rather than a Grimoire bundle.
- The Grimoire binary, Grimoire skill, Grimoire MCP surface, and Grimoire-specific package dependencies are removed from active releases.
- Repository and release naming should move away from Grimoire; `lexicon-arcana` is the intended repository/product-family name unless superseded by a later branding decision.
- Direct component use remains supported and is the normal product path.

## Agent-use consequences

The benchmark Lexicon + Arcana skill is the seed for the production agent contract:

- use Lexicon facts to locate semantic owners and relationships;
- use Arcana for bounded graph questions such as neighbours, impact, paths, and call chains;
- inspect source directly for implementation authority;
- use Git/history only when the task specifically requires historical evidence;
- stop investigation once the requested conclusion is supported.

Future agent architecture should solve open-ended exploration in Warlock's bounded inference/execution framework rather than by rebuilding Grimoire around another retrieval policy.

## Historical evidence

Existing Grimoire benchmarks, result directories, ADRs, and reports remain historical evidence. They should not be rewritten to pretend Grimoire never existed. Current documentation must distinguish those records from active architecture.

Future repository-agent comparisons should normally use Plain versus Lexicon + Arcana, with additional experimental conditions added explicitly when needed.

## Migration sequence

1. Update current architecture and product identity to Lexicon + Arcana and mark Grimoire historical.
2. Remove the Grimoire runtime, discovery internals, MCP, skill, repository state, and product-specific dependencies from active builds.
3. Change build, installer, release, CI, and repository naming to Lexicon + Arcana.
4. Promote the Lexicon + Arcana agent skill from benchmark-only assumptions to normal installed-state discovery.
5. Update Warlock and other downstream consumers to locate Lexicon and Arcana directly.
6. Preserve benchmark artifacts and historical documentation needed to explain or reproduce prior results.

## Alternatives considered

- Keep Grimoire as the lead product. Rejected because the simpler Lexicon + Arcana path already demonstrates the useful repository-analysis advantage while avoiding a large additional discovery/orchestration layer.
- Rename Grimoire but keep the same wrapper architecture. Rejected because this preserves the unnecessary ownership and complexity under a new name.
- Move source/document retrieval and sessions into Lexicon. Rejected because Lexicon's semantic-analysis boundary is already coherent and these are separate concerns.
- Merge Arcana into Lexicon. Rejected because language-semantic extraction and graph storage/querying remain distinct implementation, runtime, and state domains.

## Verification

The retirement is complete when:

- active build and release workflows no longer require or publish `grimoire`;
- current architecture documentation has no Grimoire product owner;
- Lexicon can initialize/scan and Arcana can synchronize/query without Grimoire;
- the production Lexicon + Arcana agent surface works without Grimoire environment variables or Grimoire state;
- downstream current products use direct Lexicon/Arcana boundaries;
- historical benchmark artifacts remain readable and clearly historical.

## Related docs

- [Component architecture](../architecture/components.md)
- [Analysis stack](../architecture/analysis-stack.md)
- [System overview](../architecture/system-overview.md)
- [Agent benchmark findings](../development/agent-benchmark-findings.md)

## Notes

The architectural simplification is deliberate: retire mechanics that no longer justify their ownership rather than absorbing them into the surviving products.