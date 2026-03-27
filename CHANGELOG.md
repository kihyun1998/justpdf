# Changelog

All notable changes to this project will be documented in this file.

## [0.1.3] - 2026-03-23 (justpdf-core)

### Added
- **Compression engine** — Ghostscript-class PDF compression with 4 presets
  - JPEG re-encoding with quality control
  - Image downscaling with DPI-aware CTM-based calculation
  - Font subsetting (TrueType/CIDFontType2)
  - Stream dedup (SHA-256), Flate re-compression (best level)
  - Unused resource removal, metadata/structure stripping
  - Object stream packing (PDF 1.5+)
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
  split, encrypt, decrypt, clean, sign subcommands
- **Phase 11.3**: Language bindings — C FFI, Python (PyO3), WASM (wasm-bindgen)
- **Phase 11.4**: Examples, documentation, async support
