# Reference

Reference pages are split into current Lexicon + Arcana contracts and historical Grimoire material retained for prior benchmark/design interpretation.

## Current reference

- [Installation](installation.md) — active L+A bundle installation, source builds, verification, and troubleshooting.
- [Lexicon](lexicon.md) — language-analysis commands, state, scan lifecycle, adapters, contracts, consumers, and diagnostics.
- [Arcana](arcana.md) — graph synchronization, snapshots, protocol operations, semantic vectors, diagnostics, and focused code map.

Component-local documentation remains authoritative for detailed current behavior:

- [Lexicon documentation](../../lexicon/docs/README.md)
- [Arcana documentation](../../arcana/docs/README.md)

## Historical Grimoire reference

The following pages describe retired Grimoire behavior and are preserved only as historical context for ADRs, evaluation fixtures, and benchmark reports:

- [Historical Grimoire CLI](cli.md)
- [Historical unified discovery contract](agent-query.md)
- [Historical Grimoire MCP/agent guide](agent-mcp.md)
- [Historical embedding-model integration](embedding-model.md)
- [Historical prepared indexing](indexing.md)
- [Historical document retrieval](knowledge.md)
- [Historical vector store](vector-store.md)

These pages are not active product contracts after [ADR 0006](../decisions/0006-retire-grimoire-lead-with-lexicon-arcana.md). No surviving component inherits their responsibilities automatically.

Architecture rationale belongs under [Architecture](../architecture/INDEX.md). Evaluation procedure belongs under [Development](../development/INDEX.md).
