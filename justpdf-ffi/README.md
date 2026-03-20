# justpdf-ffi

C FFI bindings for the [justpdf](https://github.com/kihyun1998/justpdf) project.

Exposes justpdf functionality through a C-compatible API using opaque handles and error codes.

## Usage

Link against the built `justpdf_ffi` shared library and include the header.

### Example

```c
#include "justpdf.h"

JustPdfDocument *doc;
justpdf_open("document.pdf", &doc);

char *text;
justpdf_extract_all_text(doc, &text);
printf("%s\n", text);

justpdf_free_string(text);
justpdf_close(doc);
```

## API Pattern

- Functions return integer error codes (0 = success)
- Document and resource handles are opaque pointers
- Strings returned by the library must be freed with `justpdf_free_string`
- Documents must be closed with `justpdf_close`

## Building

```bash
cargo build -p justpdf-ffi --release
```

The shared library will be in `target/release/`.

## Repository

[https://github.com/kihyun1998/justpdf](https://github.com/kihyun1998/justpdf)

## License

MIT OR Apache-2.0
