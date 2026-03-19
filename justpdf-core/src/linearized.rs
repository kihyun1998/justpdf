//! Linearized PDF detection, parameter parsing, and hint table support (PDF spec section 7.4).
//!
//! This module detects whether a PDF is linearized, extracts the linearization
//! parameters from the linearization dictionary, and parses page offset hint tables.
//! Linearized PDF generation is in [`crate::writer::linearize`].

use crate::object::{parse_indirect_object, PdfObject};
use crate::parser::PdfDocument;
use crate::tokenizer::Tokenizer;

/// Linearization parameters from the linearization dictionary.
#[derive(Debug, Clone)]
pub struct LinearizationParams {
    /// File length (/L).
    pub file_length: i64,
    /// Hint stream offset (/H first element).
    pub hint_offset: i64,
    /// Hint stream length (/H second element).
    pub hint_length: i64,
    /// First page object number (/O).
    pub first_page_obj_num: u32,
    /// Offset of end of first page (/E).
    pub end_of_first_page: i64,
    /// Number of pages (/N).
    pub page_count: i64,
    /// Offset of main xref table (/T).
    pub main_xref_offset: i64,
    /// Linearization version, typically 1.0.
    pub version: f64,
}

/// Find the byte offset just past the PDF header line (`%PDF-x.y` plus its line ending).
///
/// Returns `None` if no valid header is found in the first 1024 bytes.
fn skip_header(data: &[u8]) -> Option<usize> {
    let search_len = data.len().min(1024);
    let needle = b"%PDF-";

    for i in 0..search_len.saturating_sub(needle.len()) {
        if data[i..].starts_with(needle) {
            // Skip past the header line (find the next line break).
            let mut pos = i + needle.len();
            // Skip the rest of the header line (version digits etc.)
            while pos < data.len() && data[pos] != b'\n' && data[pos] != b'\r' {
                pos += 1;
            }
            // Skip the line ending itself.
            if pos < data.len() && data[pos] == b'\r' {
                pos += 1;
            }
            if pos < data.len() && data[pos] == b'\n' {
                pos += 1;
            }
            return Some(pos);
        }
    }
    None
}

/// Skip any comment lines (lines starting with `%`) after the header.
fn skip_comments(data: &[u8], mut pos: usize) -> usize {
    loop {
        // Skip whitespace between lines.
        while pos < data.len()
            && (data[pos] == b' ' || data[pos] == b'\t' || data[pos] == b'\r' || data[pos] == b'\n')
        {
            pos += 1;
        }
        if pos < data.len() && data[pos] == b'%' {
            // Skip comment line.
            while pos < data.len() && data[pos] != b'\n' && data[pos] != b'\r' {
                pos += 1;
            }
        } else {
            break;
        }
    }
    pos
}

/// Detect whether the given PDF data represents a linearized PDF, and if so,
/// parse and return the linearization parameters.
///
/// This checks if the first indirect object in the file contains a `/Linearized` key.
pub fn detect_linearization(data: &[u8]) -> Option<LinearizationParams> {
    let header_end = skip_header(data)?;
    let obj_start = skip_comments(data, header_end);

    if obj_start >= data.len() {
        return None;
    }

    let mut tokenizer = Tokenizer::new_at(data, obj_start);
    let (_iref, obj) = parse_indirect_object(&mut tokenizer).ok()?;

    let dict = match &obj {
        PdfObject::Dict(d) => d,
        _ => return None,
    };

    // Check for the /Linearized key.
    if !dict.contains_key(b"Linearized") {
        return None;
    }

    let version = dict.get_f64(b"Linearized")?;
    let file_length = dict.get_i64(b"L")?;
    let first_page_obj_num = dict.get_i64(b"O")? as u32;
    let end_of_first_page = dict.get_i64(b"E")?;
    let page_count = dict.get_i64(b"N")?;
    let main_xref_offset = dict.get_i64(b"T")?;

    // /H is an array of 2 integers: [offset length].
    let h_array = dict.get_array(b"H")?;
    if h_array.len() < 2 {
        return None;
    }
    let hint_offset = h_array[0].as_i64()?;
    let hint_length = h_array[1].as_i64()?;

    Some(LinearizationParams {
        file_length,
        hint_offset,
        hint_length,
        first_page_obj_num,
        end_of_first_page,
        page_count,
        main_xref_offset,
        version,
    })
}

/// Simple check: returns `true` if the PDF data appears to be linearized.
pub fn is_linearized(data: &[u8]) -> bool {
    detect_linearization(data).is_some()
}

/// Alternative API that works from a parsed `PdfDocument`.
///
/// Finds the first object (usually obj 1) and checks for `/Linearized`.
/// This re-parses from the raw data since the linearization dict is always the
/// first indirect object in the file.
pub fn read_linearization(doc: &PdfDocument) -> Option<LinearizationParams> {
    detect_linearization(doc.raw_data())
}

// ---------------------------------------------------------------------------
// Hint table types and parsing
// ---------------------------------------------------------------------------

/// Page offset hint table entry.
///
/// Each entry describes the location and size of a page's objects within the
/// linearized PDF body. Used by viewers to seek directly to a specific page.
#[derive(Debug, Clone)]
pub struct PageOffsetHint {
    /// Byte offset of the page's objects within the file.
    pub offset: u64,
    /// Total byte length of the page's objects.
    pub length: u64,
    /// Number of objects belonging to this page.
    pub num_objects: u32,
}

/// A bit reader for extracting variable-width fields from a hint stream.
struct BitReader<'a> {
    data: &'a [u8],
    byte_pos: usize,
    bit_pos: u8, // 0..8, bits consumed in current byte (MSB first)
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            byte_pos: 0,
            bit_pos: 0,
        }
    }

    /// Read `n_bits` (up to 64) from the stream in MSB-first order.
    fn read_bits(&mut self, n_bits: u32) -> Option<u64> {
        if n_bits == 0 {
            return Some(0);
        }
        let mut result: u64 = 0;
        let mut remaining = n_bits;
        while remaining > 0 {
            if self.byte_pos >= self.data.len() {
                return None;
            }
            let avail = 8 - self.bit_pos as u32;
            let take = remaining.min(avail);
            // Extract `take` bits from the current byte starting at bit_pos
            let shift = avail - take;
            let mask = ((1u16 << take) - 1) as u8;
            let bits = (self.data[self.byte_pos] >> shift) & mask;
            result = (result << take) | bits as u64;
            remaining -= take;
            self.bit_pos += take as u8;
            if self.bit_pos >= 8 {
                self.bit_pos = 0;
                self.byte_pos += 1;
            }
        }
        Some(result)
    }

    /// Advance to the next byte boundary.
    fn align(&mut self) {
        if self.bit_pos > 0 {
            self.bit_pos = 0;
            self.byte_pos += 1;
        }
    }
}

/// Read a 32-bit big-endian unsigned integer from a byte slice at the given offset.
fn read_u32_be(data: &[u8], offset: usize) -> Option<u32> {
    if offset + 4 > data.len() {
        return None;
    }
    Some(u32::from_be_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ]))
}

/// Parse the page offset hint table from a linearized PDF hint stream.
///
/// `data` is the raw (decoded) hint stream bytes. `params` provides the
/// linearization parameters (especially `page_count`).
///
/// The page offset hint table header (PDF spec F.3) begins at byte 0 of
/// the hint stream with a series of 32-bit big-endian fields describing
/// minimums and bit widths, followed by per-page variable-bit entries.
///
/// Returns `None` if the hint stream is too short or malformed.
pub fn parse_hint_tables(data: &[u8], params: &LinearizationParams) -> Option<Vec<PageOffsetHint>> {
    let n_pages = params.page_count as usize;
    if n_pages == 0 {
        return Some(Vec::new());
    }

    // The page offset hint table header consists of several 32-bit fields.
    // Per the spec (Table F.1), we need at least 9 x 4 = 36 bytes for the
    // header items we use.
    if data.len() < 36 {
        return None;
    }

    // Header fields (Table F.1):
    // Item 1: min obj per page (4 bytes)
    let min_objects = read_u32_be(data, 0)?;
    // Item 2: offset of first page's objects (4 bytes) - location field
    let first_page_offset = read_u32_be(data, 4)? as u64;
    // Item 3: bits needed to represent delta-objects (4 bytes)
    let bits_delta_objects = read_u32_be(data, 8)?;
    // Item 4: min page length (4 bytes)
    let min_page_length = read_u32_be(data, 12)? as u64;
    // Item 5: bits needed for delta-page-length (4 bytes)
    let bits_delta_length = read_u32_be(data, 16)?;
    // Item 6: min offset to content stream (4 bytes) - skip for basic parsing
    let _min_content_offset = read_u32_be(data, 20)?;
    // Item 7: bits for delta-content-offset (4 bytes) - skip
    let _bits_delta_content = read_u32_be(data, 24)?;
    // Item 8: min content stream length (4 bytes) - skip
    let _min_content_length = read_u32_be(data, 28)?;
    // Item 9: bits for delta-content-length (4 bytes) - skip
    let _bits_delta_content_len = read_u32_be(data, 32)?;

    // Per-page data starts at byte 36
    let per_page_data = &data[36..];
    let mut reader = BitReader::new(per_page_data);

    // Section 1: number-of-objects deltas
    let mut num_objects: Vec<u32> = Vec::with_capacity(n_pages);
    for _ in 0..n_pages {
        let delta = reader.read_bits(bits_delta_objects)? as u32;
        num_objects.push(min_objects + delta);
    }
    reader.align();

    // Section 2: page-length deltas
    let mut lengths: Vec<u64> = Vec::with_capacity(n_pages);
    for _ in 0..n_pages {
        let delta = reader.read_bits(bits_delta_length)? as u64;
        lengths.push(min_page_length + delta);
    }

    // Compute offsets: pages are laid out sequentially starting from
    // first_page_offset. The first page in the hint table is page 0;
    // the linearized first-page (at a known offset) is entry 0.
    let mut offsets: Vec<u64> = Vec::with_capacity(n_pages);
    let mut running = first_page_offset;
    for i in 0..n_pages {
        offsets.push(running);
        running += lengths[i];
    }

    let mut entries: Vec<PageOffsetHint> = Vec::with_capacity(n_pages);
    for i in 0..n_pages {
        entries.push(PageOffsetHint {
            offset: offsets[i],
            length: lengths[i],
            num_objects: num_objects[i],
        });
    }

    Some(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build a minimal linearized PDF byte stream.
    fn make_linearized_pdf(
        file_length: i64,
        hint_offset: i64,
        hint_length: i64,
        first_page_obj: u32,
        end_first_page: i64,
        page_count: i64,
        main_xref: i64,
    ) -> Vec<u8> {
        format!(
            "%PDF-1.7\n\
             1 0 obj\n\
             << /Linearized 1.0 /L {file_length} /H [{hint_offset} {hint_length}] \
             /O {first_page_obj} /E {end_first_page} /N {page_count} /T {main_xref} >>\n\
             endobj\n"
        )
        .into_bytes()
    }

    #[test]
    fn detect_linearized_pdf() {
        let data = make_linearized_pdf(12345, 200, 50, 5, 1000, 10, 9000);
        let params = detect_linearization(&data).expect("should detect linearization");
        assert_eq!(params.file_length, 12345);
        assert_eq!(params.hint_offset, 200);
        assert_eq!(params.hint_length, 50);
        assert_eq!(params.first_page_obj_num, 5);
        assert_eq!(params.end_of_first_page, 1000);
        assert_eq!(params.page_count, 10);
        assert_eq!(params.main_xref_offset, 9000);
        assert!((params.version - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn is_linearized_true() {
        let data = make_linearized_pdf(500, 100, 30, 2, 400, 3, 450);
        assert!(is_linearized(&data));
    }

    #[test]
    fn non_linearized_pdf_returns_none() {
        let data = b"%PDF-1.4\n1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n";
        assert!(detect_linearization(data).is_none());
        assert!(!is_linearized(data));
    }

    #[test]
    fn non_dict_first_object_returns_none() {
        let data = b"%PDF-1.4\n1 0 obj\n42\nendobj\n";
        assert!(detect_linearization(data).is_none());
    }

    #[test]
    fn header_with_comment_line() {
        // Some PDFs have a binary comment after the header.
        let data = b"%PDF-1.5\n%\xE2\xE3\xCF\xD3\n\
            1 0 obj\n\
            << /Linearized 1.0 /L 5000 /H [100 20] /O 3 /E 800 /N 5 /T 4500 >>\n\
            endobj\n";
        let params = detect_linearization(data).expect("should detect through comment");
        assert_eq!(params.file_length, 5000);
        assert_eq!(params.page_count, 5);
    }

    #[test]
    fn short_input_does_not_panic() {
        assert!(detect_linearization(b"").is_none());
        assert!(detect_linearization(b"%PDF").is_none());
        assert!(detect_linearization(b"%PDF-1.4\n").is_none());
        assert!(detect_linearization(b"%PDF-1.4\n1").is_none());
    }

    #[test]
    fn truncated_dict_does_not_panic() {
        let data = b"%PDF-1.4\n1 0 obj\n<< /Linearized 1.0 /L";
        assert!(detect_linearization(data).is_none());
    }

    #[test]
    fn missing_required_key_returns_none() {
        // Has /Linearized but missing /L.
        let data = b"%PDF-1.4\n1 0 obj\n\
            << /Linearized 1.0 /H [100 20] /O 3 /E 800 /N 5 /T 4500 >>\n\
            endobj\n";
        assert!(detect_linearization(data).is_none());
    }

    #[test]
    fn h_array_too_short_returns_none() {
        let data = b"%PDF-1.4\n1 0 obj\n\
            << /Linearized 1.0 /L 5000 /H [100] /O 3 /E 800 /N 5 /T 4500 >>\n\
            endobj\n";
        assert!(detect_linearization(data).is_none());
    }

    #[test]
    fn parse_all_params_correctly() {
        let data = make_linearized_pdf(999999, 512, 128, 7, 2048, 42, 88888);
        let params = detect_linearization(&data).unwrap();
        assert_eq!(params.file_length, 999999);
        assert_eq!(params.hint_offset, 512);
        assert_eq!(params.hint_length, 128);
        assert_eq!(params.first_page_obj_num, 7);
        assert_eq!(params.end_of_first_page, 2048);
        assert_eq!(params.page_count, 42);
        assert_eq!(params.main_xref_offset, 88888);
    }

    #[test]
    fn version_as_real() {
        let data = b"%PDF-1.7\n1 0 obj\n\
            << /Linearized 1.0 /L 100 /H [10 5] /O 1 /E 50 /N 1 /T 80 >>\n\
            endobj\n";
        let params = detect_linearization(data).unwrap();
        assert!((params.version - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn version_as_integer() {
        // Some generators write /Linearized 1 (integer) instead of 1.0.
        let data = b"%PDF-1.7\n1 0 obj\n\
            << /Linearized 1 /L 100 /H [10 5] /O 1 /E 50 /N 1 /T 80 >>\n\
            endobj\n";
        let params = detect_linearization(data).unwrap();
        assert!((params.version - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn read_linearization_from_document() {
        let data = make_linearized_pdf(5000, 100, 20, 3, 800, 5, 4500);
        // PdfDocument::from_bytes requires valid xref/trailer, so we test
        // detect_linearization directly for the raw-data path.
        let params = detect_linearization(&data).unwrap();
        assert_eq!(params.page_count, 5);
    }

    // --- Hint table tests ---

    /// Build a minimal page offset hint table stream for testing.
    ///
    /// Generates a header + per-page bit-packed data for `n_pages` pages,
    /// each with `objs_per_page` objects and `page_len` bytes.
    fn make_hint_stream(
        n_pages: usize,
        objs_per_page: u32,
        first_page_offset: u32,
        page_len: u32,
    ) -> Vec<u8> {
        let mut buf = Vec::new();
        // Header: 9 x u32 big-endian
        // Item 1: min objects per page
        buf.extend_from_slice(&objs_per_page.to_be_bytes());
        // Item 2: first page offset
        buf.extend_from_slice(&first_page_offset.to_be_bytes());
        // Item 3: bits for delta-objects (0 = all same)
        buf.extend_from_slice(&0u32.to_be_bytes());
        // Item 4: min page length
        buf.extend_from_slice(&page_len.to_be_bytes());
        // Item 5: bits for delta-page-length (0 = all same)
        buf.extend_from_slice(&0u32.to_be_bytes());
        // Items 6-9: content stream fields (unused, zeros)
        buf.extend_from_slice(&0u32.to_be_bytes());
        buf.extend_from_slice(&0u32.to_be_bytes());
        buf.extend_from_slice(&0u32.to_be_bytes());
        buf.extend_from_slice(&0u32.to_be_bytes());
        // Per-page sections: all deltas are 0-bit, so nothing to write
        // (0 bits * n_pages = 0 bytes for both sections)
        let _ = n_pages; // used implicitly through params.page_count
        buf
    }

    #[test]
    fn parse_hint_table_uniform_pages() {
        let params = LinearizationParams {
            file_length: 5000,
            hint_offset: 100,
            hint_length: 36,
            first_page_obj_num: 3,
            end_of_first_page: 800,
            page_count: 3,
            main_xref_offset: 4500,
            version: 1.0,
        };
        let stream = make_hint_stream(3, 5, 200, 400);
        let hints = parse_hint_tables(&stream, &params).unwrap();
        assert_eq!(hints.len(), 3);
        for (i, hint) in hints.iter().enumerate() {
            assert_eq!(hint.num_objects, 5);
            assert_eq!(hint.length, 400);
            assert_eq!(hint.offset, 200 + i as u64 * 400);
        }
    }

    #[test]
    fn parse_hint_table_zero_pages() {
        let params = LinearizationParams {
            file_length: 100,
            hint_offset: 10,
            hint_length: 5,
            first_page_obj_num: 1,
            end_of_first_page: 50,
            page_count: 0,
            main_xref_offset: 80,
            version: 1.0,
        };
        let hints = parse_hint_tables(b"", &params).unwrap();
        assert!(hints.is_empty());
    }

    #[test]
    fn parse_hint_table_too_short() {
        let params = LinearizationParams {
            file_length: 100,
            hint_offset: 10,
            hint_length: 5,
            first_page_obj_num: 1,
            end_of_first_page: 50,
            page_count: 2,
            main_xref_offset: 80,
            version: 1.0,
        };
        // Only 20 bytes, need at least 36
        assert!(parse_hint_tables(&[0u8; 20], &params).is_none());
    }

    #[test]
    fn bit_reader_basics() {
        // 0xA5 = 1010_0101
        let mut r = BitReader::new(&[0xA5]);
        assert_eq!(r.read_bits(4), Some(0b1010)); // 0xA
        assert_eq!(r.read_bits(4), Some(0b0101)); // 0x5
    }

    #[test]
    fn bit_reader_cross_byte() {
        // 0xFF 0x00 = 1111_1111 0000_0000
        let mut r = BitReader::new(&[0xFF, 0x00]);
        assert_eq!(r.read_bits(4), Some(0xF));
        assert_eq!(r.read_bits(8), Some(0xF0)); // crosses byte boundary
        assert_eq!(r.read_bits(4), Some(0x0));
    }

    #[test]
    fn bit_reader_zero_bits() {
        let mut r = BitReader::new(&[0xFF]);
        assert_eq!(r.read_bits(0), Some(0));
    }

    #[test]
    fn parse_hint_table_with_deltas() {
        // Build a hint stream where delta-objects needs 2 bits and delta-length needs 3 bits.
        // 2 pages: page0 has 3+1=4 objects, 100+5=105 bytes; page1 has 3+2=5 objects, 100+3=103 bytes.
        let mut buf = Vec::new();
        // Header
        buf.extend_from_slice(&3u32.to_be_bytes());    // min objects = 3
        buf.extend_from_slice(&500u32.to_be_bytes());   // first page offset
        buf.extend_from_slice(&2u32.to_be_bytes());     // bits for delta-objects = 2
        buf.extend_from_slice(&100u32.to_be_bytes());   // min page length = 100
        buf.extend_from_slice(&3u32.to_be_bytes());     // bits for delta-length = 3
        // Items 6-9: zeros
        for _ in 0..4 {
            buf.extend_from_slice(&0u32.to_be_bytes());
        }
        // Per-page section 1: delta-objects (2 bits each, 2 pages)
        // page0 delta=1 (0b01), page1 delta=2 (0b10)
        // bits: 01 10 = 0110_0000 = 0x60 (padded)
        // Per-page section 2: delta-length (3 bits each, 2 pages)
        // page0 delta=5 (0b101), page1 delta=3 (0b011)
        // bits: 101 011 = 1010_1100 = 0xAC (padded, after alignment)
        //
        // Section 1: 2 bits * 2 pages = 4 bits = partial byte
        // After align, section 2 starts at next byte.
        buf.push(0b0110_0000); // section 1: page0=01, page1=10, pad 0000
        buf.push(0b1010_1100); // section 2: page0=101, page1=011, pad 00

        let params = LinearizationParams {
            file_length: 5000,
            hint_offset: 100,
            hint_length: buf.len() as i64,
            first_page_obj_num: 3,
            end_of_first_page: 605,
            page_count: 2,
            main_xref_offset: 4500,
            version: 1.0,
        };

        let hints = parse_hint_tables(&buf, &params).unwrap();
        assert_eq!(hints.len(), 2);
        assert_eq!(hints[0].num_objects, 4); // 3 + 1
        assert_eq!(hints[1].num_objects, 5); // 3 + 2
        assert_eq!(hints[0].length, 105);    // 100 + 5
        assert_eq!(hints[1].length, 103);    // 100 + 3
        assert_eq!(hints[0].offset, 500);
        assert_eq!(hints[1].offset, 605);    // 500 + 105
    }
}
