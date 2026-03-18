use justpdf_core::object::{PdfDict, PdfObject};
use tiny_skia::{
    Color, FillRule, GradientStop, LinearGradient, Mask, Paint, PathBuilder, Pixmap,
    RadialGradient, SpreadMode, Transform,
};

use crate::graphics_state::Matrix;

/// Render a shading pattern into the device.
pub fn render_shading(
    pixmap: &mut Pixmap,
    shading_dict: &PdfDict,
    ctm: &Matrix,
    page_transform: &Matrix,
    clip_mask: Option<&Mask>,
) {
    let shading_type = shading_dict.get_i64(b"ShadingType").unwrap_or(0);

    match shading_type {
        1 => render_function_based(pixmap, shading_dict, ctm, page_transform, clip_mask),
        2 => render_axial(pixmap, shading_dict, ctm, page_transform, clip_mask),
        3 => render_radial(pixmap, shading_dict, ctm, page_transform, clip_mask),
        4 | 5 => render_gouraud_mesh(pixmap, shading_dict, ctm, page_transform, clip_mask),
        6 | 7 => render_patch_mesh(pixmap, shading_dict, ctm, page_transform, clip_mask),
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

    // For function-based shading, we sample the function at grid points
    // and render as a mesh of colored rectangles
    let cs_name = dict
        .get(b"ColorSpace")
        .and_then(|o| o.as_name())
        .unwrap_or(b"DeviceRGB");

    // Without actually evaluating the PDF function (which requires a full
    // function evaluator), we approximate with a gradient from the function's
    // C0/C1 if available, or just render a fallback.
    // A full implementation would need a PDF function evaluator.
    let transform = ctm.concat(page_transform).to_skia();
    let color = components_to_color(&[0.5, 0.5, 0.5], cs_name);

    let mut paint = Paint::default();
    paint.set_color(color);
    paint.anti_alias = true;

    let mut pb = PathBuilder::new();
    pb.move_to(x0 as f32, y0 as f32);
    pb.line_to(x1 as f32, y0 as f32);
    pb.line_to(x1 as f32, y1 as f32);
    pb.line_to(x0 as f32, y1 as f32);
    pb.close();

    if let Some(path) = pb.finish() {
        pixmap.fill_path(&path, &paint, FillRule::Winding, transform, clip);
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
) {
    let shading_type = dict.get_i64(b"ShadingType").unwrap_or(4);

    let cs_name = dict
        .get(b"ColorSpace")
        .and_then(|o| o.as_name())
        .unwrap_or(b"DeviceRGB");
    let n_comps = color_space_components(cs_name);

    let _bits_per_coordinate = dict.get_i64(b"BitsPerCoordinate").unwrap_or(8) as u32;
    let _bits_per_component = dict.get_i64(b"BitsPerComponent").unwrap_or(8) as u32;
    let _bits_per_flag = if shading_type == 4 {
        dict.get_i64(b"BitsPerFlag").unwrap_or(8) as u32
    } else {
        0
    };

    let decode = dict
        .get_array(b"Decode")
        .map(|a| a.iter().filter_map(|o| o.as_f64()).collect::<Vec<_>>())
        .unwrap_or_default();

    // Decode array: [xmin xmax ymin ymax c1min c1max c2min c2max ...]
    let (_x_min, _x_max) = if decode.len() >= 2 {
        (decode[0], decode[1])
    } else {
        (0.0, 1.0)
    };
    let (_y_min, _y_max) = if decode.len() >= 4 {
        (decode[2], decode[3])
    } else {
        (0.0, 1.0)
    };

    // Color decode ranges
    let mut c_ranges: Vec<(f64, f64)> = Vec::new();
    for i in 0..n_comps {
        let idx = 4 + i * 2;
        let cmin = decode.get(idx).copied().unwrap_or(0.0);
        let cmax = decode.get(idx + 1).copied().unwrap_or(1.0);
        c_ranges.push((cmin, cmax));
    }

    // For now, we don't have the actual stream data in the shading dict
    // (it would need to be passed separately). The stream data contains
    // the binary vertex data. Since we only get the dict here, we produce
    // a fallback for mesh shadings.
    //
    // A full implementation would:
    // 1. Parse the binary stream using BitsPerCoordinate, BitsPerComponent, BitsPerFlag
    // 2. Decode vertices using the Decode array
    // 3. Build triangles (Type 4: flag-based, Type 5: lattice rows x cols)
    // 4. Rasterize each triangle with barycentric color interpolation

    // Fallback: render the bounding box with a neutral color
    render_mesh_fallback(pixmap, ctm, page_transform, clip, cs_name, &decode);
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
) {
    let cs_name = dict
        .get(b"ColorSpace")
        .and_then(|o| o.as_name())
        .unwrap_or(b"DeviceRGB");

    let decode = dict
        .get_array(b"Decode")
        .map(|a| a.iter().filter_map(|o| o.as_f64()).collect::<Vec<_>>())
        .unwrap_or_default();

    // Same situation as Gouraud: we need the stream data for full parsing.
    // Coons patches have 12 control points + 4 corner colors.
    // Tensor-product patches have 16 control points + 4 corner colors.
    //
    // A full implementation would:
    // 1. Parse binary stream for each patch
    // 2. Subdivide Bézier patches into triangles (recursive de Casteljau)
    // 3. Rasterize triangles with interpolated colors

    render_mesh_fallback(pixmap, ctm, page_transform, clip, cs_name, &decode);
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
