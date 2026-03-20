use napi::bindgen_prelude::*;
use napi_derive::napi;
use justpdf_core::PdfDocument;

#[napi]
pub struct Document {
    inner: PdfDocument,
}

#[napi]
impl Document {
    /// Open a PDF file.
    #[napi(factory)]
    pub fn open(path: String) -> Result<Self> {
        let inner = PdfDocument::open(std::path::Path::new(&path))
            .map_err(|e| Error::from_reason(format!("{e}")))?;
        Ok(Self { inner })
    }

    /// Open a PDF from a Buffer.
    #[napi(factory)]
    pub fn from_buffer(data: Buffer) -> Result<Self> {
        let inner = PdfDocument::from_bytes(data.to_vec())
            .map_err(|e| Error::from_reason(format!("{e}")))?;
        Ok(Self { inner })
    }

    /// Authenticate with a password.
    #[napi]
    pub fn authenticate(&mut self, password: String) -> Result<()> {
        self.inner.authenticate(password.as_bytes())
            .map_err(|e| Error::from_reason(format!("{e}")))?;
        Ok(())
    }

    /// Number of pages.
    #[napi(getter)]
    pub fn page_count(&self) -> Result<u32> {
        justpdf_core::page::page_count(&self.inner)
            .map(|n| n as u32)
            .map_err(|e| Error::from_reason(format!("{e}")))
    }

    /// PDF version string.
    #[napi(getter)]
    pub fn version(&self) -> String {
        format!("{}.{}", self.inner.version.0, self.inner.version.1)
    }

    /// Whether the document is encrypted.
    #[napi(getter)]
    pub fn is_encrypted(&self) -> bool {
        self.inner.is_encrypted()
    }

    /// Extract text from all pages.
    #[napi]
    pub fn text(&self) -> Result<String> {
        justpdf_core::text::extract_all_text_string(&self.inner)
            .map_err(|e| Error::from_reason(format!("{e}")))
    }

    /// Extract text from a specific page (0-based).
    #[napi]
    pub fn page_text(&self, index: u32) -> Result<String> {
        let info = justpdf_core::page::get_page(&self.inner, index as usize)
            .map_err(|e| Error::from_reason(format!("{e}")))?;
        justpdf_core::text::extract_page_text_string(&self.inner, &info)
            .map_err(|e| Error::from_reason(format!("{e}")))
    }

    /// Render a page to PNG (returns Buffer).
    #[napi]
    pub fn render_page(&self, index: u32, dpi: Option<f64>) -> Result<Buffer> {
        let dpi = dpi.unwrap_or(150.0);
        let opts = justpdf_render::RenderOptions {
            dpi,
            format: justpdf_render::OutputFormat::Png,
            ..Default::default()
        };
        let data = justpdf_render::render_page(&self.inner, index as usize, &opts)
            .map_err(|e| Error::from_reason(format!("{e}")))?;
        Ok(data.into())
    }

    /// Get page width in points.
    #[napi]
    pub fn page_width(&self, index: u32) -> Result<f64> {
        let info = justpdf_core::page::get_page(&self.inner, index as usize)
            .map_err(|e| Error::from_reason(format!("{e}")))?;
        let r = info.crop_box.unwrap_or(info.media_box);
        Ok(r.width())
    }

    /// Get page height in points.
    #[napi]
    pub fn page_height(&self, index: u32) -> Result<f64> {
        let info = justpdf_core::page::get_page(&self.inner, index as usize)
            .map_err(|e| Error::from_reason(format!("{e}")))?;
        let r = info.crop_box.unwrap_or(info.media_box);
        Ok(r.height())
    }

    /// Get document title.
    #[napi(getter)]
    pub fn title(&self) -> Option<String> {
        self.get_info_string(b"Title")
    }

    /// Get document author.
    #[napi(getter)]
    pub fn author(&self) -> Option<String> {
        self.get_info_string(b"Author")
    }
}

impl Document {
    fn get_info_string(&self, key: &[u8]) -> Option<String> {
        let info_ref = self.inner.trailer().get_ref(b"Info")?;
        let info_obj = self.inner.resolve(info_ref).ok()?;
        let info_dict = info_obj.as_dict()?;
        match info_dict.get(key)? {
            justpdf_core::PdfObject::String(s) => Some(String::from_utf8_lossy(s).to_string()),
            _ => None,
        }
    }
}
