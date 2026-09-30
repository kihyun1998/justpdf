# Password authentication

## What it is
Verifies the user and owner passwords according to `/R` and obtains the file key. Opening a document tries the empty password automatically, and once `authenticate` succeeds, subsequent object loads are decrypted.

## Governing decisions
**None.**

## Design model
- R2–R4: try the user password first, then the owner password if that fails.
- R5: `verify_password_r5` compares SHA-256(password[..127] ‖ validation salt ‖ `/U`[0..48] for the owner) against `/U`[0..32] (`/O`[0..32] for the owner). `compute_file_key_r5` obtains the key by unwrapping `/UE` or `/OE`. R5 `/Perms` is not verified (R6 only).
- Opening a document tries the empty password automatically, and on success the document is fixed in the authenticated state — later `authenticate` calls return at once as "already authenticated". So if verification is wrong (always passes), the automatic empty-password attempt fixes the document with a garbage key, and even the right password can no longer read it. R5 was in exactly this state before #30 (measured 2026-09-23: a qpdf R5 file with a user password had `is_authenticated() = true` right after opening, `Ok` for any password, and text extraction gave `invalid PKCS#7 padding`).
- R6 `/Perms` verification is silently skipped when `/Perms` is shorter than 16 bytes.

## Code
- `justpdf-core/src/crypto/auth.rs` — `authenticate`, `authenticate_r234`, `authenticate_r5`, `authenticate_r6`, `verify_password_r5`, `verify_perms_r6`
- `justpdf-core/tests/integration.rs` — `test_r5_wrong_password_rejected`, `test_r5_user_password_decrypts`, `test_r5_owner_password_decrypts`, `test_r5_is_not_authenticated_on_open_when_user_password_set`
- `justpdf-core/src/parser.rs` — `authenticate`, `detect_encryption`

## Reference behaviour
2026-09-23, read the source code directly (#30):
- **pdf.js** `src/core/crypto.js` — `_hash` in `PDF17` (R5) is a single SHA-256; `PDF20` (R6) is Algorithm 2.B. `checkUserPassword`/`checkOwnerPassword` compare the hash with the first 32 bytes of `/U` and `/O`, and append the 48 bytes of `/U` to the owner hash input. The owner attempt happens only when the password is not empty.
- **MuPDF** `source/pdf/pdf-crypt.c` — `pdf_compute_encryption_key_r5` (ExtensionLevel 3 algorithm 3.2a) makes the validation hash from the same input, and `pdf_authenticate_user_password`/`pdf_authenticate_owner_password` compare it with the first 32 bytes of `/U` and `/O`.
- `/Perms`: pdf.js passes it to `#createEncryptionKey20` but does not use it in the body, and MuPDF computes it only when writing R6 — neither implementation verifies it when reading. justpdf verifies it only for R6.
- justpdf's R5 verification and key derivation agree with both implementations. The judge is an R5 file qpdf wrote (fixtures below). R5 is a revision of PDF 1.7 Adobe Extension Level 3, so it is not covered by the ISO 32000-2 text.

## Cross-cutting invariants
**None.**

## Blast radius
- [Key derivation](key-derivation.md) — input.
- [Document access](document-access.md) — authentication clears the cache and turns on decryption.
- [CLI](cli.md), [Facade](facade.md), [Language bindings](language-bindings.md) — expose `--password`/`authenticate`.
- [Compress pipeline](compress-pipeline.md) — the plan for handling encrypted input relies on re-serializing after authentication.

## Known holes / open
- **R5 fixtures** (`justpdf-core/tests/fixtures/`): two AES-256 R5 files made with qpdf 12.3.2 (pikepdf 10.13.0). A judge independent of our code. The content is one line of Helvetica, "R5 secret text".
  - `aes256_r5_user_owner.pdf` — user `userpw`, owner `ownerpw` (sha256 `42ee0c15def7f19bee64b219333bcfc476b2b771cc7ed17f718cca8a06fd627d`)
  - `aes256_r5_empty_user.pdf` — empty user password, owner `ownerpw` (sha256 `eb50511b51d24c965dd5841ea5f359e051593af37b958a1d2ee805160002876b`)
  - To regenerate: `pdf.save(path, encryption=pikepdf.Encryption(owner=..., user=..., R=5), static_id=True)` — the page is `BT /F1 24 Tf 72 720 Td (R5 secret text) Tj ET` with `/Type /Font /Subtype /Type1 /BaseFont /Helvetica` as `/F1`. The salts are random, so the bytes differ every time.
- No test exercises a password longer than 127 bytes (truncation).
- Calling `authenticate` again on an already authenticated document returns `Ok` without looking at the password (the same happens when an owner password is given to a document opened with the user password).
