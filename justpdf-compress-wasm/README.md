# justpdf-compress-wasm

PDF compression WASM module for the [justpdf](https://github.com/kihyun1998/justpdf) project.

Compress PDFs entirely in the browser — no server required, no data leaves the client.

## Features

- **4 presets** — `low`, `medium`, `high`, `extreme`
- **Image optimization** — JPEG re-encoding, downscaling, grayscale conversion (grayscale via `compress_advanced` only; no preset enables it)
- **Font subsetting** — keep only used glyphs
- **Stream optimization** — Flate re-compression, duplicate dedup
- **Structure cleanup** — unused resource removal, metadata stripping, GC
- **DPI-aware downscaling** — CTM-based effective DPI calculation
- **Detailed stats** — images processed, fonts subsetted, bytes saved, etc.

## Build

```bash
cargo install wasm-pack
wasm-pack build --target web
```

## Usage (JavaScript)

```javascript
import init, { compress, compress_advanced, analyze } from './pkg/justpdf_compress_wasm.js';

await init();

const response = await fetch('document.pdf');
const bytes = new Uint8Array(await response.arrayBuffer());

// Analyze without compressing
const info = analyze(bytes);
console.log(`Pages: ${info.pages}, Images: ${info.images}`);

// Compress with preset
const result = compress(bytes, 'high');
console.log(`${result.original_size} → ${result.compressed_size} (${(result.ratio * 100).toFixed(1)}%)`);

// Download compressed PDF
const blob = new Blob([result.data()], { type: 'application/pdf' });
const url = URL.createObjectURL(blob);

// Compress with full control
const advanced = compress_advanced(
  bytes,
  65,     // jpeg_quality 1-100 (0 = unset; images are still re-encoded at 75 when max_dpi > 0; values are not range-checked — #57)
  150.0,  // max_dpi (0 = no downscaling)
  true,   // font_subsetting
  true,   // remove_unused_resources
  true,   // strip_metadata
  false,  // strip_extras
  false,  // grayscale
);
```

## API

### `compress(data, preset)`

Compress with a preset (`"low"`, `"medium"`, `"high"`, `"extreme"`). `extreme` also removes embedded files, which breaks ZUGFeRD/Factur-X invoices (#58).

### `compress_custom(data, jpeg_quality, max_dpi)`

Compress with custom JPEG quality and DPI settings (`max_dpi` 0 = no downscaling). Other options are fixed: font subsetting and unused-resource removal on, metadata/extras stripping off, images under 5,000 bytes skipped.

### `compress_advanced(data, jpeg_quality, max_dpi, font_subsetting, remove_unused_resources, strip_metadata, strip_extras, grayscale)`

Control over most compression options. Fixed: structural cleanup and stream recompression on, images under 5,000 bytes skipped.

### `analyze(data)`

Analyze a PDF without compressing. Returns page count, image count, total image bytes, and encryption status.

### CompressResult

| Property | Description |
|---|---|
| `data()` | Compressed PDF bytes |
| `original_size` | Original file size |
| `compressed_size` | Compressed file size |
| `ratio` | Compression ratio (0.0~1.0, lower = more compression) |
| `images_found` | Number of images found |
| `images_recompressed` | Images re-encoded |
| `images_downscaled` | Images downscaled |
| `images_skipped` | Images skipped |
| `images_grayscaled` | Images converted to grayscale |
| `duplicates_removed` | Duplicate streams removed |
| `objects_removed_gc` | Objects removed by GC |
| `streams_recompressed` | Streams re-compressed |
| `fonts_subsetted` | Fonts subsetted |
| `unused_resources_removed` | Unused resources removed |
| `metadata_items_stripped` | Metadata items stripped |

## Repository

[https://github.com/kihyun1998/justpdf](https://github.com/kihyun1998/justpdf)

## License

MIT OR Apache-2.0
