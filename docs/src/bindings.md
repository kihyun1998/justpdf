# Language Bindings

justpdf provides bindings for multiple languages.

## Python (PyO3)

```python
import justpdf

doc = justpdf.Document.open("input.pdf")
print(f"Pages: {doc.page_count}")
text = doc.text()
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
import init, { Document } from 'justpdf-wasm';

await init();
const doc = Document.fromBytes(new Uint8Array(buffer));
```

## C FFI

```c
#include "justpdf.h"

JustpdfDocument *doc = justpdf_open("input.pdf");
int pages = justpdf_page_count(doc);
justpdf_free(doc);
```
