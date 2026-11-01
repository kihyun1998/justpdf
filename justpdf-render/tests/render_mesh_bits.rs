//! A mesh shading (types 4–7) whose `/BitsPerCoordinate`, `/BitsPerComponent`
//! or `/BitsPerFlag` is not one ISO 32000 allows reads it as 8, as MuPDF
//! does (#433).

mod common;

use std::sync::mpsc;
use std::time::Duration;

use common::{RED, Stream, page, rgb};

fn hex(data: &[u8]) -> String {
    let mut s: String = data.iter().map(|b| format!("{b:02X}")).collect();
    s.push('>');
    s
}

/// A red free-form mesh covering the page with 8-bit fields: two
/// triangles, each vertex flag, x, y, r, g, b.
fn free_form() -> String {
    hex(&[
        0, 0, 0, 255, 0, 0, //
        0, 255, 0, 255, 0, 0, //
        0, 0, 255, 255, 0, 0, //
        0, 255, 0, 255, 0, 0, //
        0, 255, 255, 255, 0, 0, //
        0, 0, 255, 255, 0, 0,
    ])
}

/// A red 2 × 2 lattice over the page with 8-bit fields: x, y, r, g, b.
fn lattice() -> String {
    hex(&[
        0, 0, 255, 0, 0, //
        255, 0, 255, 0, 0, //
        0, 255, 255, 0, 0, //
        255, 255, 255, 0, 0,
    ])
}

/// A red Coons patch over the page with 8-bit fields: flag, 12 boundary
/// points, then an RGB colour per corner.
fn coons() -> String {
    let pts: [(u8, u8); 12] = [
        (0, 0),
        (0, 85),
        (0, 170),
        (0, 255),
        (85, 255),
        (170, 255),
        (255, 255),
        (255, 170),
        (255, 85),
        (255, 0),
        (170, 0),
        (85, 0),
    ];
    let mut data = vec![0u8];
    for (x, y) in pts {
        data.extend([x, y]);
    }
    for _ in 0..4 {
        data.extend([255, 0, 0]);
    }
    hex(&data)
}

/// The colour at (50, 50) of a page painting the shading `dict`/`data`, or
/// `None` if rendering takes more than five seconds.
fn centre(dict: String, data: String) -> Option<[u8; 3]> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let doc = page(
            "<< /Shading << /Sh0 5 0 R >> >>",
            "/Sh0 sh",
            vec![Stream(&dict, &data)],
        );
        let _ = tx.send(rgb(&doc, 50, 50));
    });
    rx.recv_timeout(Duration::from_secs(5)).ok()
}

/// Within 3 per channel: the mesh rasterizer truncates (255 → 254).
fn near(a: [u8; 3], b: [u8; 3]) -> bool {
    a.iter().zip(b).all(|(&x, y)| x.abs_diff(y) <= 3)
}

fn assert_red(dict: String, data: String) {
    let px = centre(dict.clone(), data);
    assert!(px.is_some_and(|px| near(px, RED)), "{dict}: {px:?}");
}

#[test]
fn widths_outside_the_allowed_sets_read_as_8() {
    for bits in ["0", "3", "-8", "64", "100"] {
        for (coord, comp, flag) in [(bits, "8", "8"), ("8", bits, "8"), ("8", "8", bits)] {
            let widths =
                format!("/BitsPerCoordinate {coord} /BitsPerComponent {comp} /BitsPerFlag {flag}");
            assert_red(
                format!(
                    "/ShadingType 4 /ColorSpace /DeviceRGB {widths} \
                     /Decode [0 100 0 100 0 1 0 1 0 1] /Filter /ASCIIHexDecode"
                ),
                free_form(),
            );
            assert_red(
                format!(
                    "/ShadingType 6 /ColorSpace /DeviceRGB {widths} \
                     /Decode [0 100 0 100 0 1 0 1 0 1] /Filter /ASCIIHexDecode"
                ),
                coons(),
            );
        }
    }
}

#[test]
fn a_mesh_with_every_width_zero_finishes() {
    let zero = "/BitsPerCoordinate 0 /BitsPerComponent 0 /BitsPerFlag 0";
    assert_red(
        format!(
            "/ShadingType 4 /ColorSpace /DeviceRGB {zero} \
             /Decode [0 100 0 100 0 1 0 1 0 1] /Filter /ASCIIHexDecode"
        ),
        free_form(),
    );
    assert_red(
        format!(
            "/ShadingType 5 /ColorSpace /DeviceRGB {zero} /VerticesPerRow 2 \
             /Decode [0 100 0 100 0 1 0 1 0 1] /Filter /ASCIIHexDecode"
        ),
        lattice(),
    );
}

#[test]
fn an_allowed_width_other_than_8_is_kept() {
    // 16-bit coordinates: flag, x, y as two bytes each, then r, g, b.
    let mut data = Vec::new();
    for (x, y) in [(0, 0), (255, 0), (0, 255), (255, 0), (255, 255), (0, 255)] {
        data.extend([0, x, x, y, y, 255, 0, 0]);
    }
    assert_red(
        "/ShadingType 4 /ColorSpace /DeviceRGB /BitsPerCoordinate 16 /BitsPerComponent 8 \
         /BitsPerFlag 8 /Decode [0 100 0 100 0 1 0 1 0 1] /Filter /ASCIIHexDecode"
            .to_string(),
        hex(&data),
    );
}
