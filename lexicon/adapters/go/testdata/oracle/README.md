# Go migration oracle

These repositories freeze the legacy Go adapter's semantic behaviour before the native Rust boundary migration.

| Fixture | Frozen behaviour |
| --- | --- |
| `basic_calls` | single-module ownership, internal/external/builtin/conversion calls, recursion, concrete receiver dispatch, dynamic unresolved evidence |
| `relationships` | embedded named types, embedded interfaces, interface satisfaction, interface dispatch |
| `higher_order` | function values, callback parameters, closures, captures, immediate invocation |
| `dataflow` | typed reads/writes, compound access, fields/constants, lexical shadowing |
| `build_tags` | mutually exclusive build variants and AST-only callable contracts |
| `multi_module` | nearest-module ownership, cross-module calls/imports, `require`, plus parser-level local and external versioned `replace` observations |
| `parallel` | deterministic serial/sharded semantic analysis and reduction fan-in |

Canonical facts are committed under `../oracle_golden/`. Normal tests compare the current legacy adapter output byte-for-byte with those files. Regenerate only after an intentional, reviewed semantic change:

```text
LEXICON_UPDATE_GO_ORACLE=1 go test ./...
```

## Retained real-repository inputs

Phase 0 also pins the larger calibration inputs without copying them into this repository:

- Demon Docs: `fa5ca9aea12e20c29c378d5d018647958b862cac`
- Space Rocks: `431625042dbdb1a884954cab6ec726413aa36e2b`

Later differential calibration should use these revisions rather than whichever checkout happens to be current at that time.
