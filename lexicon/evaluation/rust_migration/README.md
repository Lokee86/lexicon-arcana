# Lexicon Rust migration oracle

The Go implementation at commit
`758af9daf6e71fc0a7ebb837875efe366f6403fd` is the behavioral reference
for the parity-first Rust migration.

`reference.json` records the pinned commit, the existing Go tests that remain
authoritative, and the captured migration fixtures. The fixture directory is
not a new product contract. It is a migration oracle derived from the existing
versioned contracts and reference tests.

The existing Go binary object goldens remain authoritative for binary v1/v2.
They are intentionally not copied here. Snapshot publication, incremental
planning, recovery, export, and full-scan behavior remain protected by their
existing Go tests until equivalent Rust slices exist.

`compare.py CANDIDATE_DIR` compares a candidate fixture bundle with the
captured reference bundle. Byte-contract artifacts use exact byte comparison;
JSON test vectors use structural comparison. Later migration slices add
candidate artifact generation rather than weakening these comparisons.

Parity changes are made in the Go reference first unless an intentional
contract migration is separately approved.
