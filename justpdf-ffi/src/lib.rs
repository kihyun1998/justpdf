//! C FFI bindings for the justpdf PDF engine.
//!
//! # Safety
//!
//! All functions in this module are `unsafe` as they deal with raw pointers
//! from C callers. The caller is responsible for:
//! - Passing valid, non-null pointers
//! - Properly freeing allocated resources with the corresponding `_free` function
//! - Not using freed pointers

#![doc(
    html_logo_url = "https://raw.githubusercontent.com/kihyun1998/justpdf/master/logo/icons/justpdf-icon-light-128.png"
)]
#![doc(
    html_favicon_url = "https://raw.githubusercontent.com/kihyun1998/justpdf/master/logo/favicon/favicon-32.png"
)]

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_double, c_int, c_uint};
use std::path::Path;
use std::slice;

use justpdf_core::page;
use justpdf_core::text;
use justpdf_core::{JustPdfError, PdfDocument};

/// Opaque document handle.
pub struct JustPdfDocument {
    inner: PdfDocument,
}

/// Opaque rendered image handle.
pub struct JustPdfImage {
    data: Vec<u8>,
}

/// Error codes.
pub const JUSTPDF_OK: c_int = 0;
pub const JUSTPDF_ERR_NULL_PTR: c_int = -1;
pub const JUSTPDF_ERR_INVALID_PATH: c_int = -2;
pub const JUSTPDF_ERR_PARSE: c_int = -3;
pub const JUSTPDF_ERR_RENDER: c_int = -4;
pub const JUSTPDF_ERR_OUT_OF_RANGE: c_int = -5;
pub const JUSTPDF_ERR_ENCRYPTED: c_int = -6;
pub const JUSTPDF_ERR_IO: c_int = -7;

/// The status code for a core error from a page lookup or a render.
fn page_error_code(e: &JustPdfError) -> c_int {
    match e {
        JustPdfError::PageOutOfRange { .. } => JUSTPDF_ERR_OUT_OF_RANGE,
        JustPdfError::EncryptedDocument => JUSTPDF_ERR_ENCRYPTED,
        _ => JUSTPDF_ERR_PARSE,
    }
}

// ---------------------------------------------------------------------------
// Document lifecycle
// ---------------------------------------------------------------------------

/// Open a PDF file. Returns a document handle via `out`.
/// Returns JUSTPDF_OK on success.
///
/// # Safety
///
/// - `path` must be null or point to a NUL-terminated string that stays valid for the duration of the call.
/// - `out` must be null or valid for writing one `*mut JustPdfDocument`.
/// - On `JUSTPDF_OK`, `*out` holds a handle the caller owns and must release with `justpdf_close`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn justpdf_open(
    path: *const c_char,
    out: *mut *mut JustPdfDocument,
) -> c_int {
    if path.is_null() || out.is_null() {
        return JUSTPDF_ERR_NULL_PTR;
    }
    let c_str = match unsafe { CStr::from_ptr(path) }.to_str() {
        Ok(s) => s,
        Err(_) => return JUSTPDF_ERR_INVALID_PATH,
    };
    match PdfDocument::open(Path::new(c_str)) {
        Ok(doc) => {
            let boxed = Box::new(JustPdfDocument { inner: doc });
            unsafe { *out = Box::into_raw(boxed) };
            JUSTPDF_OK
        }
        Err(_) => JUSTPDF_ERR_PARSE,
    }
}

/// Open a PDF from memory. `data` must point to `len` bytes.
///
/// # Safety
///
/// - `data` must be null or valid for reading `len` bytes for the duration of the call; the bytes are copied, so the buffer may be released once the call returns.
/// - `out` must be null or valid for writing one `*mut JustPdfDocument`.
/// - On `JUSTPDF_OK`, `*out` holds a handle the caller owns and must release with `justpdf_close`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn justpdf_open_memory(
    data: *const u8,
    len: usize,
    out: *mut *mut JustPdfDocument,
) -> c_int {
    if data.is_null() || out.is_null() {
        return JUSTPDF_ERR_NULL_PTR;
    }
    let bytes = unsafe { slice::from_raw_parts(data, len) }.to_vec();
    match PdfDocument::from_bytes(bytes) {
        Ok(doc) => {
            let boxed = Box::new(JustPdfDocument { inner: doc });
            unsafe { *out = Box::into_raw(boxed) };
            JUSTPDF_OK
        }
        Err(_) => JUSTPDF_ERR_PARSE,
    }
}

/// Free a document handle.
///
/// # Safety
///
/// - `doc` must be null or a handle returned by `justpdf_open` or `justpdf_open_memory` that has not been passed to `justpdf_close`.
/// - After the call `doc` is dangling and must not be used again.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn justpdf_close(doc: *mut JustPdfDocument) {
    if !doc.is_null() {
        drop(unsafe { Box::from_raw(doc) });
    }
}

/// Authenticate an encrypted document.
///
/// # Safety
///
/// - `doc` must be null or a handle returned by `justpdf_open` or `justpdf_open_memory` that has not been passed to `justpdf_close`. No other call may use the same handle while this one runs.
/// - `password` must be null or point to a NUL-terminated string that stays valid for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn justpdf_authenticate(
    doc: *mut JustPdfDocument,
    password: *const c_char,
) -> c_int {
    if doc.is_null() || password.is_null() {
        return JUSTPDF_ERR_NULL_PTR;
    }
    let doc = unsafe { &mut *doc };
    let pw = unsafe { CStr::from_ptr(password) }.to_bytes();
    match doc.inner.authenticate(pw) {
        Ok(()) => JUSTPDF_OK,
        Err(_) => JUSTPDF_ERR_ENCRYPTED,
    }
}

// ---------------------------------------------------------------------------
// Document info
// ---------------------------------------------------------------------------

/// Get page count. Writes the count to `out`.
///
/// # Safety
///
/// - `doc` must be null or a handle returned by `justpdf_open` or `justpdf_open_memory` that has not been passed to `justpdf_close`.
/// - `out` must be null or valid for writing one `c_uint`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn justpdf_page_count(
    doc: *const JustPdfDocument,
    out: *mut c_uint,
) -> c_int {
    if doc.is_null() || out.is_null() {
        return JUSTPDF_ERR_NULL_PTR;
    }
    let doc = unsafe { &*doc };
    match page::page_count(&doc.inner) {
        Ok(n) => {
            unsafe { *out = n as c_uint };
            JUSTPDF_OK
        }
        Err(_) => JUSTPDF_ERR_PARSE,
    }
}

/// Get PDF version. Writes major and minor to the provided pointers.
///
/// # Safety
///
/// - `doc` must be null or a handle returned by `justpdf_open` or `justpdf_open_memory` that has not been passed to `justpdf_close`.
/// - `major` must be null or valid for writing one `u8`.
/// - `minor` must be null or valid for writing one `u8`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn justpdf_version(
    doc: *const JustPdfDocument,
    major: *mut u8,
    minor: *mut u8,
) -> c_int {
    if doc.is_null() || major.is_null() || minor.is_null() {
        return JUSTPDF_ERR_NULL_PTR;
    }
    let doc = unsafe { &*doc };
    unsafe {
        *major = doc.inner.version.0;
        *minor = doc.inner.version.1;
    }
    JUSTPDF_OK
}

/// Check if document is encrypted. Writes 1 (encrypted) or 0 (not) to `out`.
///
/// # Safety
///
/// - `doc` must be null or a handle returned by `justpdf_open` or `justpdf_open_memory` that has not been passed to `justpdf_close`.
/// - `out` must be null or valid for writing one `c_int`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn justpdf_is_encrypted(
    doc: *const JustPdfDocument,
    out: *mut c_int,
) -> c_int {
    if doc.is_null() || out.is_null() {
        return JUSTPDF_ERR_NULL_PTR;
    }
    let doc = unsafe { &*doc };
    unsafe { *out = if doc.inner.is_encrypted() { 1 } else { 0 } };
    JUSTPDF_OK
}

// ---------------------------------------------------------------------------
// Text extraction
// ---------------------------------------------------------------------------

/// Extract text from a single page (0-based index).
/// The returned string must be freed with `justpdf_free_string`.
///
/// # Safety
///
/// - `doc` must be null or a handle returned by `justpdf_open` or `justpdf_open_memory` that has not been passed to `justpdf_close`.
/// - `out` must be null or valid for writing one `*mut c_char`.
/// - On `JUSTPDF_OK`, `*out` holds a NUL-terminated string the caller owns and must release with `justpdf_free_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn justpdf_extract_page_text(
    doc: *const JustPdfDocument,
    page_index: c_uint,
    out: *mut *mut c_char,
) -> c_int {
    if doc.is_null() || out.is_null() {
        return JUSTPDF_ERR_NULL_PTR;
    }
    let doc = unsafe { &*doc };
    let page_info = match page::get_page(&doc.inner, page_index as usize) {
        Ok(p) => p,
        Err(e) => return page_error_code(&e),
    };
    match text::extract_page_text_string(&doc.inner, &page_info) {
        Ok(s) => {
            let c_string = CString::new(s).unwrap_or_default();
            unsafe { *out = c_string.into_raw() };
            JUSTPDF_OK
        }
        Err(_) => JUSTPDF_ERR_PARSE,
    }
}

/// Extract text from all pages.
/// The returned string must be freed with `justpdf_free_string`.
///
/// # Safety
///
/// - `doc` must be null or a handle returned by `justpdf_open` or `justpdf_open_memory` that has not been passed to `justpdf_close`.
/// - `out` must be null or valid for writing one `*mut c_char`.
/// - On `JUSTPDF_OK`, `*out` holds a NUL-terminated string the caller owns and must release with `justpdf_free_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn justpdf_extract_all_text(
    doc: *const JustPdfDocument,
    out: *mut *mut c_char,
) -> c_int {
    if doc.is_null() || out.is_null() {
        return JUSTPDF_ERR_NULL_PTR;
    }
    let doc = unsafe { &*doc };
    match text::extract_all_text_string(&doc.inner) {
        Ok(s) => {
            let c_string = CString::new(s).unwrap_or_default();
            unsafe { *out = c_string.into_raw() };
            JUSTPDF_OK
        }
        Err(_) => JUSTPDF_ERR_PARSE,
    }
}

/// Free a string returned by justpdf functions.
///
/// # Safety
///
/// - `s` must be null or a string returned by `justpdf_extract_page_text` or `justpdf_extract_all_text` that has not already been freed. After the call `s` is dangling and must not be used again.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn justpdf_free_string(s: *mut c_char) {
    if !s.is_null() {
        drop(unsafe { CString::from_raw(s) });
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

/// Render a page to PNG. Returns image data via `out`.
/// The image must be freed with `justpdf_free_image`.
///
/// # Safety
///
/// - `doc` must be null or a handle returned by `justpdf_open` or `justpdf_open_memory` that has not been passed to `justpdf_close`.
/// - `out` must be null or valid for writing one `*mut JustPdfImage`.
/// - On `JUSTPDF_OK`, `*out` holds an image handle the caller owns and must release with `justpdf_free_image`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn justpdf_render_page_png(
    doc: *const JustPdfDocument,
    page_index: c_uint,
    dpi: c_double,
    out: *mut *mut JustPdfImage,
) -> c_int {
    if doc.is_null() || out.is_null() {
        return JUSTPDF_ERR_NULL_PTR;
    }
    let doc = unsafe { &*doc };
    let opts = justpdf_render::RenderOptions {
        dpi,
        format: justpdf_render::OutputFormat::Png,
        ..Default::default()
    };
    match justpdf_render::render_page(&doc.inner, page_index as usize, &opts) {
        Ok(data) => {
            let img = Box::new(JustPdfImage { data });
            unsafe { *out = Box::into_raw(img) };
            JUSTPDF_OK
        }
        Err(justpdf_render::RenderError::Core(e)) => page_error_code(&e),
        Err(_) => JUSTPDF_ERR_RENDER,
    }
}

/// Get image data pointer and length.
///
/// # Safety
///
/// - `img` must be null or a handle returned by `justpdf_render_page_png` that has not been passed to `justpdf_free_image`.
/// - `data_out` must be null or valid for writing one `*const u8`.
/// - `len_out` must be null or valid for writing one `usize`.
/// - The pointer written to `*data_out` borrows the image's buffer: it is valid for reading `*len_out` bytes only until `img` is passed to `justpdf_free_image`, and must not be written through or freed by the caller.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn justpdf_image_data(
    img: *const JustPdfImage,
    data_out: *mut *const u8,
    len_out: *mut usize,
) -> c_int {
    if img.is_null() || data_out.is_null() || len_out.is_null() {
        return JUSTPDF_ERR_NULL_PTR;
    }
    let img = unsafe { &*img };
    unsafe {
        *data_out = img.data.as_ptr();
        *len_out = img.data.len();
    }
    JUSTPDF_OK
}

/// Save image data to a file.
///
/// # Safety
///
/// - `img` must be null or a handle returned by `justpdf_render_page_png` that has not been passed to `justpdf_free_image`.
/// - `path` must be null or point to a NUL-terminated string that stays valid for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn justpdf_image_save(
    img: *const JustPdfImage,
    path: *const c_char,
) -> c_int {
    if img.is_null() || path.is_null() {
        return JUSTPDF_ERR_NULL_PTR;
    }
    let img = unsafe { &*img };
    let c_str = match unsafe { CStr::from_ptr(path) }.to_str() {
        Ok(s) => s,
        Err(_) => return JUSTPDF_ERR_INVALID_PATH,
    };
    match std::fs::write(c_str, &img.data) {
        Ok(()) => JUSTPDF_OK,
        Err(_) => JUSTPDF_ERR_IO,
    }
}

/// Free a rendered image.
///
/// # Safety
///
/// - `img` must be null or a handle returned by `justpdf_render_page_png` that has not been passed to `justpdf_free_image`.
/// - After the call `img` is dangling and must not be used again.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn justpdf_free_image(img: *mut JustPdfImage) {
    if !img.is_null() {
        drop(unsafe { Box::from_raw(img) });
    }
}

// ---------------------------------------------------------------------------
// Page info
// ---------------------------------------------------------------------------

/// Get page dimensions (width and height in points).
///
/// # Safety
///
/// - `doc` must be null or a handle returned by `justpdf_open` or `justpdf_open_memory` that has not been passed to `justpdf_close`.
/// - `width` must be null or valid for writing one `c_double`.
/// - `height` must be null or valid for writing one `c_double`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn justpdf_page_size(
    doc: *const JustPdfDocument,
    page_index: c_uint,
    width: *mut c_double,
    height: *mut c_double,
) -> c_int {
    if doc.is_null() || width.is_null() || height.is_null() {
        return JUSTPDF_ERR_NULL_PTR;
    }
    let doc = unsafe { &*doc };
    match page::get_page(&doc.inner, page_index as usize) {
        Ok(info) => {
            let r = info.crop_box.unwrap_or(info.media_box);
            unsafe {
                *width = r.width();
                *height = r.height();
            }
            JUSTPDF_OK
        }
        Err(e) => page_error_code(&e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A PDF whose object `n` (1-based) is `objects[n - 1]`, with a correct
    /// xref table and `/Root 1 0 R`.
    fn pdf(objects: &[&str]) -> Vec<u8> {
        let mut out = b"%PDF-1.7\n".to_vec();
        let mut offsets = Vec::new();
        for (i, body) in objects.iter().enumerate() {
            offsets.push(out.len());
            out.extend_from_slice(format!("{} 0 obj\n{body}\nendobj\n", i + 1).as_bytes());
        }
        let xref_at = out.len();
        out.extend_from_slice(
            format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
        );
        for off in offsets {
            out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
        }
        out.extend_from_slice(
            format!(
                "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_at}\n%%EOF\n",
                objects.len() + 1
            )
            .as_bytes(),
        );
        out
    }

    fn one_page() -> Vec<u8> {
        pdf(&[
            "<< /Type /Catalog /Pages 2 0 R >>",
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 200] >>",
        ])
    }

    /// `/Pages` lists itself as a kid.
    fn cyclic_pages() -> Vec<u8> {
        pdf(&[
            "<< /Type /Catalog /Pages 2 0 R >>",
            "<< /Type /Pages /Kids [2 0 R] /Count 1 >>",
        ])
    }

    /// `/Count` says 2 but the tree holds one page.
    fn overstated_count() -> Vec<u8> {
        pdf(&[
            "<< /Type /Catalog /Pages 2 0 R >>",
            "<< /Type /Pages /Kids [3 0 R] /Count 2 >>",
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 200] >>",
        ])
    }

    fn pages_not_a_dict() -> Vec<u8> {
        pdf(&["<< /Type /Catalog /Pages 2 0 R >>", "42"])
    }

    /// Encrypted with a non-empty user password, not authenticated.
    fn encrypted() -> Vec<u8> {
        include_bytes!("../../justpdf-core/tests/fixtures/aes256_r5_user_owner.pdf").to_vec()
    }

    struct Doc(*mut JustPdfDocument);

    impl Doc {
        fn open(bytes: &[u8]) -> Self {
            let mut doc = std::ptr::null_mut();
            let rc = unsafe { justpdf_open_memory(bytes.as_ptr(), bytes.len(), &mut doc) };
            assert_eq!(rc, JUSTPDF_OK);
            Doc(doc)
        }

        fn page_size(&self, index: c_uint) -> c_int {
            let (mut w, mut h) = (0.0, 0.0);
            unsafe { justpdf_page_size(self.0, index, &mut w, &mut h) }
        }

        fn page_text(&self, index: c_uint) -> c_int {
            let mut s = std::ptr::null_mut();
            let rc = unsafe { justpdf_extract_page_text(self.0, index, &mut s) };
            unsafe { justpdf_free_string(s) };
            rc
        }

        fn render(&self, index: c_uint) -> c_int {
            let mut img = std::ptr::null_mut();
            let rc = unsafe { justpdf_render_page_png(self.0, index, 72.0, &mut img) };
            unsafe { justpdf_free_image(img) };
            rc
        }
    }

    impl Drop for Doc {
        fn drop(&mut self) {
            unsafe { justpdf_close(self.0) };
        }
    }

    #[test]
    fn page_lookup_reports_out_of_range_only_for_an_index_past_the_last_page() {
        let doc = Doc::open(&one_page());
        assert_eq!(doc.page_size(0), JUSTPDF_OK);
        assert_eq!(doc.page_size(1), JUSTPDF_ERR_OUT_OF_RANGE);
        assert_eq!(doc.page_text(1), JUSTPDF_ERR_OUT_OF_RANGE);
    }

    #[test]
    fn page_lookup_reports_a_broken_page_tree_as_parse() {
        for bytes in [cyclic_pages(), pages_not_a_dict()] {
            let doc = Doc::open(&bytes);
            assert_eq!(doc.page_size(0), JUSTPDF_ERR_PARSE);
            assert_eq!(doc.page_text(0), JUSTPDF_ERR_PARSE);
        }
        let doc = Doc::open(&overstated_count());
        assert_eq!(doc.page_size(0), JUSTPDF_OK);
        assert_eq!(doc.page_size(1), JUSTPDF_ERR_PARSE);
        assert_eq!(doc.page_size(2), JUSTPDF_ERR_OUT_OF_RANGE);
    }

    #[test]
    fn page_lookup_reports_an_unauthenticated_document_as_encrypted() {
        let doc = Doc::open(&encrypted());
        assert_eq!(doc.page_size(0), JUSTPDF_ERR_ENCRYPTED);
        assert_eq!(doc.page_text(0), JUSTPDF_ERR_ENCRYPTED);
    }

    #[test]
    fn render_reports_core_errors_as_page_lookup_does() {
        let doc = Doc::open(&one_page());
        assert_eq!(doc.render(0), JUSTPDF_OK);
        assert_eq!(doc.render(1), JUSTPDF_ERR_OUT_OF_RANGE);
        assert_eq!(Doc::open(&cyclic_pages()).render(0), JUSTPDF_ERR_PARSE);
        assert_eq!(Doc::open(&encrypted()).render(0), JUSTPDF_ERR_ENCRYPTED);
    }
}
