# Password authentication

## What it is
Checks a password as the user or the owner password according to `/R` and returns the file key (inferred, `crypto/auth.rs — authenticate`). Opening a document tries the empty password automatically; once `PdfDocument::authenticate` succeeds, objects loaded after it are decrypted (inferred, `parser.rs`).

## Governing decisions
**None.** No ADR or issue decides the authentication model.

## Design model
- **Dispatch.** `/R` 2–4, 5 and 6 each have their own function; any other `/R` is `UnsupportedEncryption` (inferred, `authenticate`).
- **R2–R4**: the password is tried as the user password first, then as the owner password (by recovering the user password from `/O`). The computed `/U` is compared on 32 bytes for R2 and 16 bytes for R3–R4 (inferred, `authenticate_r234`).
- **R5**: `verify_password_r5` compares SHA-256(password[..127] ‖ validation salt ‖ `/U`[0..48] for the owner) with `/U`[0..32] (`/O`[0..32] for the owner). `compute_file_key_r5` gets the key by unwrapping `/UE` or `/OE`. R5 `/Perms` is not checked (inferred, `authenticate_r5`).
- **R6**: Algorithm 2.B hashes against `/U` and `/O`, then `/Perms` is checked with the recovered key (inferred, `authenticate_r6`). A `/Perms` whose `/P` bytes or metadata flag disagree with the dictionary is an `EncryptionError`, returned at once, so a correct user password is refused and the owner password is not tried (inferred, `verify_perms_r6`). The check is skipped without a message when `/Perms` is shorter than 16 bytes (inferred).
- **Fixed after success.** Opening tries the empty password, and on success the document stays authenticated. A later `authenticate` returns `Ok` at once without looking at the password (inferred, `PdfDocument::authenticate`). So if verification wrongly passes, the automatic empty-password attempt fixes the document with a garbage key, and even the right password can no longer read it. R5 was in exactly this state before #30 (measured 2026-09-23, #30: a qpdf R5 file with a user password had `is_authenticated() = true` right after opening, `Ok` for any password, and text extraction gave `invalid PKCS#7 padding`).
- On success `PdfDocument::authenticate` clears the object cache, the decoded object streams and the font program cache, so objects loaded before it are loaded again with decryption (inferred).
- **Input size**: the password comes from the caller, and the hashed values from the file. R5 and R6 hash at most 127 password bytes; R6 runs at most 287 rounds of `compute_hash_r6` per attempt ([Key derivation](key-derivation.md)). Nothing limits how often a caller may try passwords.

## Code
- `justpdf-core/src/crypto/auth.rs` — `authenticate`, `authenticate_r234`, `authenticate_r5`, `verify_password_r5`, `authenticate_r6`, `verify_perms_r6`
- `justpdf-core/src/parser.rs` — `authenticate`, `detect_encryption`, `detect_encryption_inline`, `is_authenticated`
- `justpdf-core/tests/integration.rs` — `test_r5_wrong_password_rejected`, `test_r5_user_password_decrypts`, `test_r5_owner_password_decrypts`, `test_r5_is_not_authenticated_on_open_when_user_password_set`

## Reference behaviour
Read from the source code directly on 2026-09-23 (#30):
- **pdf.js** `src/core/crypto.js` — `_hash` in `PDF17` (R5) is a single SHA-256; `PDF20` (R6) is Algorithm 2.B. `checkUserPassword`/`checkOwnerPassword` compare the hash with the first 32 bytes of `/U` and `/O`, and append the 48 bytes of `/U` to the owner hash input. The owner attempt happens only when the password is not empty.
- **MuPDF** `source/pdf/pdf-crypt.c` — `pdf_compute_encryption_key_r5` (ExtensionLevel 3 algorithm 3.2a) makes the validation hash from the same input, and `pdf_authenticate_user_password`/`pdf_authenticate_owner_password` compare it with the first 32 bytes of `/U` and `/O`.
- `/Perms`: pdf.js passes it to `#createEncryptionKey20` but does not use it in the body, and MuPDF computes it only when writing R6; neither checks it when reading. justpdf checks it for R6 only, so it refuses an R6 file whose `/Perms` disagrees with `/P` that both of them open (inferred from the comparison).
- justpdf's R5 verification and key derivation agree with both implementations. The judge is an R5 file qpdf wrote (fixtures below). R5 is a revision of PDF 1.7 Adobe Extension Level 3, so the ISO 32000-2 text does not cover it.
- **R5 fixtures** (`justpdf-core/tests/fixtures/`): two AES-256 R5 files made with qpdf 12.3.2 (pikepdf 10.13.0), a judge independent of justpdf's code. The content is one line of Helvetica, "R5 secret text".
  - `aes256_r5_user_owner.pdf` — user `userpw`, owner `ownerpw` (sha256 `42ee0c15def7f19bee64b219333bcfc476b2b771cc7ed17f718cca8a06fd627d`)
  - `aes256_r5_empty_user.pdf` — empty user password, owner `ownerpw` (sha256 `eb50511b51d24c965dd5841ea5f359e051593af37b958a1d2ee805160002876b`)
  - To regenerate: `pdf.save(path, encryption=pikepdf.Encryption(owner=..., user=..., R=5), static_id=True)`; the page is `BT /F1 24 Tf 72 720 Td (R5 secret text) Tj ET` with `/Type /Font /Subtype /Type1 /BaseFont /Helvetica` as `/F1`. The salts are random, so the bytes differ every time.

## Cross-cutting invariants
- [Encryption identity](../invariant/encryption-identity.md)

## Blast radius
- [Key derivation](key-derivation.md) — computes every value compared here. A derivation change that agrees with itself still passes the self round-trip tests; only the third-party fixtures catch it.
- [Encryption model](encryption-model.md) — the empty-password attempt runs on every encrypted file at open, so a panic in derivation from a bad dictionary surfaces here, as a crash on open.
- [Document access](document-access.md) — authentication clears the caches and turns on decryption; both assume an authenticated document never changes its key.
- [CLI](cli.md), [Facade](facade.md), [Language bindings](language-bindings.md) — expose `--password`/`authenticate` and rely on `Ok` meaning the document is readable.
- [Compress pipeline](compress-pipeline.md) — CLI `compress` authenticates, rewrites without encryption through the modifier, then compresses (inferred, `cmd_compress`).

## Known holes / open
- No test exercises a password longer than 127 bytes (truncation) (inferred).
- Calling `authenticate` again on an authenticated document returns `Ok` without looking at the password; the same happens when the owner password is given to a document opened with the user password (inferred, `PdfDocument::authenticate`). A caller cannot tell which password was accepted.
