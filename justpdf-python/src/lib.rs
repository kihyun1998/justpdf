use pyo3::prelude::*;
use pyo3::exceptions::{PyIOError, PyValueError, PyIndexError, PyRuntimeError};

use justpdf_core::PdfDocument;
use justpdf_core::page::{self, PageInfo};

/// A PDF document.
#[pyclass]
struct Document {
    inner: PdfDocument,
}

#[pymethods]
impl Document {
    /// Open a PDF file.
    #[staticmethod]
    fn open(path: &str) -> PyResult<Self> {
        let inner = PdfDocument::open(std::path::Path::new(path))
            .map_err(|e| PyIOError::new_err(format!("{e}")))?;
        Ok(Self { inner })
    }

    /// Open a PDF from bytes.
    #[staticmethod]
    fn from_bytes(data: Vec<u8>) -> PyResult<Self> {
        let inner = PdfDocument::from_bytes(data)
            .map_err(|e| PyValueError::new_err(format!("{e}")))?;
        Ok(Self { inner })
    }

    /// Authenticate an encrypted document.
    fn authenticate(&mut self, password: &str) -> PyResult<()> {
        self.inner.authenticate(password.as_bytes())
            .map_err(|e| PyRuntimeError::new_err(format!("{e}")))?;
        Ok(())
    }

    /// Number of pages.
    #[getter]
    fn page_count(&self) -> PyResult<usize> {
        page::page_count(&self.inner)
            .map_err(|e| PyRuntimeError::new_err(format!("{e}")))
    }

    /// PDF version string (e.g. "1.7").
    #[getter]
    fn version(&self) -> String {
        format!("{}.{}", self.inner.version.0, self.inner.version.1)
    }

    /// Whether the document is encrypted.
    #[getter]
    fn is_encrypted(&self) -> bool {
        self.inner.is_encrypted()
    }

    /// Get a page by index (0-based).
    fn page(&self, index: usize) -> PyResult<Page> {
        let info = page::get_page(&self.inner, index)
            .map_err(|_| PyIndexError::new_err(format!("page index {index} out of range")))?;
        Ok(Page { info })
    }

    /// Extract text from all pages.
    fn text(&self) -> PyResult<String> {
        justpdf_core::text::extract_all_text_string(&self.inner)
            .map_err(|e| PyRuntimeError::new_err(format!("{e}")))
    }

    /// Extract text from a specific page.
    fn page_text(&self, index: usize) -> PyResult<String> {
        let info = page::get_page(&self.inner, index)
            .map_err(|_| PyIndexError::new_err(format!("page index {index} out of range")))?;
        justpdf_core::text::extract_page_text_string(&self.inner, &info)
            .map_err(|e| PyRuntimeError::new_err(format!("{e}")))
    }

    /// Render a page to PNG bytes.
    #[pyo3(signature = (index, dpi=None))]
    fn render_page(&self, index: usize, dpi: Option<f64>) -> PyResult<Vec<u8>> {
        let dpi = dpi.unwrap_or(150.0);
        let opts = justpdf_render::RenderOptions {
            dpi,
            format: justpdf_render::OutputFormat::Png,
            ..Default::default()
        };
        justpdf_render::render_page(&self.inner, index, &opts)
            .map_err(|e| PyRuntimeError::new_err(format!("{e}")))
    }

    /// Render a page and save to a file.
    #[pyo3(signature = (index, path, dpi=None))]
    fn render_page_to_file(&self, index: usize, path: &str, dpi: Option<f64>) -> PyResult<()> {
        let png = self.render_page(index, dpi)?;
        std::fs::write(path, &png)
            .map_err(|e| PyIOError::new_err(format!("{e}")))?;
        Ok(())
    }

    /// Get document title.
    #[getter]
    fn title(&self) -> Option<String> {
        self.get_info_string(b"Title")
    }

    /// Get document author.
    #[getter]
    fn author(&self) -> Option<String> {
        self.get_info_string(b"Author")
    }

    /// Get document subject.
    #[getter]
    fn subject(&self) -> Option<String> {
        self.get_info_string(b"Subject")
    }

    fn __repr__(&self) -> String {
        let pages = page::page_count(&self.inner).unwrap_or(0);
        format!("Document(version={}, pages={pages})", self.version())
    }

    fn __len__(&self) -> usize {
        page::page_count(&self.inner).unwrap_or(0)
    }

    fn __getitem__(&self, index: isize) -> PyResult<Page> {
        let len = self.__len__();
        let idx = if index < 0 {
            (len as isize + index) as usize
        } else {
            index as usize
        };
        if idx >= len {
            return Err(PyIndexError::new_err(format!("page index {index} out of range")));
        }
        self.page(idx)
    }
}

impl Document {
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

/// A single page of a PDF document.
#[pyclass]
#[derive(Clone)]
struct Page {
    info: PageInfo,
}

#[pymethods]
impl Page {
    /// Page width in points.
    #[getter]
    fn width(&self) -> f64 {
        let r = self.info.crop_box.unwrap_or(self.info.media_box);
        r.width()
    }

    /// Page height in points.
    #[getter]
    fn height(&self) -> f64 {
        let r = self.info.crop_box.unwrap_or(self.info.media_box);
        r.height()
    }

    /// Page rotation in degrees.
    #[getter]
    fn rotation(&self) -> i64 {
        self.info.rotate
    }

    fn __repr__(&self) -> String {
        format!("Page(width={:.1}, height={:.1}, rotation={})", self.width(), self.height(), self.rotation())
    }
}

/// Open a PDF file. Shorthand for Document.open().
#[pyfunction]
fn open(path: &str) -> PyResult<Document> {
    Document::open(path)
}

/// justpdf Python module.
#[pymodule]
fn justpdf(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Document>()?;
    m.add_class::<Page>()?;
    m.add_function(wrap_pyfunction!(open, m)?)?;
    Ok(())
}
