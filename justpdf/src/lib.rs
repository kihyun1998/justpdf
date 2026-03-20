//! # justpdf
//!
//! Pure Rust PDF engine with a high-level API for reading, rendering,
//! extracting text, creating, and modifying PDF documents.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use justpdf::Document;
//!
//! // Open and read
//! let doc = Document::open("input.pdf").unwrap();
//! println!("Pages: {}", doc.page_count());
//!
//! // Extract text
//! let page = doc.page(0).unwrap();
//! println!("{}", page.text().unwrap());
//!
//! // Render to PNG
//! let png = page.render_png(150.0).unwrap();
//! std::fs::write("page1.png", &png).unwrap();
//! ```

mod error;

pub use error::{Error, Result};

// Re-export low-level APIs for advanced users
pub use justpdf_core as core;
pub use justpdf_render as render;

// Re-export commonly used core types
pub use justpdf_core::error::JustPdfError;
pub use justpdf_core::object::{IndirectRef, PdfDict, PdfObject};

// Re-export the builder API
pub use justpdf_core::writer::{DocumentBuilder, DocumentModifier, PageBuilder};
pub use justpdf_core::writer::{embed_jpeg, embed_png, merge_documents};

// Re-export render types
pub use justpdf_render::render::{OutputFormat, RenderOptions};

use std::path::Path;

use justpdf_core::page::{PageInfo, Rect};
use justpdf_core::PdfDocument;

/// A high-level PDF document handle.
///
/// Provides ergonomic access to pages, text, rendering, metadata,
/// annotations, forms, signatures, and modification.
pub struct Document {
    inner: PdfDocument,
    pages: Vec<PageInfo>,
}

impl Document {
    /// Open a PDF file from a path.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let inner = PdfDocument::open(path.as_ref())?;
        let pages = justpdf_core::page::collect_pages(&inner)?;
        Ok(Self { inner, pages })
    }

    /// Parse a PDF from in-memory bytes.
    pub fn from_bytes(data: Vec<u8>) -> Result<Self> {
        let inner = PdfDocument::from_bytes(data)?;
        let pages = justpdf_core::page::collect_pages(&inner)?;
        Ok(Self { inner, pages })
    }

    /// Open with memory-mapped I/O (requires `mmap` feature).
    #[cfg(feature = "mmap")]
    pub fn open_mmap<P: AsRef<Path>>(path: P) -> Result<Self> {
        let inner = PdfDocument::open_mmap(path.as_ref())?;
        let pages = justpdf_core::page::collect_pages(&inner)?;
        Ok(Self { inner, pages })
    }

    /// Authenticate an encrypted document with a password.
    pub fn authenticate(&mut self, password: &[u8]) -> Result<()> {
        self.inner.authenticate(password)?;
        // Re-collect pages after authentication
        self.pages = justpdf_core::page::collect_pages(&self.inner)?;
        Ok(())
    }

    /// Number of pages in the document.
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// Get a page by 0-based index.
    pub fn page(&self, index: usize) -> Result<Page<'_>> {
        let info = self.pages.get(index).ok_or(Error::PageOutOfRange {
            index,
            count: self.pages.len(),
        })?;
        Ok(Page {
            doc: &self.inner,
            info: info.clone(),
            index,
        })
    }

    /// Iterate over all pages.
    pub fn pages(&self) -> PageIter<'_> {
        PageIter {
            doc: self,
            index: 0,
        }
    }

    /// PDF version (e.g., (1, 7) for PDF 1.7).
    pub fn version(&self) -> (u8, u8) {
        self.inner.version
    }

    /// PDF version as a string (e.g. "1.7").
    pub fn version_string(&self) -> String {
        format!("{}.{}", self.inner.version.0, self.inner.version.1)
    }

    /// Whether the document is encrypted.
    pub fn is_encrypted(&self) -> bool {
        self.inner.is_encrypted()
    }

    /// Whether the document is authenticated (or not encrypted).
    pub fn is_authenticated(&self) -> bool {
        self.inner.is_authenticated()
    }

    /// Whether the document is linearized (web-optimized).
    pub fn is_linearized(&self) -> bool {
        justpdf_core::linearized::read_linearization(&self.inner).is_some()
    }

    /// Get document title from metadata.
    pub fn title(&self) -> Option<String> {
        self.metadata_string(b"Title")
    }

    /// Get document author from metadata.
    pub fn author(&self) -> Option<String> {
        self.metadata_string(b"Author")
    }

    /// Get document subject from metadata.
    pub fn subject(&self) -> Option<String> {
        self.metadata_string(b"Subject")
    }

    /// Get document keywords from metadata.
    pub fn keywords(&self) -> Option<String> {
        self.metadata_string(b"Keywords")
    }

    /// Get document creator from metadata.
    pub fn creator(&self) -> Option<String> {
        self.metadata_string(b"Creator")
    }

    /// Get document producer from metadata.
    pub fn producer(&self) -> Option<String> {
        self.metadata_string(b"Producer")
    }

    /// Get creation date string from metadata.
    pub fn creation_date(&self) -> Option<String> {
        self.metadata_string(b"CreationDate")
    }

    /// Get modification date string from metadata.
    pub fn modification_date(&self) -> Option<String> {
        self.metadata_string(b"ModDate")
    }

    /// Get all document metadata as key-value pairs.
    pub fn metadata(&self) -> Vec<(String, String)> {
        let mut result = Vec::new();
        for key in &[
            &b"Title"[..],
            b"Author",
            b"Subject",
            b"Keywords",
            b"Creator",
            b"Producer",
            b"CreationDate",
            b"ModDate",
        ] {
            if let Some(val) = self.metadata_string(key) {
                let key_str = String::from_utf8_lossy(key).to_string();
                result.push((key_str, val));
            }
        }
        result
    }

    /// Extract text from all pages, concatenated.
    pub fn text(&self) -> Result<String> {
        Ok(justpdf_core::text::extract_all_text_string(&self.inner)?)
    }

    /// Search for text across all pages. Returns (page_index, matches) pairs.
    pub fn search(
        &self,
        query: &str,
    ) -> Result<Vec<(usize, Vec<justpdf_core::text::search::SearchResult>)>> {
        let options = justpdf_core::text::search::SearchOptions::default();
        let mut results = Vec::new();
        for (i, page_info) in self.pages.iter().enumerate() {
            let page_text = justpdf_core::text::extract_page_text(&self.inner, page_info)?;
            let matches = justpdf_core::text::search::search_page(&page_text, query, &options);
            if !matches.is_empty() {
                results.push((i, matches));
            }
        }
        Ok(results)
    }

    /// Get bookmarks/outlines.
    pub fn outlines(&self) -> Result<Vec<justpdf_core::outline::OutlineItem>> {
        Ok(justpdf_core::outline::read_outlines(&self.inner)?)
    }

    /// Get page labels.
    pub fn page_labels(&self) -> Result<Vec<justpdf_core::page_label::PageLabelRange>> {
        Ok(justpdf_core::page_label::read_page_labels(&self.inner)?)
    }

    /// Get annotations for a specific page.
    pub fn annotations(
        &self,
        page_index: usize,
    ) -> Result<Vec<justpdf_core::annot::Annotation>> {
        let page_info = self.pages.get(page_index).ok_or(Error::PageOutOfRange {
            index: page_index,
            count: self.pages.len(),
        })?;
        Ok(justpdf_core::annot::get_annotations(&self.inner, page_info)?)
    }

    /// Get form fields (if any).
    pub fn form_fields(&self) -> Result<Option<justpdf_core::form::AcroForm>> {
        Ok(justpdf_core::form::parse_acroform(&self.inner)?)
    }

    /// Get embedded files.
    pub fn embedded_files(&self) -> Result<Vec<justpdf_core::embedded_file::FileSpec>> {
        Ok(justpdf_core::embedded_file::read_embedded_files(&self.inner)?)
    }

    /// Get digital signature information.
    pub fn signatures(&self) -> Result<Vec<justpdf_core::sign::SignatureInfo>> {
        Ok(justpdf_core::sign::detect_signatures(&self.inner)?)
    }

    /// Create a modifier for editing this document.
    ///
    /// The modifier works on a copy of the raw PDF bytes, so the original
    /// `Document` is not affected.
    pub fn modify(&self) -> Result<Modifier> {
        let bytes = self.inner.raw_data().to_vec();
        let doc = PdfDocument::from_bytes(bytes)?;
        let modifier = DocumentModifier::from_document(&doc)?;
        Ok(Modifier { modifier })
    }

    /// Get the underlying PdfDocument for low-level access.
    pub fn inner(&self) -> &PdfDocument {
        &self.inner
    }

    /// Get a mutable reference to the underlying PdfDocument.
    pub fn inner_mut(&mut self) -> &mut PdfDocument {
        &mut self.inner
    }

    /// Consume this Document and return the underlying PdfDocument.
    pub fn into_inner(self) -> PdfDocument {
        self.inner
    }

    /// Render all pages in parallel (requires `parallel` feature).
    #[cfg(feature = "parallel")]
    pub fn render_all_png(&self, dpi: f64) -> Vec<Result<Vec<u8>>> {
        let opts = RenderOptions {
            dpi,
            format: OutputFormat::Png,
            ..Default::default()
        };
        justpdf_render::render_all_pages_parallel(&self.inner, &opts)
            .into_iter()
            .map(|r| r.map_err(Error::from))
            .collect()
    }

    /// Render all pages in parallel with custom options (requires `parallel` feature).
    #[cfg(feature = "parallel")]
    pub fn render_all_parallel(&self, options: &RenderOptions) -> Vec<Result<Vec<u8>>> {
        justpdf_render::render_all_pages_parallel(&self.inner, options)
            .into_iter()
            .map(|r| r.map_err(Error::from))
            .collect()
    }

    // Internal helper to read a metadata string from the /Info dictionary.
    fn metadata_string(&self, key: &[u8]) -> Option<String> {
        let info_ref = self.inner.trailer().get_ref(b"Info")?;
        let info_obj = self.inner.resolve(info_ref).ok()?;
        let info_dict = info_obj.as_dict()?;
        match info_dict.get(key)? {
            justpdf_core::PdfObject::String(s) => {
                // Try UTF-16BE (BOM: FE FF), fallback to Latin-1
                if s.len() >= 2 && s[0] == 0xFE && s[1] == 0xFF {
                    let chars: Vec<u16> = s[2..]
                        .chunks(2)
                        .filter_map(|c| {
                            if c.len() == 2 {
                                Some(u16::from_be_bytes([c[0], c[1]]))
                            } else {
                                None
                            }
                        })
                        .collect();
                    String::from_utf16(&chars).ok()
                } else {
                    Some(s.iter().map(|&b| b as char).collect())
                }
            }
            _ => None,
        }
    }
}

/// A single page of a PDF document.
pub struct Page<'a> {
    doc: &'a PdfDocument,
    info: PageInfo,
    index: usize,
}

impl<'a> Page<'a> {
    /// 0-based page index.
    pub fn index(&self) -> usize {
        self.index
    }

    /// Page width in points (1 point = 1/72 inch).
    pub fn width(&self) -> f64 {
        self.crop_box().width()
    }

    /// Page height in points.
    pub fn height(&self) -> f64 {
        self.crop_box().height()
    }

    /// Page rotation in degrees (0, 90, 180, 270).
    pub fn rotation(&self) -> i64 {
        self.info.rotate
    }

    /// MediaBox rectangle.
    pub fn media_box(&self) -> Rect {
        self.info.media_box
    }

    /// CropBox rectangle (falls back to MediaBox).
    pub fn crop_box(&self) -> Rect {
        self.info.crop_box.unwrap_or(self.info.media_box)
    }

    /// Extract text from this page as a plain string.
    pub fn text(&self) -> Result<String> {
        Ok(justpdf_core::text::extract_page_text_string(self.doc, &self.info)?)
    }

    /// Extract structured text (with positions, fonts, etc.).
    pub fn text_structured(&self) -> Result<justpdf_core::text::PageText> {
        Ok(justpdf_core::text::extract_page_text(self.doc, &self.info)?)
    }

    /// Render to PNG at the given DPI.
    pub fn render_png(&self, dpi: f64) -> Result<Vec<u8>> {
        let opts = RenderOptions {
            dpi,
            format: OutputFormat::Png,
            ..Default::default()
        };
        Ok(justpdf_render::render::render_page_info(self.doc, &self.info, &opts)?)
    }

    /// Render to JPEG at the given DPI and quality (0-100).
    pub fn render_jpeg(&self, dpi: f64, quality: u8) -> Result<Vec<u8>> {
        let opts = RenderOptions {
            dpi,
            format: OutputFormat::Jpeg { quality },
            ..Default::default()
        };
        Ok(justpdf_render::render::render_page_info(self.doc, &self.info, &opts)?)
    }

    /// Render to SVG.
    pub fn render_svg(&self) -> Result<String> {
        Ok(justpdf_render::render::render_page_to_svg(self.doc, self.index)?)
    }

    /// Render to raw RGBA pixel data.
    pub fn render_raw(&self, dpi: f64) -> Result<justpdf_render::RenderedPixmap> {
        let opts = RenderOptions {
            dpi,
            format: OutputFormat::RawRgba,
            ..Default::default()
        };
        Ok(justpdf_render::render::render_page_to_pixmap(self.doc, self.index, &opts)?)
    }

    /// Render with custom options.
    pub fn render(&self, options: &RenderOptions) -> Result<Vec<u8>> {
        Ok(justpdf_render::render::render_page_info(self.doc, &self.info, options)?)
    }

    /// Render to a file (PNG format).
    pub fn render_to_file(&self, path: impl AsRef<Path>, dpi: f64) -> Result<()> {
        let png = self.render_png(dpi)?;
        std::fs::write(path, &png)?;
        Ok(())
    }

    /// Search for text on this page.
    pub fn search(&self, query: &str) -> Result<Vec<justpdf_core::text::search::SearchResult>> {
        let page_text = justpdf_core::text::extract_page_text(self.doc, &self.info)?;
        let options = justpdf_core::text::search::SearchOptions::default();
        Ok(justpdf_core::text::search::search_page(&page_text, query, &options))
    }

    /// Search for text on this page (case-insensitive).
    pub fn search_case_insensitive(
        &self,
        query: &str,
    ) -> Result<Vec<justpdf_core::text::search::SearchResult>> {
        let page_text = justpdf_core::text::extract_page_text(self.doc, &self.info)?;
        let options = justpdf_core::text::search::SearchOptions {
            case_insensitive: true,
            ..Default::default()
        };
        Ok(justpdf_core::text::search::search_page(&page_text, query, &options))
    }

    /// Get the underlying PageInfo for low-level access.
    pub fn info(&self) -> &PageInfo {
        &self.info
    }
}

/// Iterator over document pages.
pub struct PageIter<'a> {
    doc: &'a Document,
    index: usize,
}

impl<'a> Iterator for PageIter<'a> {
    type Item = Page<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.doc.pages.len() {
            let page = Page {
                doc: &self.doc.inner,
                info: self.doc.pages[self.index].clone(),
                index: self.index,
            };
            self.index += 1;
            Some(page)
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.doc.pages.len() - self.index;
        (remaining, Some(remaining))
    }
}

impl<'a> ExactSizeIterator for PageIter<'a> {}

/// Document modifier for editing existing PDFs.
///
/// Created via [`Document::modify()`]. Changes are accumulated in memory
/// and only written out when [`Modifier::build()`] or [`Modifier::save()`]
/// is called.
pub struct Modifier {
    modifier: DocumentModifier,
}

impl Modifier {
    /// Delete a page by 0-based index.
    pub fn delete_page(&mut self, index: usize) -> Result<()> {
        self.modifier.delete_page(index)?;
        Ok(())
    }

    /// Insert a page at the given index.
    pub fn insert_page(&mut self, index: usize, page: PageBuilder) -> Result<()> {
        self.modifier.insert_page(index, page)?;
        Ok(())
    }

    /// Reorder pages according to the given index mapping.
    pub fn reorder_pages(&mut self, order: &[usize]) -> Result<()> {
        self.modifier.reorder_pages(order)?;
        Ok(())
    }

    /// Set the document title.
    pub fn set_title(&mut self, title: &str) {
        self.modifier.set_info(b"Title", title);
    }

    /// Set the document author.
    pub fn set_author(&mut self, author: &str) {
        self.modifier.set_info(b"Author", author);
    }

    /// Set the document subject.
    pub fn set_subject(&mut self, subject: &str) {
        self.modifier.set_info(b"Subject", subject);
    }

    /// Set the document keywords.
    pub fn set_keywords(&mut self, keywords: &str) {
        self.modifier.set_info(b"Keywords", keywords);
    }

    /// Run garbage collection to remove unreachable objects.
    pub fn garbage_collect(&mut self) {
        self.modifier.garbage_collect();
    }

    /// Build the modified PDF bytes.
    pub fn build(self) -> Result<Vec<u8>> {
        Ok(self.modifier.build()?)
    }

    /// Save the modified PDF to a file.
    pub fn save(self, path: impl AsRef<Path>) -> Result<()> {
        let bytes = self.build()?;
        std::fs::write(path, bytes)?;
        Ok(())
    }

    /// Get a reference to the underlying DocumentModifier for advanced use.
    pub fn inner(&self) -> &DocumentModifier {
        &self.modifier
    }

    /// Get a mutable reference to the underlying DocumentModifier.
    pub fn inner_mut(&mut self) -> &mut DocumentModifier {
        &mut self.modifier
    }
}

/// Merge multiple PDF files into one.
pub fn merge(paths: &[impl AsRef<Path>]) -> Result<Vec<u8>> {
    let docs: Vec<PdfDocument> = paths
        .iter()
        .map(|p| PdfDocument::open(p.as_ref()))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let refs: Vec<&PdfDocument> = docs.iter().collect();
    Ok(justpdf_core::writer::merge_documents(&refs)?)
}

/// Merge PDF documents from in-memory byte vectors.
pub fn merge_bytes(pdfs: &[Vec<u8>]) -> Result<Vec<u8>> {
    let docs: Vec<PdfDocument> = pdfs
        .iter()
        .map(|d| PdfDocument::from_bytes(d.clone()))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let refs: Vec<&PdfDocument> = docs.iter().collect();
    Ok(justpdf_core::writer::merge_documents(&refs)?)
}

#[cfg(feature = "async")]
impl Document {
    /// Open a PDF file asynchronously.
    pub async fn open_async(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let data = tokio::fs::read(path.as_ref()).await?;
        Self::from_bytes(data)
    }
}

#[cfg(feature = "async")]
impl<'a> Page<'a> {
    /// Render to PNG and save to a file asynchronously.
    pub async fn render_to_file_async(&self, path: impl AsRef<std::path::Path>, dpi: f64) -> Result<()> {
        let png = self.render_png(dpi)?;
        tokio::fs::write(path, &png).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_test_pdf() -> Vec<u8> {
        let mut doc = DocumentBuilder::new();
        let font = doc.add_standard_font("Helvetica");
        let mut page = PageBuilder::new(612.0, 792.0);
        page.add_font(&font, "Helvetica");
        page.begin_text();
        page.set_font(&font, 24.0);
        page.move_to(72.0, 720.0);
        page.show_text("Hello, World!");
        page.end_text();
        doc.add_page(page);
        doc.set_title("Test Document");
        doc.set_author("justpdf");
        doc.build().unwrap()
    }

    #[test]
    fn test_document_open_from_bytes() {
        let pdf = build_test_pdf();
        let doc = Document::from_bytes(pdf).unwrap();
        assert_eq!(doc.page_count(), 1);
        assert!(!doc.is_encrypted());
        assert!(doc.is_authenticated());
    }

    #[test]
    fn test_page_dimensions() {
        let pdf = build_test_pdf();
        let doc = Document::from_bytes(pdf).unwrap();
        let page = doc.page(0).unwrap();
        assert!((page.width() - 612.0).abs() < 0.01);
        assert!((page.height() - 792.0).abs() < 0.01);
        assert_eq!(page.rotation(), 0);
    }

    #[test]
    fn test_page_text() {
        let pdf = build_test_pdf();
        let doc = Document::from_bytes(pdf).unwrap();
        let page = doc.page(0).unwrap();
        let text = page.text().unwrap();
        assert!(text.contains("Hello"), "text should contain 'Hello', got: {text}");
    }

    #[test]
    fn test_document_text() {
        let pdf = build_test_pdf();
        let doc = Document::from_bytes(pdf).unwrap();
        let text = doc.text().unwrap();
        assert!(text.contains("Hello"));
    }

    #[test]
    fn test_page_render_png() {
        let pdf = build_test_pdf();
        let doc = Document::from_bytes(pdf).unwrap();
        let page = doc.page(0).unwrap();
        let png = page.render_png(72.0).unwrap();
        // PNG magic bytes
        assert_eq!(&png[..4], &[0x89, 0x50, 0x4E, 0x47]);
    }

    #[test]
    fn test_page_render_jpeg() {
        let pdf = build_test_pdf();
        let doc = Document::from_bytes(pdf).unwrap();
        let page = doc.page(0).unwrap();
        let jpeg = page.render_jpeg(72.0, 85).unwrap();
        // JPEG magic bytes
        assert_eq!(&jpeg[..2], &[0xFF, 0xD8]);
    }

    #[test]
    fn test_page_render_svg() {
        let pdf = build_test_pdf();
        let doc = Document::from_bytes(pdf).unwrap();
        let page = doc.page(0).unwrap();
        let svg = page.render_svg().unwrap();
        assert!(svg.starts_with("<?xml") || svg.starts_with("<svg"));
    }

    #[test]
    fn test_page_render_custom() {
        let pdf = build_test_pdf();
        let doc = Document::from_bytes(pdf).unwrap();
        let page = doc.page(0).unwrap();
        let opts = RenderOptions {
            dpi: 72.0,
            format: OutputFormat::Png,
            ..Default::default()
        };
        let data = page.render(&opts).unwrap();
        assert_eq!(&data[..4], &[0x89, 0x50, 0x4E, 0x47]);
    }

    #[test]
    fn test_page_iterator() {
        let pdf = build_test_pdf();
        let doc = Document::from_bytes(pdf).unwrap();
        let pages: Vec<_> = doc.pages().collect();
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].index(), 0);
    }

    #[test]
    fn test_page_iterator_exact_size() {
        let pdf = build_test_pdf();
        let doc = Document::from_bytes(pdf).unwrap();
        let iter = doc.pages();
        assert_eq!(iter.len(), 1);
    }

    #[test]
    fn test_metadata() {
        let pdf = build_test_pdf();
        let doc = Document::from_bytes(pdf).unwrap();
        assert_eq!(doc.title().as_deref(), Some("Test Document"));
        assert_eq!(doc.author().as_deref(), Some("justpdf"));
    }

    #[test]
    fn test_metadata_map() {
        let pdf = build_test_pdf();
        let doc = Document::from_bytes(pdf).unwrap();
        let meta = doc.metadata();
        assert!(meta.iter().any(|(k, v)| k == "Title" && v == "Test Document"));
        assert!(meta.iter().any(|(k, v)| k == "Author" && v == "justpdf"));
    }

    #[test]
    fn test_page_out_of_range() {
        let pdf = build_test_pdf();
        let doc = Document::from_bytes(pdf).unwrap();
        assert!(doc.page(999).is_err());
    }

    #[test]
    fn test_document_version() {
        let pdf = build_test_pdf();
        let doc = Document::from_bytes(pdf).unwrap();
        assert_eq!(doc.version(), (1, 7));
        assert_eq!(doc.version_string(), "1.7");
    }

    #[test]
    fn test_not_pdf() {
        let result = Document::from_bytes(b"not a pdf".to_vec());
        assert!(result.is_err());
    }

    #[test]
    fn test_builder_api_chain() {
        let mut builder = DocumentBuilder::new();
        let font = builder.add_standard_font("Courier");
        let mut p = PageBuilder::new(200.0, 200.0);
        p.add_font(&font, "Courier");
        p.begin_text();
        p.set_font(&font, 12.0);
        p.move_to(10.0, 180.0);
        p.show_text("Builder test");
        p.end_text();
        builder.add_page(p);
        let bytes = builder.build().unwrap();

        let doc = Document::from_bytes(bytes).unwrap();
        assert_eq!(doc.page_count(), 1);
        let text = doc.page(0).unwrap().text().unwrap();
        assert!(text.contains("Builder"));
    }

    #[test]
    fn test_pages_map_text() {
        let pdf = build_test_pdf();
        let doc = Document::from_bytes(pdf).unwrap();
        let texts: Vec<_> = doc.pages().map(|p| p.text().unwrap()).collect();
        assert_eq!(texts.len(), 1);
        assert!(texts[0].contains("Hello"));
    }

    #[test]
    fn test_inner_access() {
        let pdf = build_test_pdf();
        let doc = Document::from_bytes(pdf).unwrap();
        let inner = doc.inner();
        assert_eq!(inner.version, (1, 7));
    }

    #[test]
    fn test_into_inner() {
        let pdf = build_test_pdf();
        let doc = Document::from_bytes(pdf).unwrap();
        let inner = doc.into_inner();
        assert_eq!(inner.version, (1, 7));
    }

    #[test]
    fn test_merge_bytes() {
        let a = build_test_pdf();
        let b = build_test_pdf();
        let merged = merge_bytes(&[a, b]).unwrap();
        let doc = Document::from_bytes(merged).unwrap();
        assert_eq!(doc.page_count(), 2);
    }

    #[test]
    fn test_is_linearized() {
        let pdf = build_test_pdf();
        let doc = Document::from_bytes(pdf).unwrap();
        // A simple built PDF is not linearized
        assert!(!doc.is_linearized());
    }

    #[test]
    fn test_outlines_empty() {
        let pdf = build_test_pdf();
        let doc = Document::from_bytes(pdf).unwrap();
        let outlines = doc.outlines().unwrap();
        assert!(outlines.is_empty());
    }

    #[test]
    fn test_page_labels_empty() {
        let pdf = build_test_pdf();
        let doc = Document::from_bytes(pdf).unwrap();
        let labels = doc.page_labels().unwrap();
        assert!(labels.is_empty());
    }

    #[test]
    fn test_embedded_files_empty() {
        let pdf = build_test_pdf();
        let doc = Document::from_bytes(pdf).unwrap();
        let files = doc.embedded_files().unwrap();
        assert!(files.is_empty());
    }

    #[test]
    fn test_form_fields_none() {
        let pdf = build_test_pdf();
        let doc = Document::from_bytes(pdf).unwrap();
        let form = doc.form_fields().unwrap();
        assert!(form.is_none());
    }

    #[test]
    fn test_signatures_empty() {
        let pdf = build_test_pdf();
        let doc = Document::from_bytes(pdf).unwrap();
        let sigs = doc.signatures().unwrap();
        assert!(sigs.is_empty());
    }

    #[test]
    fn test_annotations_empty() {
        let pdf = build_test_pdf();
        let doc = Document::from_bytes(pdf).unwrap();
        let annots = doc.annotations(0).unwrap();
        assert!(annots.is_empty());
    }

    #[test]
    fn test_annotations_out_of_range() {
        let pdf = build_test_pdf();
        let doc = Document::from_bytes(pdf).unwrap();
        assert!(doc.annotations(999).is_err());
    }

    #[test]
    fn test_modifier_metadata() {
        let pdf = build_test_pdf();
        let doc = Document::from_bytes(pdf).unwrap();
        let mut modifier = doc.modify().unwrap();
        modifier.set_title("New Title");
        modifier.set_author("New Author");
        modifier.set_subject("New Subject");
        let new_pdf = modifier.build().unwrap();

        let doc2 = Document::from_bytes(new_pdf).unwrap();
        assert_eq!(doc2.title().as_deref(), Some("New Title"));
        assert_eq!(doc2.author().as_deref(), Some("New Author"));
        assert_eq!(doc2.subject().as_deref(), Some("New Subject"));
    }

    #[test]
    fn test_modifier_delete_page() {
        // Build a 2-page PDF
        let mut builder = DocumentBuilder::new();
        let font = builder.add_standard_font("Helvetica");
        for i in 0..2 {
            let mut page = PageBuilder::new(612.0, 792.0);
            page.add_font(&font, "Helvetica");
            page.begin_text();
            page.set_font(&font, 12.0);
            page.move_to(72.0, 720.0);
            page.show_text(&format!("Page {}", i + 1));
            page.end_text();
            builder.add_page(page);
        }
        let pdf = builder.build().unwrap();

        let doc = Document::from_bytes(pdf).unwrap();
        assert_eq!(doc.page_count(), 2);

        let mut modifier = doc.modify().unwrap();
        modifier.delete_page(0).unwrap();
        let new_pdf = modifier.build().unwrap();

        let doc2 = Document::from_bytes(new_pdf).unwrap();
        assert_eq!(doc2.page_count(), 1);
    }

    #[test]
    fn test_error_display() {
        let err = Error::PageOutOfRange { index: 5, count: 3 };
        let msg = format!("{err}");
        assert!(msg.contains("5"));
        assert!(msg.contains("3"));
    }
}
