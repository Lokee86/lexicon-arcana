# Lexicon + Arcana Pitlord policy

This directory owns the repository-level architecture policy for the Lexicon + Arcana product family.

## Policies

- `repository.json` protects the retirement decision, component independence, active release boundaries, generated-state ownership, and Lexicon consumer lifecycle.
- `policy.json` is the canonical composed policy entry point.

Policy belongs here rather than in a repository-specific checker. Pitlord owns rule validation, repository evidence, and diagnostics. Focused Go, Rust, and workflow tests remain the executable proof for detailed component behavior.

## Check

The normal root workflow and CI run:

```text
pitlord validate --policy tools/pitlord/policy.json
pitlord check --repo . --policy tools/pitlord/policy.json
```

Set `PITLORD` when the executable is not available through `PATH` or the sibling Pitlord checkout. The root workflow currently expects Pitlord `v0.1.2`.

## Ownership

The policy declares invariants. The owning implementation and architecture documents remain the source of truth for why those invariants exist. Retired Grimoire runtime/MCP/discovery paths are deliberately not protected as active compatibility surfaces.
