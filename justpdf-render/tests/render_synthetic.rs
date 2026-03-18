use justpdf_render::render::compute_page_transform;

// Re-export for use in tests
use justpdf_core::page::Rect;

#[test]
fn test_page_transform_flips_y() {
    let media_box = Rect {
        llx: 0.0,
        lly: 0.0,
        urx: 612.0,
        ury: 792.0,
    };
    let t = compute_page_transform(&media_box, 1.0, 0);

    // Top-left in PDF (0, 792) → top-left in pixels (0, 0)
    let (px, py) = t.transform_point(0.0, 792.0);
    assert!((px).abs() < 0.001, "px={px}");
    assert!((py).abs() < 0.001, "py={py}");

    // Bottom-right in PDF (612, 0) → bottom-right in pixels (612, 792)
    let (px, py) = t.transform_point(612.0, 0.0);
    assert!((px - 612.0).abs() < 0.001, "px={px}");
    assert!((py - 792.0).abs() < 0.001, "py={py}");
}

#[test]
fn test_page_transform_with_offset_mediabox() {
    let media_box = Rect {
        llx: 50.0,
        lly: 50.0,
        urx: 562.0,
        ury: 742.0,
    };
    let t = compute_page_transform(&media_box, 1.0, 0);

    // Top-left of media box in PDF (50, 742) → (0, 0) in pixels
    let (px, py) = t.transform_point(50.0, 742.0);
    assert!((px).abs() < 0.001, "px={px}");
    assert!((py).abs() < 0.001, "py={py}");
}

#[test]
fn test_page_transform_at_2x_scale() {
    let media_box = Rect {
        llx: 0.0,
        lly: 0.0,
        urx: 100.0,
        ury: 200.0,
    };
    let t = compute_page_transform(&media_box, 2.0, 0);

    // (100, 0) in PDF → (200, 400) in pixels
    let (px, py) = t.transform_point(100.0, 0.0);
    assert!((px - 200.0).abs() < 0.001, "px={px}");
    assert!((py - 400.0).abs() < 0.001, "py={py}");
}
