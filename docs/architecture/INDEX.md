# Architecture

Architecture documentation describes the active Lexicon + Arcana ownership, data flow, state boundaries, and consumer model.

## Current architecture

- [Component architecture](components.md) — Lexicon and Arcana ownership, independent-use contract, dependency direction, state, and release boundary.
- [Analysis stack](analysis-stack.md) — repository source → Lexicon snapshot → Arcana graph → consumer lifecycle.
- [System overview](system-overview.md) — deterministic analysis products, consumer model, state, and failure behavior.
- [Architecture decisions](../decisions/INDEX.md) — accepted rationale and superseding decisions, including Grimoire retirement.

Active component contracts:

- [Lexicon contracts](../../lexicon/spec/README.md)
- [Lexicon architecture](../../lexicon/docs/ARCHITECTURE.md)
- [Arcana Lexicon contract](../../arcana/docs/LEXICON_CONTRACT.md)
- [Arcana architecture](../../arcana/docs/ARCHITECTURE.md)

## Historical Grimoire architecture

The following pages remain only to explain prior designs, ADRs, and benchmark evidence:

- [Historical operations and trust boundaries](operations-and-trust.md)
- [Historical prepared index](prepared-index.md)
- [Historical Grimoire maintainer map](maintainer-map.md)

They are not current product contracts after [ADR 0006](../decisions/0006-retire-grimoire-lead-with-lexicon-arcana.md). The retired implementation they describe has been removed from the active source tree.

Planned work belongs under [Planning](../planning/INDEX.md), not here.
