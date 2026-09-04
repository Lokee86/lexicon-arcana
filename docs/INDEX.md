# Lexicon + Arcana documentation

This tree contains shared architecture, decisions, development evidence, historical Grimoire material, and transition planning. Component-specific current behavior lives under the Lexicon and Arcana source roots.

## Current architecture

- [Architecture](architecture/INDEX.md) — active Lexicon→Arcana ownership, data flow, state, consumer boundaries, and transitional Grimoire retirement notes.
- [Architecture decisions](decisions/INDEX.md) — accepted and superseded decisions, including [ADR 0006](decisions/0006-retire-grimoire-lead-with-lexicon-arcana.md).
- [Lexicon documentation](../lexicon/docs/README.md) — semantic analysis, adapters, snapshots, contracts, operations, and verification.
- [Arcana documentation](../arcana/docs/README.md) — graph ingestion, packed storage, snapshots, protocol operations, vectors, and verification.

## Evidence and development

- [Development](development/INDEX.md) — tests, corpora, benchmark procedures, implementation evidence, and historical outcome interpretation.
- [Agent benchmark findings](development/agent-benchmark-findings.md) — historical Grimoire and current Lexicon + Arcana experiment results.
- [Limits](limits/INDEX.md) — constraints and failure modes; Grimoire-specific entries are transitional until retirement cleanup.
- [Planning](planning/INDEX.md) — unimplemented work and current migration planning.

## Historical Grimoire references

The existing `reference/`, prepared-index, MCP, discovery-contract, maintainer-map, and Grimoire package README documents remain temporarily so historical benchmarks and source still have understandable documentation during removal.

They are not active product contracts after [ADR 0006](decisions/0006-retire-grimoire-lead-with-lexicon-arcana.md). Current product ownership is:

| Product/surface | Owns |
| --- | --- |
| Lexicon | Language semantics, normalized facts, immutable semantic snapshots |
| Arcana | Repository/call graph ingestion, storage, traversal, impact, paths, architecture queries |
| Warlock/consumers | Agent/task/context orchestration and higher-level workflow |
| Ordinary developer tools | Literal search, direct source inspection, Git/history |
| Grimoire | Retired; historical/transitional source only |

## Documentation rules

1. Current architecture pages describe Lexicon + Arcana, not the retired Grimoire product path.
2. Historical benchmark and ADR material retains original names and results.
3. Component-specific behavior belongs with the owning component.
4. Planned migration work must not be described as already implemented.
5. Exact commands, state formats, and protocol fields must match current component code.
6. Retiring a capability does not silently transfer its ownership to a surviving component.

When behavior changes, update the owning component documentation and any shared architecture/decision page affected by the change.