# Lexicon + Arcana documentation

This tree contains shared architecture, decisions, current release/verification guidance, research evidence, historical Grimoire material, and transition planning. Component-specific current behavior lives under the Lexicon and Arcana source roots.

## Current architecture and product guidance

- [Architecture](architecture/INDEX.md) — active Lexicon → Arcana ownership, data flow, state, and consumer boundaries.
- [Architecture decisions](decisions/INDEX.md) — accepted and superseded decisions, including [ADR 0006](decisions/0006-retire-grimoire-lead-with-lexicon-arcana.md).
- [Reference](reference/INDEX.md) — current installation/Lexicon/Arcana reference plus clearly marked historical Grimoire pages.
- [Lexicon documentation](../lexicon/docs/README.md) — semantic analysis, adapters, snapshots, contracts, operations, and verification.
- [Arcana documentation](../arcana/docs/README.md) — graph ingestion, packed storage, snapshots, protocol operations, vectors, and verification.

## Evidence and development

- [Development](development/INDEX.md) — active verification/release practice plus retained benchmark/research evidence.
- [Agent benchmark findings](development/agent-benchmark-findings.md) — historical Grimoire and current Lexicon + Arcana experiment results.
- [Limits](limits/INDEX.md) — current L+A and transition limitations.
- [Planning](planning/INDEX.md) — unfinished retirement and product-family work.
- [Documentation policy](documentation-policy.md) — current ownership and historical-material rules.
- [Documentation procedure](documentation-procedure.md) — required documentation/update verification workflow.

## Historical Grimoire material

Historical Grimoire ADRs, benchmark results, reports, evaluation fixtures, and selected reference/architecture pages remain in the repository so prior experiments remain interpretable. They do not imply an active Grimoire runtime or product contract.

Current ownership is:

| Product/surface | Owns |
| --- | --- |
| Lexicon | Language semantics, normalized facts, immutable semantic snapshots |
| Arcana | Verified Lexicon ingestion, repository/call graph storage and graph queries |
| Shared root tooling | L+A build, install, release, documentation, policy, and benchmark composition |
| Warlock/consumers | Agent/task/context orchestration and higher-level workflow |
| Ordinary developer tools | Literal search, direct source inspection, Git/history |
| Grimoire | Retired; historical evidence only |

## Documentation rules

1. Current architecture/reference pages describe Lexicon + Arcana, not the retired Grimoire runtime.
2. Historical benchmark, ADR, and report material retains original names/results where needed for evidence.
3. Component-specific behavior belongs with the owning component.
4. Planned work must not be described as implemented.
5. Exact commands, state formats, and protocol fields must match current component code.
6. Retiring a capability does not silently transfer its ownership to a surviving component.

When behavior changes, update the owning component documentation and any shared architecture/decision page affected by the change.
