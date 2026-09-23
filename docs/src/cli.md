# CLI Tool

## Installation

```bash
cargo install justpdf-cli
```

Prebuilt binaries are attached to each GitHub release. Page numbers on the command line are **1-based**.

## Commands

### Document info
```bash
justpdf info input.pdf
justpdf info encrypted.pdf --password secret
```

### Extract text
```bash
justpdf text input.pdf
justpdf text input.pdf --page 1
justpdf text input.pdf --format markdown   # plain, html, json, markdown
```

### Render pages
```bash
justpdf render input.pdf -o page.png --dpi 300            # page 1
justpdf render input.pdf --page 3 -F jpeg --quality 90 -o page3.jpg
justpdf render input.pdf --all -o pages/                  # every page into a directory
justpdf render input.pdf -F svg -o page.svg
```

### Merge PDFs
```bash
justpdf merge doc1.pdf doc2.pdf -o merged.pdf
```

### Split (extract pages)
```bash
justpdf split input.pdf --pages 1-5 -o first-five.pdf     # also "1,3,5" or "2-"
```

### Compress
```bash
justpdf compress input.pdf --preset high -o smaller.pdf  # low, medium (default), high, extreme
justpdf compress input.pdf --analyze                      # pages, images, image bytes, encryption; writes nothing
justpdf compress input.pdf -o smaller.pdf --verbose       # also print what changed (stderr)
justpdf compress secret.pdf -o smaller.pdf --password pw  # output is written WITHOUT encryption
```

Override individual preset settings; a flag wins over the preset, unset knobs keep the preset value:

```bash
justpdf compress input.pdf --preset high --no-strip-metadata --jpeg-quality 80 -o out.pdf
```

- On/off pairs: `--structural`, `--compress-streams`, `--font-subsetting`, `--strip-metadata`, `--strip-extras`, `--grayscale` (each with a `--no-` form)
- Images: `--jpeg-quality <1-100>`, `--max-dpi <N>`, `--skip-below <bytes>`, `--no-image-recompress`, `--no-downscale`

Unused-resource removal follows the preset and has no flag.

### Encrypt / decrypt
```bash
justpdf encrypt input.pdf --owner-password owner --user-password user -o locked.pdf
justpdf encrypt input.pdf --owner-password owner --no-print --no-copy -o locked.pdf
justpdf decrypt locked.pdf --password user -o unlocked.pdf
```

`encrypt` uses AES-128.

### Clean
```bash
justpdf clean input.pdf -o cleaned.pdf
```

### Convert
```bash
justpdf convert document.epub -o output.pdf
justpdf convert report.docx -o preview.png
justpdf convert input.pdf -o page1.svg      # PDF → svg / png (first page) / txt
```

Supported inputs: PDF, plain text, SVG, EPUB, CBZ, XPS, DOCX/XLSX/PPTX. The output format comes from the output extension, or `-F`.

### Sign
`justpdf sign` is not implemented yet: it prints a notice and exits without signing.
