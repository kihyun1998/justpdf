# justpdf-node

Node.js bindings for the [justpdf](https://github.com/kihyun1998/justpdf) project.

Read, render, and extract text from PDF documents in Node.js via native N-API bindings.

## Installation

```bash
npm install justpdf
```

## Usage

```javascript
const { Document } = require('justpdf');

// Open from file
const doc = Document.open('document.pdf');
console.log(`Pages: ${doc.pageCount}`);
console.log(`Version: ${doc.version}`);

// Extract text
const text = doc.text();
console.log(text);

// Extract text from a specific page
const pageText = doc.pageText(0);

// Render a page to PNG
const png = doc.renderPage(0, 150.0);
require('fs').writeFileSync('page0.png', png);

// Page dimensions
const width = doc.pageWidth(0);
const height = doc.pageHeight(0);

// Open from Buffer
const buf = require('fs').readFileSync('document.pdf');
const doc2 = Document.fromBuffer(buf);

// Encrypted PDF
doc2.authenticate('password');
```

## API

| Property / Method | Description |
|---|---|
| `Document.open(path)` | Open a PDF file |
| `Document.fromBuffer(buf)` | Open a PDF from a Buffer |
| `doc.authenticate(password)` | Authenticate an encrypted PDF |
| `doc.pageCount` | Number of pages |
| `doc.version` | PDF version string |
| `doc.isEncrypted` | Whether the document is encrypted |
| `doc.title` | Document title |
| `doc.author` | Document author |
| `doc.text()` | Extract text from all pages |
| `doc.pageText(index)` | Extract text from a specific page (0-based) |
| `doc.renderPage(index, dpi?)` | Render a page to PNG (returns Buffer) |
| `doc.pageWidth(index)` | Page width in points |
| `doc.pageHeight(index)` | Page height in points |

## Building

```bash
npm install
npm run build
```

## Repository

[https://github.com/kihyun1998/justpdf](https://github.com/kihyun1998/justpdf)

## License

MIT OR Apache-2.0
