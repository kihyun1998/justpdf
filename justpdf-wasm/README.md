# justpdf-wasm

WebAssembly bindings for the justpdf PDF engine.

## Build

```bash
# Install wasm-pack
cargo install wasm-pack

# Build for web
wasm-pack build --target web

# Build for Node.js
wasm-pack build --target nodejs
```

## Usage (JavaScript)

```javascript
import init, { WasmDocument } from './pkg/justpdf_wasm.js';

await init();

const response = await fetch('document.pdf');
const bytes = new Uint8Array(await response.arrayBuffer());

const doc = new WasmDocument(bytes);
console.log(`Pages: ${doc.page_count}`);
console.log(`Version: ${doc.version}`);

// Extract text
const text = doc.text();
console.log(text);

// Render page to PNG
const png = doc.render_page_png(0, 150.0);
const blob = new Blob([png], { type: 'image/png' });
const url = URL.createObjectURL(blob);
document.getElementById('preview').src = url;
```
