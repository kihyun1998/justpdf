# Key derivation

## What it is
Password padding, file encryption key computation, generating the `/O` and `/U` values (R2–R4) and the `/OE`, `/UE` and `/Perms` values (R5–R6), and per-object key derivation (Algorithm 1). Authentication (reading) and encryption (writing) share the same functions.

## Governing decisions
**None.**

## Design model
- The only revisions writing supports are R3, R4 and R6. R2 and R5 are read-only.
- Passwords are truncated to 127 bytes. There is no SASLprep normalization.
- `generate_values_r6` is deterministic: the caller passes the file key, the four salts and `/Perms` bytes 12–15 (`perms_random`). The random values are made by `build_r6` in [Object encryption](object-encryption.md).
- The doc comment of `compute_file_key_r5` says "validation_salt", but the caller passes the key salt.

## Code
- `justpdf-core/src/crypto/key.rs` — `pad_password`, `PADDING`, `compute_file_encryption_key_r234`, `compute_o_value_r234`, `compute_u_value_r234`, `recover_user_password_from_owner_r234`, `compute_file_key_r5`, `compute_hash_r6`, `compute_object_key`, `generate_o_u_values_r234`, `generate_values_r6`

## Reference behaviour
**None.** The code cites ISO 32000-1:2008 §7.6.3.3 (Algorithm 2) and PDF Reference 1.7 numbering ("Table 3.18"), and implements the R6 algorithms 2.A/2.B without a citation (sourced from ISO 32000-2). There is no record of checking against the spec text or against encrypted files made by a third-party tool.

## Cross-cutting invariants
- [Object syntax roundtrip](../invariant/object-syntax-roundtrip.md) — the binary `/O` and `/U` values made here must stay in the file unchanged for authentication to work. #20 happened at this boundary: the cause was serialization, not key derivation.

## Blast radius
- [Password authentication](password-authentication.md), [Object encryption](object-encryption.md) — the two consumers of the same functions.
- [Object serialization](object-serialization.md) — the values must be written as hex.

## Known holes / open
- R3, R4 and R6 are verified only against their own output (generate → authenticate). R5 is verified with external fixtures qpdf wrote ([Password authentication](password-authentication.md#known-holes--open)). There is no R2 test.
