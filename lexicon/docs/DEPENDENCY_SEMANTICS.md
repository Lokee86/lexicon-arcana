# Lexicon dependency semantics

Parent index: [Lexicon Documentation](README.md)

## Purpose

This document defines Lexicon's normalized `depends-on` semantics and the evidence accepted from each language adapter and manifest format.

## Overview

Adapters emit conservative static dependency facts from source and declarative manifests. Dynamic construction, unresolved aliases, and runtime-only behavior remain unproven rather than being guessed.

All adapters emit dependency targets as facts-v1 `module` nodes. Synthetic targets use the canonical identity:

```text
dependency:<ecosystem>:<normalized-target>
```

Their paths are deterministic `@dependencies/<ecosystem>/...` paths unless the target is a repository-local path. Manifest or module nodes emit `depends-on` edges. Existing `imports` edges remain unchanged; a local `depends-on` edge is added only when the local target resolves uniquely to a scanned module/file.

Dependency edge attributes are deterministic and always use these scalar fields:

- `category`: `runtime`, `development`, `test`, `build`, `peer`, `optional`, `plugin`, `autoload`, or `local`;
- `constraint`: the literal declared version/range, or an empty string;
- `source`: the normalized manifest section or literal loader/source text;
- `optional`, `dev`, `build`, `peer`, and `path`: boolean classification flags.

The parsers read manifest text or JSON/TOML data only. They never execute manifests, resolve packages by installing them, or evaluate dynamic expressions. Malformed, computed, URL/VCS, and otherwise non-literal entries are omitted unless an adapter's existing import analysis already emits its normal unresolved classification. Ordering follows facts-v1 node/edge ordering and all repeated runs are byte-identical.

## Adapter coverage

- C/C++: quoted and system include directives, with repository-local headers resolved conservatively by relative path, exact path, or unique basename. Compiler package and linker dependencies are not inferred.
- Go: `go.mod` `require` entries, including blocks, `replace` entries, and repository-local Go imports.
- Python: literal `[project].dependencies`, `[project.optional-dependencies]`, `requirements*.txt` fallback, editable local requirements, and repository-local Python imports.
- Ruby: literal `Gemfile` `gem` calls, literal gemspec runtime/development dependency calls, `require_relative`, and `load`/`require` remain conservative for non-local targets.
- TypeScript/JavaScript/Svelte: `package.json` runtime/dev/peer/optional sections, `file:`/`link:` paths, relative imports, and unique `tsconfig`/`jsconfig` path-mapped imports.
- GDScript/Godot: enabled editor plugins, autoload entries, explicit `res://` resource/script paths in `project.godot`, and literal local `preload`/`load` references.
- Rust: Cargo normal/development/build/target/path dependencies through Cargo metadata, plus resolved local Rust module imports.

Unsupported forms include dynamic manifest construction, dependency execution, unresolved package-manager aliases, non-literal Ruby dependency calls, computed Godot paths, and unresolved Rust/Cargo metadata entries. These are not treated as proven dependency edges.

## Code map

| Language or boundary | Primary implementation | Related tests |
| --- | --- | --- |
| Go dependencies | Rust `src/adapters/go/dependencies.rs`; typed package loading in `adapters/go-semantic/` | Native Go adapter dependency/oracle tests |
| GDScript dependencies | `adapters/gdscript/dependencies.go`, `project_config.go` | GDScript adapter tests |
| Python dependencies | `adapters/python/lexicon_python/dependencies.py`, import extraction files | `adapters/python/tests/test_adapter.py` |
| Ruby dependencies | `adapters/ruby/dependencies.rb`, Ripper import/call extraction | `adapters/ruby/test/test_adapter.rb` |
| Rust dependencies | `adapters/rust/src/dependencies.rs`, `imports.rs`, `paths.rs` | Rust adapter tests and fixtures |
| JavaScript/TypeScript/Svelte dependencies | `adapters/typescript/src/dependencies.ts`, `ast-imports.ts`, `svelte.ts` | TypeScript adapter tests |
| Incremental dependency use | `internal/objectstore/dependencies.go`, `internal/scan/plan.go`, `internal/scope/repository.go` | object-store, scan-plan, and scope tests |

Dependency emitters describe evidence available to Lexicon's incremental planner. They do not claim runtime-complete dynamic dependency recovery.

## Related docs

- [Lexicon architecture](ARCHITECTURE.md)
- [Semantic acceptance gates](SEMANTIC_ACCEPTANCE.md)
- [Adapter documentation](../adapters/README.md)
- [Current status](STATUS.md)

## Notes

Dependency facts support retrieval and incremental planning but do not claim runtime-complete dependency recovery.
