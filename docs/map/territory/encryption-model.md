# Encryption model

## What it is
Parsing and serializing the `/Encrypt` dict, and the state (`SecurityState`) that decides, from `/V`, `/CF`, `/StmF` and `/StrF`, which cipher (RC4/AESV2/AESV3/None) applies to strings and streams. The read and write encryption paths both share this model.

## Governing decisions
**None.**

## Design model
- Only `/Filter /Standard` is accepted (decided when the document is opened).
- With V=4 it uses named crypt filters; an unknown V falls back to V2 (RC4).
- `to_pdf_dict` always writes `/AuthEvent /DocOpen`.
- `/EncryptMetadata false` means something only at V 4 and 5 (`metadata_left_plain`) — it is used by key derivation ([Key derivation](key-derivation.md)) and by the rule that leaves the catalog `/Metadata` stream data unencrypted ([Object decryption](object-decryption.md), [Object encryption](object-encryption.md)).

## Code
- `justpdf-core/src/crypto/types.rs` — `EncryptionDict`, `from_dict`, `to_pdf_dict`, `key_length_bytes`, `CryptFilterMap`, `CryptFilter`, `CryptMethod`, `SecurityState`, `resolve_crypt_method`

## Reference behaviour
**None.** Clauses to compare against: ISO 32000-2 §7.6.2–7.6.3, §7.6.5 (crypt filters).

## Cross-cutting invariants
**None.**

## Blast radius
- [Key derivation](key-derivation.md), [Password authentication](password-authentication.md) — consumers of dict values such as `/R`, `/O`, `/U`.
- [Object decryption](object-decryption.md), [Object encryption](object-encryption.md) — consumers of the cipher choice.
- [Document access](document-access.md) — the handler check.
- [Permissions](permissions.md) — `/P`.

## Known holes / open
**None.** No known holes.
