# Permissions (/P)

## What it is
The model that reads the `/P` bits of the encryption dictionary as permissions such as print, copy and modify (`Permissions` and its `can_*` methods), and sets them when writing (inferred, `crypto/types.rs`). `PdfDocument::permissions` returns them for an encrypted document, `None` otherwise (inferred, `parser.rs`).

## Governing decisions
No ADR yet. **Maintainer decision (2026-10-23, #60)**: justpdf does not enforce `/P`, in `justpdf-core` or in the CLI. The library reports the permissions through `Permissions::can_*`, and enforcing them is the consumer's policy. It was made against qpdf ("qpdf itself does not obey encryption restrictions already imposed on the file", `manual/cli.rst`) and MuPDF (only the viewer calls `fz_has_permission`; `pdfclean` and `mudraw` do not), and against the boundary between the library (mechanism) and its consumers (policy). Alternatives declined: the CLI requiring `--force` to `decrypt` a document opened with the user password; core checking permissions in text extraction and rendering. The decision asks for an ADR and a doc comment on `Permissions`; neither exists at HEAD (inferred, `docs/adr/`, `Permissions`).

## Design model
- **Nothing enforces permissions**, as decided above: the `can_*` methods have no callers in product code, only in `crypto/types.rs` and the integration tests (inferred, `git grep -nE 'can_(print|modify|copy|annotate)'`). A document opened with the user password becomes an unrestricted plaintext file through CLI `decrypt` (inferred, `cmd_decrypt`).
- **Bits.** Each `can_*` tests one bit with a 0-based shift: `can_print` is `1 << 2` (ISO bit 3), `can_modify` `1 << 3`, `can_copy` `1 << 4`, `can_annotate` `1 << 5`, `can_fill_forms` `1 << 8`, `can_extract_accessibility` `1 << 9`, `can_assemble` `1 << 10`, `can_print_high_quality` `1 << 11` (inferred). `allow_all` is `-4`: every bit set except the two low ones, which must be 0 (inferred).
- **Writing.** CLI `encrypt` exposes only `--no-print` and `--no-copy`: it starts from `-4` and clears `1 << 2` or `1 << 4` (inferred, `cmd_encrypt`). The bits go into `/P` and, for R6, into `/Perms` ([Key derivation](key-derivation.md)).
- **Input size**: `/P` is one integer read from the file and cast to `i32` ([Encryption model](encryption-model.md)).

## Code
- `justpdf-core/src/crypto/types.rs` — `Permissions`, `new`, `allow_all`, `can_print`, `can_modify`, `can_copy`, `can_annotate`, `can_fill_forms`, `can_extract_accessibility`, `can_assemble`, `can_print_high_quality`
- `justpdf-core/src/parser.rs` — `permissions`
- `justpdf-cli/src/main.rs` — `cmd_encrypt`

## Reference behaviour
- qpdf `manual/cli.rst` and MuPDF (`platform/gl/gl-main.c`, `source/tools/pdfclean.c`, `mudraw.c`), read 2026-10-23 on #60: neither enforces `/P` outside a viewer.
- The bit layout has not been checked against the spec text. The clause to compare against is ISO 32000-2 §7.6.4.2 (Table 22).

## Cross-cutting invariants
**None.**

## Blast radius
- [CLI](cli.md) — `encrypt` writes the bits and `decrypt` ignores them; both follow the decision that nothing enforces `/P`.
- [Encryption model](encryption-model.md) — parses and stores `/P`.
- [Key derivation](key-derivation.md), [Password authentication](password-authentication.md) — `/P` enters the R2–R4 file key and the R6 `/Perms` check, so a changed bit changes the key and what authenticates.

## Known holes / open
- The decision on #60 is not yet recorded as an ADR, and the `Permissions` doc comment does not say permissions are reported, not enforced. Tracked: #60
