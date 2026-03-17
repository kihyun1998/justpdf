/// Check if a font name is one of the PDF Standard 14 fonts.
pub fn is_standard14(name: &[u8]) -> bool {
    // Strip subset prefix (e.g., "ABCDEF+Helvetica" → "Helvetica")
    let name = strip_subset_prefix(name);

    matches!(
        name,
        b"Courier"
            | b"Courier-Bold"
            | b"Courier-Oblique"
            | b"Courier-BoldOblique"
            | b"Helvetica"
            | b"Helvetica-Bold"
            | b"Helvetica-Oblique"
            | b"Helvetica-BoldOblique"
            | b"Times-Roman"
            | b"Times-Bold"
            | b"Times-Italic"
            | b"Times-BoldItalic"
            | b"Symbol"
            | b"ZapfDingbats"
    )
}

/// Get approximate glyph widths for Standard 14 fonts.
/// Returns widths for char codes 0-255 in 1/1000 units.
pub fn standard14_widths(name: &[u8]) -> Vec<f64> {
    let name = strip_subset_prefix(name);

    if name.starts_with(b"Courier") {
        // Courier: monospaced, all glyphs 600
        vec![600.0; 256]
    } else if name.starts_with(b"Helvetica") {
        helvetica_widths(name)
    } else if name.starts_with(b"Times") {
        times_widths(name)
    } else {
        // Symbol, ZapfDingbats, or unknown: use 500 as default
        vec![500.0; 256]
    }
}

fn strip_subset_prefix(name: &[u8]) -> &[u8] {
    // Subset prefix is exactly 6 uppercase letters followed by '+'
    if name.len() > 7 && name[6] == b'+' && name[..6].iter().all(|&b| b.is_ascii_uppercase()) {
        &name[7..]
    } else {
        name
    }
}

/// Simplified Helvetica widths (most common glyphs).
fn helvetica_widths(_variant: &[u8]) -> Vec<f64> {
    let mut w = vec![278.0; 256]; // default width

    // Space and common ASCII
    w[32] = 278.0; // space
    w[33] = 278.0; // !
    w[34] = 355.0; // "
    w[35] = 556.0; // #
    w[36] = 556.0; // $
    w[37] = 889.0; // %
    w[38] = 667.0; // &
    w[39] = 191.0; // '
    w[40] = 333.0; // (
    w[41] = 333.0; // )
    w[42] = 389.0; // *
    w[43] = 584.0; // +
    w[44] = 278.0; // ,
    w[45] = 333.0; // -
    w[46] = 278.0; // .
    w[47] = 278.0; // /

    // Digits 0-9: all 556
    for i in 48..=57 {
        w[i] = 556.0;
    }

    w[58] = 278.0; // :
    w[59] = 278.0; // ;
    w[60] = 584.0; // <
    w[61] = 584.0; // =
    w[62] = 584.0; // >
    w[63] = 556.0; // ?
    w[64] = 1015.0; // @

    // Uppercase A-Z
    let upper = [
        667.0, 667.0, 722.0, 722.0, 667.0, 611.0, 778.0, 722.0, 278.0, 500.0, 667.0, 556.0,
        833.0, 722.0, 778.0, 667.0, 778.0, 722.0, 667.0, 611.0, 722.0, 667.0, 944.0, 667.0,
        667.0, 611.0,
    ];
    for (i, &width) in upper.iter().enumerate() {
        w[65 + i] = width;
    }

    // Lowercase a-z
    let lower = [
        556.0, 556.0, 500.0, 556.0, 556.0, 278.0, 556.0, 556.0, 222.0, 222.0, 500.0, 222.0,
        833.0, 556.0, 556.0, 556.0, 556.0, 333.0, 500.0, 278.0, 556.0, 500.0, 722.0, 500.0,
        500.0, 500.0,
    ];
    for (i, &width) in lower.iter().enumerate() {
        w[97 + i] = width;
    }

    w
}

/// Simplified Times-Roman widths.
fn times_widths(_variant: &[u8]) -> Vec<f64> {
    let mut w = vec![250.0; 256]; // default width

    w[32] = 250.0; // space
    w[33] = 333.0; // !
    w[34] = 408.0; // "
    w[35] = 500.0; // #
    w[36] = 500.0; // $
    w[37] = 833.0; // %
    w[38] = 778.0; // &
    w[39] = 180.0; // '
    w[40] = 333.0; // (
    w[41] = 333.0; // )

    // Digits 0-9: all 500
    for i in 48..=57 {
        w[i] = 500.0;
    }

    // Uppercase A-Z (Times-Roman approximate)
    let upper = [
        722.0, 667.0, 667.0, 722.0, 611.0, 556.0, 722.0, 722.0, 333.0, 389.0, 722.0, 611.0,
        889.0, 722.0, 722.0, 556.0, 722.0, 667.0, 556.0, 611.0, 722.0, 722.0, 944.0, 722.0,
        722.0, 611.0,
    ];
    for (i, &width) in upper.iter().enumerate() {
        w[65 + i] = width;
    }

    // Lowercase a-z (Times-Roman approximate)
    let lower = [
        444.0, 500.0, 444.0, 500.0, 444.0, 333.0, 500.0, 500.0, 278.0, 278.0, 500.0, 278.0,
        778.0, 500.0, 500.0, 500.0, 500.0, 333.0, 389.0, 278.0, 500.0, 500.0, 722.0, 500.0,
        500.0, 444.0,
    ];
    for (i, &width) in lower.iter().enumerate() {
        w[97 + i] = width;
    }

    w
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_standard14() {
        assert!(is_standard14(b"Helvetica"));
        assert!(is_standard14(b"Courier-Bold"));
        assert!(is_standard14(b"Times-Roman"));
        assert!(is_standard14(b"Symbol"));
        assert!(is_standard14(b"ZapfDingbats"));
        assert!(!is_standard14(b"Arial"));
        assert!(!is_standard14(b"CustomFont"));
    }

    #[test]
    fn test_subset_prefix() {
        assert!(is_standard14(b"ABCDEF+Helvetica"));
        assert!(is_standard14(b"XYZABC+Times-Roman"));
        assert!(!is_standard14(b"ABCDEF+CustomFont"));
    }

    #[test]
    fn test_courier_monospaced() {
        let widths = standard14_widths(b"Courier");
        // All widths should be 600 for Courier
        assert!(widths.iter().all(|&w| (w - 600.0).abs() < 0.1));
    }

    #[test]
    fn test_helvetica_widths() {
        let widths = standard14_widths(b"Helvetica");
        assert_eq!(widths[32], 278.0); // space
        assert_eq!(widths[65], 667.0); // A
        assert_eq!(widths[97], 556.0); // a
    }
}
