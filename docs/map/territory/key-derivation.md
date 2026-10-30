# Key derivation

## What it is
Password padding, the file encryption key, the `/O` and `/U` values (R2–R4), the `/OE`, `/UE` and `/Perms` values and the extended hash (R5–R6), and the per-object key (Algorithm 1) (inferred, `crypto/key.rs`). Authentication (reading) and encryption (writing) share the same functions.

## Governing decisions
**None.** No ADR covers it. The R6 `/Perms` randomness was decided on #31 and is recorded in [Object encryption](object-encryption.md).

## Design model
- **Revisions.** Writing supports R3 (RC4-128), R4 (AES-128) and R6 (AES-256) only (inferred, `EncryptionConfig::build_r3`, `build_r4`, `build_r6`). R2 and R5 are read-only.
- **Password length.** R2–R4 pad or truncate the password to 32 bytes (`pad_password`). R5 and R6 truncate it to 127 bytes (`compute_file_key_r5`, `compute_hash_r6`, `generate_values_r6`) (inferred). There is no SASLprep normalization for R5 and R6 (inferred).
- **R2–R4 file key** (`compute_file_encryption_key_r234`): MD5 of the padded password, `/O`, `/P` as little-endian `u32` and the file ID, plus `FF FF FF FF` when `/EncryptMetadata` is false at R4 or later; R3 and later rehash the first `key_length_bytes` bytes 50 times (inferred). The file ID is the trailer `/ID` first element, so a writer must write the same first element it derived the key from ([Object encryption](object-encryption.md)).
- **Object key** (`compute_object_key`): MD5 of the file key, the low 3 bytes of the object number, the low 2 bytes of the generation, and `sAlT` for AES; truncated to `min(n + 5, 16)` bytes (inferred). R5 and R6 (AESV3) use the file key directly, so their object keys depend on neither number nor generation (inferred, `decrypt_bytes`, `encrypt_bytes`).
- **R6** (`compute_hash_r6`, Algorithm 2.B): at least 64 rounds, ending when the last byte of the round's output is at most `round - 32`, so at most 287 rounds (inferred). `generate_values_r6` is deterministic: the caller passes the file key, the four salts and `/Perms` bytes 12–15 (`perms_random`). The random values are made by `build_r6` in [Object encryption](object-encryption.md).
- The doc comment of `compute_file_key_r5` says "validation_salt", but the parameter is `key_salt`, and `authenticate_r5` passes the key salt (`/U[40..48]` or `/O[40..48]`) (inferred).
- **Input size**: `/O`, `/U`, `/OE` and `/UE` come from the file; R5 and R6 check their minimum lengths before slicing (inferred, `compute_file_key_r6_user`, `compute_file_key_r6_owner`). R2–R4 derivation trusts `key_length_bytes`, which is clamped to 5..=16 bytes for `/R` 2–4 whatever `/V` says — MuPDF clamps at the same place (`fz_clampi(length/8, 0, 16)` in `pdf-crypt.c`) ([Encryption model](encryption-model.md), #410).

## Code
- `justpdf-core/src/crypto/key.rs` — `PADDING`, `pad_password`, `compute_file_encryption_key_r234`, `compute_o_value_r234`, `compute_u_value_r234`, `recover_user_password_from_owner_r234`, `compute_file_key_r5`, `compute_hash_r6`, `compute_file_key_r6_user`, `compute_file_key_r6_owner`, `compute_object_key`, `aes128_cbc_encrypt_no_padding`, `generate_o_u_values_r234`, `generate_values_r6`, `aes256_cbc_encrypt_no_padding`
- `justpdf-core/src/crypto/rc4.rs` — `rc4`, `Rc4State`
- `justpdf-core/src/crypto/aes_cipher.rs` — `encrypt_aes256_ecb_block`, `decrypt_aes256_ecb_block`

## Reference behaviour
- The code cites ISO 32000-1:2008 §7.6.3.3 (Algorithm 2) and PDF Reference 1.7 numbering ("Table 3.18"), and implements the R6 algorithms 2.A/2.B without a citation (from ISO 32000-2). There is no record of checking the code against the spec text.
- R3, R4 and R6 key derivation agrees with qpdf 12.3.2 and 12.4.1: the qpdf-written fixtures listed in [Object decryption](object-decryption.md) authenticate with their user password and decrypt (inferred, the `*_third_party_files` tests in `tests/integration.rs`).
- R5 agrees with qpdf through both passwords ([Password authentication](password-authentication.md)).

## Cross-cutting invariants
- [Object syntax roundtrip](../invariant/object-syntax-roundtrip.md) — the binary `/O`, `/U`, `/OE`, `/UE` and `/Perms` values made here must stay in the file byte for byte for authentication to work. #20 happened at this boundary: the cause was serialization, not key derivation (commit 7be23c0).
- [Encryption identity](../invariant/encryption-identity.md)

## Blast radius
- [Password authentication](password-authentication.md), [Object encryption](object-encryption.md) — the two consumers of the same functions. A change to a derivation step changes both what is written and what is accepted, so self round-trip tests still pass; only third-party fixtures catch it.
- [Object decryption](object-decryption.md) — derives per-object keys with `compute_object_key`; it assumes the number and generation it passes are the ones the writer used.
- [Object serialization](object-serialization.md) — the binary values must take the hex path to survive.
- [Encryption model](encryption-model.md) — supplies `key_length_bytes` and the parsed values these functions trust.

## Known holes / open
- The owner-password path for R3, R4 and R6 is tested only on justpdf's own output; the third-party fixtures authenticate with the user password only (inferred, `tests/integration.rs`). There is no R2 test.
- No test exercises a password longer than 127 bytes (R5/R6 truncation) (inferred).
