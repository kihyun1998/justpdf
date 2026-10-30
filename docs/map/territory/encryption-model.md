# Encryption model

## What it is
Parsing and writing the `/Encrypt` dictionary (`EncryptionDict`), and the state (`SecurityState`) that holds the file key and decides, from `/V`, `/CF`, `/StmF` and `/StrF`, which cipher (RC4, AESV2, AESV3 or none) applies to strings and to streams (inferred, `crypto/types.rs`). The read path and both write paths share this model: reading builds it in `PdfDocument::detect_encryption`, writing builds it in `EncryptionConfig::build` or carries the source's ([Document modifier](document-modifier.md), source preservation).

## Governing decisions
**None.** No ADR covers encryption. [ADR 0006](../../adr/0006-breaking-changes-are-allowed-before-1-0.md) applies to every public type here (all fields of `EncryptionDict` and `SecurityState` are `pub`) but decides nothing about the model.

## Design model
- **Handler.** Only `/Filter /Standard` is accepted; another handler is `UnsupportedEncryption` when the document is opened (inferred, `detect_encryption`, `detect_encryption_inline`). A missing `/Filter` is read as `Standard` (inferred, `EncryptionDict::from_dict`). The `/Encrypt` entry may be a reference or a direct dictionary; a direct one has no object number, so `encrypt_obj_num` is `None` (inferred).
- **Defaults when reading.** A missing `/V` or `/R` reads as 0, `/Length` as 40, `/P` as 0, `/EncryptMetadata` as true; `/P` is cast to `i32`, so the unsigned form some writers use reads as the same bits (inferred, `from_dict`).
- **Cipher choice** (`SecurityState::new`): `/V` 1 or 2 is RC4 for both strings and streams. `/V` 4 resolves `/StrF` and `/StmF` through `/CF`, where `Identity` or an absent name is no encryption and a name missing from `/CF` falls back to AESV2 (inferred, `resolve_crypt_method`). `/V` 5 is AESV3 for both, without reading `/CF`, `/StrF` or `/StmF`. Any other `/V` (0, 3, unknown) falls back to RC4 (inferred).
- **Key length.** `key_length_bytes` is 32 for `/V` 5 and `/Length / 8` otherwise, with `/Length` taken from the file unchecked (inferred). For `/V` 4 the crypt filter's own `/Length` is parsed into `CryptFilter::key_length` but not used for the key (inferred).
- **Writing.** `to_pdf_dict` writes `/EncryptMetadata false` only when the flag is false, and writes `/AuthEvent /DocOpen` and `/Type /CryptFilter` into every crypt filter dictionary (inferred).
- `/EncryptMetadata false` has an effect only at `/V` 4 and 5 (`metadata_left_plain` in `crypto/decrypt.rs`). It is used by key derivation ([Key derivation](key-derivation.md)) and by the rule that leaves the catalog `/Metadata` stream data unencrypted ([Object decryption](object-decryption.md), [Object encryption](object-encryption.md)).
- `SecurityState::file_id` is the trailer's `/ID` first element as read (`extract_file_id`), empty when absent (inferred, `parser.rs`). R2–R4 key derivation uses it ([Key derivation](key-derivation.md)).
- **Input size**: the `/Encrypt` dictionary comes from the file. Nothing bounds the length of `/O`, `/U`, `/OE`, `/UE` or `/Perms`, or the number of `/CF` entries; nothing checks that `/Length` is a multiple of 8 between 40 and 256 (inferred). A bad `/Length` panics during the empty-password attempt on open (measured; Known holes).

## Code
- `justpdf-core/src/crypto/types.rs` — `EncryptionDict`, `from_dict`, `key_length_bytes`, `to_pdf_dict`, `CryptFilterMap`, `CryptFilter`, `CryptMethod`, `SecurityState`, `new`, `is_authenticated`, `permissions`, `resolve_crypt_method`
- `justpdf-core/src/parser.rs` — `detect_encryption`, `detect_encryption_inline`, `extract_file_id`, `security_state`, `is_encrypted`

## Reference behaviour
**None.** The model has not been compared with another reader's. The clauses to compare against are ISO 32000-2 §7.6.2–7.6.3 and §7.6.5 (crypt filters).

## Cross-cutting invariants
**None.**

## Blast radius
- [Key derivation](key-derivation.md), [Password authentication](password-authentication.md) — read `/R`, `/O`, `/U`, `/OE`, `/UE`, `/Perms` and `key_length_bytes` as parsed here. They assume `/Length` is sane, which nothing here checks.
- [Object decryption](object-decryption.md), [Object encryption](object-encryption.md) — use `string_method` and `stream_method` as chosen here. A change to the choice changes what both sides think is encrypted.
- [Document access](document-access.md) — opening refuses a non-Standard handler and runs the empty-password attempt on the state built here.
- [Document modifier](document-modifier.md) — source preservation writes the source `/Encrypt` dictionary as read and reuses the `SecurityState`, so the state must describe the file exactly.
- [Permissions](permissions.md) — `/P` as parsed here.
- [Repair](repair.md) — the repaired trailer is the only source of `/Encrypt`; a repaired file gets no security state (Known holes).

## Known holes / open
- A `/Length` that gives no usable key panics while opening the file, during the automatic empty-password attempt. Measured 2026-10-30 with `target/debug/justpdf info` on hand-made `/V 2 /R 3` files: `/Length 0` panics in `rc4.rs` (remainder by zero); `/Length 256` and `/Length -8` panic in `key.rs` (slice index past the 16-byte MD5). The checks belong here (`key_length_bytes`) or in [Key derivation](key-derivation.md). Tracked: #410
- A `/V` 4 file without `/Length` derives a 5-byte file key, while AESV2 needs 16 bytes, so its objects cannot decrypt (inferred, `key_length_bytes`). Reading the crypt filter's `/Length` or fixing 16 bytes for AESV2 would cover it; not compared with another reader.
- `/V` 5 ignores `/StmF`, `/StrF` and `/CF`, so a file that marks strings or streams `Identity` there is decrypted as AESV3 (inferred, `SecurityState::new`).
- `from_bytes_with_repair` builds a trailer with only `/Root` and `/Size` and constructs the document without `detect_encryption`, so an encrypted file that needs repair opens as unencrypted and its strings and streams stay ciphertext (inferred, `repair.rs`, `PdfDocument::from_raw_parts`).
