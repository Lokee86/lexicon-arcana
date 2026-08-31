# Lexicon documentation

This directory is the authoritative documentation set for the Lexicon application, architecture, operations, development workflow, and validation evidence.

## Documentation rules

Lexicon documentation follows these rules:

- describe implemented behavior as present tense;
- label dated measurements and validation records with their execution date;
- separate current guarantees from limitations and future possibilities;
- keep command and flag references synchronized with `internal/cli`;
- keep storage and exchange-format claims synchronized with `spec/` and `internal/objectstore`;
- state ownership boundaries explicitly instead of using broad subsystem descriptions;
- link to the owning document rather than duplicating detailed rules across files;
- treat generated evaluation artifacts as evidence, not hand-maintained documentation;
- update affected documentation in the same change as behavior or contract changes.

## Direct files

| File | Responsibility |
| --- | --- |
| [APPLICATION.md](APPLICATION.md) | CLI flags, repository discovery, state layout, scan behavior, watch mode, consumers, export, garbage collection, and recovery |
| [ARCHITECTURE.md](ARCHITECTURE.md) | System ownership, components, analysis lifecycle, incremental boundaries, concurrency, storage, and consumer integration |
| [MAINTAINER_MAP.md](MAINTAINER_MAP.md) | Short routing map from common changes to canonical documents and implementation boundaries |
| [DEVELOPMENT.md](DEVELOPMENT.md) | Prerequisites, build steps, focused tests, full verification, corpus validation, packaging, and documentation checks |
| [STATUS.md](STATUS.md) | Current implementation state, adapter coverage, validated behavior, and explicit limitations |
| [DEPENDENCY_SEMANTICS.md](DEPENDENCY_SEMANTICS.md) | Cross-language `depends-on` model and adapter-specific manifest coverage |
| [SEMANTIC_ACCEPTANCE.md](SEMANTIC_ACCEPTANCE.md) | Required observable gates for dataflow, dispatch, dependencies, runtime evidence, and integration |
| [SEMANTIC_CORPUS_VALIDATION.md](SEMANTIC_CORPUS_VALIDATION.md) | Dated real-repository validation record for the non-Go adapter corpus |
| [GO_ADAPTER_VALIDATION.md](GO_ADAPTER_VALIDATION.md) | Dated Go semantic-adapter validation record and its measured limits |
| [LOTUSSCRIPT_ADAPTER_VALIDATION.md](LOTUSSCRIPT_ADAPTER_VALIDATION.md) | Dated LotusScript calibration, validation, and holdout record |
| [RELEASE_PACKAGING.md](RELEASE_PACKAGING.md) | Distribution layout, build requirements, runtime requirements, and packaging command |
| [DOMAIN_ADAPTER_PROPOSAL.md](DOMAIN_ADAPTER_PROPOSAL.md) | Clearly labeled future proposal for broad Business/Construction adapters and learned Profiles; not implemented behavior |

## Related documentation

- [Root README](../README.md): project entry point and quick start.
- [Adapter index](../adapters/README.md): language adapter ownership and per-adapter documentation.
- [Contract index](../spec/README.md): public format and compatibility contracts.
- [Evaluation harness](../evaluation/README.md): repeatable real-repository acceptance tooling.
- [Tooling scripts](../tools/README.md): validation, reporting, runtime reconciliation, smoke tests, and packaging utilities.
- [Contributing](../CONTRIBUTING.md): required change and verification workflow.

## Placement rules

Place application behavior, operational procedures, architecture, development workflow, and current status in `docs/`.

Place normative machine-consumed format contracts in `spec/`. Place language-specific extraction details in the corresponding adapter README. Place dated generated results under `evaluation/validation/`; summarize only stable conclusions in documentation.

Do not put speculative product design into current-state documents. A future proposal must be clearly labeled and must not be presented as implemented behavior or an accepted compatibility guarantee.
