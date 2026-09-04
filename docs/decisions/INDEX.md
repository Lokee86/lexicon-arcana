# Architecture decisions

These ADRs record consequential architecture choices and their supersession history. Proposed work belongs under [Planning](../planning/INDEX.md), not here.

## Current decisions

- [ADR 0006: Retire Grimoire and lead with Lexicon + Arcana](0006-retire-grimoire-lead-with-lexicon-arcana.md) — retire the downstream discovery product, preserve Lexicon and Arcana as the lead deterministic repository-analysis products, and do not transfer retired responsibilities without a new owner/evidence decision.
- [ADR 0003: Immutable generated state and atomic publication](0003-immutable-generated-state.md) — publish validated identity-bearing generations and reject stale or mismatched state; Lexicon/Arcana portions remain active.

## Historical or partially superseded decisions

- [ADR 0001: Monorepo with independent components](0001-monorepo-independent-components.md) — component independence remains valid; Grimoire-as-normal-entry-point and three-product packaging are superseded by ADR 0006.
- [ADR 0002: Progressive discovery instead of context packages](0002-progressive-discovery.md) — superseded by ADR 0006 as an active product contract; retained as Grimoire design history.
- [ADR 0004: Process and protocol boundaries between components](0004-process-protocol-boundaries.md) — Lexicon→Arcana immutable/process boundaries remain valid; Grimoire forwarding/discovery portions are superseded by ADR 0006.
- [ADR 0005: Lodestone owns native vector storage and exact search](0005-lodestone-vector-boundary.md) — retained as Grimoire history pending removal of any surviving Grimoire-specific vector dependency.

## Canonical current architecture

- [System overview](../architecture/system-overview.md)
- [Component architecture](../architecture/components.md)
- [Analysis stack](../architecture/analysis-stack.md)

An ADR is superseded only by a later explicit decision. Historical ADRs are not rewritten to erase the architecture they governed; their status and later superseding decision define which portions remain current.