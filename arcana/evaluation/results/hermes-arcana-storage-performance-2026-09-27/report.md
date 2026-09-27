# Hermes Arcana storage and sync performance — 2026-09-27

Parent reference: [Repository snapshots and incremental updates](../../../docs/repository-snapshots.md)

## Scope

This evidence measures the Phase 8 Arcana storage/sync restoration against the existing Hermes Lexicon state. All Arcana runs used the repository-built release binary. No Hermes source scan or Lexicon rebuild was performed for these measurements; Arcana consumed already-published Lexicon objects.

The evidence separates two questions:

1. whether the published Arcana representation is compact and semantically identical;
2. whether managed rebuild/overlay runtime memory is no longer pathological.

## Same-generation storage comparison

The preserved legacy Arcana generation and the new implementation were both built from Lexicon snapshot:

`sha256:a73b0627eeea8f66a9e71af9f275d39f96796d320194dc8908f817fc8cd583cc`

Semantic counts are identical:

- nodes: **1,136,365**
- visible edges: **2,401,702**
- unresolved references: **697,792**
- visible dataset checksum: `4de9550e3a439add`

The packed graph is byte-identical:

- legacy/new `graph.arcana`: **47,002,416 B**
- SHA-256: `f0e601a5b65b33d98e181fabff15d612b5ecaace25b8d440e388756baaf00010`

| Artifact | Legacy bytes | New bytes |
| --- | ---: | ---: |
| `graph.arcana` | 47,002,416 | 47,002,416 |
| `catalogue.tsv` | 336,985,393 | — |
| `unresolved.tsv` | 132,699,422 | — |
| `facts.tsv` | 697,987,622 | — |
| `repository.arcana` | — | 494,198,208 |
| manifests + snapshot binding | 943 | 845 |
| **generation total** | **1,214,675,796** | **541,201,469** |

The new generation is **673,474,327 B smaller**, a **55.44% total reduction**. Excluding the unchanged packed graph, repository metadata falls from **1,167,673,380 B** to **494,199,053 B**, a **57.68% reduction**.

This confirms that the graph itself was never the storage problem: the 1.14-million-node / 2.40-million-edge Hermes graph remains about **47 MB**.

## Current Hermes rebuild

The current Hermes Lexicon snapshot used by the permanent rebuild sample was:

`sha256:ee46a8a475c443f908e0eff6db9efca62e3871fe1f541e62251ebd9f5ed2c621`

Results:

| Measurement | Result |
| --- | ---: |
| Sync mode | rebuild |
| Wall time | **156.946 s** |
| Peak process-tree RSS | **1,592,414,208 B** |
| Nodes | **1,125,126** |
| Edges | **2,367,423** |
| Unresolved | **699,349** |
| `graph.arcana` | **46,411,248 B** |
| `repository.arcana` | **489,778,032 B** |
| Published generation total | **536,190,125 B** |

Raw evidence: [rebuild.json](rebuild.json).

## Managed overlay

A controlled successor snapshot preserving the stable node set exercised the managed `sync` overlay path.

Results:

| Measurement | Result |
| --- | ---: |
| Sync mode | overlay |
| Wall time | **201.966 s** |
| Peak process-tree RSS | **2,219,360,256 B** |
| Nodes | **1,125,126** |
| Edges | **2,367,423** |
| Unresolved | **699,349** |
| `graph.arcana` | **46,411,248 B** |
| `repository.arcana` | **489,778,032 B** |
| Published generation total | **536,190,125 B** |

Raw evidence: [overlay.json](overlay.json).

A second controlled parity run changed one Hermes TypeScript object while preserving node identities. Its overlay result was **120.623 s / 2,001,457,152 B peak RSS**; a clean rebuild of the exact same synthetic target was **96.947 s / 1,606,561,792 B peak RSS**. This particular changed file was graph-neutral, so managed sync selected `mode=overlay` but did not need an `overlay.arcana` payload. Raw evidence: [parity.json](parity.json). The two results matched exactly on:

- `repository.arcana` SHA-256;
- visible graph checksum;
- node count;
- visible edge count.

## Conclusion

**Storage restoration passes.** The old three-TSV repository metadata representation is gone, the packed graph is byte-identical on the same Hermes generation, and total published state is less than half the former metadata-heavy layout.

**Semantic parity passes.** Same-generation graph bytes/checksums and counts are unchanged, and controlled overlay output is identical to a clean rebuild.

**Runtime-memory restoration is not complete.** Rebuild is bounded at roughly **1.6 GB RSS**, but managed overlay currently peaks around **2.0–2.2 GB** and can be slower than a clean rebuild. The remaining overlay-specific peak is consistent with the current safe `RepositoryStore` reader retaining the complete ~490 MB `repository.arcana` byte backing while the complete current Lexicon facts are also materialized. The previous-generation `RepositoryFacts`/TSV reconstruction pathology is gone, but the packed-store byte backing is still repository-wide rather than mapped or range-read.

Phase 8 therefore closes the requested measurement/parity work, but it does **not** establish that managed sync peak memory is fully restored.
