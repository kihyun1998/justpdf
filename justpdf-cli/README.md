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
| `render`  | Render pages to PNG/JPEG         |
| `merge`   | Merge multiple PDFs              |
| `split`   | Split a PDF into pages           |
| `encrypt` | Encrypt a PDF                    |
| `decrypt` | Decrypt a PDF                    |
| `clean`   | Remove metadata/annotations      |
| `convert` | Convert between formats          |

## Examples

```bash
justpdf info document.pdf
justpdf text document.pdf
justpdf render document.pdf --dpi 150 -o page.png
justpdf merge a.pdf b.pdf -o merged.pdf
justpdf encrypt doc.pdf --owner-password secret -o secured.pdf
```

## Repository

[https://github.com/kihyun1998/justpdf](https://github.com/kihyun1998/justpdf)

## License

MIT OR Apache-2.0
