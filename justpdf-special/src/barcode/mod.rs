//! Barcode generation.
//!
//! QR codes through the `qrcode` crate; Code 128, EAN-13, Code 39,
//! DataMatrix, PDF417 and Aztec through the `rxing` crate.

use rxing::{BarcodeFormat, EncodeHintValue, EncodeHints, MultiFormatWriter, Writer};

use crate::{Result, SpecialError};

/// Supported barcode types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarcodeType {
    QrCode,
    Code128,
    Ean13,
    Code39,
    DataMatrix,
    Pdf417,
    Aztec,
}

/// Generated barcode image.
pub struct BarcodeImage {
    /// Raw RGBA pixel data.
    pub data: Vec<u8>,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

/// Generate a QR code image.
pub fn generate_qr(data: &str, size: u32) -> Result<BarcodeImage> {
    use qrcode::QrCode;

    let code = QrCode::new(data.as_bytes()).map_err(|e| SpecialError::Feature {
        detail: format!("QR generation failed: {e}"),
    })?;

    let img = code
        .render::<image::Luma<u8>>()
        .min_dimensions(size, size)
        .build();

    let width = img.width();
    let height = img.height();

    // Convert grayscale to RGBA
    let rgba: Vec<u8> = img
        .pixels()
        .flat_map(|p| {
            let v = p.0[0];
            [v, v, v, 255]
        })
        .collect();

    Ok(BarcodeImage {
        data: rgba,
        width,
        height,
    })
}

/// Generate a QR code and return as PNG bytes.
pub fn generate_qr_png(data: &str, size: u32) -> Result<Vec<u8>> {
    let barcode = generate_qr(data, size)?;
    encode_png(&barcode)
}

/// Generate a barcode image of any [`BarcodeType`].
///
/// 1D symbols are `height` pixels tall and fit within `width`. QR is at
/// least `width` square ([`generate_qr`]). DataMatrix, PDF417 and Aztec fit
/// within `width` x `height`. A request too small for one pixel per module
/// gets one pixel per module. Every symbol except QR includes its standard
/// minimum quiet zone; QR includes the `qrcode` crate's 4-module one.
pub fn generate_barcode(
    data: &str,
    barcode_type: BarcodeType,
    width: u32,
    height: u32,
) -> Result<BarcodeImage> {
    let (symbol, quiet) = match barcode_type {
        BarcodeType::QrCode => return generate_qr(data, width),
        BarcodeType::Code128 => (
            encode_symbol(data, BarcodeFormat::CODE_128)?,
            QuietZone::sides(10, 10),
        ),
        BarcodeType::Ean13 => (encode_ean13(data)?, QuietZone::sides(11, 7)),
        BarcodeType::Code39 => (encode_code39(data)?, QuietZone::sides(10, 10)),
        BarcodeType::DataMatrix => (encode_datamatrix(data)?, QuietZone::all(1)),
        BarcodeType::Pdf417 => (encode_pdf417(data)?, QuietZone::all(2)),
        BarcodeType::Aztec => (encode_aztec(data)?, QuietZone::all(0)),
    };

    let (total_w, total_h) = quiet.total(&symbol);
    if symbol.height == 1 {
        let module = (width as usize / total_w).max(1);
        Ok(render(&symbol, quiet, module, height.max(1) as usize))
    } else {
        let module = (width as usize / total_w)
            .min(height as usize / total_h)
            .max(1);
        Ok(render(&symbol, quiet, module, module))
    }
}

/// Generate a barcode image of any [`BarcodeType`] as PNG bytes.
pub fn generate_barcode_png(
    data: &str,
    barcode_type: BarcodeType,
    width: u32,
    height: u32,
) -> Result<Vec<u8>> {
    let barcode = generate_barcode(data, barcode_type, width, height)?;
    encode_png(&barcode)
}

// ---------------------------------------------------------------------------
// PNG encoding helper
// ---------------------------------------------------------------------------

fn encode_png(img: &BarcodeImage) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut buf);
    image::ImageEncoder::write_image(
        encoder,
        &img.data,
        img.width,
        img.height,
        image::ExtendedColorType::Rgba8,
    )
    .map_err(|e| SpecialError::Feature {
        detail: format!("PNG encode: {e}"),
    })?;
    Ok(buf)
}

// ---------------------------------------------------------------------------
// Symbol encoding (rxing)
// ---------------------------------------------------------------------------

/// A barcode symbol as a module grid without quiet zone; `true` is dark.
/// A 1D symbol is a single row.
struct Symbol {
    width: usize,
    height: usize,
    modules: Vec<bool>,
}

impl Symbol {
    fn get(&self, x: usize, y: usize) -> bool {
        self.modules[y * self.width + x]
    }
}

/// Quiet zone around a [`Symbol`], in modules. 1D symbols use only the sides.
#[derive(Clone, Copy)]
struct QuietZone {
    left: usize,
    right: usize,
    top_bottom: usize,
}

impl QuietZone {
    fn sides(left: usize, right: usize) -> Self {
        Self {
            left,
            right,
            top_bottom: 0,
        }
    }

    fn all(n: usize) -> Self {
        Self {
            left: n,
            right: n,
            top_bottom: n,
        }
    }

    /// Symbol size including the quiet zone, in modules.
    fn total(&self, symbol: &Symbol) -> (usize, usize) {
        (
            symbol.width + self.left + self.right,
            symbol.height + 2 * self.top_bottom,
        )
    }
}

/// Encode `data` with rxing at one pixel per module and no margin.
/// A panic inside rxing is returned as an error.
fn encode_symbol(data: &str, format: BarcodeFormat) -> Result<Symbol> {
    let hints = EncodeHints::default().with(EncodeHintValue::Margin("0".into()));
    let encoded = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        MultiFormatWriter.encode_with_hints(data, &format, 0, 0, &hints)
    }));
    let matrix = match encoded {
        Ok(Ok(matrix)) => matrix,
        Ok(Err(e)) => {
            return Err(SpecialError::Feature {
                detail: format!("{format:?}: {e}"),
            });
        }
        Err(_) => {
            return Err(SpecialError::Feature {
                detail: format!("{format:?}: data cannot be encoded"),
            });
        }
    };
    let (w, h) = (matrix.getWidth(), matrix.getHeight());
    let modules = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .map(|(x, y)| matrix.get(x, y))
        .collect();
    Ok(Symbol {
        width: w as usize,
        height: h as usize,
        modules,
    })
}

fn require_data(data: &str, name: &str) -> Result<()> {
    if data.is_empty() {
        return Err(SpecialError::Feature {
            detail: format!("{name}: empty data"),
        });
    }
    Ok(())
}

/// EAN-13 takes exactly 13 digits; rxing rejects a wrong check digit.
fn encode_ean13(data: &str) -> Result<Symbol> {
    if data.len() != 13 || !data.chars().all(|c| c.is_ascii_digit()) {
        return Err(SpecialError::Feature {
            detail: format!("EAN-13 requires exactly 13 digits, got: {data}"),
        });
    }
    encode_symbol(data, BarcodeFormat::EAN_13)
}

/// Code 39 encodes the upper-cased input.
fn encode_code39(data: &str) -> Result<Symbol> {
    encode_symbol(&data.to_uppercase(), BarcodeFormat::CODE_39)
}

fn encode_datamatrix(data: &str) -> Result<Symbol> {
    require_data(data, "DataMatrix")?;
    encode_symbol(data, BarcodeFormat::DATA_MATRIX)
}

/// PDF417 with rows 4 modules tall, wider than tall and unrotated.
fn encode_pdf417(data: &str) -> Result<Symbol> {
    require_data(data, "PDF417")?;
    encode_symbol(data, BarcodeFormat::PDF_417)
}

fn encode_aztec(data: &str) -> Result<Symbol> {
    require_data(data, "Aztec")?;
    encode_symbol(data, BarcodeFormat::AZTEC)
}

/// Render `symbol` with its quiet zone at `module_w` x `module_h` pixels per module.
fn render(symbol: &Symbol, quiet: QuietZone, module_w: usize, module_h: usize) -> BarcodeImage {
    let (total_w, total_h) = quiet.total(symbol);
    let (img_w, img_h) = (total_w * module_w, total_h * module_h);
    let mut rgba = vec![255u8; img_w * img_h * 4];

    for y in 0..symbol.height {
        for x in 0..symbol.width {
            if !symbol.get(x, y) {
                continue;
            }
            let px = (x + quiet.left) * module_w;
            let py = (y + quiet.top_bottom) * module_h;
            for row in py..py + module_h {
                for col in px..px + module_w {
                    let offset = (row * img_w + col) * 4;
                    rgba[offset..offset + 3].fill(0);
                }
            }
        }
    }

    BarcodeImage {
        data: rgba,
        width: img_w as u32,
        height: img_h as u32,
    }
}

// ---------------------------------------------------------------------------
// 2D entry points
// ---------------------------------------------------------------------------

/// Generate a DataMatrix (ECC 200) barcode image at `module_size` pixels per module.
pub fn generate_datamatrix(data: &str, module_size: u32) -> Result<BarcodeImage> {
    let module = module_size.max(1) as usize;
    Ok(render(
        &encode_datamatrix(data)?,
        QuietZone::all(1),
        module,
        module,
    ))
}

/// Generate a PDF417 barcode image that fits within `width` x `height`.
pub fn generate_pdf417(data: &str, width: u32, height: u32) -> Result<BarcodeImage> {
    generate_barcode(data, BarcodeType::Pdf417, width, height)
}

/// Generate an Aztec barcode image that fits within a `size` x `size` square.
pub fn generate_aztec(data: &str, size: u32) -> Result<BarcodeImage> {
    generate_barcode(data, BarcodeType::Aztec, size, size)
}

/// Generate a DataMatrix barcode as PNG bytes.
pub fn generate_datamatrix_png(data: &str, module_size: u32) -> Result<Vec<u8>> {
    let barcode = generate_datamatrix(data, module_size)?;
    encode_png(&barcode)
}

/// Generate a PDF417 barcode as PNG bytes.
pub fn generate_pdf417_png(data: &str, width: u32, height: u32) -> Result<Vec<u8>> {
    let barcode = generate_pdf417(data, width, height)?;
    encode_png(&barcode)
}

/// Generate an Aztec barcode as PNG bytes.
pub fn generate_aztec_png(data: &str, size: u32) -> Result<Vec<u8>> {
    let barcode = generate_aztec(data, size)?;
    encode_png(&barcode)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rxing::BarcodeFormat;

    /// Decode `img` as produced (no added border) and return the text.
    fn decode(img: &BarcodeImage, format: BarcodeFormat) -> String {
        let luma: Vec<u8> = img.data.chunks_exact(4).map(|p| p[0]).collect();
        rxing::helpers::detect_in_luma(luma, img.width, img.height, Some(format))
            .unwrap_or_else(|e| panic!("{format:?} did not decode: {e}"))
            .getText()
            .to_string()
    }

    #[test]
    fn roundtrip_qr() {
        let img = generate_barcode("HELLO123", BarcodeType::QrCode, 200, 200).unwrap();
        assert_eq!(decode(&img, BarcodeFormat::QR_CODE), "HELLO123");
    }

    #[test]
    fn roundtrip_code128() {
        for data in ["Hello world 123!", "01", "a"] {
            let img = generate_barcode(data, BarcodeType::Code128, 600, 80).unwrap();
            assert_eq!(decode(&img, BarcodeFormat::CODE_128), data);
        }
    }

    #[test]
    fn roundtrip_ean13() {
        for data in ["5901234123457", "0012345678905", "4006381333931"] {
            let img = generate_barcode(data, BarcodeType::Ean13, 400, 80).unwrap();
            assert_eq!(decode(&img, BarcodeFormat::EAN_13), data);
        }
    }

    #[test]
    fn roundtrip_code39() {
        let img = generate_barcode("HELLO-123", BarcodeType::Code39, 600, 80).unwrap();
        assert_eq!(decode(&img, BarcodeFormat::CODE_39), "HELLO-123");
    }

    #[test]
    fn roundtrip_datamatrix() {
        for data in ["HELLO123", "Hello, DataMatrix! 0123456789"] {
            let img = generate_datamatrix(data, 4).unwrap();
            assert_eq!(decode(&img, BarcodeFormat::DATA_MATRIX), data);
        }
    }

    #[test]
    fn roundtrip_pdf417() {
        for data in ["HELLO123", "Hello, PDF417! 0123456789"] {
            let img = generate_pdf417(data, 600, 300).unwrap();
            assert_eq!(decode(&img, BarcodeFormat::PDF_417), data);
        }
    }

    #[test]
    fn roundtrip_aztec() {
        for data in ["HELLO123", "Hello, Aztec! 0123456789"] {
            let img = generate_aztec(data, 200).unwrap();
            assert_eq!(decode(&img, BarcodeFormat::AZTEC), data);
        }
    }

    #[test]
    fn roundtrip_2d_via_generate_barcode() {
        for (btype, format) in [
            (BarcodeType::DataMatrix, BarcodeFormat::DATA_MATRIX),
            (BarcodeType::Pdf417, BarcodeFormat::PDF_417),
            (BarcodeType::Aztec, BarcodeFormat::AZTEC),
        ] {
            let img = generate_barcode("HELLO123", btype, 300, 300).unwrap();
            assert_eq!(decode(&img, format), "HELLO123", "{btype:?}");
        }
    }

    #[test]
    fn roundtrip_code39_upper_cases() {
        let img = generate_barcode("hello-123", BarcodeType::Code39, 600, 80).unwrap();
        assert_eq!(decode(&img, BarcodeFormat::CODE_39), "HELLO-123");
    }

    /// Whether the column of pixels at `x` is entirely light.
    fn column_is_light(img: &BarcodeImage, x: u32) -> bool {
        (0..img.height).all(|y| img.data[((y * img.width + x) * 4) as usize] == 255)
    }

    /// Whether the row of pixels at `y` is entirely light.
    fn row_is_light(img: &BarcodeImage, y: u32) -> bool {
        (0..img.width).all(|x| img.data[((y * img.width + x) * 4) as usize] == 255)
    }

    #[test]
    fn quiet_zones_have_standard_width() {
        // (type, data, left, right, top/bottom) in modules, rendered at 1 px per module.
        for (btype, data, left, right, vertical) in [
            (BarcodeType::Code128, "HELLO123", 10, 10, 0),
            (BarcodeType::Ean13, "5901234123457", 11, 7, 0),
            (BarcodeType::Code39, "HELLO123", 10, 10, 0),
            (BarcodeType::DataMatrix, "HELLO123", 1, 1, 1),
            (BarcodeType::Pdf417, "HELLO123", 2, 2, 2),
            (BarcodeType::Aztec, "HELLO123", 0, 0, 0),
        ] {
            let img = generate_barcode(data, btype, 1, 1).unwrap();
            let (w, h) = (img.width, img.height);
            assert!(
                (0..left).all(|x| column_is_light(&img, x)),
                "{btype:?} left"
            );
            assert!(!column_is_light(&img, left), "{btype:?} left edge");
            assert!(
                (w - right..w).all(|x| column_is_light(&img, x)),
                "{btype:?} right"
            );
            assert!(
                !column_is_light(&img, w - right - 1),
                "{btype:?} right edge"
            );
            if vertical > 0 {
                assert!(
                    (0..vertical).all(|y| row_is_light(&img, y)),
                    "{btype:?} top"
                );
                assert!(!row_is_light(&img, vertical), "{btype:?} top edge");
                assert!(
                    (h - vertical..h).all(|y| row_is_light(&img, y)),
                    "{btype:?} bottom"
                );
                assert!(
                    !row_is_light(&img, h - vertical - 1),
                    "{btype:?} bottom edge"
                );
            }
        }
    }

    #[test]
    fn pdf417_is_upright() {
        // Every symbol row starts with the start pattern's 8-module bar.
        for n in [1, 3, 12, 40, 160, 400] {
            let data: String = "A1b2".chars().cycle().take(n).collect();
            let img = generate_barcode(&data, BarcodeType::Pdf417, 1, 1).unwrap();
            let dark = |x: u32, y: u32| img.data[((y * img.width + x) * 4) as usize] == 0;
            assert!(
                img.width > img.height,
                "n={n}: {}x{}",
                img.width,
                img.height
            );
            assert!(
                (2..img.height - 2).all(|y| (2..10).all(|x| dark(x, y))),
                "n={n}: rows do not start with the start bar"
            );
        }
    }

    #[test]
    fn datamatrix_over_capacity_is_an_error() {
        // rxing 0.9.3 panics past the 144x144 symbol's capacity.
        let data = "x".repeat(2336);
        assert!(generate_datamatrix(&data, 1).is_err());
        assert!(generate_datamatrix(&"x".repeat(2335), 1).is_ok());
    }

    #[test]
    fn ean13_wrong_check_digit_is_rejected() {
        assert!(generate_barcode("5901234123458", BarcodeType::Ean13, 400, 80).is_err());
    }

    #[test]
    fn ean13_twelve_digits_is_rejected() {
        assert!(generate_barcode("590123412345", BarcodeType::Ean13, 400, 80).is_err());
    }

    #[test]
    fn generate_barcode_fits_requested_size() {
        for btype in [
            BarcodeType::DataMatrix,
            BarcodeType::Pdf417,
            BarcodeType::Aztec,
        ] {
            let img = generate_barcode("HELLO123", btype, 200, 200).unwrap();
            assert!(
                img.width <= 200 && img.height <= 200,
                "{btype:?}: {}x{}",
                img.width,
                img.height
            );
            assert!(
                img.width >= 100 || img.height >= 100,
                "{btype:?} far below the request"
            );
        }
        // The qrcode crate renders at least the requested size; one module of overshoot.
        let qr = generate_barcode("HELLO123", BarcodeType::QrCode, 200, 200).unwrap();
        assert!(
            qr.width <= 200 + 10 && qr.height <= 200 + 10,
            "QR: {}x{}",
            qr.width,
            qr.height
        );
    }

    #[test]
    fn test_generate_qr() {
        let result = generate_qr("Hello, World!", 256);
        assert!(result.is_ok());
        let img = result.unwrap();
        assert!(img.width > 0);
        assert!(img.height > 0);
        assert_eq!(img.data.len(), (img.width * img.height * 4) as usize);
    }

    #[test]
    fn test_generate_qr_png() {
        let png = generate_qr_png("https://example.com", 256).unwrap();
        assert_eq!(&png[..4], &[0x89, 0x50, 0x4E, 0x47]); // PNG magic
    }

    #[test]
    fn test_generate_ean13() {
        let result = generate_barcode("4006381333931", BarcodeType::Ean13, 200, 100);
        assert!(result.is_ok());
        let img = result.unwrap();
        assert!(img.width > 0);
    }

    #[test]
    fn test_ean13_invalid_length() {
        let result = generate_barcode("123", BarcodeType::Ean13, 200, 100);
        assert!(result.is_err());
    }

    #[test]
    fn test_generate_code39() {
        let result = generate_barcode("HELLO", BarcodeType::Code39, 300, 80);
        assert!(result.is_ok());
    }

    #[test]
    fn test_generate_code128() {
        let result = generate_barcode("Hello123", BarcodeType::Code128, 300, 80);
        assert!(result.is_ok());
    }

    #[test]
    fn test_barcode_png_output() {
        let png = generate_barcode_png("TEST", BarcodeType::Code39, 200, 60).unwrap();
        assert_eq!(&png[..4], &[0x89, 0x50, 0x4E, 0x47]);
    }

    #[test]
    fn test_qr_roundtrip_data() {
        // Generate QR, verify it's a valid image
        let img = generate_qr("test data 12345", 128).unwrap();
        assert!(img.width >= 128);
        assert!(img.height >= 128);
    }

    // --- DataMatrix tests ---

    #[test]
    fn test_generate_datamatrix() {
        let result = generate_datamatrix("Hello", 4);
        assert!(result.is_ok());
        let img = result.unwrap();
        assert!(img.width > 0);
        assert!(img.height > 0);
        assert_eq!(img.data.len(), (img.width * img.height * 4) as usize);
    }

    #[test]
    fn test_datamatrix_empty_data() {
        let result = generate_datamatrix("", 4);
        assert!(result.is_err());
    }

    #[test]
    fn test_datamatrix_png() {
        let png = generate_datamatrix_png("Test123", 4).unwrap();
        assert_eq!(&png[..4], &[0x89, 0x50, 0x4E, 0x47]);
    }

    // --- PDF417 tests ---

    #[test]
    fn test_generate_pdf417() {
        let result = generate_pdf417("Hello PDF417", 300, 100);
        assert!(result.is_ok());
        let img = result.unwrap();
        assert!(img.width > 0);
        assert!(img.height > 0);
        assert_eq!(img.data.len(), (img.width * img.height * 4) as usize);
    }

    #[test]
    fn test_pdf417_empty_data() {
        let result = generate_pdf417("", 300, 100);
        assert!(result.is_err());
    }

    #[test]
    fn test_pdf417_png() {
        let png = generate_pdf417_png("Test", 200, 80).unwrap();
        assert_eq!(&png[..4], &[0x89, 0x50, 0x4E, 0x47]);
    }

    #[test]
    fn test_pdf417_via_generate_barcode() {
        let result = generate_barcode("Test", BarcodeType::Pdf417, 300, 100);
        assert!(result.is_ok());
    }

    // --- Aztec tests ---

    #[test]
    fn test_generate_aztec() {
        let result = generate_aztec("Hello Aztec", 200);
        assert!(result.is_ok());
        let img = result.unwrap();
        assert!(img.width > 0);
        assert!(img.height > 0);
        assert_eq!(img.data.len(), (img.width * img.height * 4) as usize);
    }

    #[test]
    fn test_aztec_empty_data() {
        let result = generate_aztec("", 100);
        assert!(result.is_err());
    }

    #[test]
    fn test_aztec_png() {
        let png = generate_aztec_png("Test", 100).unwrap();
        assert_eq!(&png[..4], &[0x89, 0x50, 0x4E, 0x47]);
    }

    #[test]
    fn test_aztec_bulls_eye() {
        // Verify center pixel is dark (bull's eye center)
        let img = generate_aztec("A", 1).unwrap();
        let center_x = img.width / 2;
        let center_y = img.height / 2;
        let offset = ((center_y * img.width + center_x) * 4) as usize;
        assert_eq!(img.data[offset], 0); // R=0 (black)
    }

    // --- Cross-type tests ---

    #[test]
    fn test_all_2d_barcode_types_via_generate_barcode() {
        for btype in [
            BarcodeType::DataMatrix,
            BarcodeType::Pdf417,
            BarcodeType::Aztec,
        ] {
            let result = generate_barcode("Test", btype, 200, 200);
            assert!(result.is_ok(), "failed for {btype:?}");
        }
    }
}
