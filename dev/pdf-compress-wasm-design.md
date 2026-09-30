# PDF compression WASM design document

> Goal: implement a compression-only PDF WASM module for use in a browser extension
> Reach Ghostscript-level compression ratios in pure Rust/WASM

---

## 1. Product overview

### Usage scenario
1. The user selects a PDF file in the browser (or drags and drops it)
2. The WASM module compresses the PDF on the client side
3. The compressed PDF is downloaded

### Core requirements
- **No server needed** — all processing is done inside the browser
- **Privacy** — the PDF is never sent anywhere
- **Reasonable speed** — within a few seconds for a 10MB PDF
- **Meaningful compression ratio** — target size reduction of 50–80% for image PDFs, 20–50% for text PDFs

---

## 2. Crate structure

```
justpdf/
├── justpdf-core/                    # existing — parsing, modification, serialization
│   └── src/writer/
│       ├── compress.rs              # compression engine (all techniques combined)
│       ├── clean.rs                 # existing — dedup
│       ├── modify.rs                # existing — DocumentModifier
│       └── encode.rs                # existing — FlateDecode
│
├── justpdf-wasm/                    # existing — general-purpose WASM (unchanged)
│
└── justpdf-compress-wasm/           # compression-only WASM (no renderer, small bundle)
    ├── Cargo.toml
    └── src/lib.rs                   # exposes only compress(), analyze()
```

---

## 3. Industry standard comparison

> Online tools such as iLovePDF and SmallPDF use Ghostscript-family engines internally.

| Technique | Ghostscript | qpdf | Ours (current) | Ours (target) |
|------|:-----------:|:----:|:----------:|:----------:|
| Image JPEG re-encoding | ✓ | ✓ | ✓ | ✓ |
| Image downsampling | ✓ | — | ✓ | ✓ |
| Font subsetting | ✓ | — | ✓ (TrueType only) | ✓ |
| Font stream recompression | ✓ | — | ✓ | ✓ |
| Flate recompression (highest level) | ✓ | ✓ | ✓ | ✓ |
| Duplicate stream dedup | ✓ | — | ✓ | ✓ |
| Unused resource removal | ✓ | — | ✓ | ✓ |
| Metadata/structure removal | ✓ | — | ✓ | ✓ |
| Object Stream compression | — | ✓ | — (implemented, disabled) | ✓ |
| Color → Grayscale | ✓ | — | ✓ (option) | ✓ |
| GC (unused objects) | ✓ | ✓ | ✓ | ✓ |
| Uncompressed → FlateDecode | ✓ | ✓ | ✓ | ✓ |

---

## 4. Techniques applied per preset

| Technique | low | medium | high | extreme |
|------|:---:|:------:|:----:|:-------:|
| **Images** | | | | |
| JPEG re-encoding | — | q75 | q65 | q40 |
| Image downscaling | — | — | 150dpi | 96dpi |
| RGB → Grayscale | — | — | — | — (only through the `grayscale` option; no preset turns it on) |
| **Fonts** | | | | |
| Font subsetting | — | ✓ | ✓ | ✓ |
| **Streams** | | | | |
| Uncompressed → FlateDecode | ✓ | ✓ | ✓ | ✓ |
| Flate recompression (best) | ✓ | ✓ | ✓ | ✓ |
| **Structure** | | | | |
| GC (unused objects) | ✓ | ✓ | ✓ | ✓ |
| Duplicate stream dedup | ✓ | ✓ | ✓ | ✓ |
| Unused resource removal | — | ✓ | ✓ | ✓ |
| **Removal** | | | | |
| Metadata (XMP etc.) | — | — | ✓ | ✓ |
| Structure tree/thumbnails | — | — | ✓ | ✓ |
| Embedded files | — | — | — | ✓ |
| JavaScript/actions | — | — | — | ✓ |
| Output Intent/ICC | — | — | ✓ | ✓ |
| **Information loss** | None | Minimal | Moderate | Large |

---

## 5. Implementation roadmap

### Phase A: core compression engine — ✅ done

```
[v0.1 — commit fa3bfd1]
  ✅ A-1. Image JPEG re-encoding (quality control)
  ✅ A-2. Non-JPEG → JPEG conversion (PNG, Raw, etc.)
  ✅ A-3. Image downscaling (max DPI, Lanczos3)
  ✅ A-4. Automatic FlateDecode for uncompressed streams
  ✅ A-5. GC — unused object removal
  ✅ A-6. 4 presets (low/medium/high/extreme) + custom
  ✅ A-7. compress_pdf(), analyze_pdf() API
  ✅ A-8. justpdf-compress-wasm WASM binding
  ✅ A-9. compress_pdf CLI example
  ✅ A-10. 13 unit tests
```

---

### Phase B: stream optimization — ✅ done

Recompress every stream (fonts, content, images, etc.) at the highest compression level.

```
Implementation:
  ✅ B-1. Flate recompression — decode FlateDecode streams → re-encode with Compression::best()
  ✅ B-2. Stronger uncompressed stream detection — handle every stream with /Length and no /Filter

Tests:
  ✅ B-T1. Recompress a FlateDecode stream → output size ≤ original (lossless)
  ✅ B-T2. Recompression round trip → decoded result identical to the original
  ✅ B-T3. Stream already at best level → no or negligible size change
  ✅ B-T4. 20-page text PDF → improvement confirmed after recompression
```

---

### Phase C: duplicate stream dedup — ✅ done

Detect identical stream data by SHA-256 hash and merge the references.

```
Implementation:
  ✅ C-1. Compute SHA-256 hashes of stream data
  ✅ C-2. Objects with the same hash → keep only the first, remap the other references
  ✅ C-3. After remapping, remove orphaned objects with GC

Tests:
  ✅ C-T1. Embed 2 identical images → merged into 1 after dedup, references intact
  ✅ C-T2. 2 streams with the same dictionary and data → merged into 1 after dedup, references intact (replaced in #27 — the old test checked nothing, since its input had no font stream)
  ✅ C-T3. Different streams → not deduplicated (no false positives)
  ✅ C-T4. Re-parse the PDF after dedup → page count and text extraction correct
```

---

### Phase D: font subsetting — ✅ done

Keep only the glyphs used, cutting font size by 50–90%. Uses the `font/subset.rs` code.

```
Implementation:
  ✅ D-1. Collect the glyphs used per page (content stream parsing Tf/Tj/TJ)
  ✅ D-2. Build a FontFile2 stream subset with subset_font()
  ✅ D-3. Replace the FontFile2 stream in the FontDescriptor
  ✅ D-4. Update the Widths array (remapping based on gid_map) — removed in #51: GIDs are kept, so the font dictionary is left as is
  ✅ D-5. CID font support (Type0 → CIDFontType2 → FontFile2)
  ✅ D-6. Update CIDToGIDMap (GID remapping after subsetting) — removed in #51: GIDs are kept, so the font dictionary is left as is
  ✅ D-7. Extract 2-byte CID character codes

Notes:
  ✅ CFF fonts not supported (TrueType/glyf only) → skipped automatically
  ✅ CID fonts (CIDFontType2) handled
  ✅ Keep the original when subsetting fails (safeguard)

Tests:
  ✅ D-T1. Standard font → subsetting safely skipped
  ✅ D-T2. Subsetting disabled → not processed
  ✅ D-T3. Per-preset settings confirmed (low=off, medium+=on)
  ✅ D-T4. Image PDF + subsetting pipeline does not crash
  ✅ D-T5. Non-TrueType font → skipped
```

---

### Phase E: unused resource removal — ✅ done

Remove fonts/images/ExtGState that a page does not actually reference from its Resources.

```
Implementation:
  ✅ E-1. Parse page content streams → collect the resource names used
         (font names in Tf, XObject names in Do, ExtGState names in gs)
  ✅ E-2. Remove unused entries from the Resources dict
  ✅ E-3. Collect orphaned objects automatically with GC

Tests:
  ✅ E-T1. PDF valid after unused resources are removed
  ✅ E-T2. Text extraction correct after removal
  ✅ E-T3. low preset → removal disabled
  ✅ E-T4. Image PDF → images in use preserved
```

---

### Phase F: unnecessary data removal — ✅ done

Remove metadata, structure trees, thumbnails, embedded files, etc.

```
Implementation:
  ✅ F-1. Remove the XMP metadata stream (Catalog /Metadata)
  ✅ F-2. Remove the structure tree (Catalog /StructTreeRoot)
  ✅ F-3. Remove page thumbnails (Page /Thumb)
  ✅ F-4. Remove Output Intents (Catalog /OutputIntents)
  ✅ F-5. Remove embedded files (Catalog /Names → /EmbeddedFiles)
  ✅ F-6. Remove JavaScript (Catalog /Names → /JavaScript, page /AA)
  ✅ F-7. Remove app-specific data such as /PieceInfo, /LastModified, /MarkInfo
  ✅ F-8. Apply the removal scope per preset (high: strip_metadata / extreme: +strip_extras)

Tests:
  ✅ F-T1. low preset → metadata not removed
  ✅ F-T2. high preset → metadata removed, PDF valid
  ✅ F-T3. extreme preset → removal including extras, PDF valid
  ✅ F-T4. Text extraction correct after removal
  ✅ F-T5. Per-preset strip settings confirmed
  ✅ F-T6. Image PDF → images preserved
```

---

### Phase G: color conversion (Grayscale) — ✅ done

Convert RGB/CMYK images to Grayscale for a large size reduction.

```
Implementation:
  ✅ G-1. RGB → Grayscale conversion (luminance: 0.299R + 0.587G + 0.114B)
  ✅ G-2. CMYK → Grayscale conversion
  ✅ G-3. CompressOptions.grayscale: bool option (default false, explicit opt-in)
  ✅ G-4. Update color operators in content streams (rg→g, RG→G, k→g, K→G)

Tests:
  ✅ G-T1. RGB image → size reduced after Grayscale conversion
  ✅ G-T2. Re-parse correct after Grayscale conversion
  ✅ G-T3. grayscale=false → no color conversion
  ✅ G-T4. Text-only PDF + grayscale=true → no crash
```

---

### Phase H: Object Stream compression — ⚠️ implemented, disabled in the pipeline

Pack small dictionary objects into Object Streams as a PDF 1.5+ optimization.

```
Implementation:
  ✅ H-1. Fixed the catalog_ref invalidation problem (track the catalog obj_num)
  ✅ H-2. pack_object_streams() → PackResult (including compressed info)
  ✅ H-3. xref stream generation — write_xref_stream() (type 0/1/2 entries)
  ✅ H-4. serialize_pdf_with_xref_stream() + build_with_xref_stream()
  ⚠️ H-5. compress_pdf integration — the code path (`pack_into_object_streams`) exists but is disabled.
         Because of xref stream compatibility problems in some viewers (the `compress_pdf` Step 5 comment)

Tests:
  ✅ H-T1. Packing eligible objects → object count reduced
  ✅ H-T2. ObjStm metadata (Type, N, First) correct
  ✅ H-T3. catalog, pages root, Stream → not included in an object stream
```

---

### Phase I: advanced WASM features — ✅ done

```
Implementation:
  ✅ I-1. Expose all of CompressOptions to WASM — compress_advanced() function
         (jpeg_quality, max_dpi, font_subsetting, remove_unused_resources,
          strip_metadata, strip_extras, grayscale)
  ✅ I-2. Expose every CompressStats field — 14 getters
         (original_size, compressed_size, images_found, images_recompressed,
          images_downscaled, images_skipped, duplicates_removed, objects_removed_gc,
          streams_recompressed, fonts_subsetted, unused_resources_removed,
          metadata_items_stripped, images_grayscaled, ratio)
  ✅ I-3. Verified wasm-pack build --target web (697KB, added the getrandom js feature)
  ✅ I-4. npm package release — @kihyun1998/justpdf-compress-wasm (see npm for the current version)
```

---

### Phase J: precise DPI calculation — ✅ done

```
Implementation:
  ✅ J-1. Track cm/q/Q operators in content streams → extract the CTM right before Do
  ✅ J-2. effective DPI = image_px / (ctm_scale / 72)
  ✅ J-3. Prefer the CTM-based approach; without a CTM, fall back to pixel-based
  ✅ J-4. When the same image is used on several pages, use the largest display size

Tests:
  ✅ J-T1. 4000x4000px image in 200x200pt → DPI 1440 detected, downscaled to 150
  ✅ J-T2. Full-page image (300 DPI) → downscaled to 150 DPI
  ✅ J-T3. Already within the DPI budget → not downscaled
  ✅ J-T4. No CTM → pixel-based fallback, same as the previous behaviour
  ✅ J-T5. Matrix multiplication correctness verified
```

---

## 6. Real-world test results

### As of v0.1 (Phase A)

**interest_free_loans_brochure.pdf** (0.51 MB, mostly images)

| Preset | Output size | Reduction | Re-encoded | Downscaled |
|--------|----------|--------|---------|-----------|
| low | 0.51 MB | 0.3% | 0 | 0 |
| medium | 0.50 MB | 4.1% | 2 | 0 |
| high | 0.45 MB | 12.8% | 4 | 1 |
| extreme | 0.29 MB | **43.7%** | 4 | 1 |

**translated_33_45.pdf** (69.4 MB, mostly text/fonts)

| Preset | Output size | Reduction | Notes |
|--------|----------|--------|------|
| high | 69.26 MB | 0.2% | Images are only 9.3MB (13%); the rest is fonts/text |
| extreme | 69.05 MB | 0.6% | Large improvement expected once Phases B–D apply |

### Expected effect as Phases complete

| Phase | interest_free (images) | translated (text) |
|-------|:---------------------:|:-------------------:|
| A (current) | 43.7% | 0.6% |
| + B (Flate recompression) | ~45% | ~5% |
| + C (dedup) | ~45% | ~10% |
| + D (font subsetting) | ~45% | ~30~50% |
| + E (unused resources) | ~46% | ~35~55% |
| + F (data removal) | ~48% | ~40~60% |

---

## 7. Risks & decisions

| Item | Decision |
|------|------|
| CMYK JPEG | Skip — risk of loss in color conversion |
| Images with an SMask (transparency) | Skip — JPEG does not support alpha |
| Inline images (BI/ID/EI) | Only XObjects are handled; inline images are lower priority |
| Encrypted PDF | Return an error |
| Larger than the original after re-encoding | Cancel the replacement (safeguard) |
| Font subsetting fails | Keep the original (safeguard) |
| CFF font subsetting | Not supported → TrueType only |
| Grayscale conversion | Only through an option flag (presets do not turn it on) |
| Structure tree removal | Loses accessibility — high/extreme only |
| clean_objects renumbering | Invalidates catalog_ref → use GC only |

---

## 8. WASM constraints

| Constraint | Response |
|------|------|
| Single thread | Use a Web Worker to avoid blocking the UI |
| Memory ~2GB | Warn for PDFs of 100MB+ |
| No file system | `&[u8]` ↔ `Vec<u8>` |
| Bundle size | Renderer excluded, ~1-1.5MB expected |

All dependencies are pure Rust → no WASM blockers.
