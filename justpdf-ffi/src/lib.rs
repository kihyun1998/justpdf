//! C FFI bindings for the justpdf PDF engine.
//!
//! # Safety
//!
//! All functions in this module are `unsafe` as they deal with raw pointers
//! from C callers. The caller is responsible for:
//! - Passing valid, non-null pointers
//! - Properly freeing allocated resources with the corresponding `_free` function
//! - Not using freed pointers

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_double, c_int, c_uint};
use std::path::Path;
use std::slice;

use justpdf_core::PdfDocument;
use justpdf_core::page;
use justpdf_core::text;

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

// ---------------------------------------------------------------------------
// Document lifecycle
// ---------------------------------------------------------------------------

/// Open a PDF file. Returns a document handle via `out`.
/// Returns JUSTPDF_OK on success.
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
#[unsafe(no_mangle)]
pub unsafe extern "C" fn justpdf_close(doc: *mut JustPdfDocument) {
    if !doc.is_null() {
        drop(unsafe { Box::from_raw(doc) });
    }
}

/// Authenticate an encrypted document.
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
        Err(_) => return JUSTPDF_ERR_OUT_OF_RANGE,
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
        Err(_) => JUSTPDF_ERR_RENDER,
    }
}

/// Get image data pointer and length.
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
        Err(_) => JUSTPDF_ERR_OUT_OF_RANGE,
    }
}
