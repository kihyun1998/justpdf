//! A tiling pattern's cell lands in pattern space at every resolution: it
//! goes through the CTM and page transform once, and its `/Matrix` moves and
//! scales the tiling (#396).

mod common;

use common::{Stream, page};
use justpdf_render::{RenderOptions, render_page_to_pixmap};

const RED: [u8; 3] = [255, 0, 0];
const WHITE: [u8; 3] = [255, 255, 255];

/// A page filled with tiling pattern `/T` (object 5): a 20 × 20 cell whose
/// only mark is a red 10 × 5 strip at its origin, so a flip or a shift moves
/// it. `matrix` is the pattern's `/Matrix` entry (possibly empty).
fn tiled(matrix: &str, content: &str) -> justpdf_core::PdfDocument {
    let pattern = format!(
        "/PatternType 1 /PaintType 1 /TilingType 1 /BBox [0 0 20 20] \
         /XStep 20 /YStep 20 /Resources << >> {matrix}"
    );
    page(
        "<< /Pattern << /T 5 0 R >> >>",
        content,
        vec![Stream(&pattern, "1 0 0 rg 0 0 10 5 re f")],
    )
}

const FILL: &str = "/Pattern cs /T scn 0 0 100 100 re f";

/// RGB at user point (x, y) of a 100 × 100 pt page rendered at `dpi`.
fn at(doc: &justpdf_core::PdfDocument, dpi: f64, x: f64, y: f64) -> [u8; 3] {
    let opts = RenderOptions {
        dpi,
        ..Default::default()
    };
    let px = render_page_to_pixmap(doc, 0, &opts).unwrap();
    let s = dpi / 72.0;
    let (dx, dy) = ((x * s) as u32, ((100.0 - y) * s) as u32);
    let i = ((dy * px.width + dx) * 4) as usize;
    [px.data[i], px.data[i + 1], px.data[i + 2]]
}

/// Red exactly where `red` says, at each of `points`.
fn marks(doc: &justpdf_core::PdfDocument, dpi: f64, points: &[(f64, f64, bool)]) {
    for &(x, y, red) in points {
        let want = if red { RED } else { WHITE };
        assert_eq!(at(doc, dpi, x, y), want, "({x}, {y}) at {dpi} dpi");
    }
}

/// The strip repeats every 20 units from the origin.
const GRID: [(f64, f64, bool); 6] = [
    (5.0, 2.0, true),
    (15.0, 2.0, false),
    (5.0, 8.0, false),
    (5.0, 17.0, false),
    (25.0, 22.0, true),
    (65.0, 82.0, true),
];

#[test]
fn a_tiling_pattern_lands_in_pattern_space_at_72_dpi() {
    marks(&tiled("", FILL), 72.0, &GRID);
}

#[test]
fn a_tiling_pattern_lands_in_pattern_space_at_144_dpi() {
    marks(&tiled("", FILL), 144.0, &GRID);
}

#[test]
fn a_tiling_pattern_lands_in_pattern_space_at_300_dpi() {
    marks(&tiled("", FILL), 300.0, &GRID);
}

#[test]
fn the_pattern_matrix_moves_the_tiling() {
    // Shifted 3 units right: the strip covers x 3..13, 23..33, ...
    let doc = tiled("/Matrix [1 0 0 1 3 0]", FILL);
    let points = [(12.0, 2.0, true), (2.0, 2.0, false), (24.0, 2.0, true)];
    marks(&doc, 72.0, &points);
    marks(&doc, 144.0, &points);
}

#[test]
fn the_pattern_matrix_scales_the_tiling() {
    // Doubled: a 40-unit cell with a 20 × 10 strip.
    let doc = tiled("/Matrix [2 0 0 2 0 0]", FILL);
    let points = [
        (15.0, 7.0, true),
        (25.0, 7.0, false),
        (15.0, 15.0, false),
        (55.0, 47.0, true),
    ];
    marks(&doc, 72.0, &points);
    marks(&doc, 144.0, &points);
}

#[test]
fn a_tiling_pattern_stroke_lands_in_pattern_space() {
    // A 30-wide stroke along y = 15 covers y 0..30.
    let doc = tiled("", "30 w /Pattern CS /T SCN 0 15 m 100 15 l S");
    marks(
        &doc,
        144.0,
        &[(5.0, 2.0, true), (15.0, 2.0, false), (25.0, 22.0, true)],
    );
}

#[test]
fn a_scaled_tiling_keeps_sharp_cell_edges() {
    // The cell is drawn at the resolution the doubled tiling is painted at,
    // so the pixels either side of the strip's right edge stay unblended.
    let doc = tiled("/Matrix [2 0 0 2 0 0]", FILL);
    marks(&doc, 144.0, &[(19.75, 7.0, true), (20.25, 7.0, false)]);
}
