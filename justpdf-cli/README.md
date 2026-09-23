# justpdf-cli

Command-line tool for working with PDF files. Part of the [justpdf](https://github.com/kihyun1998/justpdf) project.

## Install

```bash
cargo install justpdf-cli
```

## Commands

| Command   | Description                      |
|-----------|----------------------------------|
| `info`    | Show PDF metadata and page count |
| `text`    | Extract text from a PDF          |
| `render`  | Render pages to PNG/JPEG/SVG     |
| `merge`   | Merge multiple PDFs              |
| `split`   | Extract a page range             |
| `encrypt` | Encrypt a PDF                    |
| `decrypt` | Decrypt a PDF                    |
| `clean`   | Rebuild the file                 |
| `compress`| Compress with a preset + overrides |
| `convert` | Convert between formats          |
| `sign`    | Not implemented yet              |

## Examples

```bash
justpdf info document.pdf
justpdf text document.pdf
justpdf render document.pdf --dpi 150 -o page.png
justpdf merge a.pdf b.pdf -o merged.pdf
justpdf compress document.pdf --preset high -o smaller.pdf
justpdf compress document.pdf --analyze
justpdf compress document.pdf --preset high --no-strip-metadata --jpeg-quality 80 -o out.pdf
justpdf split document.pdf --pages 1-5 -o part.pdf
justpdf encrypt doc.pdf --owner-password secret -o secured.pdf
```

Page numbers are 1-based. Prebuilt binaries are attached to each GitHub release.

## Repository

[https://github.com/kihyun1998/justpdf](https://github.com/kihyun1998/justpdf)

## License

MIT OR Apache-2.0
