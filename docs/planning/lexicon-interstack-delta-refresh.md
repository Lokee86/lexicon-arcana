# Lexicon Interstack Incremental Refresh — Phase 5 Repair

Parent index: [Planning](INDEX.md)
Status: Design captured; **not implemented**. Full pinned-Hermes Phase 5 remains unaccepted.

## Purpose

Make derived interstack refresh proportional to verified source/fact impact without losing cross-file and cross-language relationships or introducing a second graph owner.

## Overview

A pinned 544-file Hermes subtree proves that an ordinary two-file scoped Python analysis takes only 0.332 seconds while refreshing interstack from all 544 source files and 545 fact objects takes 6.008 seconds. A warm comment edit takes 12.531 seconds overall, exceeding the 10-second target. Simply skipping interstack when any source edit looks trivial is not safe: its detectors depend on global constants, callable identity and ownership, source line positions, HTTP providers, route names and cross-language consumers.

**Canonical owner:** the existing interstack subsystem owns derived relationship semantics; storage owns immutable indexes and snapshot lifetime. The scan engine consumes one narrow verified refresh decision. Do not create a CLI-side candidate cache, duplicate dependency graph, mutable previous/current owners, or a second contract detector.

## Required contract

- Track provenance for each interstack input contribution: normalized per-file candidate source evidence and exact source positions, canonical file/shared node facts, global constants, HTTP path providers and the consumers they can affect. Reuse the existing detector extraction rules rather than approximate regex subsets.
- Bind any persisted input fingerprint, reverse-reference lookup and derived output root to the exact published source/fact generation. Changed file content by itself is not proof that interstack output changed; a matching old/new node ID by itself is not proof of semantic stability.
- An unchanged-input decision must prove **all** relevant contributions unchanged, including callable positions and global-provider effects. If any evidence is unavailable, malformed, ambiguous, or from an older generation, run the existing complete refresh.
- Allow safe reuse of the immutable previous interstack language entry only after the exact-input proof and adapter-fingerprint check succeed. Never silently drop unrelated source edges or unresolved records.
- Follow normal pending publication, recovery, verified-root and consumer-pinned garbage-collection rules. No dual-read compatibility owner or periodically reconstructed global fact graph on the warm no-op path.

## Implementation sequence

1. **Oracle and baseline.** Freeze exact interstack (not merely Python) fact exports for pinned source states: unchanged, repeated EOF comments, a changed literal/global constant, HTTP provider/consumer changes, an altered callable span, file rename/add/remove and a cross-language contract. Record per-stage source, node-loading, index and publication reads.
2. **Internal input index.** Extract one storage-backed immutable per-source contribution fingerprint and dependency/reverse-reference lookup from the existing interstack detection inputs. The index builder belongs to the canonical subsystem and is constructed from already available full-analysis facts and source. Define version/generation validation and a one-time verified legacy bootstrap only if required.
3. **Verified no-op hard cut.** Compare exact changed contributions after language publication. If the old and new normalized interstack inputs are identical and there is no adapter drift or unproven global invalidation, carry forward the old immutable derived language entry. Otherwise use complete refresh. Avoid special cases for filename extensions or comments.
4. **Scoped re-resolution.** Only after the no-op gate and independent oracle are stable, propagate changed constants/providers through explicit reverse references and re-resolve affected consumers. Unknown relationships take complete fallback. Keep one canonical output materialization.
5. **Production acceptance.** Re-run the bounded 544-file edited-state full oracle and verify no unrelated node objects or full source walk on proven no-op changes. Then scale through larger pinned samples before the exact full 7,114-file paired Hermes release gate (cold publication, ≤5 s unchanged, ≤10 s and ≤20% of full adapter for one-file edit, ten-file bound, complete semantic parity, RSS and recovery).

## Verification

A passing performance sample alone is insufficient. Tests must compare canonical full-versus-incremental derived facts, including edge attributes, unresolved candidates, global provider invalidation and exact source spans. Corrupt or cross-generation input proofs must fail closed. Preserve the old 544-file result as a negative regression until the new path demonstrates an exact semantic oracle and actual bounded reads.

## Related docs

- [Scaling and publication evidence](../development/lexicon-phase5-scaling-publication-2026-10-01.md)
- [Current incremental repair plan](lexicon-incremental-scan-performance.md)
- [Phase 5 original acceptance](../development/lexicon-incremental-phase5-2026-10-01.md)

## Notes

Interstack dependency provenance is broader than the ordinary one-hop language dependency index. Reuse proven source/fact identities, but do not treat the language planner's context set as sufficient to determine cross-language contract impact.
