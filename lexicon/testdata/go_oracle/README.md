# Frozen Go semantic oracle

These repositories and canonical facts freeze the pre-cutover Go adapter behaviour used to validate the native Rust `GoAdapter`.

The retired standalone Go adapter implementation is intentionally no longer retained. The immutable goldens are the compatibility contract.

| Fixture | Frozen behaviour |
| --- | --- |
| `basic_calls` | single-module ownership, internal/external/builtin/conversion calls, recursion, concrete receiver dispatch, dynamic unresolved evidence |
| `relationships` | embedded named types, embedded interfaces, interface satisfaction, interface dispatch |
| `higher_order` | function values, callback parameters, closures, captures, immediate invocation |
| `dataflow` | typed reads/writes, compound access, fields/constants, lexical shadowing |
| `build_tags` | mutually exclusive build variants and AST-only callable contracts |
| `multi_module` | nearest-module ownership, cross-module calls/imports, `require`, plus local and external versioned `replace` observations |
| `parallel` | deterministic serial/sharded semantic analysis and reduction fan-in |

Fixture repositories live under `repositories/`. Canonical facts live under `golden/`.

Normal Rust Go-adapter tests run the native adapter against every fixture and compare the full semantic header plus all nodes, edges, unresolved records, spans, attributes, ownership, and identities against the frozen goldens. The `parallel` fixture is also checked across multiple worker/shard/fan-in configurations.

The goldens must not be regenerated from the current implementation. Any intentional semantic contract change requires an explicit review of the affected golden records and the versioned Lexicon facts contract.

## Retained real-repository calibration inputs

The completed Phase 16 calibration used:

- Demon Docs: `fa5ca9aea12e20c29c378d5d018647958b862cac`
- Space Rocks: `431625042dbdb1a884954cab6ec726413aa36e2b`

Those revisions remain historical calibration evidence; routine tests use the compact fixture corpus here.

## Phase 1 frontend-seam reconciliation

The canonical frontend seam was reconciled against this oracle after responsibility was split into:

```text
Go compiler frontend -> semantic observations -> Rust Go adapter policy -> facts-v1
```

Reconciliation covers all seven fixtures: `basic_calls`, `relationships`, `higher_order`,
`dataflow`, `build_tags`, `multi_module`, and `parallel`.

The adjudication result is deliberately simple:

- regressions: **0**;
- intentional facts-contract/oracle changes: **0**;
- obsolete non-contractual expectations: **0**;
- golden files changed: **0**.

The migrated seam therefore preserves the frozen facts contract exactly. Internal helper record shapes,
phase-specific migration structures, and transport mechanics are not compatibility requirements; only the
published semantic header and facts are compared. Any future difference is a regression until it is
explicitly reviewed as either an intentional contract change or an obsolete non-contractual expectation.

Full/incremental equivalence remains covered by the Go incremental tests. Deterministic scheduling remains
covered by the serial and alternate worker/shard/fan-in oracle runs.
