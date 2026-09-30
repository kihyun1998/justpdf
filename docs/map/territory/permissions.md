# Permissions (/P)

## What it is
The model that interprets the `/P` bits of the encryption dictionary as permissions such as print, copy and modify.

## Governing decisions
**None.**

## Design model
- **Nothing enforces permissions**: the `can_*` methods have no callers in product code (only `crypto/types.rs` and the integration tests). Even a document opened with the user password becomes unrestricted plaintext through CLI `decrypt`.
- CLI `encrypt` exposes only `--no-print` and `--no-copy`.

## Code
- `justpdf-core/src/crypto/types.rs` — `Permissions`, `can_print`, `can_print_high_quality`, `allow_all`
- `justpdf-core/src/parser.rs` — `permissions`

## Reference behaviour
**None.** Clause to compare against: ISO 32000-2 §7.6.4.2 (Table 22).

## Cross-cutting invariants
**None.**

## Blast radius
- [CLI](cli.md) — encrypt/decrypt.
- [Encryption model](encryption-model.md) — stores `/P`.

## Known holes / open
- There is no record of deciding not to enforce permissions (whether it is intended or an omission is unknown).
- Tracked: #60 (permissions not enforced)
