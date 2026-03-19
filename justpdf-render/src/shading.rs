use justpdf_core::function::PdfFunction;
use justpdf_core::object::{PdfDict, PdfObject};
use tiny_skia::{
    Color, FillRule, GradientStop, LinearGradient, Mask, Paint, PathBuilder, Pixmap,
    RadialGradient, SpreadMode, Transform,
};

use crate::graphics_state::Matrix;

/// Render a shading pattern into the device.
/// `stream_data` is the decoded binary stream for mesh shadings (Type 4/5/6/7).
pub fn render_shading(
    pixmap: &mut Pixmap,
    shading_dict: &PdfDict,
    ctm: &Matrix,
    page_transform: &Matrix,
    clip_mask: Option<&Mask>,
    stream_data: Option<&[u8]>,
) {
    let shading_type = shading_dict.get_i64(b"ShadingType").unwrap_or(0);

    match shading_type {
        1 => render_function_based(pixmap, shading_dict, ctm, page_transform, clip_mask),
        2 => render_axial(pixmap, shading_dict, ctm, page_transform, clip_mask),
        3 => render_radial(pixmap, shading_dict, ctm, page_transform, clip_mask),
        4 | 5 => render_gouraud_mesh(pixmap, shading_dict, ctm, page_transform, clip_mask, stream_data),
        6 | 7 => render_patch_mesh(pixmap, shading_dict, ctm, page_transform, clip_mask, stream_data),
        _ => {} // unsupported
    }
}

// ---------------------------------------------------------------------------
// Type 1: Function-based shading
// ---------------------------------------------------------------------------

fn render_function_based(
    pixmap: &mut Pixmap,
    dict: &PdfDict,
    ctm: &Matrix,
    page_transform: &Matrix,
    clip: Option<&Mask>,
) {
    // Domain defaults to [0 1 0 1]
    let domain = dict.get_array(b"Domain");
    let x0 = domain
        .and_then(|a| a.first())
        .and_then(|o| o.as_f64())
        .unwrap_or(0.0);
    let x1 = domain
        .and_then(|a| a.get(1))
        .and_then(|o| o.as_f64())
        .unwrap_or(1.0);
    let y0 = domain
        .and_then(|a| a.get(2))
        .and_then(|o| o.as_f64())
        .unwrap_or(0.0);
    let y1 = domain
        .and_then(|a| a.get(3))
        .and_then(|o| o.as_f64())
        .unwrap_or(1.0);

    let cs_name = dict
        .get(b"ColorSpace")
        .and_then(|o| o.as_name())
        .unwrap_or(b"DeviceRGB");

    // Matrix from function domain to shading space
    let shading_matrix = if let Some(matrix_arr) = dict.get_array(b"Matrix") {
        if matrix_arr.len() >= 6 {
            Matrix {
                a: matrix_arr[0].as_f64().unwrap_or(1.0),
                b: matrix_arr[1].as_f64().unwrap_or(0.0),
                c: matrix_arr[2].as_f64().unwrap_or(0.0),
                d: matrix_arr[3].as_f64().unwrap_or(1.0),
                e: matrix_arr[4].as_f64().unwrap_or(0.0),
                f: matrix_arr[5].as_f64().unwrap_or(0.0),
            }
        } else {
            Matrix::identity()
        }
    } else {
        Matrix::identity()
    };

    // Try to resolve the function
    let func = dict.get(b"Function").and_then(PdfFunction::parse);

    let transform = shading_matrix.concat(ctm).concat(page_transform).to_skia();

    // Sample the function on a grid and render as colored rectangles
    let samples = 64; // Grid resolution
    let dx = (x1 - x0) / samples as f64;
    let dy = (y1 - y0) / samples as f64;

    for iy in 0..samples {
        for ix in 0..samples {
            let sx = x0 + (ix as f64 + 0.5) * dx;
            let sy = y0 + (iy as f64 + 0.5) * dy;

            let color = if let Some(ref f) = func {
                let result = f.evaluate(&[sx, sy]);
                components_to_color(&result, cs_name)
            } else {
                // No function — gray fallback
                components_to_color(&[0.5, 0.5, 0.5], cs_name)
            };

            let mut paint = Paint::default();
            paint.set_color(color);

            let rx = x0 + ix as f64 * dx;
            let ry = y0 + iy as f64 * dy;

            let mut pb = PathBuilder::new();
            pb.move_to(rx as f32, ry as f32);
            pb.line_to((rx + dx) as f32, ry as f32);
            pb.line_to((rx + dx) as f32, (ry + dy) as f32);
            pb.line_to(rx as f32, (ry + dy) as f32);
            pb.close();

            if let Some(path) = pb.finish() {
                pixmap.fill_path(&path, &paint, FillRule::Winding, transform, clip);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Type 2: Axial (linear) gradient
// ---------------------------------------------------------------------------

fn render_axial(
    pixmap: &mut Pixmap,
    dict: &PdfDict,
    ctm: &Matrix,
    page_transform: &Matrix,
    clip: Option<&Mask>,
) {
    let coords = match dict.get_array(b"Coords") {
        Some(arr) if arr.len() >= 4 => arr,
        _ => return,
    };

    let x0 = coords[0].as_f64().unwrap_or(0.0) as f32;
    let y0 = coords[1].as_f64().unwrap_or(0.0) as f32;
    let x1 = coords[2].as_f64().unwrap_or(0.0) as f32;
    let y1 = coords[3].as_f64().unwrap_or(0.0) as f32;

    let stops = extract_color_stops(dict);
    if stops.is_empty() {
        return;
    }

    let gradient = match LinearGradient::new(
        tiny_skia::Point::from_xy(x0, y0),
        tiny_skia::Point::from_xy(x1, y1),
        stops,
        SpreadMode::Pad,
        Transform::identity(),
    ) {
        Some(g) => g,
        None => return,
    };

    let mut paint = Paint::default();
    paint.shader = gradient;
    paint.anti_alias = true;

    let transform = ctm.concat(page_transform).to_skia();

    let mut pb = PathBuilder::new();
    pb.move_to(-10000.0, -10000.0);
    pb.line_to(20000.0, -10000.0);
    pb.line_to(20000.0, 20000.0);
    pb.line_to(-10000.0, 20000.0);
    pb.close();

    if let Some(path) = pb.finish() {
        pixmap.fill_path(&path, &paint, FillRule::Winding, transform, clip);
    }
}

// ---------------------------------------------------------------------------
// Type 3: Radial gradient
// ---------------------------------------------------------------------------

fn render_radial(
    pixmap: &mut Pixmap,
    dict: &PdfDict,
    ctm: &Matrix,
    page_transform: &Matrix,
    clip: Option<&Mask>,
) {
    let coords = match dict.get_array(b"Coords") {
        Some(arr) if arr.len() >= 6 => arr,
        _ => return,
    };

    let x0 = coords[0].as_f64().unwrap_or(0.0) as f32;
    let y0 = coords[1].as_f64().unwrap_or(0.0) as f32;
    let _r0 = coords[2].as_f64().unwrap_or(0.0) as f32;
    let x1 = coords[3].as_f64().unwrap_or(0.0) as f32;
    let y1 = coords[4].as_f64().unwrap_or(0.0) as f32;
    let r1 = coords[5].as_f64().unwrap_or(0.0) as f32;

    let stops = extract_color_stops(dict);
    if stops.is_empty() {
        return;
    }

    let gradient = match RadialGradient::new(
        tiny_skia::Point::from_xy(x0, y0),
        tiny_skia::Point::from_xy(x1, y1),
        r1,
        stops,
        SpreadMode::Pad,
        Transform::identity(),
    ) {
        Some(g) => g,
        None => return,
    };

    let mut paint = Paint::default();
    paint.shader = gradient;
    paint.anti_alias = true;

    let transform = ctm.concat(page_transform).to_skia();

    let mut pb = PathBuilder::new();
    pb.move_to(-10000.0, -10000.0);
    pb.line_to(20000.0, -10000.0);
    pb.line_to(20000.0, 20000.0);
    pb.line_to(-10000.0, 20000.0);
    pb.close();

    if let Some(path) = pb.finish() {
        pixmap.fill_path(&path, &paint, FillRule::Winding, transform, clip);
    }
}

// ---------------------------------------------------------------------------
// Type 4/5: Free-form / Lattice-form Gouraud-shaded triangle mesh
// ---------------------------------------------------------------------------

/// A vertex with position and color.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub struct Vertex {
    x: f64,
    y: f64,
    r: u8,
    g: u8,
    b: u8,
}

fn render_gouraud_mesh(
    pixmap: &mut Pixmap,
    dict: &PdfDict,
    ctm: &Matrix,
    page_transform: &Matrix,
    clip: Option<&Mask>,
    stream_data: Option<&[u8]>,
) {
    let shading_type = dict.get_i64(b"ShadingType").unwrap_or(4);

    let cs_name = dict
        .get(b"ColorSpace")
        .and_then(|o| o.as_name())
        .unwrap_or(b"DeviceRGB");
    let n_comps = color_space_components(cs_name);

    let bits_per_coordinate = dict.get_i64(b"BitsPerCoordinate").unwrap_or(8) as u32;
    let bits_per_component = dict.get_i64(b"BitsPerComponent").unwrap_or(8) as u32;
    let bits_per_flag = if shading_type == 4 {
        dict.get_i64(b"BitsPerFlag").unwrap_or(8) as u32
    } else {
        0
    };

    let decode = dict
        .get_array(b"Decode")
        .map(|a| a.iter().filter_map(|o| o.as_f64()).collect::<Vec<_>>())
        .unwrap_or_default();

    let data = match stream_data {
        Some(d) if !d.is_empty() => d,
        _ => {
            // No stream data — fallback
            render_mesh_fallback(pixmap, ctm, page_transform, clip, cs_name, &decode);
            return;
        }
    };

    let transform = ctm.concat(page_transform).to_skia();

    if shading_type == 4 {
        // Type 4: Free-form Gouraud triangle mesh
        let triangles = parse_gouraud_triangles(
            data,
            bits_per_flag,
            bits_per_coordinate,
            bits_per_component,
            n_comps,
            &decode,
            cs_name,
        );

        for tri in &triangles {
            rasterize_triangle(
                pixmap,
                (tri[0].x as f32, tri[0].y as f32, [tri[0].r, tri[0].g, tri[0].b, 255]),
                (tri[1].x as f32, tri[1].y as f32, [tri[1].r, tri[1].g, tri[1].b, 255]),
                (tri[2].x as f32, tri[2].y as f32, [tri[2].r, tri[2].g, tri[2].b, 255]),
                transform,
                clip,
            );
        }
    } else {
        // Type 5: Lattice-form Gouraud mesh
        let vertices_per_row = dict.get_i64(b"VerticesPerRow").unwrap_or(2) as usize;
        if vertices_per_row < 2 {
            return;
        }

        let vertices = parse_lattice_vertices(
            data,
            bits_per_coordinate,
            bits_per_component,
            n_comps,
            &decode,
            cs_name,
        );

        // Build triangles from lattice grid
        let n_rows = vertices.len() / vertices_per_row;
        for row in 0..n_rows.saturating_sub(1) {
            for col in 0..vertices_per_row - 1 {
                let i0 = row * vertices_per_row + col;
                let i1 = i0 + 1;
                let i2 = i0 + vertices_per_row;
                let i3 = i2 + 1;

                if i3 < vertices.len() {
                    let v0 = &vertices[i0];
                    let v1 = &vertices[i1];
                    let v2 = &vertices[i2];
                    let v3 = &vertices[i3];

                    // Two triangles per quad
                    rasterize_triangle(
                        pixmap,
                        (v0.x as f32, v0.y as f32, [v0.r, v0.g, v0.b, 255]),
                        (v1.x as f32, v1.y as f32, [v1.r, v1.g, v1.b, 255]),
                        (v2.x as f32, v2.y as f32, [v2.r, v2.g, v2.b, 255]),
                        transform,
                        clip,
                    );
                    rasterize_triangle(
                        pixmap,
                        (v1.x as f32, v1.y as f32, [v1.r, v1.g, v1.b, 255]),
                        (v3.x as f32, v3.y as f32, [v3.r, v3.g, v3.b, 255]),
                        (v2.x as f32, v2.y as f32, [v2.r, v2.g, v2.b, 255]),
                        transform,
                        clip,
                    );
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Type 6/7: Coons / Tensor-product patch mesh
// ---------------------------------------------------------------------------

fn render_patch_mesh(
    pixmap: &mut Pixmap,
    dict: &PdfDict,
    ctm: &Matrix,
    page_transform: &Matrix,
    clip: Option<&Mask>,
    stream_data: Option<&[u8]>,
) {
    let shading_type = dict.get_i64(b"ShadingType").unwrap_or(6);

    let cs_name = dict
        .get(b"ColorSpace")
        .and_then(|o| o.as_name())
        .unwrap_or(b"DeviceRGB");
    let n_comps = color_space_components(cs_name);

    let bits_per_coordinate = dict.get_i64(b"BitsPerCoordinate").unwrap_or(8) as u32;
    let bits_per_component = dict.get_i64(b"BitsPerComponent").unwrap_or(8) as u32;
    let bits_per_flag = dict.get_i64(b"BitsPerFlag").unwrap_or(8) as u32;

    let decode = dict
        .get_array(b"Decode")
        .map(|a| a.iter().filter_map(|o| o.as_f64()).collect::<Vec<_>>())
        .unwrap_or_default();

    let data = match stream_data {
        Some(d) if !d.is_empty() => d,
        _ => {
            render_mesh_fallback(pixmap, ctm, page_transform, clip, cs_name, &decode);
            return;
        }
    };

    let transform = ctm.concat(page_transform).to_skia();

    // Number of control points per patch: 12 for Coons (Type 6), 16 for Tensor (Type 7)
    let points_per_patch: usize = if shading_type == 7 { 16 } else { 12 };

    let patches = parse_patch_mesh(
        data,
        bits_per_flag,
        bits_per_coordinate,
        bits_per_component,
        n_comps,
        points_per_patch,
        &decode,
        cs_name,
    );

    // Subdivide each patch into triangles and rasterize
    for patch in &patches {
        subdivide_and_rasterize_patch(pixmap, patch, transform, clip, 3);
    }
}

/// Fallback renderer for mesh shadings when we don't have stream data.
/// Renders the decode-range bounding box with averaged color.
fn render_mesh_fallback(
    pixmap: &mut Pixmap,
    ctm: &Matrix,
    page_transform: &Matrix,
    clip: Option<&Mask>,
    cs_name: &[u8],
    decode: &[f64],
) {
    let x_min = decode.first().copied().unwrap_or(0.0) as f32;
    let x_max = decode.get(1).copied().unwrap_or(1.0) as f32;
    let y_min = decode.get(2).copied().unwrap_or(0.0) as f32;
    let y_max = decode.get(3).copied().unwrap_or(1.0) as f32;

    // Use mid-range colors as fallback
    let n_comps = color_space_components(cs_name);
    let mut mid_comps = Vec::new();
    for i in 0..n_comps {
        let idx = 4 + i * 2;
        let cmin = decode.get(idx).copied().unwrap_or(0.0);
        let cmax = decode.get(idx + 1).copied().unwrap_or(1.0);
        mid_comps.push((cmin + cmax) / 2.0);
    }

    let color = components_to_color(&mid_comps, cs_name);
    let transform = ctm.concat(page_transform).to_skia();

    let mut paint = Paint::default();
    paint.set_color(color);
    paint.anti_alias = true;

    let mut pb = PathBuilder::new();
    pb.move_to(x_min, y_min);
    pb.line_to(x_max, y_min);
    pb.line_to(x_max, y_max);
    pb.line_to(x_min, y_max);
    pb.close();

    if let Some(path) = pb.finish() {
        pixmap.fill_path(&path, &paint, FillRule::Winding, transform, clip);
    }
}

/// Rasterize a single triangle with per-vertex colors using barycentric interpolation.
/// This is the core routine for mesh shading.
pub fn rasterize_triangle(
    pixmap: &mut Pixmap,
    v0: (f32, f32, [u8; 4]),
    v1: (f32, f32, [u8; 4]),
    v2: (f32, f32, [u8; 4]),
    transform: Transform,
    clip: Option<&Mask>,
) {
    // Transform vertices to pixel coordinates
    let p0 = transform_point(transform, v0.0, v0.1);
    let p1 = transform_point(transform, v1.0, v1.1);
    let p2 = transform_point(transform, v2.0, v2.1);

    // Bounding box
    let min_x = p0.0.min(p1.0).min(p2.0).floor().max(0.0) as i32;
    let max_x = p0.0.max(p1.0).max(p2.0).ceil().min(pixmap.width() as f32) as i32;
    let min_y = p0.1.min(p1.1).min(p2.1).floor().max(0.0) as i32;
    let max_y = p0.1.max(p1.1).max(p2.1).ceil().min(pixmap.height() as f32) as i32;

    let area = edge_function(p0, p1, p2);
    if area.abs() < 0.001 {
        return; // degenerate triangle
    }
    let inv_area = 1.0 / area;

    let width = pixmap.width() as i32;
    let pixels = pixmap.pixels_mut();

    for y in min_y..max_y {
        for x in min_x..max_x {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let p = (px, py);

            let w0 = edge_function(p1, p2, p) * inv_area;
            let w1 = edge_function(p2, p0, p) * inv_area;
            let w2 = edge_function(p0, p1, p) * inv_area;

            if w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0 {
                // Check clip mask
                // Note: clip mask check for mesh triangles is not implemented
                let _ = clip;

                let r = (w0 * v0.2[0] as f32 + w1 * v1.2[0] as f32 + w2 * v2.2[0] as f32) as u8;
                let g = (w0 * v0.2[1] as f32 + w1 * v1.2[1] as f32 + w2 * v2.2[1] as f32) as u8;
                let b = (w0 * v0.2[2] as f32 + w1 * v1.2[2] as f32 + w2 * v2.2[2] as f32) as u8;
                let a = (w0 * v0.2[3] as f32 + w1 * v1.2[3] as f32 + w2 * v2.2[3] as f32) as u8;

                let idx = (y * width + x) as usize;
                if idx < pixels.len() {
                    // Premultiply alpha for tiny-skia
                    if let Some(color) =
                        tiny_skia::PremultipliedColorU8::from_rgba(r, g, b, a)
                    {
                        pixels[idx] = color;
                    }
                }
            }
        }
    }
}

fn edge_function(a: (f32, f32), b: (f32, f32), c: (f32, f32)) -> f32 {
    (c.0 - a.0) * (b.1 - a.1) - (c.1 - a.1) * (b.0 - a.0)
}

fn transform_point(t: Transform, x: f32, y: f32) -> (f32, f32) {
    (
        t.sx * x + t.kx * y + t.tx,
        t.ky * x + t.sy * y + t.ty,
    )
}

// ---------------------------------------------------------------------------
// Mesh stream parser (for when stream data is available)
// ---------------------------------------------------------------------------

/// Parse vertices from a Type 4 (free-form Gouraud) mesh stream.
pub fn parse_gouraud_triangles(
    data: &[u8],
    bits_per_flag: u32,
    bits_per_coordinate: u32,
    bits_per_component: u32,
    n_components: usize,
    decode: &[f64],
    cs_name: &[u8],
) -> Vec<[Vertex; 3]> {
    let mut reader = BitReader::new(data);
    let mut vertices: Vec<Vertex> = Vec::new();
    let mut triangles: Vec<[Vertex; 3]> = Vec::new();

    let (x_min, x_max) = (
        decode.first().copied().unwrap_or(0.0),
        decode.get(1).copied().unwrap_or(1.0),
    );
    let (y_min, y_max) = (
        decode.get(2).copied().unwrap_or(0.0),
        decode.get(3).copied().unwrap_or(1.0),
    );

    let coord_max = ((1u64 << bits_per_coordinate) - 1) as f64;
    let comp_max = ((1u64 << bits_per_component) - 1) as f64;

    loop {
        // Read flag
        let flag = if bits_per_flag > 0 {
            match reader.read_bits(bits_per_flag) {
                Some(f) => f as u8,
                None => break,
            }
        } else {
            0
        };

        // Read coordinates
        let raw_x = match reader.read_bits(bits_per_coordinate) {
            Some(v) => v,
            None => break,
        };
        let raw_y = match reader.read_bits(bits_per_coordinate) {
            Some(v) => v,
            None => break,
        };

        let x = x_min + (raw_x as f64 / coord_max) * (x_max - x_min);
        let y = y_min + (raw_y as f64 / coord_max) * (y_max - y_min);

        // Read color components
        let mut comps = Vec::with_capacity(n_components);
        for i in 0..n_components {
            let raw_c = match reader.read_bits(bits_per_component) {
                Some(v) => v,
                None => break,
            };
            let c_min = decode.get(4 + i * 2).copied().unwrap_or(0.0);
            let c_max = decode.get(4 + i * 2 + 1).copied().unwrap_or(1.0);
            comps.push(c_min + (raw_c as f64 / comp_max) * (c_max - c_min));
        }
        if comps.len() != n_components {
            break;
        }

        let color = components_to_color(&comps, cs_name);
        let [r, g, b, _] = color_to_rgba8(color);

        let vertex = Vertex { x, y, r, g, b };

        match flag {
            0 => {
                // New triangle: need 3 vertices
                vertices.clear();
                vertices.push(vertex);
            }
            1 => {
                // Continue from last 2 vertices
                if vertices.len() >= 2 {
                    let v_prev1 = vertices[vertices.len() - 2];
                    let v_prev2 = vertices[vertices.len() - 1];
                    vertices.push(vertex);
                    triangles.push([v_prev2, vertices.last().copied().unwrap(), v_prev1]);
                } else {
                    vertices.push(vertex);
                }
            }
            2 => {
                // Continue from first and last vertex
                if vertices.len() >= 2 {
                    let v_first = vertices[0];
                    let v_last = vertices[vertices.len() - 1];
                    vertices.push(vertex);
                    triangles.push([v_last, vertices.last().copied().unwrap(), v_first]);
                } else {
                    vertices.push(vertex);
                }
            }
            _ => {
                vertices.push(vertex);
            }
        }

        // When we have 3 vertices with flag=0, form a triangle
        if flag == 0 && vertices.len() == 3 {
            triangles.push([vertices[0], vertices[1], vertices[2]]);
        } else if flag == 0 && vertices.len() < 3 {
            continue; // need more vertices
        }
    }

    triangles
}

/// Parse vertices from a Type 5 (lattice-form Gouraud) mesh stream.
/// No flag bits — vertices are read in row-major order.
pub fn parse_lattice_vertices(
    data: &[u8],
    bits_per_coordinate: u32,
    bits_per_component: u32,
    n_components: usize,
    decode: &[f64],
    cs_name: &[u8],
) -> Vec<Vertex> {
    let mut reader = BitReader::new(data);
    let mut vertices: Vec<Vertex> = Vec::new();

    let (x_min, x_max) = (
        decode.first().copied().unwrap_or(0.0),
        decode.get(1).copied().unwrap_or(1.0),
    );
    let (y_min, y_max) = (
        decode.get(2).copied().unwrap_or(0.0),
        decode.get(3).copied().unwrap_or(1.0),
    );

    let coord_max = ((1u64 << bits_per_coordinate) - 1) as f64;
    let comp_max = ((1u64 << bits_per_component) - 1) as f64;

    loop {
        let raw_x = match reader.read_bits(bits_per_coordinate) {
            Some(v) => v,
            None => break,
        };
        let raw_y = match reader.read_bits(bits_per_coordinate) {
            Some(v) => v,
            None => break,
        };

        let x = x_min + (raw_x as f64 / coord_max) * (x_max - x_min);
        let y = y_min + (raw_y as f64 / coord_max) * (y_max - y_min);

        let mut comps = Vec::with_capacity(n_components);
        let mut ok = true;
        for i in 0..n_components {
            match reader.read_bits(bits_per_component) {
                Some(raw_c) => {
                    let c_min = decode.get(4 + i * 2).copied().unwrap_or(0.0);
                    let c_max = decode.get(4 + i * 2 + 1).copied().unwrap_or(1.0);
                    comps.push(c_min + (raw_c as f64 / comp_max) * (c_max - c_min));
                }
                None => {
                    ok = false;
                    break;
                }
            }
        }
        if !ok {
            break;
        }

        let color = components_to_color(&comps, cs_name);
        let [r, g, b, _] = color_to_rgba8(color);
        vertices.push(Vertex { x, y, r, g, b });
    }

    vertices
}

/// A patch with 4 corner positions and colors (subdivided from Coons/Tensor control points).
#[derive(Debug, Clone)]
pub struct Patch {
    /// Corner positions: [p00, p01, p10, p11] (or more control points)
    pub corners: [(f64, f64); 4],
    /// Corner colors: [c00, c01, c10, c11]
    pub colors: [[u8; 4]; 4],
}

/// Parse patches from a Type 6/7 mesh stream.
pub fn parse_patch_mesh(
    data: &[u8],
    bits_per_flag: u32,
    bits_per_coordinate: u32,
    bits_per_component: u32,
    n_components: usize,
    points_per_patch: usize,
    decode: &[f64],
    cs_name: &[u8],
) -> Vec<Patch> {
    let mut reader = BitReader::new(data);
    let mut patches: Vec<Patch> = Vec::new();

    let (x_min, x_max) = (
        decode.first().copied().unwrap_or(0.0),
        decode.get(1).copied().unwrap_or(1.0),
    );
    let (y_min, y_max) = (
        decode.get(2).copied().unwrap_or(0.0),
        decode.get(3).copied().unwrap_or(1.0),
    );

    let coord_max = ((1u64 << bits_per_coordinate) - 1).max(1) as f64;
    let comp_max = ((1u64 << bits_per_component) - 1).max(1) as f64;

    let mut prev_points: Vec<(f64, f64)> = Vec::new();
    let mut prev_colors: Vec<[u8; 4]> = Vec::new();

    loop {
        let flag = match reader.read_bits(bits_per_flag) {
            Some(f) => f as u8,
            None => break,
        };

        // Determine how many new points/colors to read based on flag
        let (n_points, n_colors) = if flag == 0 {
            (points_per_patch, 4) // Full patch
        } else {
            // Continuation: reuse some points from previous patch
            // Type 6: flag 1/2/3 reuse 4 points from previous edge, read 8 new + 2 colors
            // Type 7: flag 1/2/3 reuse 4 points, read 12 new + 2 colors
            if points_per_patch == 16 {
                (12, 2)
            } else {
                (8, 2)
            }
        };

        // Read control points
        let mut points: Vec<(f64, f64)> = Vec::with_capacity(n_points);
        let mut ok = true;
        for _ in 0..n_points {
            let raw_x = match reader.read_bits(bits_per_coordinate) {
                Some(v) => v,
                None => { ok = false; break; }
            };
            let raw_y = match reader.read_bits(bits_per_coordinate) {
                Some(v) => v,
                None => { ok = false; break; }
            };
            let x = x_min + (raw_x as f64 / coord_max) * (x_max - x_min);
            let y = y_min + (raw_y as f64 / coord_max) * (y_max - y_min);
            points.push((x, y));
        }
        if !ok { break; }

        // Read colors
        let mut colors: Vec<[u8; 4]> = Vec::with_capacity(n_colors);
        for _ in 0..n_colors {
            let mut comps = Vec::with_capacity(n_components);
            for i in 0..n_components {
                match reader.read_bits(bits_per_component) {
                    Some(raw_c) => {
                        let c_min = decode.get(4 + i * 2).copied().unwrap_or(0.0);
                        let c_max = decode.get(4 + i * 2 + 1).copied().unwrap_or(1.0);
                        comps.push(c_min + (raw_c as f64 / comp_max) * (c_max - c_min));
                    }
                    None => { ok = false; break; }
                }
            }
            if !ok { break; }
            let color = components_to_color(&comps, cs_name);
            let [r, g, b, a] = color_to_rgba8(color);
            colors.push([r, g, b, a]);
        }
        if !ok { break; }

        // For simplicity, extract the 4 corner positions and colors
        // (ignoring intermediate bezier control points — approximation)
        let (corners, corner_colors) = if flag == 0 && points.len() >= 4 && colors.len() >= 4 {
            // Full patch: corners are at indices 0, 3, 6, 9 for Type 6 (12 points)
            // or 0, 3, 8, 11 for Type 7 (16 points)
            let c0 = points[0];
            let c1 = if points_per_patch == 16 { points[3] } else { points[3] };
            let c2 = if points_per_patch == 16 { points[12] } else { points[9] };
            let c3 = if points_per_patch == 16 {
                points.get(15).copied().unwrap_or(c2)
            } else {
                points.get(6).copied().unwrap_or(c0)
            };
            (
                [c0, c1, c2, c3],
                [colors[0], colors[1], colors[2], colors[3]],
            )
        } else if !prev_points.is_empty() && points.len() >= 4 && colors.len() >= 2 {
            // Continuation: approximate with new points
            let c0 = points[0];
            let c1 = points.get(3).copied().unwrap_or(c0);
            let c2 = points.last().copied().unwrap_or(c0);
            let c3 = points.get(points.len() / 2).copied().unwrap_or(c0);
            let pc = if prev_colors.len() >= 4 {
                [prev_colors[1], prev_colors[2]]
            } else {
                [[128, 128, 128, 255]; 2]
            };
            (
                [c0, c1, c2, c3],
                [pc[0], colors[0], pc[1], colors[1]],
            )
        } else {
            continue;
        };

        prev_points = points;
        prev_colors = Vec::from(corner_colors);

        patches.push(Patch {
            corners,
            colors: corner_colors,
        });
    }

    patches
}

/// Subdivide a patch into triangles and rasterize them.
/// Uses bilinear interpolation across the 4 corners.
fn subdivide_and_rasterize_patch(
    pixmap: &mut Pixmap,
    patch: &Patch,
    transform: Transform,
    clip: Option<&Mask>,
    subdivisions: usize,
) {
    let n = subdivisions.max(1);
    let step = 1.0 / n as f64;

    for i in 0..n {
        for j in 0..n {
            let u0 = i as f64 * step;
            let v0 = j as f64 * step;
            let u1 = u0 + step;
            let v1 = v0 + step;

            let p00 = bilinear_point(&patch.corners, u0, v0);
            let p10 = bilinear_point(&patch.corners, u1, v0);
            let p01 = bilinear_point(&patch.corners, u0, v1);
            let p11 = bilinear_point(&patch.corners, u1, v1);

            let c00 = bilinear_color(&patch.colors, u0, v0);
            let c10 = bilinear_color(&patch.colors, u1, v0);
            let c01 = bilinear_color(&patch.colors, u0, v1);
            let c11 = bilinear_color(&patch.colors, u1, v1);

            // Two triangles per quad
            rasterize_triangle(
                pixmap,
                (p00.0 as f32, p00.1 as f32, c00),
                (p10.0 as f32, p10.1 as f32, c10),
                (p01.0 as f32, p01.1 as f32, c01),
                transform,
                clip,
            );
            rasterize_triangle(
                pixmap,
                (p10.0 as f32, p10.1 as f32, c10),
                (p11.0 as f32, p11.1 as f32, c11),
                (p01.0 as f32, p01.1 as f32, c01),
                transform,
                clip,
            );
        }
    }
}

/// Bilinear interpolation of a point on a quad defined by 4 corners.
fn bilinear_point(corners: &[(f64, f64); 4], u: f64, v: f64) -> (f64, f64) {
    let x = corners[0].0 * (1.0 - u) * (1.0 - v)
        + corners[1].0 * u * (1.0 - v)
        + corners[2].0 * (1.0 - u) * v
        + corners[3].0 * u * v;
    let y = corners[0].1 * (1.0 - u) * (1.0 - v)
        + corners[1].1 * u * (1.0 - v)
        + corners[2].1 * (1.0 - u) * v
        + corners[3].1 * u * v;
    (x, y)
}

/// Bilinear interpolation of color on a quad.
fn bilinear_color(colors: &[[u8; 4]; 4], u: f64, v: f64) -> [u8; 4] {
    let mut result = [0u8; 4];
    for ch in 0..4 {
        let c = colors[0][ch] as f64 * (1.0 - u) * (1.0 - v)
            + colors[1][ch] as f64 * u * (1.0 - v)
            + colors[2][ch] as f64 * (1.0 - u) * v
            + colors[3][ch] as f64 * u * v;
        result[ch] = c.clamp(0.0, 255.0) as u8;
    }
    result
}

fn color_to_rgba8(color: Color) -> [u8; 4] {
    [
        (color.red() * 255.0) as u8,
        (color.green() * 255.0) as u8,
        (color.blue() * 255.0) as u8,
        (color.alpha() * 255.0) as u8,
    ]
}

/// Simple bit reader for parsing mesh stream data.
struct BitReader<'a> {
    data: &'a [u8],
    byte_pos: usize,
    bit_pos: u8, // 0-7, MSB first
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            byte_pos: 0,
            bit_pos: 0,
        }
    }

    fn read_bits(&mut self, n: u32) -> Option<u64> {
        if n == 0 || n > 64 {
            return Some(0);
        }

        let mut result: u64 = 0;
        let mut remaining = n;

        while remaining > 0 {
            if self.byte_pos >= self.data.len() {
                return None;
            }

            let available = 8 - self.bit_pos as u32;
            let to_read = remaining.min(available);

            let byte = self.data[self.byte_pos];
            let shift = available - to_read;
            let mask = ((1u16 << to_read) - 1) as u8;
            let bits = (byte >> shift) & mask;

            result = (result << to_read) | bits as u64;
            remaining -= to_read;

            self.bit_pos += to_read as u8;
            if self.bit_pos >= 8 {
                self.bit_pos = 0;
                self.byte_pos += 1;
            }
        }

        Some(result)
    }
}

// ---------------------------------------------------------------------------
// Color stop extraction (for Type 2/3 shadings)
// ---------------------------------------------------------------------------

fn extract_color_stops(dict: &PdfDict) -> Vec<GradientStop> {
    let cs_name = dict
        .get(b"ColorSpace")
        .and_then(|o| o.as_name())
        .unwrap_or(b"DeviceRGB");

    if let Some(func_obj) = dict.get(b"Function") {
        if let PdfObject::Dict(func) = func_obj {
            return extract_stops_from_function(func, cs_name);
        }
        if let PdfObject::Array(funcs) = func_obj {
            if let Some(PdfObject::Dict(func)) = funcs.first() {
                return extract_stops_from_function(func, cs_name);
            }
        }
    }

    vec![
        GradientStop::new(0.0, Color::BLACK),
        GradientStop::new(1.0, Color::WHITE),
    ]
}

fn extract_stops_from_function(func: &PdfDict, cs_name: &[u8]) -> Vec<GradientStop> {
    let func_type = func.get_i64(b"FunctionType").unwrap_or(0);

    if func_type == 2 {
        let c0 = func
            .get_array(b"C0")
            .map(|a| a.iter().map(|o| o.as_f64().unwrap_or(0.0)).collect::<Vec<_>>())
            .unwrap_or_else(|| vec![0.0]);
        let c1 = func
            .get_array(b"C1")
            .map(|a| a.iter().map(|o| o.as_f64().unwrap_or(0.0)).collect::<Vec<_>>())
            .unwrap_or_else(|| vec![1.0]);

        vec![
            GradientStop::new(0.0, components_to_color(&c0, cs_name)),
            GradientStop::new(1.0, components_to_color(&c1, cs_name)),
        ]
    } else if func_type == 3 {
        let bounds = func
            .get_array(b"Bounds")
            .map(|a| a.iter().filter_map(|o| o.as_f64()).collect::<Vec<_>>())
            .unwrap_or_default();
        let functions = func.get_array(b"Functions").unwrap_or(&[]);

        let mut color_stops: Vec<(f32, Color)> = Vec::new();

        for (i, sub_func) in functions.iter().enumerate() {
            if let PdfObject::Dict(sub) = sub_func {
                let sub_colors = extract_colors_from_function(sub, cs_name);
                let t_start = if i == 0 {
                    0.0
                } else {
                    bounds.get(i - 1).copied().unwrap_or(0.0) as f32
                };
                let t_end = if i < bounds.len() {
                    bounds[i] as f32
                } else {
                    1.0
                };

                if let Some(c) = sub_colors.first() {
                    color_stops.push((t_start.clamp(0.0, 1.0), *c));
                }
                if let Some(c) = sub_colors.last() {
                    color_stops.push((t_end.clamp(0.0, 1.0), *c));
                }
            }
        }

        color_stops.dedup_by(|a, b| (a.0 - b.0).abs() < 0.001);

        if color_stops.len() < 2 {
            color_stops = vec![(0.0, Color::BLACK), (1.0, Color::WHITE)];
        }

        color_stops
            .into_iter()
            .map(|(pos, color)| GradientStop::new(pos, color))
            .collect()
    } else {
        vec![
            GradientStop::new(0.0, Color::BLACK),
            GradientStop::new(1.0, Color::WHITE),
        ]
    }
}

fn extract_colors_from_function(func: &PdfDict, cs_name: &[u8]) -> Vec<Color> {
    let c0 = func
        .get_array(b"C0")
        .map(|a| a.iter().map(|o| o.as_f64().unwrap_or(0.0)).collect::<Vec<_>>())
        .unwrap_or_else(|| vec![0.0]);
    let c1 = func
        .get_array(b"C1")
        .map(|a| a.iter().map(|o| o.as_f64().unwrap_or(0.0)).collect::<Vec<_>>())
        .unwrap_or_else(|| vec![1.0]);

    vec![
        components_to_color(&c0, cs_name),
        components_to_color(&c1, cs_name),
    ]
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn components_to_color(comps: &[f64], cs_name: &[u8]) -> Color {
    match cs_name {
        b"DeviceGray" | b"CalGray" | b"G" => {
            let g = (comps.first().copied().unwrap_or(0.0).clamp(0.0, 1.0) * 255.0) as u8;
            Color::from_rgba8(g, g, g, 255)
        }
        b"DeviceCMYK" | b"CMYK" => {
            let c = comps.first().copied().unwrap_or(0.0);
            let m = comps.get(1).copied().unwrap_or(0.0);
            let y = comps.get(2).copied().unwrap_or(0.0);
            let k = comps.get(3).copied().unwrap_or(0.0);
            let r = ((1.0 - c) * (1.0 - k) * 255.0) as u8;
            let g = ((1.0 - m) * (1.0 - k) * 255.0) as u8;
            let b = ((1.0 - y) * (1.0 - k) * 255.0) as u8;
            Color::from_rgba8(r, g, b, 255)
        }
        _ => {
            let r = (comps.first().copied().unwrap_or(0.0).clamp(0.0, 1.0) * 255.0) as u8;
            let g = (comps.get(1).copied().unwrap_or(0.0).clamp(0.0, 1.0) * 255.0) as u8;
            let b = (comps.get(2).copied().unwrap_or(0.0).clamp(0.0, 1.0) * 255.0) as u8;
            Color::from_rgba8(r, g, b, 255)
        }
    }
}

fn color_space_components(cs_name: &[u8]) -> usize {
    match cs_name {
        b"DeviceGray" | b"CalGray" | b"G" => 1,
        b"DeviceRGB" | b"CalRGB" | b"RGB" => 3,
        b"DeviceCMYK" | b"CMYK" => 4,
        _ => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bit_reader_8bit() {
        let data = [0xAB, 0xCD];
        let mut r = BitReader::new(&data);
        assert_eq!(r.read_bits(8), Some(0xAB));
        assert_eq!(r.read_bits(8), Some(0xCD));
        assert_eq!(r.read_bits(8), None);
    }

    #[test]
    fn test_bit_reader_4bit() {
        let data = [0xAB]; // 1010 1011
        let mut r = BitReader::new(&data);
        assert_eq!(r.read_bits(4), Some(0xA)); // 1010
        assert_eq!(r.read_bits(4), Some(0xB)); // 1011
    }

    #[test]
    fn test_bit_reader_mixed() {
        let data = [0b11001010, 0b01010101];
        let mut r = BitReader::new(&data);
        assert_eq!(r.read_bits(2), Some(0b11));
        assert_eq!(r.read_bits(3), Some(0b001));
        assert_eq!(r.read_bits(3), Some(0b010));
        // 8 bits read, next byte: 01010101
        assert_eq!(r.read_bits(8), Some(0b01010101));
    }

    #[test]
    fn test_bit_reader_16bit() {
        let data = [0x12, 0x34];
        let mut r = BitReader::new(&data);
        assert_eq!(r.read_bits(16), Some(0x1234));
    }

    #[test]
    fn test_components_to_color_rgb() {
        let c = components_to_color(&[1.0, 0.0, 0.5], b"DeviceRGB");
        assert_eq!(c.red(), 1.0);
        assert_eq!(c.green(), 0.0);
        assert!((c.blue() - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_components_to_color_gray() {
        let c = components_to_color(&[0.5], b"DeviceGray");
        assert!((c.red() - c.green()).abs() < 0.01);
        assert!((c.green() - c.blue()).abs() < 0.01);
    }

    #[test]
    fn test_rasterize_triangle_basic() {
        let mut pixmap = Pixmap::new(10, 10).unwrap();
        rasterize_triangle(
            &mut pixmap,
            (1.0, 1.0, [255, 0, 0, 255]),
            (9.0, 1.0, [0, 255, 0, 255]),
            (5.0, 9.0, [0, 0, 255, 255]),
            Transform::identity(),
            None,
        );

        // Check that some pixels were filled (not all transparent)
        let has_colored = pixmap.pixels().iter().any(|p| p.alpha() > 0);
        assert!(has_colored, "triangle should have colored some pixels");
    }

    #[test]
    fn test_edge_function() {
        let a = (0.0, 0.0);
        let b = (10.0, 0.0);
        let c = (5.0, 5.0);
        let area = edge_function(a, b, c);
        // The sign depends on winding order; the important thing is it's non-zero
        assert!(area.abs() > 0.0, "degenerate triangle should not have zero area");
    }
}
