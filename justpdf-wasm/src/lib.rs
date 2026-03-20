use wasm_bindgen::prelude::*;
use justpdf_core::PdfDocument;

/// A PDF document handle for use from JavaScript.
#[wasm_bindgen]
pub struct WasmDocument {
    inner: PdfDocument,
}

#[wasm_bindgen]
impl WasmDocument {
    /// Open a PDF from a Uint8Array.
    #[wasm_bindgen(constructor)]
    pub fn new(data: &[u8]) -> Result<WasmDocument, JsValue> {
        let inner = PdfDocument::from_bytes(data.to_vec())
            .map_err(|e| JsValue::from_str(&format!("{e}")))?;
        Ok(Self { inner })
    }

    /// Number of pages.
    #[wasm_bindgen(getter)]
    pub fn page_count(&self) -> Result<usize, JsValue> {
        justpdf_core::page::page_count(&self.inner)
            .map_err(|e| JsValue::from_str(&format!("{e}")))
    }

    /// PDF version string.
    #[wasm_bindgen(getter)]
    pub fn version(&self) -> String {
        format!("{}.{}", self.inner.version.0, self.inner.version.1)
    }

    /// Whether the document is encrypted.
    #[wasm_bindgen(getter)]
    pub fn is_encrypted(&self) -> bool {
        self.inner.is_encrypted()
    }

    /// Authenticate with a password.
    pub fn authenticate(&mut self, password: &str) -> Result<(), JsValue> {
        self.inner.authenticate(password.as_bytes())
            .map_err(|e| JsValue::from_str(&format!("{e}")))
    }

    /// Extract text from all pages.
    pub fn text(&self) -> Result<String, JsValue> {
        justpdf_core::text::extract_all_text_string(&self.inner)
            .map_err(|e| JsValue::from_str(&format!("{e}")))
    }

    /// Extract text from a specific page (0-based).
    pub fn page_text(&self, index: usize) -> Result<String, JsValue> {
        let info = justpdf_core::page::get_page(&self.inner, index)
            .map_err(|e| JsValue::from_str(&format!("{e}")))?;
        justpdf_core::text::extract_page_text_string(&self.inner, &info)
            .map_err(|e| JsValue::from_str(&format!("{e}")))
    }

    /// Render a page to PNG bytes (returned as Uint8Array).
    pub fn render_page_png(&self, index: usize, dpi: f64) -> Result<Vec<u8>, JsValue> {
        let opts = justpdf_render::RenderOptions {
            dpi,
            format: justpdf_render::OutputFormat::Png,
            ..Default::default()
        };
        justpdf_render::render_page(&self.inner, index, &opts)
            .map_err(|e| JsValue::from_str(&format!("{e}")))
    }

    /// Get page width in points.
    pub fn page_width(&self, index: usize) -> Result<f64, JsValue> {
        let info = justpdf_core::page::get_page(&self.inner, index)
            .map_err(|e| JsValue::from_str(&format!("{e}")))?;
        let r = info.crop_box.unwrap_or(info.media_box);
        Ok(r.width())
    }

    /// Get page height in points.
    pub fn page_height(&self, index: usize) -> Result<f64, JsValue> {
        let info = justpdf_core::page::get_page(&self.inner, index)
            .map_err(|e| JsValue::from_str(&format!("{e}")))?;
        let r = info.crop_box.unwrap_or(info.media_box);
        Ok(r.height())
    }

    /// Get document title.
    pub fn title(&self) -> Option<String> {
        self.get_info_string(b"Title")
    }

    /// Get document author.
    pub fn author(&self) -> Option<String> {
        self.get_info_string(b"Author")
    }
}

impl WasmDocument {
    fn get_info_string(&self, key: &[u8]) -> Option<String> {
        let trailer = self.inner.trailer();
        let info_ref = trailer.get_ref(b"Info")?;
        let info_obj = self.inner.resolve(info_ref).ok()?;
        let info_dict = info_obj.as_dict()?;
        match info_dict.get(key)? {
            justpdf_core::PdfObject::String(s) => Some(String::from_utf8_lossy(s).to_string()),
            _ => None,
        }
    }
}
