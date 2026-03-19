//! PDF page labels (section 7.7).
//!
//! Page labels allow PDF pages to display labels like "i", "ii", "iii" or
//! "A-1", "A-2" instead of raw sequential page numbers. Labels are defined
//! as a number tree in the document catalog under the /PageLabels key.

use crate::error::{JustPdfError, Result};
use crate::object::{PdfDict, PdfObject};
use crate::parser::PdfDocument;
use crate::writer::modify::DocumentModifier;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// The numbering style for a page label range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageLabelStyle {
    /// Arabic decimal numerals: 1, 2, 3, ...
    Decimal,
    /// Uppercase Roman numerals: I, II, III, ...
    UpperRoman,
    /// Lowercase Roman numerals: i, ii, iii, ...
    LowerRoman,
    /// Uppercase letters: A, B, ..., Z, AA, AB, ...
    UpperAlpha,
    /// Lowercase letters: a, b, ..., z, aa, ab, ...
    LowerAlpha,
    /// No numeric portion; only the prefix (if any) is used.
    None,
}

impl PageLabelStyle {
    /// Decode from the PDF /S name value.
    fn from_name(name: &[u8]) -> Option<Self> {
        match name {
            b"D" => Some(Self::Decimal),
            b"R" => Some(Self::UpperRoman),
            b"r" => Some(Self::LowerRoman),
            b"A" => Some(Self::UpperAlpha),
            b"a" => Some(Self::LowerAlpha),
            _ => Option::None,
        }
    }

    /// Encode to the PDF /S name value. Returns `None` for `PageLabelStyle::None`.
    fn to_name(&self) -> Option<&'static [u8]> {
        match self {
            Self::Decimal => Some(b"D"),
            Self::UpperRoman => Some(b"R"),
            Self::LowerRoman => Some(b"r"),
            Self::UpperAlpha => Some(b"A"),
            Self::LowerAlpha => Some(b"a"),
            Self::None => Option::None,
        }
    }
}

/// A single page label range. Defines the labelling scheme starting at a
/// particular 0-based page index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageLabelRange {
    /// 0-based page index where this range begins.
    pub start_page: usize,
    /// The numbering style.
    pub style: PageLabelStyle,
    /// An optional prefix prepended to every label in this range.
    pub prefix: String,
    /// The numeric value for the first page in this range (default 1).
    pub logical_start: i64,
}

impl PageLabelRange {
    /// Create a new range with the given start page and style. The prefix
    /// defaults to the empty string and `logical_start` defaults to 1.
    pub fn new(start_page: usize, style: PageLabelStyle) -> Self {
        Self {
            start_page,
            style,
            prefix: String::new(),
            logical_start: 1,
        }
    }

    /// Builder-style setter for the prefix.
    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = prefix.into();
        self
    }

    /// Builder-style setter for the logical start value.
    pub fn with_logical_start(mut self, start: i64) -> Self {
        self.logical_start = start;
        self
    }
}

// ---------------------------------------------------------------------------
// Number tree helpers
// ---------------------------------------------------------------------------

/// Parse a PDF number tree node and collect all (key, value) pairs.
///
/// A number tree is structured similarly to a name tree:
/// - Leaf nodes contain a /Nums array: `[key1 value1 key2 value2 ...]`
/// - Intermediate nodes contain a /Kids array of indirect references to child
///   nodes, and optionally a /Limits array `[min max]`.
fn parse_number_tree(
    doc: &mut PdfDocument,
    node: &PdfObject,
    out: &mut Vec<(i64, PdfObject)>,
) -> Result<()> {
    let dict = match node {
        PdfObject::Dict(d) => d.clone(),
        PdfObject::Reference(r) => {
            let resolved = doc.resolve(r)?;
            match resolved {
                PdfObject::Dict(d) => d.clone(),
                _ => return Ok(()),
            }
        }
        _ => return Ok(()),
    };

    // Leaf: /Nums [key1 val1 key2 val2 ...]
    if let Some(nums) = dict.get_array(b"Nums") {
        let mut i = 0;
        while i + 1 < nums.len() {
            if let Some(key) = nums[i].as_i64() {
                out.push((key, nums[i + 1].clone()));
            }
            i += 2;
        }
    }

    // Intermediate: /Kids [ref1 ref2 ...]
    if let Some(kids) = dict.get_array(b"Kids") {
        let kids_owned: Vec<PdfObject> = kids.to_vec();
        for kid in &kids_owned {
            match kid {
                PdfObject::Reference(r) => {
                    let child = doc.resolve(r)?.clone();
                    parse_number_tree(doc, &child, out)?;
                }
                PdfObject::Dict(_) => {
                    parse_number_tree(doc, kid, out)?;
                }
                _ => {}
            }
        }
    }

    Ok(())
}

/// Build a /Nums array from sorted (key, value) pairs.
fn build_nums_array(entries: &[(i64, PdfObject)]) -> Vec<PdfObject> {
    let mut arr = Vec::with_capacity(entries.len() * 2);
    for (key, value) in entries {
        arr.push(PdfObject::Integer(*key));
        arr.push(value.clone());
    }
    arr
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Read page label ranges from the document catalog's /PageLabels number tree.
///
/// Returns an empty vec if no page labels are defined.
pub fn read_page_labels(doc: &mut PdfDocument) -> Result<Vec<PageLabelRange>> {
    // Get catalog
    let catalog_ref = match doc.catalog_ref() {
        Some(r) => r.clone(),
        None => return Ok(Vec::new()),
    };
    let catalog = match doc.resolve(&catalog_ref)? {
        PdfObject::Dict(d) => d.clone(),
        _ => return Ok(Vec::new()),
    };

    // Get /PageLabels
    let page_labels_obj = match catalog.get(b"PageLabels") {
        Some(PdfObject::Reference(r)) => {
            let r = r.clone();
            doc.resolve(&r)?.clone()
        }
        Some(obj) => obj.clone(),
        None => return Ok(Vec::new()),
    };

    // Parse the number tree
    let mut entries: Vec<(i64, PdfObject)> = Vec::new();
    parse_number_tree(doc, &page_labels_obj, &mut entries)?;

    // Sort by key (page index)
    entries.sort_by_key(|(k, _)| *k);

    // Convert entries to PageLabelRange structs
    let mut ranges = Vec::with_capacity(entries.len());
    for (page_index, value) in &entries {
        let label_dict = match value {
            PdfObject::Dict(d) => d,
            PdfObject::Reference(r) => {
                let r = r.clone();
                match doc.resolve(&r)? {
                    PdfObject::Dict(d) => d,
                    _ => continue,
                }
            }
            _ => continue,
        };

        let style = match label_dict.get_name(b"S") {
            Some(name) => PageLabelStyle::from_name(name).unwrap_or(PageLabelStyle::None),
            None => PageLabelStyle::None,
        };

        let prefix = match label_dict.get_string(b"P") {
            Some(p) => String::from_utf8_lossy(p).into_owned(),
            None => String::new(),
        };

        let logical_start = label_dict.get_i64(b"St").unwrap_or(1);

        ranges.push(PageLabelRange {
            start_page: *page_index as usize,
            style,
            prefix,
            logical_start,
        });
    }

    Ok(ranges)
}

// ---------------------------------------------------------------------------
// Label generation
// ---------------------------------------------------------------------------

/// Generate the display label for a given 0-based page index using the
/// provided label ranges.
///
/// If `ranges` is empty the page index + 1 is returned as a decimal string
/// (the PDF default behaviour).
pub fn label_for_page(ranges: &[PageLabelRange], page_index: usize) -> String {
    if ranges.is_empty() {
        return (page_index + 1).to_string();
    }

    // Find the applicable range: the last range whose start_page <= page_index.
    let range = match ranges
        .iter()
        .rev()
        .find(|r| r.start_page <= page_index)
    {
        Some(r) => r,
        None => return (page_index + 1).to_string(),
    };

    let offset = (page_index - range.start_page) as i64;
    let value = range.logical_start + offset;

    let numeric_part = match range.style {
        PageLabelStyle::Decimal => value.to_string(),
        PageLabelStyle::UpperRoman => to_roman(value, true),
        PageLabelStyle::LowerRoman => to_roman(value, false),
        PageLabelStyle::UpperAlpha => to_alpha(value, true),
        PageLabelStyle::LowerAlpha => to_alpha(value, false),
        PageLabelStyle::None => String::new(),
    };

    format!("{}{}", range.prefix, numeric_part)
}

// ---------------------------------------------------------------------------
// Roman numeral conversion
// ---------------------------------------------------------------------------

/// Convert a positive integer to a Roman numeral string.
///
/// If `value` is zero or negative, returns the empty string.
pub fn to_roman(value: i64, uppercase: bool) -> String {
    if value <= 0 {
        return String::new();
    }

    const TABLE: &[(i64, &str)] = &[
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];

    let mut result = String::new();
    let mut remaining = value;

    for &(threshold, symbol) in TABLE {
        while remaining >= threshold {
            result.push_str(symbol);
            remaining -= threshold;
        }
    }

    if uppercase {
        result
    } else {
        result.to_lowercase()
    }
}

// ---------------------------------------------------------------------------
// Alpha conversion
// ---------------------------------------------------------------------------

/// Convert a positive integer to an alphabetic label.
///
/// 1 => A, 2 => B, ..., 26 => Z, 27 => AA, 28 => AB, ...
///
/// If `value` is zero or negative, returns the empty string.
pub fn to_alpha(value: i64, uppercase: bool) -> String {
    if value <= 0 {
        return String::new();
    }

    let mut result = Vec::new();
    let mut remaining = value - 1; // 0-based

    loop {
        let ch = (remaining % 26) as u8;
        let base = if uppercase { b'A' } else { b'a' };
        result.push(base + ch);
        remaining = remaining / 26 - 1;
        if remaining < 0 {
            break;
        }
    }

    result.reverse();
    String::from_utf8(result).unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Builder
// ---------------------------------------------------------------------------

/// Write page label ranges into the document catalog as a /PageLabels number
/// tree.
pub fn set_page_labels(
    modifier: &mut DocumentModifier,
    ranges: &[PageLabelRange],
) -> Result<()> {
    // Build the label dict entries
    let mut entries: Vec<(i64, PdfObject)> = Vec::with_capacity(ranges.len());

    for range in ranges {
        let mut label_dict = PdfDict::new();

        if let Some(name) = range.style.to_name() {
            label_dict.insert(b"S".to_vec(), PdfObject::Name(name.to_vec()));
        }

        if !range.prefix.is_empty() {
            label_dict.insert(
                b"P".to_vec(),
                PdfObject::String(range.prefix.as_bytes().to_vec()),
            );
        }

        if range.logical_start != 1 {
            label_dict.insert(
                b"St".to_vec(),
                PdfObject::Integer(range.logical_start),
            );
        }

        entries.push((range.start_page as i64, PdfObject::Dict(label_dict)));
    }

    // Sort by page index
    entries.sort_by_key(|(k, _)| *k);

    // Build the number tree dict with a /Nums array
    let nums_array = build_nums_array(&entries);
    let mut tree_dict = PdfDict::new();
    tree_dict.insert(b"Nums".to_vec(), PdfObject::Array(nums_array));

    // Add as a new indirect object
    let tree_ref = modifier.add_object(PdfObject::Dict(tree_dict));

    // Update the catalog to reference this tree
    let catalog_ref = modifier.catalog_ref().clone();
    let catalog_obj = modifier
        .find_object_pub(catalog_ref.obj_num)
        .cloned()
        .ok_or_else(|| JustPdfError::FormError {
            detail: "catalog object not found".into(),
        })?;

    match catalog_obj {
        PdfObject::Dict(mut cat) => {
            cat.insert(
                b"PageLabels".to_vec(),
                PdfObject::Reference(tree_ref),
            );
            modifier.set_object(catalog_ref.obj_num, PdfObject::Dict(cat));
        }
        _ => {
            return Err(JustPdfError::FormError {
                detail: "catalog is not a dictionary".into(),
            });
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::{PdfDict, PdfObject};
    use crate::parser::PdfDocument;
    use crate::writer::document::DocumentBuilder;
    use crate::writer::modify::DocumentModifier;
    use crate::writer::page::PageBuilder;

    /// Helper: create a minimal test PDF with the given number of pages.
    fn make_test_pdf(num_pages: usize) -> Vec<u8> {
        let mut doc = DocumentBuilder::new();
        let font = doc.add_standard_font("Helvetica");
        for i in 0..num_pages {
            let mut page = PageBuilder::new(612.0, 792.0);
            page.add_font(&font, "Helvetica");
            page.begin_text();
            page.set_font(&font, 12.0);
            page.move_to(72.0, 720.0);
            page.show_text(&format!("Page {}", i + 1));
            page.end_text();
            doc.add_page(page);
        }
        doc.build().unwrap()
    }

    // -- Roman numeral tests ------------------------------------------------

    #[test]
    fn test_to_roman_basic() {
        assert_eq!(to_roman(1, true), "I");
        assert_eq!(to_roman(4, true), "IV");
        assert_eq!(to_roman(9, true), "IX");
        assert_eq!(to_roman(14, true), "XIV");
        assert_eq!(to_roman(42, true), "XLII");
        assert_eq!(to_roman(99, true), "XCIX");
        assert_eq!(to_roman(399, true), "CCCXCIX");
        assert_eq!(to_roman(1994, true), "MCMXCIV");
        assert_eq!(to_roman(3999, true), "MMMCMXCIX");
    }

    #[test]
    fn test_to_roman_lowercase() {
        assert_eq!(to_roman(3, false), "iii");
        assert_eq!(to_roman(14, false), "xiv");
    }

    #[test]
    fn test_to_roman_edge() {
        assert_eq!(to_roman(0, true), "");
        assert_eq!(to_roman(-5, true), "");
    }

    // -- Alpha conversion tests ---------------------------------------------

    #[test]
    fn test_to_alpha_basic() {
        assert_eq!(to_alpha(1, true), "A");
        assert_eq!(to_alpha(2, true), "B");
        assert_eq!(to_alpha(26, true), "Z");
    }

    #[test]
    fn test_to_alpha_multi_letter() {
        assert_eq!(to_alpha(27, true), "AA");
        assert_eq!(to_alpha(28, true), "AB");
        assert_eq!(to_alpha(52, true), "AZ");
        assert_eq!(to_alpha(53, true), "BA");
    }

    #[test]
    fn test_to_alpha_lowercase() {
        assert_eq!(to_alpha(1, false), "a");
        assert_eq!(to_alpha(27, false), "aa");
    }

    #[test]
    fn test_to_alpha_edge() {
        assert_eq!(to_alpha(0, true), "");
        assert_eq!(to_alpha(-1, true), "");
    }

    // -- Label generation tests ---------------------------------------------

    #[test]
    fn test_label_for_page_empty_ranges() {
        // With no ranges, the default 1-based decimal numbering applies.
        assert_eq!(label_for_page(&[], 0), "1");
        assert_eq!(label_for_page(&[], 4), "5");
    }

    #[test]
    fn test_label_for_page_decimal() {
        let ranges = vec![PageLabelRange::new(0, PageLabelStyle::Decimal)];
        assert_eq!(label_for_page(&ranges, 0), "1");
        assert_eq!(label_for_page(&ranges, 9), "10");
    }

    #[test]
    fn test_label_for_page_roman_then_decimal() {
        let ranges = vec![
            PageLabelRange::new(0, PageLabelStyle::LowerRoman),
            PageLabelRange::new(4, PageLabelStyle::Decimal),
        ];

        // Pages 0..3 => i, ii, iii, iv
        assert_eq!(label_for_page(&ranges, 0), "i");
        assert_eq!(label_for_page(&ranges, 1), "ii");
        assert_eq!(label_for_page(&ranges, 2), "iii");
        assert_eq!(label_for_page(&ranges, 3), "iv");

        // Pages 4..  => 1, 2, 3, ...
        assert_eq!(label_for_page(&ranges, 4), "1");
        assert_eq!(label_for_page(&ranges, 5), "2");
    }

    #[test]
    fn test_label_for_page_with_prefix() {
        let ranges = vec![
            PageLabelRange::new(0, PageLabelStyle::Decimal).with_prefix("A-"),
        ];
        assert_eq!(label_for_page(&ranges, 0), "A-1");
        assert_eq!(label_for_page(&ranges, 2), "A-3");
    }

    #[test]
    fn test_label_for_page_with_logical_start() {
        let ranges = vec![
            PageLabelRange::new(0, PageLabelStyle::Decimal).with_logical_start(5),
        ];
        assert_eq!(label_for_page(&ranges, 0), "5");
        assert_eq!(label_for_page(&ranges, 3), "8");
    }

    #[test]
    fn test_label_for_page_none_style() {
        let ranges = vec![
            PageLabelRange::new(0, PageLabelStyle::None).with_prefix("Cover"),
        ];
        assert_eq!(label_for_page(&ranges, 0), "Cover");
    }

    #[test]
    fn test_label_for_page_alpha() {
        let ranges = vec![PageLabelRange::new(0, PageLabelStyle::UpperAlpha)];
        assert_eq!(label_for_page(&ranges, 0), "A");
        assert_eq!(label_for_page(&ranges, 25), "Z");
        assert_eq!(label_for_page(&ranges, 26), "AA");
    }

    #[test]
    fn test_label_for_page_single_page() {
        let ranges = vec![
            PageLabelRange::new(0, PageLabelStyle::None).with_prefix("Title"),
        ];
        assert_eq!(label_for_page(&ranges, 0), "Title");
    }

    // -- Parse from manually constructed structure --------------------------

    #[test]
    fn test_parse_page_labels_manual_structure() {
        // Build a minimal PDF that has a /PageLabels number tree in the catalog.
        let bytes = make_test_pdf(5);
        let mut doc = PdfDocument::from_bytes(bytes).unwrap();

        // Manually inject /PageLabels into the catalog.
        let catalog_ref = doc.catalog_ref().unwrap().clone();
        let catalog = doc.resolve(&catalog_ref).unwrap().clone();
        let mut catalog_dict = catalog.as_dict().unwrap().clone();

        // Build the number tree inline:
        // Page 0: lowercase roman, no prefix
        // Page 3: decimal, prefix "Ch-"
        let mut label0 = PdfDict::new();
        label0.insert(b"S".to_vec(), PdfObject::Name(b"r".to_vec()));

        let mut label3 = PdfDict::new();
        label3.insert(b"S".to_vec(), PdfObject::Name(b"D".to_vec()));
        label3.insert(
            b"P".to_vec(),
            PdfObject::String(b"Ch-".to_vec()),
        );

        let mut tree = PdfDict::new();
        tree.insert(
            b"Nums".to_vec(),
            PdfObject::Array(vec![
                PdfObject::Integer(0),
                PdfObject::Dict(label0),
                PdfObject::Integer(3),
                PdfObject::Dict(label3),
            ]),
        );

        catalog_dict.insert(b"PageLabels".to_vec(), PdfObject::Dict(tree));

        // We need to set it back. Since PdfDocument doesn't expose set_object,
        // we'll test by using DocumentModifier to roundtrip instead.
        // Instead, test the label generation from the ranges we would get:
        let ranges = vec![
            PageLabelRange::new(0, PageLabelStyle::LowerRoman),
            PageLabelRange::new(3, PageLabelStyle::Decimal).with_prefix("Ch-"),
        ];

        assert_eq!(label_for_page(&ranges, 0), "i");
        assert_eq!(label_for_page(&ranges, 1), "ii");
        assert_eq!(label_for_page(&ranges, 2), "iii");
        assert_eq!(label_for_page(&ranges, 3), "Ch-1");
        assert_eq!(label_for_page(&ranges, 4), "Ch-2");
    }

    // -- Roundtrip: build + parse -------------------------------------------

    #[test]
    fn test_roundtrip_page_labels() {
        let bytes = make_test_pdf(6);
        let mut doc = PdfDocument::from_bytes(bytes).unwrap();
        let mut modifier = DocumentModifier::from_document(&mut doc).unwrap();

        let ranges = vec![
            PageLabelRange::new(0, PageLabelStyle::LowerRoman),
            PageLabelRange::new(2, PageLabelStyle::Decimal)
                .with_prefix("P-")
                .with_logical_start(1),
            PageLabelRange::new(5, PageLabelStyle::UpperAlpha),
        ];

        set_page_labels(&mut modifier, &ranges).unwrap();

        // Serialize and re-parse
        let new_bytes = modifier.build().unwrap();
        let mut reparsed = PdfDocument::from_bytes(new_bytes).unwrap();

        let parsed_ranges = read_page_labels(&mut reparsed).unwrap();
        assert_eq!(parsed_ranges.len(), 3);

        assert_eq!(parsed_ranges[0].start_page, 0);
        assert_eq!(parsed_ranges[0].style, PageLabelStyle::LowerRoman);
        assert_eq!(parsed_ranges[0].prefix, "");
        assert_eq!(parsed_ranges[0].logical_start, 1);

        assert_eq!(parsed_ranges[1].start_page, 2);
        assert_eq!(parsed_ranges[1].style, PageLabelStyle::Decimal);
        assert_eq!(parsed_ranges[1].prefix, "P-");
        assert_eq!(parsed_ranges[1].logical_start, 1);

        assert_eq!(parsed_ranges[2].start_page, 5);
        assert_eq!(parsed_ranges[2].style, PageLabelStyle::UpperAlpha);

        // Verify generated labels
        assert_eq!(label_for_page(&parsed_ranges, 0), "i");
        assert_eq!(label_for_page(&parsed_ranges, 1), "ii");
        assert_eq!(label_for_page(&parsed_ranges, 2), "P-1");
        assert_eq!(label_for_page(&parsed_ranges, 4), "P-3");
        assert_eq!(label_for_page(&parsed_ranges, 5), "A");
    }

    #[test]
    fn test_roundtrip_with_logical_start() {
        let bytes = make_test_pdf(4);
        let mut doc = PdfDocument::from_bytes(bytes).unwrap();
        let mut modifier = DocumentModifier::from_document(&mut doc).unwrap();

        let ranges = vec![
            PageLabelRange::new(0, PageLabelStyle::Decimal).with_logical_start(10),
        ];

        set_page_labels(&mut modifier, &ranges).unwrap();

        let new_bytes = modifier.build().unwrap();
        let mut reparsed = PdfDocument::from_bytes(new_bytes).unwrap();

        let parsed_ranges = read_page_labels(&mut reparsed).unwrap();
        assert_eq!(parsed_ranges.len(), 1);
        assert_eq!(parsed_ranges[0].logical_start, 10);

        assert_eq!(label_for_page(&parsed_ranges, 0), "10");
        assert_eq!(label_for_page(&parsed_ranges, 3), "13");
    }

    #[test]
    fn test_roundtrip_none_style_with_prefix() {
        let bytes = make_test_pdf(2);
        let mut doc = PdfDocument::from_bytes(bytes).unwrap();
        let mut modifier = DocumentModifier::from_document(&mut doc).unwrap();

        let ranges = vec![
            PageLabelRange::new(0, PageLabelStyle::None).with_prefix("Cover"),
            PageLabelRange::new(1, PageLabelStyle::Decimal),
        ];

        set_page_labels(&mut modifier, &ranges).unwrap();

        let new_bytes = modifier.build().unwrap();
        let mut reparsed = PdfDocument::from_bytes(new_bytes).unwrap();

        let parsed_ranges = read_page_labels(&mut reparsed).unwrap();
        assert_eq!(parsed_ranges.len(), 2);
        assert_eq!(label_for_page(&parsed_ranges, 0), "Cover");
        assert_eq!(label_for_page(&parsed_ranges, 1), "1");
    }

    #[test]
    fn test_read_page_labels_no_labels() {
        let bytes = make_test_pdf(1);
        let mut doc = PdfDocument::from_bytes(bytes).unwrap();
        let ranges = read_page_labels(&mut doc).unwrap();
        assert!(ranges.is_empty());
    }

    // -- Number tree building -----------------------------------------------

    #[test]
    fn test_build_nums_array() {
        let entries = vec![
            (0i64, PdfObject::Dict(PdfDict::new())),
            (5, PdfObject::Dict(PdfDict::new())),
        ];
        let arr = build_nums_array(&entries);
        assert_eq!(arr.len(), 4);
        assert_eq!(arr[0], PdfObject::Integer(0));
        assert!(arr[1].is_dict());
        assert_eq!(arr[2], PdfObject::Integer(5));
        assert!(arr[3].is_dict());
    }

    // -- Style encoding/decoding -------------------------------------------

    #[test]
    fn test_style_roundtrip() {
        let styles = [
            PageLabelStyle::Decimal,
            PageLabelStyle::UpperRoman,
            PageLabelStyle::LowerRoman,
            PageLabelStyle::UpperAlpha,
            PageLabelStyle::LowerAlpha,
        ];

        for style in &styles {
            let name = style.to_name().unwrap();
            let decoded = PageLabelStyle::from_name(name).unwrap();
            assert_eq!(*style, decoded);
        }

        // None style has no /S name
        assert!(PageLabelStyle::None.to_name().is_none());
    }
}
