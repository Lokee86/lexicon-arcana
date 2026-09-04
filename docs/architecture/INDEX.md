# Architecture

Architecture documentation describes implemented ownership, data flow, state transitions, and the active retirement boundary.

- [Component architecture](components.md) — Lexicon and Arcana ownership, independent-use contract, dependency direction, state, and release boundary.
- [Analysis stack](analysis-stack.md) — repository source → Lexicon snapshot → Arcana graph → consumer lifecycle.
- [System overview](system-overview.md) — active deterministic analysis products, consumer model, state, and failure behavior.
- [Architecture decisions](../decisions/INDEX.md) — accepted rationale and superseding decisions, including Grimoire retirement.
- [Operations and trust boundaries](operations-and-trust.md) — transitional operations documentation; Grimoire-specific sections remain until implementation removal.
- [Prepared index](prepared-index.md) — historical/transitional Grimoire prepared-index architecture pending removal.
- [Grimoire maintainer map](maintainer-map.md) — historical/transitional ownership routing pending removal.

Active component contracts:

- [Lexicon contracts](../../lexicon/spec/README.md)
- [Lexicon architecture](../../lexicon/docs/ARCHITECTURE.md)
- [Arcana Lexicon contract](../../arcana/docs/LEXICON_CONTRACT.md)
- [Arcana architecture](../../arcana/docs/ARCHITECTURE.md)

Historical Grimoire discovery/MCP/reference documents remain in the tree during retirement so prior benchmark and design records stay understandable. They are not current product contracts after [ADR 0006](../decisions/0006-retire-grimoire-lead-with-lexicon-arcana.md).

Planned work belongs under [Planning](../planning/INDEX.md), not here.