# Split the workspace by external dependency layer

The primary principle behind splitting the PDF engine into 8 crates is **layering by external dependency**. Users pull in only as far as the layer they need, which lets them control binary size and compile time.

## Layers

- `justpdf-core` — external deps only (`flate2`, `ttf-parser`, `aes`, the signing stack and so on). The PDF specification itself (parser/object model/crypto/signing/text extraction).
- `justpdf-render` — + `tiny-skia`. Rasterization only.
- `justpdf-formats` — + `zip`, `roxmltree`. Adapters from non-PDF input formats (EPUB/XPS/Office/CBZ/MOBI/FB2/SVG/Plaintext) → PDF.
- `justpdf-special` — + `qrcode`, `unicode-bidi`. Add-on features inside PDF (OCR/Barcode/ZUGFeRD/BiDi/Deskew).
- `justpdf` — the combined `core + render` entry point. `formats`/`special` are **deliberately left out** — in line with the primary principle, it "pulls in only the minimum".

## Trade-offs

The alternative considered was **a single crate + combinations of feature flags**. Rust's compilation unit is the crate, so splitting into crates isolates compile time more effectively, and users can look at the dependency graph and explicitly pick only what they need, so the split was chosen.

## Consequences

- `compress-wasm` not depending on `render` is in line with the primary principle — it saves size in the browser.
- When adding a new PDF feature, "which layer's deps does it need?" becomes the criterion for where it goes.
