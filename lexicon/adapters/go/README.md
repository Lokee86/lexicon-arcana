# Go adapter

The Go adapter scans a Go module or a repository containing multiple Go modules and emits deterministic Lexicon facts v1 JSONL. It discovers every `go.mod`, assigns each source file to its nearest module root, analyzes modules independently, and merges their facts into one repository graph. Semantic extraction combines `golang.org/x/tools/go/packages`, Go type information, SSA, and variable-type analysis.

## Run

```bash
go run . -repo /path/to/repository -output facts.jsonl
python ../../tools/validate_jsonl.py facts.jsonl
```

The standalone adapter also accepts `-workers`, `-shards`, and `-merge-fan-in`. Lexicon normally selects these values automatically from repository size and the available CPU budget. Semantic files are processed by a bounded worker pool, shard-local facts are combined through a deterministic reduction tree, and the final SSA/VTA pass resolves repository-wide dispatch. Output must remain byte-identical for every worker count and reduction shape.

## Modeled semantics

The adapter models:

- packages, files, types, functions, methods, tests, imports, and containment;
- internal, standard-library, external, and built-in callable contracts;
- definite calls through `calls`;
- conservative dynamic targets through `possible-calls`;
- interfaces and implementation relationships;
- function values, callbacks, method values, and returned function values;
- closures, nested calls, and captured variables;
- type conversions through `converts-to`;
- mutually exclusive build-tag declarations under one logical symbol identity;
- AST-only callable contracts for files excluded from the active host build.

The scanner excludes `.git`, `.worktrees`, `.workingtrees`, `.ddocs`, `.lexicon`, `.arcana`, `.grimoire`, `.pitlord`, `.cantrip`, `.homunculus`, `.incubus`, `.ritual`, `.warlock`, and `vendor` directories. Every node ID follows the Lexicon SHA-256 identity contract. File content IDs hash the unmodified file bytes.

`calls` indicates one definite callable contract. Multiple sound runtime targets remain explicit `possible-calls` relationships rather than being promoted to certainty.

## Dispatch and relationship boundaries

The semantic pass emits `extends` for named embedded/base relationships, `implements` for repository-local interface satisfaction, `uses-trait`/`includes` for embedded implementation relationships, and `overrides` from concrete methods to inherited or interface contract methods. Interface declarations are contracts only and are never runtime call targets. A proven concrete target emits `calls`; multiple concrete implementations emit `possible-calls`.

Reflection, `reflect`-derived calls, external packages, generated methods without repository evidence, unsafe runtime mutation, and otherwise dynamic function values remain unresolved or externally classified. Build-tag variants are merged only where their callable identity is stable.

## Canonical identities

The SHA-256 payload defined by the shared contract uses these Go identity strings:

| Kind | Canonical identity |
| --- | --- |
| repository | `repository:<root module path>` or `repository:<repository directory name>` for a multi-module root |
| directory | `directory:<repository-relative path>` |
| file | `file:<repository-relative path>` |
| module | `package:<import path>:<package name>` |
| namespace | `namespace:<import path or synthetic namespace>` |
| import | `import:<internal-or-external>:<import path>` |
| type | `type:<import path>:<type name>` |
| function | `function:<import path>:<function name>` |
| method | `method:<import path>:<receiver>.<method name>` |
| interface method | `interface-method:<import path>:<interface>.<method name>` |
| test | `test:<import path>:<test name>` |
| closure | `closure:<import path>:<file>:<line>:<column>` |
| captured variable | `variable:<owning import path>:<file>:<line>:<column>:<name>` |

Compiler-generated wrappers and external closures use deterministic `ssa-function:` identities. Synthetic built-in and type-expression nodes use stable language namespaces such as `go:builtins` and `go:types`. Absolute checkout paths are never part of an identity.

## Dependency semantics

The adapter emits repository `depends-on` edges for literal `go.mod` `require` directives, both single-line and parenthesized forms, and for literal `replace` directives. Each target is a facts-v1 `module` node using `dependency:go:<normalized-target>` identity; its synthetic path is `@dependencies/go/...`. Edges carry deterministic `category`, `constraint`, `source`, `optional`, `dev`, `build`, `peer`, and `path` attributes. Repository-local Go imports additionally emit module-to-module `depends-on` edges when the imported package is uniquely scanned, while preserving `imports`.

Malformed directives, dynamic module construction, and unresolved external package contents are not inferred. The adapter does not execute `go.mod` or install dependencies.
## Dataflow facts

The adapter emits conservative `reads` and `writes` edges from the containing callable to repository-local parameters, variables, constants, and fields. Assignments write, compound assignments and increment/decrement read and write, and initializer, argument, and return expressions contribute reads. Lexical shadowing is respected. Unresolved selectors, built-ins, external package values, reflection, and unsafe aliasing are omitted rather than guessed.

## Migration oracle

The legacy adapter is frozen as the semantic oracle for the native Rust Go-adapter migration. Permanent repositories live under `testdata/oracle/`; their byte-canonical facts-v1 outputs live under `testdata/oracle_golden/`. Normal Go tests rescan every fixture twice, require deterministic output, and compare it byte-for-byte with the committed golden. A separate parser check freezes literal `go.mod` `require` plus local and versioned-external `replace` observations even where final dependency-edge deduplication collapses transport detail.

The retained real-repository calibration inputs are pinned in the oracle README. Golden regeneration is an explicit migration-oracle maintenance action, not part of normal tests.

## Private semantic helper protocol

The migration boundary now includes a private version-1 semantic protocol implemented by `semantic_protocol_*.go`. It is deliberately independent of facts-v1 and persistence. Requests contain the canonical absolute repository root, the eligible repository-relative `.go`/`go.mod` inventory, optional module roots and module paths, and the normalized worker/shard/merge-fan-in execution parameters.

Responses are one JSON document containing typed declaration, relationship, call, dataflow, unresolved, and diagnostic records. Semantic records carry canonical Go identity strings such as `method:example.com/foo:Thing.Run`, repository-relative owner paths, and complete source spans. They do not carry Lexicon SHA IDs. Rust is responsible for translating those semantic identities into Lexicon node identities when the helper boundary is wired into the native adapter.

Decoding is fail-closed: unsupported protocol versions, unknown fields or record kinds, malformed repository paths, invalid semantic identities, and incomplete source spans are rejected. This protocol does not expose snapshots, `Analysis`, incremental publication, JSONL, object-store concepts, or other Lexicon persistence state. The legacy scanner is not routed through this protocol yet; this phase defines and tests only the private transport contract.

## Native Rust shell

The Rust `AdapterHost` now registers a native `GoAdapter` at `src/adapters/go/`. The shell retains adapter version `0.1.0`, fingerprints its Rust-side implementation together with the private helper version, and invokes language-native helpers through the shared internal helper runner rather than through the retired subprocess-adapter/facts handoff. Helper discovery is deterministic from an explicit environment override, the configured adapter root, or packaged executable adjacency.

The runner uses a single JSON request/response frame over stdin/stdout, validates the protocol handshake before decoding the typed response, bounds stderr capture, reports non-zero exits, and kills/reaps a helper that emits malformed or incompatible protocol data. At this migration stage only an empty semantic response is materialized into a native `Analysis`; real Go semantic records remain owned by the legacy oracle until the extraction phases begin.

## Code map

| Concern | Primary implementation | Related tests |
| --- | --- | --- |
| Entry, modules, and package loading | `main.go`, `adapter.go`, `modules.go` | adapter, build-variant, and package tests |
| AST declarations and base facts | `ast_*.go`, `facts.go`, `facts_json.go` | adapter and contract tests |
| Typed semantic model | `semantic.go`, `semantic_*.go`, `semantic_ssa.go` | semantic, invariant, and advanced-resolution tests |
| Private migration protocol | `semantic_protocol_*.go`; Rust mirror in `src/adapters/go/protocol.rs` | protocol round-trip and strict-validation tests |
| Native Rust shell/helper runner | `src/adapters/go/`, `src/adapters/helper.rs`, `src/adapters/helper_capture.rs` | Rust Go adapter shell/handshake tests |
| Calls and dataflow | `semantic_calls.go`, `semantic_dataflow.go` | call and dataflow tests |
| Dependencies | `dependencies.go` | package/dependency coverage |
| Parallel execution | `parallel.go`, `semantic_parallel.go` | `semantic_parallel_test.go` |
| Synthetic nodes and identities | `synthetic_nodes.go`, `semantic_nodes.go`, `semantic_symbols.go` | semantic identity tests |

The adapter models source-visible Go semantics; runtime reflection and unscanned external implementation remain outside its authority.
