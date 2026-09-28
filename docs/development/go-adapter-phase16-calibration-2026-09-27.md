# Go Adapter Phase 16 Calibration — 2026-09-27

Parent index: [Development Documentation](INDEX.md)

## Status

**Phase 16 — real-repository calibration: COMPLETE.**

The native Rust `GoAdapter` plus private Go semantic helper now satisfies the retained legacy Go adapter on the Phase 15 differential fixture matrix, the pinned Demon Docs and Space Rocks repositories, the Lexicon self-hosting case, deterministic repeat runs, and Arcana ingestion/query acceptance.

Real-repository calibration used **4 workers / 8 logical shards / merge fan-in 4**.

## Regression found during calibration

Space Rocks exposed a semantic regression introduced when Go semantic analysis was bounded by module during performance restoration.

The module-local helper passes no longer preserved two legacy merge rules across module boundaries:

- a non-interface callsite already resolved by an earlier module pass must remain authoritative rather than being widened by a later package/test view;
- when an interface invoke already has a concrete target, an `interface-method:` contract target returned by VTA must not survive beside it.

Before the fix, Space Rocks had 33 fewer `calls` edges and 93 extra `possible-calls` edges on the native path.

The fix preserves bounded module lifetimes. Repository state carries only a compact set of already-resolved callsite keys between module passes; it does not retain prior package/type graphs. Interface reconciliation also discards contract targets once concrete targets are present.

Focused helper regressions cover both cases.

## Verification gates

- Go semantic-helper suite: green.
- Native Rust Go-adapter suite: **33/33 green**.
- Phase 15 live legacy/native fixture differential matrix: green.
- Parallel execution-shape determinism: green.
- Current legacy Go oracle rebuilt from source before final real-repository comparison.

## Real-repository results

| Case | Revision | Records | Calls | Possible calls | Unresolved | Raw SHA-256 | Result |
| --- | --- | ---: | ---: | ---: | ---: | --- | --- |
| Demon Docs | `fa5ca9aea12e20c29c378d5d018647958b862cac` | 140,018 | 20,057 | 404 | 0 | `ae3064c2085aa479b058f026a26d7dc3ce2dd05ebee036023c94e7543e650f0e` | exact legacy/native parity; repeated native output identical |
| Space Rocks | `431625042dbdb1a884954cab6ec726413aa36e2b` | 199,132 | 25,243 | 1,142 | 7 | `75ed77d31d74e78ae4943dca60ddb2ab0dcf02b64f4788788b810f6ac0dc13ec` | exact legacy/native parity; repeated native output identical |
| Lexicon self-host | Phase 16 worktree at pre-commit base `2348089` plus the Phase 16 fix | 122,972 | 15,837 | 60 | 21 | `232610807472d13800bc76c2a0e1ad4f87adc5c1f9dbaa77055f831ecd7caeab` | exact legacy/native parity |

Semantic comparison hashes were also identical for every legacy/native pair:

- Demon Docs: `d964ffe27fa80b947b0f808bf6846ab8e2bc05aa42ca60cff1d9efb553ea5717`
- Space Rocks: `bd0109fcfe74f14789a843685cb27935444d8ef20f0d3c636c84113fa3f26f25`
- Lexicon self-host: `c872514f84445a4cde42b0817f4a99c69aa0a5639904da71109950be4cfcee57`

The Space Rocks unresolved set remained exactly **2 `dynamic-target` + 5 `missing-target`**. The self-host unresolved set remained exactly **6 `dynamic-target` + 15 `missing-target`**.

## Arcana acceptance

The exact Space Rocks native facts were imported through Arcana successfully:

- source facts: 34,920 nodes and 7 unresolved references;
- compiled graph: 34,920 nodes and 98,953 graph edges;
- `graph.arcana`: 1,746,320 bytes;
- `repository.arcana`: 15,452,200 bytes.

The `arcana.query.v1` protocol then passed:

- `stats`;
- exact `resolve_symbol` for `SpawnBullet` in `services/game-server/internal/game/control_spawn.go`;
- `search_nodes` for `SpawnBullet`.

The exact symbol query resolved the expected method with identity
`sha256:b36b5fdbc1e186cdcdc3042ce96131f85b8f2aa5a50b840ce98f577e7d464f83`.

Arcana's compiled edge count is its deduplicated graph representation and is not expected to equal the raw facts-v1 edge count.

## Conclusion

Phase 16 closes with **no unexplained semantic regression** on the required fixture, pinned real-repository, self-host, determinism, and Arcana gates.

The next migration phase is **Phase 17 — cutover**: make the native Rust `GoAdapter` authoritative and remove the standalone legacy Go facts-v1 adapter from the production runtime path while retaining it only as migration oracle/history until final cleanup.
