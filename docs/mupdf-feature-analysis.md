# Full analysis of MuPDF features

> Analysis based on the MuPDF source code (the mupdf-reference checkout used at the time of writing is not included in this repository)
> A feature reference for the justpdf project

---

## 1. Supported input formats

### Document formats
| Format | Extension | Description |
|------|--------|------|
| PDF | `.pdf` | PDF 1.0 ~ 2.0 |
| XPS | `.xps`, `.oxps` | XML Paper Specification / OpenXPS |
| EPUB | `.epub` | Electronic Publication (ZIP-based) |
| MOBI | `.mobi`, `.prc`, `.pdb` | Mobipocket eBook |
| FB2 | `.fb2` | FictionBook 2.0 |
| HTML | `.html`, `.htm`, `.xhtml` | HTML5 / XHTML |
| TXT | `.txt`, `.text`, `.log` | Plain text |
| SVG | `.svg` | Scalable Vector Graphics |
| CBZ/CBT/CBR | `.cbz`, `.cbt`, `.cbr` | Comic Book Archive (ZIP/TAR/RAR) |
| Office | `.docx`, `.xlsx`, `.pptx`, `.hwpx` | MS Office / Hancom |

### Image formats (input)
| Format | Description |
|------|------|
| JPEG | DCT-compressed image |
| PNG | Portable Network Graphics |
| TIFF | Tagged Image File Format |
| BMP | Bitmap |
| GIF | Graphics Interchange Format |
| PSD | Adobe Photoshop |
| PNM/PBM/PGM/PPM/PAM | Portable Anymap family |
| JBIG2 | Bi-level image compression |
| JPEG2000 (JP2/JPX/J2K) | Wavelet-based image compression |
| JPEG-XR (JXR/HDP/WDP) | Microsoft HD Photo |

---

## 2. Supported output formats

| Format | File | Description |
|------|------|------|
| PDF | `output-pdfocr.c` | PDF with an OCR text layer |
| CBZ | `output-cbz.c` | Comic Book ZIP |
| SVG | `output-svg.c` | Scalable Vector Graphics |
| PNG | `output-png.c` | Raster image |
| JPEG | `output-jpeg.c` | Raster image |
| PNM | `output-pnm.c` | Portable Anymap |
| PSD | `output-psd.c` | Adobe Photoshop |
| PCL | `output-pcl.c` | Printer Command Language |
| PCLM | `output-pclm.c` | PCL for Mobile |
| PWG | `output-pwg.c` | Printer Working Group Raster |
| PostScript | `output-ps.c` | PostScript |
| CSV | `output-csv.c` | Text extraction (tabular) |
| DOCX | `output-docx.c` | Microsoft Word |

---

## 3. Core PDF features

### 3.1 PDF object model
- **Basic types**: Null, Bool, Int, Real, Name, String, Array, Dict, Stream, Indirect Reference
- Full CRUD: **create/read/update/delete objects**
- **Cross-reference (xref) table** management
  - Regular xref, incremental xref
  - xref repair (recovering damage)
  - Linearized PDF support
- **Object Streams** support
- **Deep copy**, compare, marking

### 3.2 Annotations

**Supported annotation types (28)**:
| Type | Description |
|------|------|
| Text | Text note (popup) |
| Link | Hyperlink |
| FreeText | Free text (including callout) |
| Line | Straight line (including leader lines, caption) |
| Square | Rectangle |
| Circle | Circle/ellipse |
| Polygon | Polygon |
| PolyLine | Polyline |
| Highlight | Text highlight |
| Underline | Underline |
| Squiggly | Squiggly underline |
| StrikeOut | Strikethrough |
| Redact | Redaction mark |
| Stamp | Stamp (including image stamps) |
| Caret | Insertion caret |
| Ink | Freehand drawing (ink list) |
| Popup | Popup window |
| FileAttachment | File attachment |
| Sound | Sound |
| Movie | Movie |
| RichMedia | Rich media |
| Widget | Form field |
| Screen | Screen |
| PrinterMark | Printer's mark |
| TrapNet | Trap network |
| Watermark | Watermark |
| 3D | 3D content |
| Projection | Projection |

**Annotation properties**:
- Rect, Color, Interior Color, Opacity, Border (width/style/dash/effect)
- Line Ending Styles: None, Square, Circle, Diamond, OpenArrow, ClosedArrow, Butt, ROpenArrow, RClosedArrow, Slash
- Border Styles: Solid, Dashed, Beveled, Inset, Underline
- Border Effects: None, Cloudy
- Intent: Default, FreeText Callout/Typewriter, Line Arrow/Dimension, Polygon Cloud/Dimension, Stamp Image/Snapshot
- Flags: Invisible, Hidden, Print, NoZoom, NoRotate, NoView, ReadOnly, Locked, ToggleNoView, LockedContents
- Quad Points (text markup), Ink List (ink), Vertices (polygon)
- Callout Line, Line Leader/Extension/Offset, Caption
- Author, Contents, CreationDate, ModificationDate
- Default Appearance (font, size, color)
- Rich Contents, Rich Defaults
- File Specification (attachment)
- Appearance Stream generation/compositing

### 3.3 Form fields (Interactive Forms / AcroForms)

**Widget types**:
| Type | Description |
|------|------|
| Button | Push button |
| Checkbox | Checkbox |
| RadioButton | Radio button |
| Text | Text input |
| ComboBox | Combo box (drop-down) |
| ListBox | List box |
| Signature | Signature field |

**Text field formats**: None, Number, Special, Date, Time

**Field flags**:
- ReadOnly, Required, NoExport
- Text: Multiline, Password, FileSelect, DoNotSpellCheck, DoNotScroll, Comb, RichText
- Button: NoToggleToOff, Radio, Pushbutton, RadiosInUnison
- Choice: Combo, Edit, Sort, MultiSelect, DoNotSpellCheck, CommitOnSelChange

**Form operations**:
- Create/read/update/delete fields
- Form calculate, reset
- Keystroke/Validate/Calculate/Format events
- Document events (WillClose, WillSave, DidSave, WillPrint, DidPrint)
- Page events (Open, Close)
- Annotation events (Enter, Exit, Down, Up, Focus, Blur, PageOpen/Close/Visible/Invisible)
- Baking (turning forms into static content)

### 3.4 Digital Signatures

**Signature features**:
- PKCS#7-based signing/verification
- Signature creation (Signer interface)
- Signature verification (Verifier interface)
- Certificate chain verification
- Digest verification
- Signer information extraction (Distinguished Name)
- Signature preview (Display List / Pixmap)
- Signature appearance customization (Labels, DN, Date, TextName, GraphicName, Logo)
- Incremental change detection (whether it changed after signing)
- Byte Range verification
- Field locking (Locked Fields)

**Signature error codes**: OK, NoSignatures, NoCertificate, DigestFailure, SelfSigned, SelfSignedInChain, NotTrusted, NotSigned, Unknown

### 3.5 Encryption/security (Encryption)

**Encryption methods**:
| Method | Description |
|------|------|
| RC4-40 | 40-bit RC4 (PDF 1.1~1.3) |
| RC4-128 | 128-bit RC4 (PDF 1.4) |
| AES-128 | 128-bit AES (PDF 1.5) |
| AES-256 | 256-bit AES (PDF 1.7 ext3, 2.0) |

**Permission flags**:
| Flag | Description |
|--------|------|
| Print | Printing |
| Modify | Modifying the document |
| Copy | Copying text/images |
| Annotate | Adding/modifying annotations |
| Form | Filling form fields |
| Accessibility | Text extraction for accessibility |
| Assemble | Assembling the document (inserting/deleting pages) |
| PrintHQ | High-quality printing |

**Encryption operations**:
- Password authentication (Owner/User password)
- Stream/string encryption/decryption
- Metadata encryption option

### 3.6 Page manipulation

**Page Box types**: MediaBox, CropBox, BleedBox, TrimBox, ArtBox

**Page operations**:
- Page create/insert/delete/delete range
- Page transform, bounding box
- Transparency detection
- Separations extraction
- Resource/content/group access
- Page rendering (all, contents only, annotations only, widgets only)
- Page filtering (contents/annotations)
- Page editing (Redaction)
- Page vectorizing (Vectorize)
- Page clipping
- Presentation transitions
- Default color space load/update

**Redaction options**:
- black_boxes: cover with black boxes
- image_method: None, Remove, Pixels, RemoveUnlessInvisible
- line_art: None, RemoveIfCovered, RemoveIfTouched
- text: Remove, None, RemoveInvisible

### 3.7 Document-level features

**Bookmarks/outlines**: loading, iterator, tree traversal

**Layers/Optional Content (OCG)**:
- Layer configuration (config) management
- Layer enable/disable
- Layer UI information (checkbox, radiobutton, label)
- Saving layer configuration

**Page labels**: read/set/delete, styles (Decimal, UpperRoman, LowerRoman, UpperAlpha, LowerAlpha)

**Metadata**: Title, Author, Subject, Keywords, Creator, Producer, CreationDate, ModDate, etc.

**Journal/Undo-Redo**:
- Journal enabling
- Begin/end/discard operation
- Undo/Redo
- Journal serialization/deserialization
- Journal save/load

**Graft (page grafting)**:
- Bringing objects/pages in from another PDF
- Efficient copying through a Graft Map

**Document save options**:
| Option | Description |
|------|------|
| incremental | Incremental save |
| pretty | Pretty-printed format |
| ascii | ASCII encoding |
| compress | Stream compression |
| compress_images | Image compression |
| compress_fonts | Font compression |
| decompress | Stream decompression |
| garbage | Removing unused objects (levels 0~4) |
| linear | Linearized PDF generation |
| clean | Syntax cleanup |
| sanitize | Sanitizing |
| appearance | Appearance stream generation |
| encrypt | Encryption method setting |
| snapshot | Snapshot save |
| preserve_metadata | Keeping metadata |
| use_objstms | Using object streams |
| compression_effort | Compression effort level |
| labels | Including page labels |

### 3.8 JavaScript support
- JS engine enable/disable
- Event handling (init, result, validate, keystroke)
- Script execution
- Console access

### 3.9 ZUGFeRD (electronic invoices)
- Profile detection: Comfort, Basic, Extended, BasicWL, Minimum, XRechnung
- XML data extraction

### 3.10 Image rewriting (Image Rewriter)

**Subsampling methods**: Average, Bicubic

**Recompression methods**: Never, Same, Lossless, JPEG, J2K, FAX

**Options**:
- Subsampling threshold and target DPI per color/grayscale/bi-level image
- Recompression quality setting
- Size comparison (only when smaller / always)

### 3.11 Recoloring
- Page color conversion (Gray, RGB, CMYK)
- Output Intent removal

### 3.12 Document cleanup/repair (Clean)
- Structure handling: Drop, Keep
- Vectorizing: Yes, No
- File cleanup (`pdf_clean_file`)
- Page rearranging (`pdf_rearrange_pages`)
- Image/font optimization

---

## 4. Rendering engine (Fitz Core)

### 4.1 Device model
| Device | Description |
|----------|------|
| Draw Device | Rasterization (rendering to a Pixmap) |
| Display List Device | Command recording/replay |
| Text Device | Structured text extraction |
| BBox Device | Bounding box computation |
| Trace Device | Debug output |
| SVG Device | SVG vector output |
| OCR Device | Tesseract OCR integration |
| XML Text Device | XML text output |

### 4.2 Path rendering
- MoveTo, LineTo, CurveTo (cubic Bézier), ClosePath
- Degenerate curve handling
- Rect shorthand
- Fill (Even-Odd / Winding) and Stroke
- Line Cap: Butt, Round, Square, Triangle
- Line Join: Miter, Round, Bevel, MiterXPS
- Dash Pattern support
- Miter Limit

### 4.3 Blend modes (16)
- Normal
- Multiply, Screen, Overlay
- Darken, Lighten
- ColorDodge, ColorBurn
- HardLight, SoftLight
- Difference, Exclusion
- Hue, Saturation, Color, Luminosity
- Isolated / Knockout groups

### 4.4 Color Spaces
| Color space | Description |
|--------|------|
| DeviceGray | Grayscale |
| DeviceRGB | RGB |
| DeviceBGR | BGR (screen order) |
| DeviceCMYK | CMYK |
| Lab | CIE L*a*b |
| Indexed | Palette-based |
| Separation | Spot color |
| DeviceN | Multiple colorants |
| ICC Profile | ICC profile-based (all types) |
| CalGray | Calibrated Gray |
| CalRGB | Calibrated RGB |

**Color space features**:
- Up to 32 colorants
- ICC profile color conversion (LCMS integration)
- Rendering intents: Perceptual, RelativeColorimetric, Saturation, AbsoluteColorimetric
- Black Point Compensation
- Overprint support
- Default color space management

### 4.5 Font system
**Supported font types**: TrueType, Type1, Type3, CFF, OpenType

**Font features**:
- Font embedding/subsetting
- Synthetic Bold/Italic
- Small-caps conversion
- Glyph substitution and fallback
- HarfBuzz text shaping
- CJK font support (CNS, GB, Japan, Korea)
- Noto fonts (100+ scripts)
- Base14 standard PDF fonts
- Ligature handling/expansion
- Glyph caching
- FreeType integration

**Encodings**: ISO-8859-1, ISO-8859-7, KOI8-U, KOI8-R, Windows-1250/1251/1252, MacRoman, MacExpert, AdobeStandard, WinAnsi, PDFDocEncoding

### 4.6 Image processing
- Decoding: JPEG, PNG, TIFF, BMP, GIF, PSD, PNM, JBIG2, JPEG2000, JPEG-XR
- Encoding: PNG, JPEG
- Pixmap manipulation (create, convert, scale, gamma correction, invert, tint)
- Image masks, soft masks
- Inline images

### 4.7 Shading/gradients
- Function-based (Type 1)
- Axial (Type 2, linear)
- Radial (Type 3, circular)
- Free-form Gouraud (Type 4)
- Lattice-form Gouraud (Type 5)
- Coons patch (Type 6)
- Tensor-product patch (Type 7)

---

## 5. Text extraction

### 5.1 Extraction modes/options
| Option | Description |
|------|------|
| Preserve Ligatures | Keep ligatures vs expand them |
| Preserve Whitespace | Keep whitespace vs normalize it |
| Extract Images | Extract images within the text flow |
| Inhibit Spaces | Suppress automatic space insertion |
| Dehyphenate | Soft hyphen handling |
| Preserve Spans | Split spans by font/color/size |
| Clip to MediaBox | MediaBox clipping |
| Collect Structure | Collect document structure |
| Collect Vectors | Collect vector graphics |
| Page Segmentation | Page segmentation |
| Table Detection | Table detection/marking |
| CID/GID to Unicode | CID/GID to Unicode conversion |

### 5.2 Structured Text
- **Blocks**: text blocks, image blocks
- **Lines**: direction (wmode), bounding box
- **Characters**: position, size, font, color, Unicode code point
- Text search (including regular expressions)
- Table extraction (stext-table)
- Paragraph detection (stext-para)
- Classification (stext-classify)
- Boxer (stext-boxer) - layout analysis

---

## 6. Cryptography/compression

### 6.1 Hash functions
- MD5
- SHA-256, SHA-384, SHA-512

### 6.2 Symmetric ciphers
- AES (128/192/256-bit, CBC mode)
- RC4/ARC4 (stream cipher)

### 6.3 Compression/filters
| Filter | Description |
|------|------|
| Flate/Deflate | LZ77-based (configurable window) |
| LZW | Lempel-Ziv-Welch |
| CCITT Fax | Group 3 (1D), Group 4 (2D) |
| DCT/JPEG | JPEG compression |
| JBIG2 | Bi-level image compression (globals supported) |
| JPX/JPEG2000 | Wavelet-based |
| Brotli | Modern compression (levels 0-11) |
| Run-Length | Run-length encoding |
| ASCII85 | ASCII85 decoding |
| ASCIIHex | Hexadecimal decoding |
| Predictor | PNG/TIFF predictors |
| SGI Log | Grayscale log encoding |
| Thunder | Thunder compression |

---

## 7. Special features

### 7.1 Barcodes
**Types supported for generation (20+)**:
Aztec, CODABAR, Code39, Code93, Code128, DataBar, DataBarExpanded, DataBarLimited, DataMatrix, DXFilmEdge, EAN8, EAN13, ITF, MaxiCode, PDF417, QRCode, MicroQR, RectMicroQR, UPC-A, UPC-E

**Barcode features**: generation, decoding (from a page/pixmap/display list), error correction level, margins, human-readable text

### 7.2 OCR (Tesseract)
- Tesseract OCR engine integration
- Text layer generation through the OCR device
- PDF-OCR output (scanned document → searchable PDF)

### 7.3 Bidirectional text (BiDi)
- Unicode Bidirectional Algorithm
- RTL/LTR direction detection
- Script-aware text handling

### 7.4 Hyphenation
- Per-language hyphenation dictionaries
- Word-splitting patterns

### 7.5 Page transitions
Split, Blinds, Box, Wipe, Dissolve, Glitter, Fly, Push, Cover, Uncover, Fade

### 7.6 Story/Layout engine
- HTML/CSS-style text layout
- Multi-page story placement
- Text overflow handling
- Image embedding

### 7.7 Deskew (skew correction)
- Skew detection in scanned documents
- Automatic correction (rotation)
- Border handling options

### 7.8 Archive handling
- Reading ZIP, TAR, Directory
- Archive enumeration and content access
- Transparent GZIP decompression

### 7.9 Warp (distortion correction)
- Image/page distortion correction

---

## 8. Command-line tool (mutool)

| Tool | Description |
|------|------|
| `mutool draw` | Render a document to images |
| `mutool convert` | Convert document formats |
| `mutool clean` | Clean up/repair/optimize PDF syntax |
| `mutool merge` | Merge several PDFs |
| `mutool extract` | Extract embedded fonts/images |
| `mutool create` | Create a PDF from text pages |
| `mutool show` | Show internal PDF objects |
| `mutool info` | Show PDF resource information |
| `mutool pages` | Show page information |
| `mutool poster` | Split large pages into tiles |
| `mutool sign` | Manage digital signatures |
| `mutool trace` | Trace rendering device calls |
| `mutool run` | JavaScript execution environment |
| `mutool grep` | Text search |
| `mutool audit` | PDF usage statistics |
| `mutool bake` | Turn forms into static content |
| `mutool barcode` | Barcode encoding/decoding |
| `mutool recolor` | PDF color space conversion |
| `mutool trim` | Trim page contents |

---

## 9. Architecture layers

```
+--------------------------------------------------+
|              Application / Tools Layer            |
|  (mutool, viewers, language bindings)             |
+--------------------------------------------------+
|                   PDF Layer                       |
|  (pdf-annot, pdf-form, pdf-crypt, pdf-font,      |
|   pdf-page, pdf-xref, pdf-write, pdf-signature,  |
|   pdf-layer, pdf-js, pdf-zugferd, ...)            |
+--------------------------------------------------+
|            Fitz Core Layer (format-agnostic)      |
|  - Document abstraction (multi-format)            |
|  - Device model (draw, list, text, svg, ...)      |
|  - Rendering engine (path, glyph, image, shade)   |
|  - Color management (ICC, LCMS)                   |
|  - Font system (FreeType, HarfBuzz)               |
|  - Image codecs (JPEG, PNG, TIFF, JBIG2, JP2...) |
|  - Compression (Flate, LZW, Fax, Brotli, ...)    |
|  - Crypto (AES, RC4, MD5, SHA)                    |
|  - Text extraction (structured text)              |
|  - Geometry, Pixmap, Buffer, Stream, XML, JSON    |
+--------------------------------------------------+
|          Format Parsers                           |
|  XPS | HTML/EPUB | CBZ | SVG | MOBI | FB2 | TXT  |
+--------------------------------------------------+
|        Third-party Libraries                      |
|  FreeType | HarfBuzz | libjpeg | libpng | zlib   |
|  OpenJPEG | jbig2dec | LCMS2 | Brotli | Tesseract|
+--------------------------------------------------+
```

---

## 10. Suggested implementation priorities for justpdf

### Phase 1: Core (MVP)
- [ ] PDF parser (object model, xref, streams)
- [ ] Basic rendering (paths, text, images)
- [ ] Basic color spaces (Gray, RGB, CMYK)
- [ ] Basic fonts (Type1, TrueType)
- [ ] Basic compression (Flate, DCT)
- [ ] Text extraction
- [ ] PDF creation/saving

### Phase 2: Completeness
- [ ] Annotations (all)
- [ ] Form fields
- [ ] Encryption/decryption (RC4, AES)
- [ ] Bookmarks/outlines
- [ ] More image formats (PNG, TIFF, JBIG2, JP2)
- [ ] ICC color management
- [ ] Font subsetting

### Phase 3: Advanced features
- [ ] Digital signatures
- [ ] JavaScript support
- [ ] Optional Content (layers)
- [ ] Redaction
- [ ] Linearized PDF
- [ ] Incremental save

### Phase 4: Expansion
- [ ] XPS, EPUB, HTML support
- [ ] SVG input/output
- [ ] Office formats
- [ ] OCR integration
- [ ] Barcodes
