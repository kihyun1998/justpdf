# Encryption identity

## The fact
Two values tie an encrypted file's objects to its key, and every site that writes or reads the file must agree on both:

1. **`/ID[0]` is the key's file identifier.** For R2–R4 the file key is derived from the password, `/O`, `/P` and the first element of the trailer `/ID`. The `/ID[0]` written must be the identifier the key was derived from, and the reader derives the key from the `/ID[0]` it finds. Changing the first element of a file whose objects are already encrypted makes it open with no password; only the second element may be fresh.
2. **The `/Encrypt` dictionary is recognised by its object number.** Its strings are never encrypted (ISO 32000-1 §7.6.1, as cited in [Object decryption](../territory/object-decryption.md)), and each site that skips it does so by comparing `obj_num` with `SecurityState::encrypt_obj_num`. The number set in the state must be the number the dictionary is written at.

## Why it is cross-cutting
No site calls another for either value (inferred, read at 88b2a08):
- `/ID[0]`: `serialize_writer_encrypted` (`writer/serialize.rs`) derives the key from `permanent_id` and writes it as `/ID[0]`; `DocumentModifier::build_with_source_encryption` (`writer/modify.rs`) writes `state.file_id` as `/ID[0]` with a fresh second element; `incremental_trailer` copies the previous `/ID` whole; the reader takes `/ID[0]` from the trailer in `parser.rs` and passes it to `crypto/auth.rs — authenticate_r234`.
- `/Encrypt` number: `crypto/encrypt.rs` and `crypto/decrypt.rs` each skip `state.encrypt_obj_num`; `serialize_writer_encrypted` and `build_with_source_encryption` each set it after `add_object`; `writer/object_stream.rs` keeps that number out of object streams; `DocumentModifier` drops the source's `/Encrypt` number from the objects it copies (the `encrypt_obj_num` filter in `writer/modify.rs`).

A change on one side alone — #77's planned `/ID` rewrite, a renumbering pass, a new writer — breaks the file without breaking that side's own tests, since a justpdf round trip that re-derives from what it wrote sees nothing wrong.

## Territories it holds in
- [Object encryption](../territory/object-encryption.md) — `serialize_writer_encrypted`, `build_with_source_encryption`; the `encrypt_obj_num` skip in `encrypt_object_for_writing`.
- [Object decryption](../territory/object-decryption.md) — the `encrypt_obj_num` skip in `decrypt_object`.
- [Key derivation](../territory/key-derivation.md) — `compute_file_encryption_key_r234` takes the file identifier.
- [Password authentication](../territory/password-authentication.md) — `authenticate_r234` with `state.file_id` read from `/ID[0]`.
- [Incremental save](../territory/incremental-save.md) — `incremental_trailer` carries the previous `/ID` unchanged.
- [Object streams](../territory/object-streams.md) — the packer excludes `encrypt_obj_num`.
- [Document modifier](../territory/document-modifier.md) — drops the source `/Encrypt` object when copying, so a rewrite does not carry two.

## What a violation looks like
- A rewrite that replaces both `/ID` elements of an R2–R4 source kept with `preserve_encryption`: neither password opens the output, and justpdf's own reader reports a wrong password.
- A pass that renumbers objects after `encrypt_obj_num` is set: the `/Encrypt` dictionary's `/O` and `/U` are encrypted like any string, and authentication fails for every password.
- Neither has been reproduced; both are inferred from the code (2026-10-30).

## Discovery history
Reported by the writing-and-encryption rewrite of the map on 2026-10-30 as two candidates from reading code; made one invariant note on the maintainer's answer the same day. #77 (2026-10-10, maintainer decision: keep the source's first element, fresh second) is the change most likely to meet it.

## Where it will recur
**Code that writes a trailer `/ID` for an encrypted file, or that adds, removes or renumbers objects while a `SecurityState` is live.** Commands: `rg -n 'b"ID"' justpdf-core/src` and `rg -n 'encrypt_obj_num' justpdf-core/src`. Test with an R3 (RC4) or R4 (AES-128) file — R6 derives its key without `/ID`, so it is not evidence — opened with each password after the write, and with an external reader (qpdf, as [Source generation](source-generation.md) does).
