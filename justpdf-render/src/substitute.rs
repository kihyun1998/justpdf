//! The bundled URW base-14 font programs a font with no embedded program is
//! drawn with (`base14-fonts`).

use std::sync::Arc;

use justpdf_core::font::FontProgramData;

/// The bare CFF program standing in for the Standard 14 font `name`, as
/// `justpdf_core::font::recovery::find_substitute` names it, built once per
/// process.
#[cfg(feature = "base14-fonts")]
pub fn base14_program(name: &[u8]) -> Option<Arc<FontProgramData>> {
    macro_rules! urw {
        ($file:literal) => {{
            static PROGRAM: std::sync::OnceLock<Arc<FontProgramData>> = std::sync::OnceLock::new();
            PROGRAM
                .get_or_init(|| {
                    Arc::new(FontProgramData::new(
                        &include_bytes!(concat!("../fonts/urw/", $file))[..],
                    ))
                })
                .clone()
        }};
    }
    Some(match name {
        b"Helvetica" => urw!("NimbusSans-Regular.cff"),
        b"Helvetica-Bold" => urw!("NimbusSans-Bold.cff"),
        b"Helvetica-Oblique" => urw!("NimbusSans-Italic.cff"),
        b"Helvetica-BoldOblique" => urw!("NimbusSans-BoldItalic.cff"),
        b"Times-Roman" => urw!("NimbusRoman-Regular.cff"),
        b"Times-Bold" => urw!("NimbusRoman-Bold.cff"),
        b"Times-Italic" => urw!("NimbusRoman-Italic.cff"),
        b"Times-BoldItalic" => urw!("NimbusRoman-BoldItalic.cff"),
        b"Courier" => urw!("NimbusMonoPS-Regular.cff"),
        b"Courier-Bold" => urw!("NimbusMonoPS-Bold.cff"),
        b"Courier-Oblique" => urw!("NimbusMonoPS-Italic.cff"),
        b"Courier-BoldOblique" => urw!("NimbusMonoPS-BoldItalic.cff"),
        b"Symbol" => urw!("StandardSymbolsPS.cff"),
        b"ZapfDingbats" => urw!("Dingbats.cff"),
        _ => return None,
    })
}

/// No substitutes without `base14-fonts`.
#[cfg(not(feature = "base14-fonts"))]
pub fn base14_program(_name: &[u8]) -> Option<Arc<FontProgramData>> {
    None
}
