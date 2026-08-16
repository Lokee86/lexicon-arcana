# Arcana application and operations reference

Parent index: [Arcana Documentation](README.md)

## Purpose

This document defines Arcana's executable commands, managed state, locking, publication, diagnostics, exit behavior, and operator-facing workflows.

## Overview

Arcana imports or synchronizes normalized facts, publishes immutable graph generations, serves deterministic queries, optionally builds semantic graph vectors, and runs graph-storage benchmarks.

This document describes the current `arcana` executable and its on-disk operating model. It is grounded in `arcana/src/main.rs`, `arcana/src/cli.rs`, the command-specific CLI modules, the library modules they call, and their focused tests.

Use the narrower documents for detailed contracts:

- [MAINTAINER_MAP.md](MAINTAINER_MAP.md) routes unfamiliar changes to the owning document and implementation boundary.
- [LEXICON_CONTRACT.md](LEXICON_CONTRACT.md) defines the Lexicon ingestion and incremental synchronization boundary.
- [repository-snapshots.md](repository-snapshots.md) defines standalone repository snapshot artifacts and changed-file updates.
- [vector-index.md](vector-index.md) describes semantic document, cache, and index behavior.
- [../README.md](../README.md) gives the product boundary, graph format overview, and benchmark rationale.

## Invocation, help, version, and exit behavior

Run Arcana through the built binary or Cargo:

```text
arcana <command> [options]
cargo run --manifest-path arcana/Cargo.toml -- <command> [options]
```

Implemented top-level behavior:

- `arcana` with no arguments prints the product name, description, and top-level usage, then succeeds.
- Bare `arcana -h` and `arcana --help` print the same help and succeed.
- Bare `arcana -V` and `arcana --version` print `Arcana <version>` and succeed. The build uses `GRIMOIRE_RELEASE_VERSION` when set, otherwise the Cargo package version.
- Only `benchmark` has command-specific `-h`/`--help` handling. Other commands treat `--help` as an unknown command flag.
- CLI parse errors print `arcana: <error>`, followed by top-level usage, to stderr and return exit status 2.
- Benchmark parse errors print `arcana benchmark: <error>`, followed by benchmark usage, and also return exit status 2.
- Runtime command failures print `arcana <command>: <error>` to stderr and return a failure status.
- Successful command summaries and query results are written to stdout. `protocol` reserves stdout for JSONL responses.

The parser accepts both `--option value` and `--option=value`. Duplicate single-value options are rejected. The boolean flags implemented by the general parser are `--reverse`, `--register`, and `--json`.

## Command summary

| Command | Input boundary | Output/state boundary | Key distinction |
| --- | --- | --- | --- |
| `import-facts` | One complete canonical TSV fact file | One new standalone repository snapshot directory | Full compilation; no `.arcana/CURRENT` management. |
| `update-facts` | A verified base `repository.manifest`, a complete replacement fact file, and changed paths | One new standalone repository snapshot directory | File-scoped replacement and cumulative edge overlay; node identities must remain unchanged. |
| `sync` | Lexicon’s immutable snapshot store | Managed `.arcana` state and atomic `CURRENT` | Chooses existing, overlay, or rebuild mode internally. |
| `query` | Explicit packed graph and catalogue files | Human-readable stdout | Direct packed-base lookup; does not open `repository.manifest` or apply an overlay. |
| `protocol` | One repository snapshot directory plus JSONL stdin | JSONL stdout | Long-lived stdin/stdout session over one validated, overlay-aware snapshot. |
| `vectorize` | Managed Arcana `CURRENT` plus an embedding endpoint | Cache and index under the Arcana state root | Explicit optional build; ordinary sync and graph queries remain embedding-free. |
| `semantic-query` | Managed Arcana `CURRENT`, existing vector index, and embedding endpoint | Human or JSON stdout | Semantic entry-point retrieval, not authoritative graph traversal. |
| `benchmark` | Synthetic graph options | Human report, optional CSV, temporary work files | Benchmarks generated datasets, not the current repository state. |

## `import-facts`

```text
arcana import-facts \
  --facts <FILE> \
  --output <NEW-DIRECTORY> \
  [--adapter <NAME>] \
  [--adapter-version <VERSION>]
```

Required options are `--facts` and `--output`. Adapter metadata defaults to `manual` and version `1`.

The command:

1. Refuses to run if the output path already exists.
2. Reads the complete fact file as UTF-8 text and parses Arcana’s canonical TSV fact schema.
3. Compiles stable node keys into deterministic snapshot-local dense IDs and packed edges.
4. Creates the output directory.
5. Writes `graph.arcana`, then `graph.manifest`, `catalogue.tsv`, `unresolved.tsv`, and `facts.tsv`.
6. Publishes `repository.manifest` after the component files have been written and validated.

Success output reports node, edge, and unresolved counts plus packed, metadata, and total byte counts. The resulting directory is standalone; the command does not create or update `.arcana/CURRENT`.

The output-directory rule is strict and tested: rerunning against the same path fails instead of replacing it. Compilation occurs before directory creation, but failures after creation do not use the cleanup path implemented by `update-facts`; operators should treat any failed import output as incomplete and remove or inspect it before retrying with the same path.

See [repository-snapshots.md](repository-snapshots.md) for the artifact contract rather than treating this command summary as the format specification.

## `update-facts`

```text
arcana update-facts \
  --base <repository.manifest> \
  --facts <COMPLETE-REPLACEMENT-FILE> \
  --changed <REPOSITORY-PATH> \
  [--changed <REPOSITORY-PATH> ...] \
  --output <NEW-DIRECTORY>
```

All four option classes are required, and at least one `--changed` path must be supplied. `--base` names the manifest file, not merely its containing directory.

The command opens and fully validates the base repository snapshot, parses the replacement fact file, partitions current and replacement facts by owner, replaces only the declared file partitions, and recompiles the visible repository. The replacement file is currently complete input; it is not a partial fact batch.

An incremental update succeeds only when the stable node-key-to-dense-ID mapping is unchanged. Edge additions and removals are computed relative to the original packed base, which makes the new `overlay.arcana` cumulative even when the input snapshot already had an overlay. Node additions, removals, or renames return an explicit rebuild-required error.

The output directory must not exist. A successful update contains the same repository metadata as a full import, copies the packed base to `graph.arcana`, and includes `overlay.arcana` only when there are edge changes. It preserves the base snapshot’s adapter name and adapter version. Success output reports changed-file, added-edge, removed-edge, and overlay-present values.

If writing the new generation fails after its directory is created, `update-facts` attempts to remove the entire output directory. It never mutates the base snapshot.

See [repository-snapshots.md](repository-snapshots.md) for ownership, cumulative-overlay, and rebuild boundaries.

## `sync`

```text
arcana sync \
  [--lexicon <DIRECTORY>] \
  [--state <DIRECTORY>] \
  [--register]
```

Defaults:

- Lexicon input: `.lexicon`
- Arcana state: `.arcana`
- Registration: disabled

`--lexicon` may point directly at a Lexicon store or at a repository root. Arcana uses the supplied path directly when it is named `.lexicon` or already contains both `CURRENT` and `snapshots`; otherwise it appends `.lexicon`. `--state` is used directly as the Arcana state root.

Sync is the managed publication command. It reads and verifies Lexicon’s `CURRENT`, content-addressed snapshot manifest, and referenced fact objects before publishing Arcana state. Its success summary includes the Lexicon snapshot ID, selected mode, registration flag, and compatibility-warning count.

The implemented modes are:

- `existing`: the target digest directory already has a matching `lexicon.snapshot` sidecar and a repository snapshot that opens successfully.
- `overlay`: a usable prior Lexicon/Arcana pair exists, no language-level shared object changed, changed file objects are known, and file-scoped planning preserves the node-ID mapping.
- `rebuild`: the first sync, a shared-object change, missing/unusable prior state, node-set change, failed incremental planning, or any path that does not qualify for the overlay fast path.

The mode is diagnostic output, not a user-selected option. All modes publish the same repository snapshot interface.

### Synchronization and publication lifecycle

For one invocation, Arcana:

1. Creates the state root if necessary and takes the exclusive state lock at `<state>/LOCK`.
2. Opens and verifies Lexicon `CURRENT` and prints any compatibility degradations to stderr.
3. Resolves the target as `<state>/snapshots/<lexicon-digest>`; the directory name omits the `sha256:` prefix.
4. Reuses a complete target or removes an incomplete target at that exact digest.
5. Builds a new generation under `<state>/snapshots/.<digest>.tmp-<pid>` when needed.
6. Writes `lexicon.snapshot` and optional `compatibility.warnings`, then renames the temporary directory to the immutable target path.
7. Atomically replaces `<state>/CURRENT` with the full `sha256:<digest>` Lexicon snapshot ID.
8. If requested, writes the Lexicon consumer registration.
9. Releases the lock when the command returns.

Component writers refuse in-place replacement, and repository readers revalidate manifests and component checksums. The managed `CURRENT` pointer is not changed before the target directory is assembled. Graph/repository build failures remove the temporary directory on the guarded build path.

One ordering detail matters operationally: consumer registration occurs after Arcana `CURRENT` publication. If registration itself fails, the command reports failure even though the new Arcana snapshot may already be current.

### Lexicon consumer registration

`arcana sync --register` writes `<lexicon-store>/consumers/arcana.json` with:

- registration version `1`;
- the absolute path of the currently running Arcana executable; and
- a one-shot `sync --lexicon <absolute-store> --state <absolute-state>` argument list.

The registered argument list does not include `--register`, so later Lexicon-triggered invocations synchronize without rewriting the registration on every scan. Registration uses the same synchronized atomic file-replacement helper as Arcana `CURRENT`.

Registration is a trigger path, not state transfer. Explicit `arcana sync` remains valid because immutable Lexicon snapshots are the durable handoff. See [LEXICON_CONTRACT.md](LEXICON_CONTRACT.md) for the producer/consumer contract and compatibility-warning policy.

## `query`

```text
arcana query \
  --graph <graph.arcana> \
  --catalogue <catalogue.tsv> \
  --name <EXACT-NAME> \
  [--reverse] \
  [--relation <RELATION>]
```

This is a small human-readable inspection command. It opens the packed graph and catalogue directly, looks up every exact name match, and prints each matching node followed by forward neighbors or, with `--reverse`, reverse neighbors. `--relation` filters the displayed adjacency.

The CLI parser currently accepts these relation filters:

```text
contains, defines, references, imports, calls, possible-calls, passes-to,
converts-to, implements, uses-trait, overrides, reads, writes, annotates,
extends, includes, depends-on, tests, documents, generates
```

Without a filter, the renderer can print any relation code known by the current repository model. An unknown filter is a parse error. No exact-name match is a successful result with a `no exact-name matches` message.

Important distinction: `query` does not open `graph.manifest` or `repository.manifest`. Passing `graph.arcana` from an overlaid generation queries only that packed base. Use `protocol` for validated, overlay-aware repository queries.

## `protocol`

```text
arcana protocol --snapshot <REPOSITORY-SNAPSHOT-DIRECTORY>
```

The argument is the directory containing `repository.manifest`. Arcana validates the complete repository snapshot once at startup, including the graph manifest, packed base, optional overlay, catalogue, facts, and unresolved store. Startup failure terminates the command with an `arcana protocol:` error.

After startup, the command reads JSON Lines from stdin until EOF and writes and flushes one JSON response per input line. Every response carries protocol ID `arcana.query.v1`, echoes the request `id` when parseable, and has either `ok: true` with `result` or `ok: false` with a structured error code/message. Invalid JSON receives an `invalid_json` response with a null ID. Per-request errors do not stop the server; this continuation behavior is covered by the protocol tests.

Implemented operations are:

```text
capabilities, search_nodes, resolve_symbol, resolve_file, list_nodes,
export_graph, neighbors, paths, reachability, impact, shortest_call_chain,
dead_symbols, operational_role, architecture_summary, unresolved, stats, diff
```

`capabilities` reports protocol identity, protocol version, implementation version, and the supported operation set. Integration consumers use it to reject incompatible Arcana binaries before treating a graph snapshot as queryable.

The request shapes, owning query modules, limits, and graph-export behavior are mapped in this document's code map and the protocol implementation. The protocol is overlay-aware and deterministic; it is the machine boundary for repeated exact graph work, not an HTTP service or semantic-vector endpoint.

## `vectorize`

```text
arcana vectorize \
  [--state <DIRECTORY>] \
  [--endpoint <URL>] \
  [--batch-size <N>] \
  [--batch-concurrency <N>]
```

Defaults are state `.arcana`, endpoint `http://127.0.0.1:9876/v1`, batch size `32`, and batch concurrency `1`. Batch values must be positive. The CLI uses the fixed current embedding model/identity/dimension constants; there are no CLI flags for replacing them.

The endpoint must use plain `http://`; the current client does not implement HTTPS. Arcana appends `/embeddings` unless the supplied endpoint already ends with that path.

The command resolves `<state>/CURRENT`, opens its verified repository snapshot, renders eligible graph documents, reuses valid content-addressed cache objects, embeds only missing objects, and materializes a snapshot-specific index. A valid exact index returns `mode=existing` without an embedding request. Incomplete, corrupt, stale, or policy-incompatible indexes are rebuilt.

Builds take an exclusive cache build lock and an exclusive per-snapshot/per-identity index lock. Successful batches persist cache objects immediately, so a retry can reuse work completed before a later batch failed. Publication uses a temporary index and rollback path; tests cover exact reuse, cross-snapshot reuse, interrupted-build resume, bounded concurrency, oversized-batch splitting, corrupt-object repair, and restoration of the previous index on publication failure.

Arcana checks `CURRENT` before publishing and again after replacement. If the graph changes during the build, the command fails with a retry diagnostic rather than claiming the index belongs to the new snapshot.

Success output reports mode, document and unique-vector counts, dimensions, embedded/reused vectors, exact-snapshot reuse, request count, index bytes, elapsed milliseconds, and output directory.

See [vector-index.md](vector-index.md) for the document/cache/index contract. Version-sensitive implementation constants live in `arcana/src/vector/`; in particular, current source uses semantic eligibility policy version 6 even though the focused document still describes version 5.

## `semantic-query`

```text
arcana semantic-query \
  --query <TEXT> \
  [--state <DIRECTORY>] \
  [--expected-snapshot <sha256:ID>] \
  [--endpoint <URL>] \
  [--limit <N>] \
  [--json]
```

`--query` is required. Defaults are state `.arcana`, the same plain-HTTP embedding endpoint as `vectorize`, limit `10`, and human-readable output. `--limit` must be positive.

The command requires an existing index matching Arcana `CURRENT`, the repository/graph snapshot IDs, embedding model identity, policy version, and dimensions. `--expected-snapshot` additionally requires `CURRENT` to equal that full `sha256:<digest>` value before the query proceeds.

Search takes a shared index lock, embeds the query, validates records and vector values during the scoring pass, sorts by descending score with node key as the deterministic tie-breaker, and checks `CURRENT` during and after the operation. A concurrent sync that changes `CURRENT` causes failure rather than mixed-snapshot results.

Human output starts with `semantic matches: <count>` and prints score, key, kind, path, and name per hit. `--json` emits one object with a `matches` array. Matches are graph entry points only; use exact protocol operations for relationships, paths, and impact.

## `benchmark`

```text
arcana benchmark \
  [--tier <small|medium|large|stress>] \
  [--topology <modular|entangled|hub-heavy|layered|dense-subsystem>] \
  [--queries <COUNT>] \
  [--samples <COUNT>] \
  [--seed <NUMBER>] \
  [--csv <PATH>] \
  [--work-dir <PATH>] \
  [--keep-files]
```

`arcana benchmark --help` prints this command’s focused usage. Defaults are tier `small`, topology `modular`, 1,000 queries, 3 samples, seed 0, work directory `target/arcana-benchmark`, no CSV, and temporary-file cleanup enabled. Zero query or sample counts parse as numbers but fail runtime configuration validation.

The benchmark generates one deterministic synthetic base, creates the standard mutation plans, and compares an overlay path with a rebuilt packed graph. It alternates measurement order by sample, validates visible dataset checksums, performs validated reopen measurements, and runs shared random, sequential, and hot-node forward/reverse query workloads. It does not read `.arcana/CURRENT` or benchmark a live repository.

Generated packed, overlay, and manifest files are removed when the run object is dropped unless `--keep-files` is set; the work directory itself remains. Human output reports medians, speedups, throughput, and file sizes. `--csv` creates parent directories as needed, writes raw samples including fingerprints with replacement semantics, and prints `raw samples: <path>` after success.

The benchmark tests cover option parsing and all tier/topology presets, plus checksum/query equivalence and default cleanup across every standard mutation pattern. See [../README.md](../README.md#benchmarks) for the benchmark’s purpose and workload overview.

## Managed repository state layout

A sync-managed state root has this shape; optional files/directories are marked:

```text
.arcana/
  CURRENT
  LOCK
  snapshots/
    <lexicon-snapshot-digest>/
      graph.arcana
      overlay.arcana              # optional
      graph.manifest
      catalogue.tsv
      unresolved.tsv
      facts.tsv
      repository.manifest
      lexicon.snapshot
      compatibility.warnings      # optional
  vector-cache/                   # created by vectorize
    <embedding-identity>/
      .build.lock
      objects/
        <digest-prefix>/
          <object-digest>.avec
  vectors/                        # created by vectorize; read by semantic-query
    <lexicon-snapshot-digest>/
      .<embedding-identity>.lock
      <embedding-identity>/
        manifest.json
        nodes.jsonl
        vectors.f32
```

`CURRENT` contains the full Lexicon `sha256:` snapshot ID; snapshot and vector directory names use only its 64-character digest. `LOCK` and vector lock files are coordination files and may remain present when unlocked. Snapshot directories and cache/index objects are content- or identity-bound and are not mutable working copies.

Standalone `import-facts` and `update-facts` outputs have the inner repository artifacts but do not automatically gain `CURRENT`, `LOCK`, `lexicon.snapshot`, vector storage, or managed snapshot placement.

## Locking and concurrency

- Sync uses a non-blocking exclusive lock on `<state>/LOCK`. A concurrent writer receives an `Arcana state is busy` error rather than waiting.
- `CURRENT` and Lexicon consumer registration are replaced through synchronized temporary files and atomic replacement.
- Packed graphs, overlays, graph manifests, and repository manifests refuse in-place replacement at their immutable publication paths.
- Vector builds serialize cache mutation with `.build.lock` and serialize index publication with an exclusive per-identity lock.
- Semantic queries take the same index lock in shared mode, allowing readers while excluding an index replacement.
- Vector build/query operations also compare `CURRENT` at critical points to detect cross-snapshot races.

These lock domains are separate: `.arcana/LOCK` protects synchronization/publication, while vector locks protect cache/index materialization and reading.

## Failure behavior and operational diagnostics

Arcana has no general log file. Operational evidence is command output, stderr, immutable manifests, and optional warning/index files.

Useful success diagnostics:

- `import-facts`: graph counts and byte totals.
- `update-facts`: changed-file and edge-delta counts plus overlay presence.
- `sync`: consumed Lexicon ID, `existing|overlay|rebuild` mode, registration state, and warning count.
- `query`: exact-match count and node/neighbor metadata.
- `protocol`: structured success/error response per request.
- `vectorize`: reuse/build mode, cache effectiveness, embedding request count, size, duration, and directory.
- `semantic-query`: bounded scored matches in human or JSON form.
- `benchmark`: mutation/reopen/query comparisons and optional raw CSV location.

Compatibility degradations are emitted as `arcana sync WARNING: ...` and persisted in the published snapshot’s `compatibility.warnings`. Hard Lexicon corruption or malformed required fields fail sync instead of becoming warnings.

For state diagnosis:

1. Read `.arcana/CURRENT` and verify that `snapshots/<digest>/lexicon.snapshot` contains the same full ID.
2. Open or inspect `repository.manifest` only as the publication root; its component checksums and cross-artifact validation are authoritative.
3. Check `compatibility.warnings` when the sync summary reports a nonzero warning count.
4. Use the sync mode to distinguish reuse, incremental overlay, and full rebuild.
5. Use `protocol` rather than direct `query` when overlay visibility matters.
6. Use `vectorize` output to distinguish exact-index reuse from cache-assisted rebuild, and treat stale/current-change errors as retry signals.

## Test evidence

The operational behavior above is covered by focused tests rather than inferred from intended design alone:

- `arcana/src/cli_tests.rs`: argument parsing/defaults, errors, import output, refusal to replace, exact query results, and missing-name behavior.
- `arcana/src/cli_update_tests.rs`: verified cumulative overlay publication and adapter metadata retention.
- `arcana/src/cli_sync_tests.rs`: direct managed Lexicon roots, rebuild/existing modes, registration shape, `CURRENT`, sidecar publication, and persisted compatibility warnings.
- Inline tests in `arcana/src/cli_sync_state.rs`: exclusive writer serialization and atomic replacement.
- `arcana/src/protocol/tests.rs`: snapshot/diff operations, graph export, visible overlays, request-error continuation, bounded analysis/traversal, and runtime/architecture evidence.
- `arcana/src/vector/index_tests.rs`: build/reuse/search, cache reuse and repair, resumability, concurrency bounds, batch splitting, and publication rollback.
- `arcana/src/benchmark/cli_tests.rs` and `arcana/src/benchmark/mutation_runner_tests.rs`: command options, valid presets, backend equivalence, and cleanup.

## Code map

| Command surface | Primary implementation | Related tests |
| --- | --- | --- |
| Parsing and dispatch | `src/cli.rs`, `src/main.rs` | `src/cli_tests.rs` |
| `import-facts` | `src/cli_commands.rs`, repository compiler and storage writer | repository and storage tests |
| `update-facts` | `src/cli_update.rs` | `src/cli_update_tests.rs` |
| `sync` and managed state | `src/cli_sync.rs`, `src/cli_sync_state.rs`, `src/lexicon/` | `src/cli_sync_tests.rs`, Lexicon module tests |
| Direct `query` | `src/cli_query.rs` | CLI and storage tests |
| JSONL protocol | `src/cli_protocol.rs`, `src/protocol/` | `src/protocol/tests.rs` |
| Vector commands | `src/cli_vectors.rs`, `src/vector/` | vector module tests |
| Benchmark command | `src/benchmark/`, `src/synthetic/` | benchmark and synthetic tests |

The direct `query` command is not the overlay-aware repository protocol. Language parsing remains outside Arcana.

## Related docs

- [Arcana architecture](ARCHITECTURE.md)
- [Lexicon ingestion contract](LEXICON_CONTRACT.md)
- [Repository snapshots](repository-snapshots.md)
- [Vector index](vector-index.md)

## Notes

Direct commands expose Arcana's specialist surface; Grimoire normally coordinates synchronization and protocol sessions automatically.
