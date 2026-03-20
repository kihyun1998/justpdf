# Changelog

All notable changes to this project will be documented in this file.

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
