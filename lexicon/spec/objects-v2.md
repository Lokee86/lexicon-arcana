# Lexicon fact objects v2

Fact-object binary encoding v2 uses the magic `LXOBJ\0\2\0`. Writers emit v2. Readers must continue accepting binary v1 (`LXOBJ\0\1\0`) and legacy canonical JSON fact objects.

The JSON-level fact contract remains `facts-v1`. V2 changes only durable representation: decoding v2 must reproduce the same metadata and normalized node, edge, and unresolved records as v1/JSON.

## Object layout

Fields are written in this order:

1. 8-byte v2 magic;
2. object version as uvarint;
3. fact schema version as uvarint;
4. v2 string table;
5. language string reference;
6. object owner string reference;
7. source-content identity;
8. adapter-version string reference;
9. analysis-config identity;
10. bounded external-reference identity table;
11. length-prefixed node section;
12. length-prefixed edge section;
13. length-prefixed unresolved section.

All indexes are bounds checked. Trailing bytes are invalid.

Object content identity continues to use the existing `lexicon:fact-object:v1\0` digest domain. Changing representation therefore changes the object ID because the encoded bytes change, without changing the store's verification domain or snapshot semantics.

## Identities

An identity is encoded as:

- tag `0`: one string-table reference;
- tag `1`: exactly 32 raw SHA-256 digest bytes.

Only canonical `sha256:<64 lowercase hexadecimal digits>` values use tag `1`. All other values, including SHA-256-like strings with uppercase hexadecimal, use the lossless string fallback.

## String table

The v2 string table is deterministic and front-coded.

- Entry `0` is the empty-string sentinel.
- Remaining unique strings are sorted lexicographically before indexes are assigned.
- Each entry stores:
  - common byte-prefix length with the previous reconstructed entry as a uvarint;
  - the remaining UTF-8 suffix as a length-prefixed byte string.
- Readers reject prefixes longer than the previous entry, reconstructed values above the configured string-size limit, and invalid UTF-8.

String references elsewhere in the object are uvarint indexes into the reconstructed table.

## Node and external references

Node records are emitted before edges and unresolved records.

Edges and unresolved records encode node references as:

- tag `0` + one-based object-local node ordinal;
- tag `1` + one-based external-reference-table index.

The external table contains unique nonlocal identities in deterministic first-reference order. Each entry uses the identity encoding above. Local node identities are not duplicated into the external table.

## Common vocabulary codes

Known node kinds and relations use stable nonzero uvarint codes. Code `0` is followed by a string-table reference and preserves unknown or language-specific values losslessly.

### Node kinds

| Code | Kind | Code | Kind |
| ---: | --- | ---: | --- |
| 1 | `repository` | 12 | `method` |
| 2 | `directory` | 13 | `constructor` |
| 3 | `file` | 14 | `field` |
| 4 | `module` | 15 | `variable` |
| 5 | `namespace` | 16 | `constant` |
| 6 | `symbol` | 17 | `parameter` |
| 7 | `type` | 18 | `import` |
| 8 | `interface` | 19 | `test` |
| 9 | `protocol` | 20 | `http-endpoint` |
| 10 | `trait` | 21 | `message-channel` |
| 11 | `function` | 22 | `config-key` |

### Relations

| Code | Relation | Code | Relation |
| ---: | --- | ---: | --- |
| 1 | `contains` | 14 | `writes` |
| 2 | `defines` | 15 | `annotates` |
| 3 | `imports` | 16 | `includes` |
| 4 | `calls` | 17 | `depends-on` |
| 5 | `possible-calls` | 18 | `tests` |
| 6 | `passes-to` | 19 | `documents` |
| 7 | `converts-to` | 20 | `generates` |
| 8 | `references` | 21 | `calls-endpoint` |
| 9 | `extends` | 22 | `handled-by` |
| 10 | `implements` | 23 | `publishes` |
| 11 | `uses-trait` | 24 | `consumes` |
| 12 | `overrides` | 25 | `reads-config` |
| 13 | `reads` |  |  |

The code tables are storage vocabulary only. The extensibility rules in `facts-v1.md` remain authoritative.

## Repeated field factoring

V2 removes repeated values only when exact equality proves they can be reconstructed.

Owner and node-path fields use:

- factor `0`: empty;
- factor `1`: exactly the fact object's owner;
- factor `2`: following string-table reference.

Node `qualified_name` uses:

- factor `0`: following string-table reference;
- factor `1`: exactly the node `name`;
- factor `2`: exactly the node `path`;
- factor `3`: exactly the fact object's owner.

No semantic inference is permitted. Values that do not exactly match a defined sentinel use the string fallback.

## Section records

The node section contains, per record:

1. attributes;
2. content identity;
3. node identity;
4. kind code/string;
5. name string reference;
6. factored owner;
7. factored path;
8. factored qualified name;
9. optional source span.

The edge section contains:

1. attributes;
2. factored owner;
3. relation code/string;
4. source node reference;
5. optional source span;
6. target node reference.

The unresolved section contains:

1. attributes;
2. candidate-name string reference;
3. candidate-namespace string reference;
4. expression string reference;
5. factored owner;
6. reason string reference;
7. relation code/string;
8. source node reference;
9. optional source span.

## Node-only reads

The `LanguageNodes` path may decode only the header, metadata, string/external tables, and node section. Edge and unresolved sections remain integrity-protected by the object digest but need not be decoded or materialized.

## Compatibility and determinism

For identical input facts, v2 byte output is deterministic. Readers reject malformed lengths, prefixes, tags, codes, indexes, UTF-8, and trailing bytes.

Compatibility is covered by:

- Lexicon v1 and v2 golden fixtures;
- v2 round-trip and deterministic-byte tests;
- unknown kind/relation fallback tests;
- factored owner/path/qualified-name tests;
- Unicode front-coding and malformed-prefix tests;
- external-reference bounds and deduplication tests;
- node-only read tests;
- Arcana v1/v2 Go-produced golden ingestion tests.
