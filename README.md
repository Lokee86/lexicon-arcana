# Lexicon + Arcana

Lexicon and Arcana are complementary deterministic repository-analysis tools for humans, agents, and higher-level developer systems.

- **Lexicon** performs polyglot semantic analysis and publishes immutable facts about files, symbols, calls, dataflow, dependencies, and unresolved relationships.
- **Arcana** consumes a verified Lexicon snapshot and publishes a queryable repository/call graph with deterministic traversal, impact, paths, call chains, architecture summaries, and unresolved-reference queries.

The former **Grimoire** repository-discovery product is retired. Its runtime, source/document retrieval layer, stable handles, investigation sessions, MCP surface, repository state, and installed skill have been removed from the active source tree. Historical ADRs, reference material, fixtures, and benchmark results remain for evidence. See [ADR 0006](docs/decisions/0006-retire-grimoire-lead-with-lexicon-arcana.md).

## Product model

```text
repository source
    -> Lexicon
       semantic facts + immutable .lexicon snapshot
    -> Arcana
       repository/call graph + immutable .arcana snapshot
    -> humans, agents, Warlock, Pitlord, other consumers
```

Ordinary developer tools remain first-class alongside the analysis stack:

```text
source files + Git + ripgrep + IDE/file reads
```

Lexicon and Arcana are not intended to replace direct source inspection. They provide semantic and structural information that is expensive or error-prone to rediscover repeatedly.

## Why the pair

Lexicon owns language semantics. Arcana owns graph semantics. Keeping those domains separate lets each remain deterministic, independently testable, and independently usable.

Recent repository-agent experiments also support the simpler direct surface. On the current frozen Detekt investigation, completion-bounded Lexicon + Arcana reduced the agent from 25 to 19 started inference/tool items, 470.2s to 300.6s wall time, 1.906M to 1.472M total input tokens, and 195k to 113k fresh input tokens relative to plain repository exploration while preserving the accepted answer quality and grounding. These are task-specific measurements, not universal performance guarantees. See [Agent benchmark findings](docs/development/agent-benchmark-findings.md).

## Quick start

Build Lexicon and Arcana from this checkout:

```text
cd lexicon
go build -o ../bin/lexicon ./cmd/lexicon
cd ..
cargo build --release --locked --manifest-path arcana/Cargo.toml
```

Initialize repository semantic state:

```text
bin/lexicon init --repo /path/to/repository --adapters lexicon/adapters
```

Synchronize Arcana from the published Lexicon snapshot and optionally register it for later Lexicon publications:

```text
arcana/target/release/arcana sync \
  --lexicon /path/to/repository/.lexicon \
  --state /path/to/repository/.arcana \
  --register
```

Later semantic updates use the normal Lexicon lifecycle:

```text
bin/lexicon scan --repo /path/to/repository
```

## Lexicon

Lexicon currently provides semantic adapters for major repository languages including Go, Rust, Python, Ruby, JavaScript/TypeScript, Svelte, GDScript, C/C++, C#, Java, Kotlin, LotusScript, and conservative generic fallback surfaces.

Its stable responsibilities are:

- normalized semantic facts;
- immutable content-addressed fact objects;
- atomic repository snapshots;
- incremental analysis with deterministic fallback;
- adapter/runtime validation;
- direct export and consumer hooks.

See [Lexicon documentation](lexicon/docs/README.md) and [Lexicon contracts](lexicon/spec/README.md).

## Arcana

Arcana consumes Lexicon snapshots without rebuilding language parsers. Its stable responsibilities are:

- packed forward/reverse graph storage;
- immutable snapshots, overlays, and compaction;
- symbol and file resolution;
- neighbours and transitive impact;
- bounded paths and shortest call chains;
- unresolved-reference evidence;
- operational-role and architecture summaries;
- optional semantic graph entry points through an external OpenAI-compatible embedding endpoint.

The stable machine protocol identifier is `arcana.query.v1`.

See [Arcana documentation](arcana/docs/README.md) and [Lexicon integration contract](arcana/docs/LEXICON_CONTRACT.md).

## Agent use

The intended agent pattern is deliberately simple:

1. use Lexicon facts to find semantic owners and language relationships;
2. use Arcana only when a graph question is useful;
3. inspect source directly for implementation authority;
4. use Git/history only when historical evidence is actually required;
5. stop once the requested conclusion is supported.

The benchmark-only `evaluation/skills/lexicon-arcana/SKILL.md` is being promoted into the production agent surface as part of the retirement migration.

## State

| Directory | Owner | Status |
| --- | --- | --- |
| `.lexicon/` | Lexicon | Active |
| `.arcana/` | Arcana | Active |
| `.grimoire/` | Retired Grimoire layer | Migration/history only |

Arcana records the exact Lexicon snapshot it consumed. Neither component mutates the other's private state.

## Repository transition

The source repository is still named `grimoire` while the final naming/downstream migration remains incomplete. The active architecture and release surface are Lexicon + Arcana. Historical Grimoire ADRs, selected docs, evaluation fixtures, and benchmark artifacts are retained deliberately rather than deleted indiscriminately.

Historical Grimoire benchmark results remain intentionally preserved.

## Development

Current component-owned verification lives under the component roots:

```text
cd lexicon
go test ./...

cargo test --all-targets --locked --manifest-path arcana/Cargo.toml
```

The root workflow builds, tests, installs, and packages only Lexicon + Arcana. The retired Grimoire runtime is absent from the active source tree and is not a build, test, install, or release dependency.

## Architecture

- [ADR 0006 — Retire Grimoire and lead with Lexicon + Arcana](docs/decisions/0006-retire-grimoire-lead-with-lexicon-arcana.md)
- [Component architecture](docs/architecture/components.md)
- [Lexicon–Arcana analysis stack](docs/architecture/analysis-stack.md)
- [System overview](docs/architecture/system-overview.md)
- [Documentation index](docs/INDEX.md) — current L+A architecture/reference guidance and clearly marked historical Grimoire evidence
- [Lexicon reference](docs/reference/lexicon.md) — transitional shared reference; component docs are authoritative
- [Arcana reference](docs/reference/arcana.md) — transitional shared reference; component docs are authoritative
- [Architecture decisions](docs/decisions/INDEX.md)

## License

The source is available under the repository's [PolyForm Shield License 1.0.0](LICENSE.md). Competing products and services require a separate commercial license. See [LICENSING.md](LICENSING.md).