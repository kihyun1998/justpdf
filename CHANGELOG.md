# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Added
- `writer::embed_rgb` (also `justpdf::embed_rgb`) — embed 8-bit RGB pixels as a Flate-compressed image XObject, for `PageBuilder::add_image` / `draw_image` (#88)
- **CLI `compress` subcommand** — `justpdf compress <file> --preset low|medium|high|extreme -o <out>` (#10)
- **CLI `compress` options** — per-knob overrides on top of `--preset` (`--jpeg-quality`, `--max-dpi`, `--no-strip-metadata`, …) (#11), `--password` for encrypted input (output is unencrypted) (#13), `--analyze` preview (#12), `--verbose` breakdown (#14)
- **Release workflow** — pushing a `v*` tag builds CLI binaries for Linux, macOS (x86_64, aarch64) and Windows and attaches them to the GitHub release (#15)
- `crypto::random_file_id` — a random 16-byte file identifier for a newly written file (#31)
- `DocumentModifier::set_encryption` — `build` then writes the document encrypted, keeping `/Info`; the trailer `/ID` keeps the source's first element and gets a new second element (#75)
- `ContentOp` is now `PartialEq` (#29)

### Changed
- `justpdf-special`: `justpdf-render` is now optional and only pulled in by the `ocr` feature (#1)
- `justpdf-formats`: `justpdf-render` is now optional and only pulled in by the `plaintext`, `mobi` and `fb2` features (#2)
- **`incremental_save` appends only what changed** — it takes the source document instead of its bytes (`incremental_save(&doc, modifier)`, where `doc` is the document the modifier was created from). It appends only the objects whose value differs from the document's and the new ones, marks the objects the modifier removed as free (`65535 f`), and returns the original bytes unchanged when nothing changed; before, every object was appended again (#25)

### Fixed
- **Integer text beyond the 64-bit range** — the tokenizer rejected an integer such as `100000000000000000000` as an invalid token, so justpdf could not open files other tools wrote with one, and the content stream parsers silently read it as `0`. Both now read it as the nearest real, as pdf.js does (#84)
- **Generated content wrote NaN and infinity** — `PageBuilder` (coordinates, colors, font size), the form-field, annotation and signature appearance streams and the redaction overlay wrote numbers with Rust's float formatting, so a NaN or infinite input came out as `NaN`/`inf` (read as an operator) and an integer value beyond the 32-bit range as a long integer. They now write NaN as `0`, infinity as `±f32::MAX`, integer values within the 32-bit range as integers and other values as reals; ordinary output is unchanged (#90)
- **CBZ, SVG and OCR output drew their page image as a 1-point dot** — `to_pdf` for CBZ and SVG and `make_searchable_pdf` put the page raster in the content stream as an inline image without scaling it, so it covered 1×1 pt (a CBZ page rendered blank), and pixel data holding whitespace + `EI` + whitespace cut the image short. They now embed the raster as an image XObject with `embed_rgb` and draw it over the whole page (#88)
- **Redaction and grayscale corrupted the page content they kept** — both rewrite a page's content stream after parsing it, each with its own writer. Redaction wrote a string holding one unbalanced parenthesis (a list item `1)`) without escaping it, so the text changed; both lost every inline image (redaction wrote the text `<inline-image>`, grayscale only `BI`); names with a space or other delimiter split into two operands; reals beyond the 64-bit integer range read back as other numbers, and grayscale rounded every real to four decimals. Both now go through one writer that uses the same rules as `PdfObject`'s `Display` and writes inline images back byte for byte; the gray values grayscale writes are rounded to four decimals and no longer come out as `inf`/`NaN`. `ContentOp`'s `Display` shows that writer's output. Because inline images now survive, redaction removes the ones that overlap a redaction area, judged like image `Do` operations (by the last `cm`). The content parser read an inline image's dictionary value `<< … >>` as a hex string and `null` as a name, so `/DP << /Predictor 15 … >>` came out as garbage; it now reads them as a dictionary and null (#29)
- **Content stream parser kept carriage returns in literal strings** — an unescaped CR or CRLF inside a literal string was read as-is, while ISO 32000-1 §7.3.4.2 reads it as a line feed (as the file tokenizer already did), so text extraction and rendering saw bytes other readers do not. Both content parsers (the default and `arena`) now read it as a line feed (#87)
- **Other hand-written PDF syntax did not escape** — `PageBuilder::set_font`, `draw_image` and `draw_inline_image` wrote resource names unescaped; the form-field, signature and stamp appearance streams and `sign_pdf`'s `/Name`, `/Reason` and `/Location` wrote carriage returns raw (read as a line feed); `sign_pdf` wrote the widget's `/Rect` and the appearance dictionary's `/BBox` with Rust's float formatting, so a NaN or infinite `appearance_rect` produced `NaN`/`inf` there; `add_embedded_file` escaped the MIME type's `/` and the serializer escaped it again, so other readers saw `application#2Fpdf`. All of these now use `PdfObject`'s rules. Strings holding bytes outside printable ASCII — `PageBuilder::show_text`, the appearance streams' text and the signature's `/Name`, `/Reason` and `/Location` — are now written as hex strings (the same bytes). Reals inside the generated content streams (coordinates and colors of `PageBuilder` and the appearance streams) are still written as before. MIME types written twice-escaped by earlier versions still read as `application/pdf` (#29)
- **Grayscale compression panicked on some images** — `compress_pdf` with `grayscale` converts pixels by the component count the image decoder reports, which is right only when `/ColorSpace` is a name; an image whose color space is a reference or an array (an Indexed palette, an ICCBased profile) came out with too few or too many gray pixels, and the JPEG encoder panicked on the length. Such an image is now left as it is (#89)
- **Written values that read back differently** — objects written by `PdfObject`'s `Display` (every non-stream object `serialize_object` writes) did not always read back as the same value. A carriage return in a literal string was written raw and read back as a line feed, so encrypting a document whose `/ID` contained one produced a file neither justpdf nor qpdf could open. An integer-valued real (`1.0`, `-0.0`) was written without a decimal point and read back as an integer, and one beyond the 64-bit integer range (`1e20`) read back as a parse error. NaN and infinity were written as `NaN`/`inf`, which is not PDF. Carriage returns are now written as `\r`, reals always carry a decimal point, NaN is written as `0.0` and infinity as `±f32::MAX` (#28)
- **Stream deduplication merged streams that were not equal** — `compress_pdf`'s stream dedup (the `structural` step, on in every preset) compared only stream data, so two streams with the same bytes but different dictionaries (for example `/Width`, `/Height`, `/Filter` or `/ColorSpace`) were merged into one. It now merges only streams whose dictionary (apart from `/Length`, which the writer recomputes) and data are both equal, repeating until no merge makes further streams equal. `clean_objects` judged identity by the objects' `Display` text, which leaves out stream data and prints `Real(1.0)` and `Integer(1)` alike; it now compares values (#27)
- **Strings in stream dictionaries were not encrypted** — encryption and decryption skipped the strings in a stream's dictionary, which ISO 32000-1 §7.6.1 encrypts like any other string: justpdf wrote them in plaintext (qpdf read them as garbage) and read third-party encrypted files' values as ciphertext. Both directions now handle them with the string method; cross-reference streams stay unencrypted (§7.5.8.2). A stream-dictionary string that does not decrypt is kept as written instead of failing the stream. Encrypted files written by earlier justpdf versions can now read such strings (for example an attachment's `/Params /ModDate`) as garbage, as other readers already did
- **`PdfWriter::set_object` at a new number** — later `add_object` calls could hand out the same number again, so a document carried two objects with one number; when `DocumentModifier` encrypted such a document, the `/Encrypt` dictionary took the number and the other object was written in plaintext. `set_object` now moves the next free number above the one it sets (#75)
- **CLI `encrypt` lost `/Info` and the changing `/ID`** — the command assembled the encrypted file itself, passing no `/Info` and writing the source's first `/ID` element twice. It now goes through `DocumentModifier::set_encryption`, so the document information dictionary survives and the second `/ID` element is new (#75)
- **`DocumentModifier::from_document` on encrypted documents** — an encrypted document that had not been authenticated lost nearly every object silently (every resolve failed), so CLI `encrypt` on an encrypted input exited successfully with a file no reader could open; it is now refused with `EncryptedDocument`. An authenticated one had its old `/Encrypt` dictionary copied along, so `justpdf decrypt` output still carried the old `/O`/`/U` values in plaintext; the dictionary is no longer copied (#75)
- **Encryption randomness and file identifiers** — AES IVs, the AES-256 file key and the R6 salts were derived from the system clock, the random bytes of R6 `/Perms` were zeros, and every encrypted document (`DocumentBuilder` and CLI `encrypt`) got the same `/ID`, so RC4/AES-128 documents with the same passwords and permissions shared one file key. All of these now come from the operating system's random source; `DocumentBuilder` writes a random `/ID`, and CLI `encrypt` keeps the input's first `/ID` element when it has one (#31)
- **Incremental save and signing lost the trailer's document keys** — the new trailer only carried `/Size /Root (/Info) /Prev`, and readers (justpdf, pdf.js, MuPDF) read only the newest trailer, so `/Encrypt`, `/ID` and — when signing — `/Info` disappeared; an encrypted document came back unencrypted with its old objects unreadable. The update trailer now carries every key of the previous trailer. `incremental_save` encrypts the appended objects with the document's key (the modifier must come from an authenticated document) and writes them through the serializer instead of `Display`, which corrupted streams; `sign_pdf` refuses encrypted input (#26, #25 in part)
- **AES-256 R5 password check** — `verify_password_r5` computed the hash and returned `true` without comparing it, so any password "authenticated". Because opening a document tries the empty password first, an R5 file with a user password was locked to a garbage key on open and could not be read even with the correct password. The hash is now compared with `/U` / `/O` (#30)
- **Font subsetting dropped the glyphs it was meant to keep** — `compress_pdf` with `font_subsetting` (medium, high and extreme presets) picked glyphs by treating each character code as a glyph ID, then renumbered them while leaving the font's `cmap` and the PDF's `/CIDToGIDMap` pointing at the old numbers, so embedded TrueType text rendered as blanks even though text extraction still worked. `subset_font` now keeps every glyph ID (unused glyphs become empty; `gid_map` is the identity), glyphs are chosen through the font's own lookup paths, and a font is left whole when its use cannot be fully collected — referenced from a form XObject, an annotation appearance or inherited `/Resources`, or shown with `"` (#51)
- **CI feature-isolation check** — `cargo tree` prints ASCII glyphs on CI, so the script's checks were failing or vacuous; they now match dependency names directly (#62)
- **Encryption round-trip** — binary strings with bytes ≥ 0x7F (e.g. `/O`, `/U`) were written as literal strings and corrupted, so some password pairs produced files that could not be opened. Such strings are now hex-encoded (#20)

## [0.1.4] - 2026-05-08 (justpdf-core)

### Fixed
- **ObjStm `/First` off-by-one** — `load_compressed_object` inferred the
  data-section start from `tokenizer.pos()` after consuming the index
  pairs, which was off by the whitespace padding before the first
  object. Compressed objects' `abs_offset` landed one byte short
  (typically on the previous object's closing `>`), producing an
  `unexpected '>'` parse error. Affected PDFs that use object streams
  with whitespace padding before the data section — common in files
  with incremental updates. Now reads `/First` and `/N` from the dict.

## [0.1.3] - 2026-03-23 (justpdf-core)

### Added
- **Compression engine** — Ghostscript-class PDF compression with 4 presets
  - JPEG re-encoding with quality control
  - Image downscaling with DPI-aware CTM-based calculation
  - Font subsetting (TrueType/CIDFontType2)
  - Stream dedup (SHA-256), Flate re-compression (best level)
  - Unused resource removal, metadata/structure stripping
  - Object stream packing (PDF 1.5+) — implemented, but not enabled in `compress_pdf` (disabled for viewer compatibility)
  - RGB/CMYK to grayscale conversion
- **Visible signature appearance** — Form XObject generation for digital signatures
- **Signing time attribute** — UTCTime in CMS signed attributes (standard compliance)
- **RFC 3161 timestamp support** — `create_timestamp_request()` / `parse_timestamp_response()` public API
- **OCGState enum methods** — `from_name()` / `to_name()` for type-safe layer state handling

### Changed
- `SigningOptions` now supports `visible`, `appearance_rect`, `timestamp_token` fields
- OCG builder uses `OCGState` enum instead of raw byte literals

## [0.1.0] - 2026-03-20

### Added
- **Phase 0-7**: Complete PDF engine with parsing, rendering, text extraction,
  writing, annotations, forms, encryption, digital signatures, bookmarks,
  layers, ICC color management, font subsetting, CJK support, and PDF repair
- **Phase 8**: Performance optimization — interior mutability (`resolve(&self)`),
  multi-threaded rendering (rayon), tile-based rendering, arena allocator,
  benchmarks
- **Phase 11.1**: High-level API crate (`justpdf`) with `Document`, `Page`,
  `Metadata`, `Modifier`, `merge()` convenience functions
- **Phase 11.2**: CLI tool (`justpdf-cli`) with info, text, render, merge,
  split, encrypt, decrypt, clean subcommands (`sign` is a placeholder)
- **Phase 11.3**: Language bindings — C FFI, Python (PyO3), WASM (wasm-bindgen)
- **Phase 11.4**: Examples, documentation, async support
