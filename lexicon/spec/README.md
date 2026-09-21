# Lexicon contracts

This directory owns Lexicon's normative cross-process and cross-repository compatibility contracts.

## Purpose

The specifications define the stable boundaries between:

- language adapters and the Lexicon application;
- Lexicon snapshots and snapshot consumers such as Arcana;
- static facts and optional runtime evidence;
- current binary writers and compatible legacy readers.

Implementation documentation may explain these formats, but `spec/` defines their required meaning.

## Does not own

This directory does not define:

- language-specific parsing strategies;
- CLI command behavior;
- scheduling heuristics;
- graph query APIs;
- retrieval ranking;
- documentation policy;
- future unimplemented formats.

Those concerns belong to the owning application or consumer documentation.

## Direct files

| File | Contract |
| --- | --- |
| [facts-v1.md](facts-v1.md) | Adapter JSONL header and records, stable IDs, ownership, source spans, relation semantics, unresolved evidence, incremental removals, and sorting |
| [objects-v2.md](objects-v2.md) | Current deterministic binary per-file/shared fact-object encoding with compact identities/references and v1/JSON reader compatibility |
| [objects-v1.md](objects-v1.md) | Legacy binary v1 fact-object encoding retained as a reader compatibility contract |
| [snapshots-v1.md](snapshots-v1.md) | Immutable snapshot manifests, object references, publication order, recovery, and consumer consistency |
| [runtime-evidence-v1.md](runtime-evidence-v1.md) | Optional run-specific observations and reconciliation with one static snapshot |

## Versioning policy

A versioned contract is immutable in meaning. Compatible clarifications may improve wording, but they must not change bytes, identity payloads, ownership, ordering, deletion semantics, or interpretation.

A breaking change requires a new versioned contract rather than silently redefining v1.

Readers may support multiple versions. Writers must emit one explicitly identified version. Existing immutable objects and manifests are never rewritten in place to simulate a format migration.

## Compatibility requirements

Contract changes must preserve or explicitly migrate:

- stable node identities;
- object and snapshot hash domains;
- canonical record ordering;
- definite versus possible relationships;
- unresolved evidence;
- source ownership and removal scope;
- atomic consumer visibility;
- legacy object decoding where documented.

Consumer-specific convenience must not be added to a shared contract unless it represents language-analysis evidence that other consumers can interpret independently.

## Change checklist

Before changing a contract:

1. identify whether the change is clarification, compatible extension, or breaking revision;
2. update the specification before or with implementation;
3. update every writer, reader, validator, exporter, and golden fixture;
4. test deterministic bytes and hashes;
5. test malformed and truncated input rejection;
6. document migration and legacy-read behavior;
7. coordinate external consumers;
8. update `docs/ARCHITECTURE.md`, `docs/STATUS.md`, and affected adapter documentation.

See [Development and verification](../docs/DEVELOPMENT.md) for the required test workflow.

## Placement rules

Normative format and semantic rules belong here. Examples should be minimal and must demonstrate the contract rather than a particular implementation.

Dated benchmark numbers, corpus results, implementation plans, and adapter-specific limitations do not belong in `spec/`.
