# Lexicon fact contract v1

Lexicon adapters emit UTF-8 JSON Lines. Every line is one JSON object. Object keys must be serialized in lexicographic order and records after the header must use the canonical ordering defined below.

This contract is the shared language-analysis boundary for Arcana and other Warlock tools. It is not Arcana's packed storage representation.

## Header

The first record is:

```json
{"adapter_version":"0.1.0","language":"go","mode":"full","record":"lexicon","repository":"example/module","schema_version":1}
```

Required fields:

- `record`: always `lexicon`;
- `schema_version`: integer `1`;
- `adapter_version`: adapter release version;
- `language`: canonical lower-case language name;
- `repository`: repository or module identity discovered by the adapter.

Optional fields:

- `mode`: `full` or `incremental`; omitted means `full` for v1 compatibility;
- `changed_files`: sorted repository-relative paths owned by an incremental emission;
- `removed_files`: sorted repository-relative paths removed since the previous emission;
- `shared_complete`: boolean declaring that all emitted records without file ownership are the complete replacement shared set for this language view.

A `full` stream describes one complete repository view. An `incremental` stream is valid only when both `changed_files` and `removed_files` are present, even when either array is empty. Consumers that replace shared synthetic records require `shared_complete: true`; otherwise they must retain previous shared records and apply only identity-matched updates.

## Stable identities

Stable IDs use lower-case SHA-256:

```text
sha256:<64 hexadecimal characters>
```

A node identity is the digest of this UTF-8 string:

```text
lexicon:v1\0<language>\0<kind>\0<canonical identity>
```

A content identity is the SHA-256 digest of the unmodified file bytes. Canonical identities must not include absolute checkout paths. Each adapter documents its language-specific canonical identities while using the common node kinds whenever semantics align.

Arcana compacts these transport identities into its own packed IDs during import. The original Lexicon identity remains the authoritative cross-tool identity.

## Source spans and provenance

Spans use one-based inclusive start positions and one-based exclusive end positions:

```json
{"end_column":9,"end_line":12,"path":"src/example.go","start_column":1,"start_line":12}
```

A missing or synthetic span is omitted rather than encoded with sentinel values.

Records may include `owner`, a repository-relative source file path. Ownership is determined in this order:

1. explicit `owner`;
2. `span.path`;
3. for file nodes, the node's `path`;
4. for edges or unresolved records, the owning source node.

Directory, repository, external, built-in, and other synthetic records may have no file owner. Provenance attributes may contain deterministic scalar values or sorted scalar arrays, including parser confidence, build configuration, generated status, or adapter-specific evidence.

## Node record

```json
{"attributes":{},"content_id":"sha256:...","id":"sha256:...","kind":"file","name":"example.go","owner":"src/example.go","path":"src/example.go","qualified_name":"src/example.go","record":"node","span":{...}}
```

Required fields:

- `record`: `node`;
- `id`;
- `kind`;
- `name`;
- `path`;
- `qualified_name`.

Optional fields:

- `content_id` for files;
- `span`;
- `owner`;
- `attributes`.

Common node kinds:

- `repository`;
- `directory`;
- `file`;
- `module`;
- `namespace`;
- `symbol`;
- `type`;
- `interface`;
- `protocol`;
- `trait`;
- `function`;
- `method`;
- `constructor`;
- `field`;
- `variable`;
- `constant`;
- `parameter`;
- `import`;
- `test`;
- `http-endpoint`;
- `message-channel`;
- `config-key`.

Adapters may add language-specific kinds, but consumers may reject kinds they do not support.

Language-neutral semantic protocol nodes use the versioned [semantic fact contract v1](semantic-facts-v1.md). The shared validator enforces its capability, error-handler/action, error-flow, and outcome-obligation vocabularies in addition to the generic facts-v1 structure.

## Edge record

```json
{"owner":"src/example.go","record":"edge","relation":"calls","source":"sha256:...","span":{...},"target":"sha256:..."}
```

Required fields:

- `record`: `edge`;
- `source`;
- `target`;
- `relation`.

Optional fields:

- `span`;
- `owner`;
- `attributes`.

Common relations:

- `contains`;
- `defines`;
- `imports`;
- `calls`;
- `possible-calls`;
- `passes-to`;
- `converts-to`;
- `references`;
- `extends`;
- `implements`;
- `uses-trait`;
- `overrides`;
- `reads`;
- `writes`;
- `annotates`;
- `includes`;
- `depends-on`;
- `tests`;
- `documents`;
- `generates`;
- `calls-endpoint`;
- `handled-by`;
- `publishes`;
- `consumes`;
- `reads-config`.

`calls` means one definite statically identified callable contract. Multiple sound runtime targets must use `possible-calls`. Consumers must not silently merge `possible-calls` into definite calls.

`passes-to` records a proven direct value-flow step from a caller-side parameter, local, field, or constant to a callee parameter. It does not imply mutation, ownership transfer, aliasing beyond the call boundary, or flow through unsupported compound expressions.

Cross-stack relationships use normalized synthetic contract nodes. `calls-endpoint` connects a request-producing callable to an `http-endpoint`; `handled-by` connects that endpoint to its repository-local handler. `publishes` and `consumes` connect callables through a `message-channel`. `reads-config` connects a callable to a shared `config-key`. These records may be emitted in a synthetic `interstack` library while referencing stable node IDs owned by other language libraries in the same snapshot. Consumers must validate those references across the complete exported snapshot, not only within one JSONL file.

## Unresolved record

```json
{"expression":"factory()","owner":"src/example.go","reason":"dynamic-target","record":"unresolved","relation":"calls","source":"sha256:...","span":{...}}
```

Required fields:

- `record`: `unresolved`;
- `source`;
- `relation`;
- `expression`;
- `reason`.

Optional fields:

- `candidate_name`;
- `candidate_namespace`;
- `span`;
- `owner`;
- `attributes`.

Common reasons:

- `missing-target`;
- `ambiguous-target`;
- `unsupported-form`;
- `dynamic-target`;
- `external-target`;
- `builtin-target`;
- `generated-target`;
- `unsupported-macro-expansion`;
- `macro-argument-mismatch`;
- `macro-expansion-cycle`;
- `macro-expansion-depth`.

## Incremental ownership and removals

A full stream replaces the consumer's complete logical fact set.

For an incremental stream:

- every emitted node, edge, and unresolved record must resolve to an owner in `changed_files`;
- all previously stored records owned by `changed_files` are removed before the new records are applied;
- all records owned by `removed_files` are removed and no replacement records for those files may appear;
- records owned by files not listed in either array remain unchanged;
- when `shared_complete` is absent or false, shared synthetic records without file ownership are retained unless their identity is explicitly re-emitted with changed attributes;
- when `shared_complete` is true, all previous shared synthetic records are removed before the emitted shared records are applied;
- removing a node also removes incident edges and unresolved records whose source no longer exists;
- an adapter must emit a full stream when it cannot determine file ownership safely.

These rules make absence meaningful only inside the declared incremental scope. Absence outside that scope is never a deletion signal.

## Sorting

After the header:

1. nodes by `(id, kind, path, qualified_name)`;
2. edges by `(source, target, relation, span path/start/end)`;
3. unresolved records by `(source, relation, expression, reason, span path/start/end)`.

All node records precede edge records; all edge records precede unresolved records. Arrays inside records must be sorted unless their order is semantically meaningful and documented.

## Consumer requirements

Consumers must:

- validate the header and ID syntax;
- reject duplicate node IDs with conflicting records;
- reject edges or unresolved references to unknown source nodes;
- preserve definite versus possible relationship semantics;
- normalize repository paths without accepting absolute paths or parent traversal;
- apply incremental removals only within the declared ownership scope;
- retain source spans and provenance when their internal model supports them.
