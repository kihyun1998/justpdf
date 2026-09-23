# Language Bindings

justpdf provides bindings for multiple languages. They cover opening documents, authentication, page count, text extraction and PNG rendering.

## Python (PyO3)

```python
import justpdf

doc = justpdf.Document.open("input.pdf")   # or justpdf.open(...)
print(f"Pages: {doc.page_count}")
text = doc.text()
png = doc.render_page(0, dpi=150)
```

## Node.js (napi-rs)

```javascript
const { Document } = require('justpdf');

const doc = Document.open('input.pdf');
console.log(`Pages: ${doc.pageCount}`);
const text = doc.text();
const png = doc.renderPage(0, 150);
```

## WebAssembly

```javascript
import init, { WasmDocument } from 'justpdf-wasm';

await init();
const doc = new WasmDocument(new Uint8Array(buffer));
console.log(doc.page_count);
const png = doc.render_page_png(0, 150);
```

For PDF compression in the browser, use `justpdf-compress-wasm` instead.

## C FFI

```c
#include "justpdf.h"

JustPdfDocument *doc = NULL;
if (justpdf_open("input.pdf", &doc) == JUSTPDF_OK) {
    unsigned int pages = 0;
    justpdf_page_count(doc, &pages);
    justpdf_close(doc);
}
```

Functions return a status code (`JUSTPDF_OK` on success) and write results through out-parameters.
