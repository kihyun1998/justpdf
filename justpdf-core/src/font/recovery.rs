//! Font recovery: substitution of missing or damaged fonts with Standard 14 equivalents.

use super::{FontDescriptor, FontInfo, FontWidths, Encoding};
use super::standard14::standard14_widths;

/// Case-insensitive check whether `haystack` contains `needle` (ASCII only).
fn contains_ci(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.len() > haystack.len() {
        return false;
    }
    haystack
        .windows(needle.len())
        .any(|w| w.eq_ignore_ascii_case(needle))
}

/// Check if a font name looks like it should be bold.
///
/// Matches names containing "Bold", "Heavy", "Black", or "Demi" (case-insensitive).
pub fn is_bold_name(name: &[u8]) -> bool {
    contains_ci(name, b"Bold")
        || contains_ci(name, b"Heavy")
        || contains_ci(name, b"Black")
        || contains_ci(name, b"Demi")
}

/// Check if a font name looks like it should be italic.
///
/// Matches names containing "Italic", "Oblique", "Slant", or "Inclined" (case-insensitive).
pub fn is_italic_name(name: &[u8]) -> bool {
    contains_ci(name, b"Italic")
        || contains_ci(name, b"Oblique")
        || contains_ci(name, b"Slant")
        || contains_ci(name, b"Inclined")
}

/// Check if a font name looks like it should be monospace.
///
/// Matches common monospace font families (case-insensitive).
pub fn is_monospace_name(name: &[u8]) -> bool {
    contains_ci(name, b"Courier")
        || contains_ci(name, b"Consolas")
        || contains_ci(name, b"Mono")
        || contains_ci(name, b"Menlo")
        || contains_ci(name, b"Inconsolata")
        || contains_ci(name, b"SourceCode")
        || contains_ci(name, b"FiraMono")
        || contains_ci(name, b"RobotoMono")
        || contains_ci(name, b"AndaleMono")
        || contains_ci(name, b"LucidaConsole")
        || contains_ci(name, b"DejaVuSansMono")
}

/// Check if a font name looks like a serif font.
///
/// Matches common serif font families (case-insensitive).
pub fn is_serif_name(name: &[u8]) -> bool {
    contains_ci(name, b"Times")
        || contains_ci(name, b"Cambria")
        || contains_ci(name, b"Georgia")
        || contains_ci(name, b"Garamond")
        || contains_ci(name, b"Palatino")
        || contains_ci(name, b"BookAntiqua")
        || contains_ci(name, b"Bookman")
        || contains_ci(name, b"Century")
        || contains_ci(name, b"Didot")
        || contains_ci(name, b"Baskerville")
        || contains_ci(name, b"Bodoni")
        || contains_ci(name, b"Minion")
        || contains_ci(name, b"Serif")
}

/// Check if a font name is a symbol/dingbats font.
fn is_symbol_name(name: &[u8]) -> bool {
    contains_ci(name, b"Symbol")
}

fn is_dingbats_name(name: &[u8]) -> bool {
    contains_ci(name, b"Wingding")
        || contains_ci(name, b"Dingbat")
        || contains_ci(name, b"ZapfDingbats")
        || contains_ci(name, b"Webdings")
}

/// Strip a subset prefix (e.g. "ABCDEF+Arial" -> "Arial") for matching purposes.
fn strip_subset_prefix(name: &[u8]) -> &[u8] {
    if name.len() > 7 && name[6] == b'+' && name[..6].iter().all(|&b| b.is_ascii_uppercase()) {
        &name[7..]
    } else {
        name
    }
}

/// Select the appropriate Standard 14 variant given a base family and style flags.
fn select_variant(family: &str, bold: bool, italic: bool) -> &'static [u8] {
    match family {
        "Helvetica" => match (bold, italic) {
            (false, false) => b"Helvetica",
            (true, false) => b"Helvetica-Bold",
            (false, true) => b"Helvetica-Oblique",
            (true, true) => b"Helvetica-BoldOblique",
        },
        "Times" => match (bold, italic) {
            (false, false) => b"Times-Roman",
            (true, false) => b"Times-Bold",
            (false, true) => b"Times-Italic",
            (true, true) => b"Times-BoldItalic",
        },
        "Courier" => match (bold, italic) {
            (false, false) => b"Courier",
            (true, false) => b"Courier-Bold",
            (false, true) => b"Courier-Oblique",
            (true, true) => b"Courier-BoldOblique",
        },
        _ => b"Helvetica",
    }
}

/// Suggest a Standard 14 substitute for a missing or unknown font.
///
/// Maps common font names to their closest Standard 14 equivalents:
/// - Arial, Verdana, Calibri, Tahoma, etc. -> Helvetica family
/// - Times New Roman, Cambria, Georgia, etc. -> Times-Roman family
/// - Courier New, Consolas, Menlo, etc. -> Courier family
/// - Symbol -> Symbol
/// - Wingdings -> ZapfDingbats
///
/// Bold and italic variants in the font name are respected: e.g.
/// "Arial-BoldItalic" maps to "Helvetica-BoldOblique".
///
/// Completely unknown fonts fall back to Helvetica (the most common
/// substitute in PDF viewers).
pub fn find_substitute(font_name: &[u8]) -> &'static [u8] {
    let name = strip_subset_prefix(font_name);

    // Exact Standard 14 names pass through (but callers would normally not
    // reach here for those).
    if super::standard14::is_standard14(font_name) {
        // Return the canonical name. We match on the stripped name.
        return match name {
            n if n.starts_with(b"Courier") => select_variant("Courier", is_bold_name(n), is_italic_name(n)),
            n if n.starts_with(b"Helvetica") => select_variant("Helvetica", is_bold_name(n), is_italic_name(n)),
            n if n.starts_with(b"Times") => select_variant("Times", is_bold_name(n), is_italic_name(n)),
            b"Symbol" => b"Symbol",
            b"ZapfDingbats" => b"ZapfDingbats",
            _ => b"Helvetica",
        };
    }

    // Special symbol/dingbats fonts first — these should not be mapped to text fonts.
    if is_dingbats_name(name) {
        return b"ZapfDingbats";
    }
    if is_symbol_name(name) {
        return b"Symbol";
    }

    let bold = is_bold_name(name);
    let italic = is_italic_name(name);

    // Monospace detection
    if is_monospace_name(name) {
        return select_variant("Courier", bold, italic);
    }

    // Serif detection
    if is_serif_name(name) {
        return select_variant("Times", bold, italic);
    }

    // Common sans-serif font families (case-insensitive check via contains_ci
    // on the canonical portion of the name).
    let sans_serif_families: &[&[u8]] = &[
        b"Arial",
        b"Verdana",
        b"Calibri",
        b"Tahoma",
        b"Trebuchet",
        b"SegoeUI",
        b"Segoe",
        b"LucidaGrande",
        b"LucidaSans",
        b"Geneva",
        b"Optima",
        b"Futura",
        b"GillSans",
        b"Candara",
        b"Franklin",
        b"Corbel",
        b"SansSerif",
        b"Roboto",
        b"OpenSans",
        b"Lato",
        b"Noto",
        b"Ubuntu",
        b"DejaVuSans",
        b"Liberation",
        b"FreeSans",
    ];

    for family in sans_serif_families {
        if contains_ci(name, family) {
            return select_variant("Helvetica", bold, italic);
        }
    }

    // PDF-specific name variants: ArialMT, TimesNewRomanPSMT, CourierNewPSMT, etc.
    if contains_ci(name, b"ArialMT") || contains_ci(name, b"Arial-") || contains_ci(name, b"ArialNarrow") {
        return select_variant("Helvetica", bold, italic);
    }
    if contains_ci(name, b"TimesNewRoman") || contains_ci(name, b"TimesNewRomanPS") {
        return select_variant("Times", bold, italic);
    }
    if contains_ci(name, b"CourierNew") || contains_ci(name, b"CourierNewPS") {
        return select_variant("Courier", bold, italic);
    }

    // Default: Helvetica (most commonly used fallback in PDF viewers).
    select_variant("Helvetica", bold, italic)
}

/// Get a fallback `FontInfo` using a Standard 14 substitute.
///
/// This creates a usable `FontInfo` with approximate widths from the
/// selected Standard 14 font, suitable for rendering when the original
/// font data is missing or damaged.
pub fn fallback_font_info(font_name: &[u8]) -> FontInfo {
    let substitute = find_substitute(font_name);
    let widths_vec = standard14_widths(substitute);

    let widths = if widths_vec.is_empty() {
        FontWidths::None {
            default_width: 500.0,
        }
    } else {
        FontWidths::Simple {
            first_char: 0,
            widths: widths_vec,
            default_width: 600.0,
        }
    };

    let bold = is_bold_name(font_name);
    let italic = is_italic_name(font_name);
    let mono = substitute.starts_with(b"Courier");
    let serif = substitute.starts_with(b"Times");

    let mut flags = FontDescriptor::NONSYMBOLIC;
    if mono {
        flags |= FontDescriptor::FIXED_PITCH;
    }
    if serif {
        flags |= FontDescriptor::SERIF;
    }
    if italic {
        flags |= FontDescriptor::ITALIC;
    }

    let descriptor = FontDescriptor {
        font_name: substitute.to_vec(),
        font_family: None,
        flags,
        font_b_box: None,
        italic_angle: if italic { -12.0 } else { 0.0 },
        ascent: 750.0,
        descent: -250.0,
        cap_height: Some(718.0),
        x_height: Some(523.0),
        stem_v: if bold { 140.0 } else { 76.0 },
        stem_h: None,
        avg_width: None,
        max_width: None,
        missing_width: None,
        leading: None,
        font_file_ref: None,
        font_file2_ref: None,
        font_file3_ref: None,
    };

    FontInfo {
        base_font: substitute.to_vec(),
        subtype: b"Type1".to_vec(),
        encoding: Encoding::WinAnsiEncoding,
        widths,
        to_unicode: None,
        is_standard14: true,
        descriptor: Some(descriptor),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------------------------------------------------------
    // find_substitute: common font -> Standard 14 mappings
    // ---------------------------------------------------------------

    #[test]
    fn test_arial_to_helvetica() {
        assert_eq!(find_substitute(b"Arial"), b"Helvetica");
    }

    #[test]
    fn test_arial_bold_to_helvetica_bold() {
        assert_eq!(find_substitute(b"Arial-Bold"), b"Helvetica-Bold");
        assert_eq!(find_substitute(b"Arial,Bold"), b"Helvetica-Bold");
    }

    #[test]
    fn test_arial_italic() {
        assert_eq!(find_substitute(b"Arial-Italic"), b"Helvetica-Oblique");
    }

    #[test]
    fn test_arial_bold_italic() {
        assert_eq!(find_substitute(b"Arial-BoldItalic"), b"Helvetica-BoldOblique");
    }

    #[test]
    fn test_arialmt_to_helvetica() {
        assert_eq!(find_substitute(b"ArialMT"), b"Helvetica");
    }

    #[test]
    fn test_arialmt_bold() {
        assert_eq!(find_substitute(b"Arial-BoldMT"), b"Helvetica-Bold");
    }

    #[test]
    fn test_times_new_roman_to_times() {
        assert_eq!(find_substitute(b"TimesNewRoman"), b"Times-Roman");
        assert_eq!(find_substitute(b"TimesNewRomanPSMT"), b"Times-Roman");
        assert_eq!(find_substitute(b"Times New Roman"), b"Times-Roman");
    }

    #[test]
    fn test_times_new_roman_bold() {
        assert_eq!(find_substitute(b"TimesNewRoman,Bold"), b"Times-Bold");
        assert_eq!(find_substitute(b"TimesNewRomanPS-BoldMT"), b"Times-Bold");
    }

    #[test]
    fn test_times_new_roman_italic() {
        assert_eq!(find_substitute(b"TimesNewRoman-Italic"), b"Times-Italic");
    }

    #[test]
    fn test_times_new_roman_bold_italic() {
        assert_eq!(find_substitute(b"TimesNewRoman-BoldItalic"), b"Times-BoldItalic");
    }

    #[test]
    fn test_courier_new_to_courier() {
        assert_eq!(find_substitute(b"CourierNew"), b"Courier");
        assert_eq!(find_substitute(b"CourierNewPSMT"), b"Courier");
        assert_eq!(find_substitute(b"Courier New"), b"Courier");
    }

    #[test]
    fn test_courier_new_bold() {
        assert_eq!(find_substitute(b"CourierNew-Bold"), b"Courier-Bold");
    }

    #[test]
    fn test_verdana_to_helvetica() {
        assert_eq!(find_substitute(b"Verdana"), b"Helvetica");
    }

    #[test]
    fn test_calibri_to_helvetica() {
        assert_eq!(find_substitute(b"Calibri"), b"Helvetica");
        assert_eq!(find_substitute(b"Calibri-Bold"), b"Helvetica-Bold");
    }

    #[test]
    fn test_consolas_to_courier() {
        assert_eq!(find_substitute(b"Consolas"), b"Courier");
        assert_eq!(find_substitute(b"Consolas-Bold"), b"Courier-Bold");
    }

    #[test]
    fn test_cambria_to_times() {
        assert_eq!(find_substitute(b"Cambria"), b"Times-Roman");
    }

    #[test]
    fn test_georgia_to_times() {
        assert_eq!(find_substitute(b"Georgia"), b"Times-Roman");
        assert_eq!(find_substitute(b"Georgia-Bold"), b"Times-Bold");
        assert_eq!(find_substitute(b"Georgia-BoldItalic"), b"Times-BoldItalic");
    }

    #[test]
    fn test_symbol_passthrough() {
        assert_eq!(find_substitute(b"Symbol"), b"Symbol");
    }

    #[test]
    fn test_wingdings_to_zapf() {
        assert_eq!(find_substitute(b"Wingdings"), b"ZapfDingbats");
        assert_eq!(find_substitute(b"Wingdings2"), b"ZapfDingbats");
    }

    #[test]
    fn test_menlo_to_courier() {
        assert_eq!(find_substitute(b"Menlo-Regular"), b"Courier");
        assert_eq!(find_substitute(b"Menlo-Bold"), b"Courier-Bold");
        assert_eq!(find_substitute(b"Menlo-BoldItalic"), b"Courier-BoldOblique");
    }

    // ---------------------------------------------------------------
    // Subset prefixed names
    // ---------------------------------------------------------------

    #[test]
    fn test_subset_prefix_stripped() {
        assert_eq!(find_substitute(b"ABCDEF+Arial"), b"Helvetica");
        assert_eq!(find_substitute(b"XYZABC+TimesNewRoman-Bold"), b"Times-Bold");
    }

    // ---------------------------------------------------------------
    // CJK fonts: best available fallback is Helvetica
    // ---------------------------------------------------------------

    #[test]
    fn test_cjk_msgothic_fallback() {
        assert_eq!(find_substitute(b"MSGothic"), b"Helvetica");
        assert_eq!(find_substitute(b"MS-Gothic"), b"Helvetica");
    }

    #[test]
    fn test_cjk_simsun_fallback() {
        assert_eq!(find_substitute(b"SimSun"), b"Helvetica");
    }

    #[test]
    fn test_cjk_mingliu_fallback() {
        assert_eq!(find_substitute(b"MingLiU"), b"Helvetica");
    }

    #[test]
    fn test_cjk_msmincho_fallback() {
        assert_eq!(find_substitute(b"MS-Mincho"), b"Helvetica");
    }

    #[test]
    fn test_cjk_malgun_gothic_fallback() {
        assert_eq!(find_substitute(b"MalgunGothic"), b"Helvetica");
    }

    // ---------------------------------------------------------------
    // Completely unknown fonts -> Helvetica
    // ---------------------------------------------------------------

    #[test]
    fn test_unknown_font_fallback() {
        assert_eq!(find_substitute(b"MyCustomFont"), b"Helvetica");
        assert_eq!(find_substitute(b"SomeRandomName"), b"Helvetica");
        assert_eq!(find_substitute(b"XXXXXX"), b"Helvetica");
    }

    #[test]
    fn test_unknown_bold_font_fallback() {
        assert_eq!(find_substitute(b"MyCustomFont-Bold"), b"Helvetica-Bold");
    }

    #[test]
    fn test_unknown_italic_font_fallback() {
        assert_eq!(find_substitute(b"SomeFont-Italic"), b"Helvetica-Oblique");
    }

    #[test]
    fn test_unknown_bold_italic_font_fallback() {
        assert_eq!(find_substitute(b"SomeFont-BoldOblique"), b"Helvetica-BoldOblique");
    }

    // ---------------------------------------------------------------
    // is_bold_name
    // ---------------------------------------------------------------

    #[test]
    fn test_is_bold_name() {
        assert!(is_bold_name(b"Arial-Bold"));
        assert!(is_bold_name(b"Helvetica-BoldOblique"));
        assert!(is_bold_name(b"SomeFont-Heavy"));
        assert!(is_bold_name(b"FuturaBlack"));
        assert!(is_bold_name(b"DemiBold"));
        assert!(!is_bold_name(b"Arial"));
        assert!(!is_bold_name(b"Helvetica"));
        assert!(!is_bold_name(b"Times-Italic"));
    }

    // ---------------------------------------------------------------
    // is_italic_name
    // ---------------------------------------------------------------

    #[test]
    fn test_is_italic_name() {
        assert!(is_italic_name(b"Arial-Italic"));
        assert!(is_italic_name(b"Helvetica-Oblique"));
        assert!(is_italic_name(b"SomeFont-Slant"));
        assert!(is_italic_name(b"CustomInclined"));
        assert!(!is_italic_name(b"Arial"));
        assert!(!is_italic_name(b"Helvetica-Bold"));
    }

    // ---------------------------------------------------------------
    // is_monospace_name
    // ---------------------------------------------------------------

    #[test]
    fn test_is_monospace_name() {
        assert!(is_monospace_name(b"Courier"));
        assert!(is_monospace_name(b"CourierNew"));
        assert!(is_monospace_name(b"Consolas"));
        assert!(is_monospace_name(b"DejaVuSansMono"));
        assert!(is_monospace_name(b"Menlo-Regular"));
        assert!(is_monospace_name(b"LucidaConsole"));
        assert!(!is_monospace_name(b"Arial"));
        assert!(!is_monospace_name(b"TimesNewRoman"));
    }

    // ---------------------------------------------------------------
    // is_serif_name
    // ---------------------------------------------------------------

    #[test]
    fn test_is_serif_name() {
        assert!(is_serif_name(b"Times-Roman"));
        assert!(is_serif_name(b"TimesNewRoman"));
        assert!(is_serif_name(b"Cambria"));
        assert!(is_serif_name(b"Georgia"));
        assert!(is_serif_name(b"Garamond"));
        assert!(is_serif_name(b"Palatino"));
        assert!(is_serif_name(b"Baskerville"));
        assert!(!is_serif_name(b"Arial"));
        assert!(!is_serif_name(b"Helvetica"));
    }

    // ---------------------------------------------------------------
    // fallback_font_info
    // ---------------------------------------------------------------

    #[test]
    fn test_fallback_font_info_basic() {
        let info = fallback_font_info(b"Arial");
        assert_eq!(info.base_font, b"Helvetica");
        assert_eq!(info.subtype, b"Type1");
        assert!(info.is_standard14);
        assert_eq!(info.encoding, super::super::Encoding::WinAnsiEncoding);
    }

    #[test]
    fn test_fallback_font_info_has_widths() {
        let info = fallback_font_info(b"TimesNewRoman-Bold");
        assert_eq!(info.base_font, b"Times-Bold");
        // Should have real width data
        let w = info.widths.get_width(65); // 'A'
        assert!(w > 0.0, "Expected positive width for 'A', got {w}");
    }

    #[test]
    fn test_fallback_font_info_courier() {
        let info = fallback_font_info(b"Consolas");
        assert_eq!(info.base_font, b"Courier");
        // Courier is monospaced at 600
        assert_eq!(info.widths.get_width(65), 600.0);
        assert_eq!(info.widths.get_width(97), 600.0);
    }

    // ---------------------------------------------------------------
    // CamelCase and hyphenated name handling
    // ---------------------------------------------------------------

    #[test]
    fn test_camelcase_names() {
        assert_eq!(find_substitute(b"ArialNarrow"), b"Helvetica");
        assert_eq!(find_substitute(b"ArialNarrow-Bold"), b"Helvetica-Bold");
        assert_eq!(find_substitute(b"BookAntiqua"), b"Times-Roman");
        assert_eq!(find_substitute(b"LucidaConsole"), b"Courier");
    }

    #[test]
    fn test_hyphenated_names() {
        assert_eq!(find_substitute(b"Courier-BoldOblique"), b"Courier-BoldOblique");
        assert_eq!(find_substitute(b"Helvetica-Bold"), b"Helvetica-Bold");
        assert_eq!(find_substitute(b"Times-BoldItalic"), b"Times-BoldItalic");
    }
}
