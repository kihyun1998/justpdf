# CLI Tool

## Installation

```bash
cargo install justpdf-cli
```

## Commands

### Extract text
```bash
justpdf text input.pdf
justpdf text input.pdf --page 0
```

### Render pages
```bash
justpdf render input.pdf --output page.png --dpi 300
justpdf render input.pdf --format jpeg --pages 0-4
```

### Document info
```bash
justpdf info input.pdf
```

### Merge PDFs
```bash
justpdf merge doc1.pdf doc2.pdf -o merged.pdf
```

### Split PDF
```bash
justpdf split input.pdf --output-dir pages/
```
