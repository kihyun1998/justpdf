//! Fonts with no embedded program are drawn from the bundled URW base-14
//! substitutes (`base14-fonts`), and as placeholder boxes without them.

mod common;

use common::{Plain, page};
use justpdf_core::PdfDocument;
use justpdf_render::{RenderOptions, render_page_to_pixmap};

/// A page showing the hex string `shown` at 100 pt from (5, 10) in the font
/// dictionary `font` (object 5); `rest` are objects 6 on.
fn show(font: &str, shown: &str, rest: Vec<common::Obj>) -> PdfDocument {
    let content = format!("BT /F1 100 Tf 5 10 Td <{shown}> Tj ET");
    let mut objects = vec![Plain(font)];
    objects.extend(rest);
    page("<< /Font << /F1 5 0 R >> >>", &content, objects)
}

/// A non-embedded `/Subtype /Type1` font named `base_font`, with `entries`
/// added to its dictionary.
fn non_embedded(base_font: &str, entries: &str, shown: &str) -> PdfDocument {
    let font = format!("<< /Type /Font /Subtype /Type1 /BaseFont /{base_font} {entries} >>");
    show(&font, shown, vec![])
}

fn pixels(doc: &PdfDocument) -> Vec<u8> {
    render_page_to_pixmap(doc, 0, &RenderOptions::default())
        .unwrap()
        .data
}

/// Whether device pixel (x, y) of a 100x100 page at 72 dpi is dark.
fn dark(px: &[u8], x: usize, y: usize) -> bool {
    px[(y * 100 + x) * 4] < 128
}

#[cfg(feature = "base14-fonts")]
mod with_substitutes {
    use super::*;
    use common::Stream;

    const NIMBUS_SANS: &[u8] = include_bytes!("../fonts/urw/NimbusSans-Regular.cff");
    const NIMBUS_SANS_BOLD: &[u8] = include_bytes!("../fonts/urw/NimbusSans-Bold.cff");
    const NIMBUS_ROMAN_BOLD: &[u8] = include_bytes!("../fonts/urw/NimbusRoman-Bold.cff");
    const NIMBUS_MONO_ITALIC: &[u8] = include_bytes!("../fonts/urw/NimbusMonoPS-Italic.cff");
    const SYMBOL: &[u8] = include_bytes!("../fonts/urw/StandardSymbolsPS.cff");
    const DINGBATS: &[u8] = include_bytes!("../fonts/urw/Dingbats.cff");

    fn hex(data: &[u8]) -> String {
        let mut s: String = data.iter().map(|b| format!("{b:02X}")).collect();
        s.push('>');
        s
    }

    /// `/FirstChar 0 /Widths` of 722 for every code.
    fn widths_722() -> String {
        format!(
            "/FirstChar 0 /LastChar 255 /Widths [{}]",
            vec!["722"; 256].join(" ")
        )
    }

    /// The same text in a font that embeds `cff` as `/FontFile3 /Type1C`.
    fn embedded(cff: &[u8], entries: &str, shown: &str) -> PdfDocument {
        let font = format!(
            "<< /Type /Font /Subtype /Type1 /BaseFont /Embedded /FontDescriptor 6 0 R {entries} >>"
        );
        let descriptor = "<< /Type /FontDescriptor /FontName /Embedded /Flags 32 \
                          /FontBBox [0 0 1000 1000] /ItalicAngle 0 /Ascent 1000 /Descent 0 \
                          /CapHeight 729 /StemV 80 /FontFile3 7 0 R >>";
        let program = hex(cff);
        show(
            &font,
            shown,
            vec![
                Plain(descriptor),
                Stream("/Subtype /Type1C /Filter /ASCIIHexDecode", &program),
            ],
        )
    }

    #[test]
    fn helvetica_draws_the_glyph_shape() {
        // `H` at 100 pt from (5, 10): stems x 83–176 and 551–644, crossbar
        // y 332–414 (glyph units), so the counters stay empty.
        let px = pixels(&non_embedded("Helvetica", "", "48"));
        assert!(dark(&px, 18, 70), "left stem");
        assert!(dark(&px, 41, 52), "crossbar");
        assert!(!dark(&px, 41, 73), "lower counter");
        assert!(!dark(&px, 41, 33), "upper counter");
    }

    #[test]
    fn each_family_draws_from_its_substitute() {
        let blank = pixels(&non_embedded("Helvetica", "", ""));
        // Symbol 0x61 is `alpha` and ZapfDingbats 0x21 is `a1` in their
        // built-in encodings.
        for (base_font, cff, code) in [
            ("Times-Bold", NIMBUS_ROMAN_BOLD, "48"),
            ("Courier-Oblique", NIMBUS_MONO_ITALIC, "48"),
            ("Symbol", SYMBOL, "61"),
            ("ZapfDingbats", DINGBATS, "21"),
        ] {
            let substitute = pixels(&non_embedded(base_font, &widths_722(), code));
            assert_ne!(substitute, blank, "{base_font} draws something");
            assert_eq!(
                substitute,
                pixels(&embedded(cff, &widths_722(), code)),
                "{base_font}"
            );
        }
    }

    #[test]
    fn a_non_standard_14_font_takes_the_substitute_its_name_picks() {
        let font = format!(
            "<< /Type /Font /Subtype /TrueType /BaseFont /Arial-BoldMT {} \
               /Encoding /WinAnsiEncoding /FontDescriptor 6 0 R >>",
            widths_722()
        );
        let descriptor = "<< /Type /FontDescriptor /FontName /Arial-BoldMT /Flags 32 \
                          /FontBBox [0 0 1000 1000] /ItalicAngle 0 /Ascent 1000 /Descent 0 \
                          /CapHeight 729 /StemV 80 >>";
        let arial = pixels(&show(&font, "48", vec![Plain(descriptor)]));
        let entries = format!("{} /Encoding /WinAnsiEncoding", widths_722());
        assert_eq!(arial, pixels(&embedded(NIMBUS_SANS_BOLD, &entries, "48")));
        assert_ne!(arial, pixels(&embedded(NIMBUS_SANS, &entries, "48")));
    }

    #[test]
    fn differences_choose_the_named_glyph() {
        let entries = format!("{} /Encoding << /Differences [65 /Eacute] >>", widths_722());
        let differences = pixels(&non_embedded("Helvetica", &entries, "41"));
        let winansi = format!("{} /Encoding /WinAnsiEncoding", widths_722());
        let eacute = pixels(&non_embedded("Helvetica", &winansi, "C9"));
        assert_ne!(eacute, pixels(&non_embedded("Helvetica", &winansi, "41")));
        assert_eq!(differences, eacute);
    }

    #[test]
    fn glyphs_advance_by_widths() {
        // `H` is 722 units in the font; /Widths makes it 300, so the second H
        // overlaps the first.
        let entries = "/FirstChar 72 /LastChar 72 /Widths [300]";
        let substitute = pixels(&non_embedded("Helvetica", entries, "4848"));
        assert_eq!(substitute, pixels(&embedded(NIMBUS_SANS, entries, "4848")));
        assert_ne!(
            substitute,
            pixels(&non_embedded("Helvetica", &widths_722(), "4848"))
        );
    }

    #[test]
    fn an_embedded_program_that_cannot_be_read_takes_the_substitute() {
        // `/Embedded` is no known family, so `find_substitute` picks Helvetica.
        let unreadable = pixels(&embedded(b"not a font program", &widths_722(), "48"));
        assert_eq!(
            unreadable,
            pixels(&non_embedded("Helvetica", &widths_722(), "48"))
        );
    }
}

#[cfg(not(feature = "base14-fonts"))]
#[test]
fn without_the_substitutes_glyphs_are_boxes() {
    // The box fills where `H`'s upper counter would be.
    let px = pixels(&non_embedded("Helvetica", "", "48"));
    assert!(dark(&px, 41, 33));
}

#[cfg(feature = "base14-fonts")]
mod not_substituted {
    use super::*;

    /// `H` from the Helvetica substitute, at the same place and width.
    fn helvetica_h() -> Vec<u8> {
        pixels(&non_embedded(
            "Helvetica",
            "/FirstChar 72 /LastChar 72 /Widths [722]",
            "48",
        ))
    }

    #[test]
    fn a_type0_font_without_a_program() {
        // CID 41 is GID 41, `H` in NimbusSans, were the substitute used.
        let type0 = "<< /Type /Font /Subtype /Type0 /BaseFont /Helvetica \
                     /Encoding /Identity-H /DescendantFonts [6 0 R] >>";
        let cid_font = "<< /Type /Font /Subtype /CIDFontType0 /BaseFont /Helvetica \
                        /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> \
                        /DW 722 >>";
        let px = pixels(&show(type0, "0029", vec![Plain(cid_font)]));
        assert_ne!(px, helvetica_h());
    }

    #[test]
    fn a_type3_font() {
        let type3 = "<< /Type /Font /Subtype /Type3 /BaseFont /Helvetica \
                     /FontBBox [0 0 1000 1000] /FontMatrix [0.001 0 0 0.001 0 0] \
                     /CharProcs << >> /Encoding << /Differences [72 /H] >> \
                     /FirstChar 72 /LastChar 72 /Widths [722] >>";
        assert_ne!(pixels(&show(type3, "48", vec![])), helvetica_h());
    }
}
