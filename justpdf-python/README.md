# justpdf-python

Python bindings for the [justpdf](https://github.com/kihyun1998/justpdf) project.

Read, render, and extract text from PDF documents in Python via native Rust bindings (PyO3).

## Installation

```bash
pip install justpdf
```

## Usage

```python
import justpdf

# Open a PDF
doc = justpdf.open("document.pdf")
# or
doc = justpdf.Document.open("document.pdf")

print(f"Pages: {doc.page_count}")
print(f"Version: {doc.version}")

# Extract text
text = doc.text()
print(text)

# Extract text from a specific page
page_text = doc.page_text(0)

# Render a page to PNG
png_bytes = doc.render_page(0, dpi=150.0)
with open("page0.png", "wb") as f:
    f.write(png_bytes)

# Page info
page = doc.page(0)
print(f"Size: {page.width} x {page.height}")
print(f"Rotation: {page.rotation}")

# Open from bytes
with open("document.pdf", "rb") as f:
    doc2 = justpdf.Document.from_bytes(f.read())

# Encrypted PDF
doc2.authenticate("password")

# Pythonic access
print(len(doc))       # page count
print(doc[0])         # first page
print(doc[-1])        # last page
```

## API

### Document

| Property / Method | Description |
|---|---|
| `justpdf.open(path)` | Open a PDF file (shorthand) |
| `Document.open(path)` | Open a PDF file |
| `Document.from_bytes(data)` | Open a PDF from bytes |
| `doc.authenticate(password)` | Authenticate an encrypted PDF |
| `doc.page_count` | Number of pages |
| `doc.version` | PDF version string |
| `doc.is_encrypted` | Whether the document is encrypted |
| `doc.title` | Document title |
| `doc.author` | Document author |
| `doc.subject` | Document subject |
| `doc.text()` | Extract text from all pages |
| `doc.page_text(index)` | Extract text from a specific page (0-based) |
| `doc.render_page(index, dpi=150.0)` | Render a page to PNG bytes |
| `doc.render_page_to_file(index, path, dpi=150.0)` | Render and save to file |
| `doc.page(index)` | Get a Page object |
| `len(doc)` | Page count |
| `doc[index]` | Get page by index (supports negative) |

### Page

| Property | Description |
|---|---|
| `page.width` | Page width in points |
| `page.height` | Page height in points |
| `page.rotation` | Page rotation in degrees |

## Building

```bash
pip install maturin
maturin develop --release
```

## Repository

[https://github.com/kihyun1998/justpdf](https://github.com/kihyun1998/justpdf)

## License

MIT OR Apache-2.0
