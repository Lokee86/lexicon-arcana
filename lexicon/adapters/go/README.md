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

The SHA-256 payload defined by the shared contract uses these Go identity strings. During the native migration, `src/adapters/go/identities.rs` is the authoritative producer and validator for the semantic identity → Lexicon kind → SHA mapping; the legacy Go `hashIdentity` path remains only as oracle implementation evidence:

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

Compiler-generated wrappers and external closures use deterministic `ssa-function:` identities. Synthetic built-in and type-expression nodes use stable language namespaces such as `go:builtins` and `go:types`. Absolute checkout paths are never part of an identity. Rust rejects path-bearing semantic identities that contain absolute or Windows-style path material. Go package namespaces ending in `_test` are canonicalized to the corresponding internal package namespace only when the suffix-free namespace belongs to a discovered module, matching the legacy typed-semantic behavior.

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

Decoding is fail-closed: unsupported protocol versions, unknown fields or record kinds, malformed repository paths, invalid semantic identities, and incomplete source spans are rejected. This protocol does not expose snapshots, `Analysis`, incremental publication, JSONL, object-store concepts, or other Lexicon persistence state. Structural declaration records are now produced by the extracted `adapters/go-semantic/` helper and materialized into Lexicon facts only on the Rust side.

## Native Rust shell

The Rust `AdapterHost` now registers a native `GoAdapter` at `src/adapters/go/`. The shell retains adapter version `0.1.0`, fingerprints its Rust-side implementation together with the private helper version, and invokes language-native helpers through the shared internal helper runner rather than through the retired subprocess-adapter/facts handoff. Helper discovery is deterministic from an explicit environment override, the configured adapter root, or packaged executable adjacency.

The runner uses a single JSON request/response frame over stdin/stdout, validates the protocol handshake before decoding the typed response, bounds stderr capture, reports non-zero exits, and kills/reaps a helper that emits malformed or incompatible protocol data. Structural declaration records are materialized now; typed relationships, calls, SSA/VTA, captures, and dataflow remain on the legacy side until their later migration phases.

## Rust repository ownership

Rust now owns the Go adapter's repository boundary in `src/adapters/go/discovery.rs`, with module parsing and ownership isolated in `module_ownership.rs`. It walks the repository without following directory symlinks, applies the legacy Go adapter's permanent exclusion set, records every visible directory, and inventories only `.go` and `go.mod` inputs. Every discovered `go.mod` is parsed for its module path, files are assigned deterministically to the nearest containing module root, and repository identity remains the root module path when a root `go.mod` exists or the repository directory name for a multi-module root.

Only this Rust-owned sorted inventory and module table are sent across the private helper protocol. Rust also emits repository, directory, and file nodes plus their containment edges directly, including exact legacy node identities and file content hashes.

## Extracted structural helper

`adapters/go-semantic/` is the extracted language-native structural layer. It uses only Go's standard `go/parser`, `go/ast`, and token APIs and never emits Lexicon node IDs, facts-v1 records, hashes, snapshots, or persistence state. It parses exactly the Rust-supplied inventory, so it has no independent repository crawler. Every supplied `.go` file is parsed regardless of the active host build configuration, preserving inactive build-tag declarations.

The helper reports packages, imports, named types, interfaces through their named type declarations, functions, methods, tests, interface methods, and closures as canonical semantic declarations with owner paths, spans, containment metadata, and import classification. Rust translates those declarations into module/import/type/function/method/test nodes plus `contains`, `defines`, and `imports` relationships. The seven frozen migration fixtures compare the Rust-materialized declaration slice against the legacy oracle, including mutually exclusive build-tag variants.

## Code map

| Concern | Primary implementation | Related tests |
| --- | --- | --- |
| Entry, modules, and package loading | `main.go`, `adapter.go`, `modules.go` | adapter, build-variant, and package tests |
| AST declarations and base facts | `ast_*.go`, `facts.go`, `facts_json.go` | adapter and contract tests |
| Typed semantic model | `semantic.go`, `semantic_*.go`, `semantic_ssa.go` | semantic, invariant, and advanced-resolution tests |
| Private migration protocol | `semantic_protocol_*.go`; Rust mirror in `src/adapters/go/protocol.rs` | protocol round-trip and strict-validation tests |
| Native Rust shell/helper runner | `src/adapters/go/`, `src/adapters/helper.rs`, `src/adapters/helper_capture.rs` | Rust Go adapter shell/handshake tests |
| Rust repository discovery/module ownership | `src/adapters/go/discovery.rs`, `src/adapters/go/module_ownership.rs`, `src/adapters/go/facts.rs` | `src/adapters/go/discovery_tests.rs`, `discovery_boundary_tests.rs`, and legacy oracle goldens |
| Extracted structural semantic helper | `adapters/go-semantic/`, `src/adapters/go/protocol_records.rs`, `semantic_facts*.rs` | helper Go tests plus seven-fixture Rust declaration parity |
| Native Go identity authority | `src/adapters/go/identities.rs` | legacy identity vectors, `_test` namespace tests, and seven-fixture node-ID parity |
| Calls and dataflow | `semantic_calls.go`, `semantic_dataflow.go` | call and dataflow tests |
| Dependencies | `dependencies.go` | package/dependency coverage |
| Parallel execution | `parallel.go`, `semantic_parallel.go` | `semantic_parallel_test.go` |
| Synthetic nodes and identities | `synthetic_nodes.go`, `semantic_nodes.go`, `semantic_symbols.go` | semantic identity tests |

The adapter models source-visible Go semantics; runtime reflection and unscanned external implementation remain outside its authority.
