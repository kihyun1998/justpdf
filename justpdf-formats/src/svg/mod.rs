//! SVG format support.
//!
//! Parses SVG XML using `roxmltree`, extracts text, and renders basic shapes
//! using tiny-skia (via image-based PDF conversion).

use std::path::Path;

use crate::common::{FormatDocument, FormatMetadata, FormatPage, RenderedPage};
use crate::error::FormatError;
use crate::Result;

/// A parsed SVG document.
pub struct SvgDocument {
    /// Raw SVG source.
    source: String,
    /// Document width in points.
    width: f64,
    /// Document height in points.
    height: f64,
    /// Extracted text content.
    text_content: String,
    /// Parsed elements for rendering.
    elements: Vec<SvgElement>,
    /// Document title from `<title>` element.
    title: Option<String>,
}

/// An SVG element we know how to render.
#[derive(Debug, Clone)]
enum SvgElement {
    Rect {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        style: ElementStyle,
    },
    Circle {
        cx: f64,
        cy: f64,
        r: f64,
        style: ElementStyle,
    },
    Ellipse {
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
        style: ElementStyle,
    },
    Line {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        style: ElementStyle,
    },
    Path {
        commands: Vec<PathCommand>,
        style: ElementStyle,
    },
    Polygon {
        points: Vec<(f64, f64)>,
        style: ElementStyle,
    },
    Polyline {
        points: Vec<(f64, f64)>,
        style: ElementStyle,
    },
    Text {
        x: f64,
        y: f64,
        content: String,
        style: ElementStyle,
    },
}

#[derive(Debug, Clone)]
struct ElementStyle {
    fill: Option<(u8, u8, u8, u8)>,
    stroke: Option<(u8, u8, u8, u8)>,
    stroke_width: f64,
    opacity: f64,
    transform: Transform,
}

impl Default for ElementStyle {
    fn default() -> Self {
        Self {
            fill: Some((0, 0, 0, 255)),
            stroke: None,
            stroke_width: 1.0,
            opacity: 1.0,
            transform: Transform::identity(),
        }
    }
}

/// Affine transform matrix.
#[derive(Debug, Clone, Copy)]
struct Transform {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    e: f64,
    f: f64,
}

impl Transform {
    fn identity() -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            e: 0.0,
            f: 0.0,
        }
    }

    fn translate(tx: f64, ty: f64) -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            e: tx,
            f: ty,
        }
    }

    fn scale(sx: f64, sy: f64) -> Self {
        Self {
            a: sx,
            b: 0.0,
            c: 0.0,
            d: sy,
            e: 0.0,
            f: 0.0,
        }
    }

    fn rotate(angle_deg: f64) -> Self {
        let r = angle_deg.to_radians();
        let cos = r.cos();
        let sin = r.sin();
        Self {
            a: cos,
            b: sin,
            c: -sin,
            d: cos,
            e: 0.0,
            f: 0.0,
        }
    }

    fn multiply(&self, other: &Transform) -> Transform {
        Transform {
            a: self.a * other.a + self.c * other.b,
            b: self.b * other.a + self.d * other.b,
            c: self.a * other.c + self.c * other.d,
            d: self.b * other.c + self.d * other.d,
            e: self.a * other.e + self.c * other.f + self.e,
            f: self.b * other.e + self.d * other.f + self.f,
        }
    }

    fn apply(&self, x: f64, y: f64) -> (f64, f64) {
        (
            self.a * x + self.c * y + self.e,
            self.b * x + self.d * y + self.f,
        )
    }
}

/// SVG path commands.
#[derive(Debug, Clone)]
enum PathCommand {
    MoveTo(f64, f64),
    LineTo(f64, f64),
    CurveTo(f64, f64, f64, f64, f64, f64),
    QuadTo(f64, f64, f64, f64),
    HorizTo(f64),
    VertTo(f64),
    Close,
}

impl SvgDocument {
    /// Open an SVG file.
    pub fn open(path: &Path) -> Result<Self> {
        let data = std::fs::read_to_string(path)?;
        Self::from_string(&data)
    }

    /// Parse SVG from bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        let text = std::str::from_utf8(data)
            .map_err(|e| FormatError::Format {
                detail: format!("SVG is not valid UTF-8: {e}"),
            })?;
        Self::from_string(text)
    }

    /// Parse SVG from a string.
    pub fn from_string(source: &str) -> Result<Self> {
        let doc = roxmltree::Document::parse(source)
            .map_err(|e| FormatError::Xml(format!("{e}")))?;

        let root = doc.root_element();
        if root.tag_name().name() != "svg" {
            return Err(FormatError::Format {
                detail: "root element is not <svg>".into(),
            });
        }

        // Parse dimensions from viewBox or width/height attributes
        let (width, height) = parse_svg_dimensions(&root)?;

        // Extract title
        let title = root
            .descendants()
            .find(|n| n.tag_name().name() == "title")
            .and_then(|n| n.text())
            .map(|s| s.to_string());

        // Parse elements and collect text
        let mut elements = Vec::new();
        let mut text_parts = Vec::new();
        parse_node(&root, &Transform::identity(), &mut elements, &mut text_parts);
        let text_content = text_parts.join("\n");

        Ok(Self {
            source: source.to_string(),
            width,
            height,
            text_content,
            elements,
            title,
        })
    }

    /// Render the SVG to RGBA pixels at the given scale.
    fn render_rgba(&self, scale: f64) -> Result<(Vec<u8>, u32, u32)> {
        let w = (self.width * scale).ceil() as u32;
        let h = (self.height * scale).ceil() as u32;

        if w == 0 || h == 0 {
            return Err(FormatError::Format {
                detail: "SVG has zero dimensions".into(),
            });
        }

        // Create white background RGBA buffer
        let mut pixels = vec![255u8; (w * h * 4) as usize];

        // Simple software rasterizer — we draw filled rectangles, circles, etc.
        for elem in &self.elements {
            render_element(elem, &mut pixels, w, h, scale);
        }

        Ok((pixels, w, h))
    }
}

fn parse_svg_dimensions(
    root: &roxmltree::Node<'_, '_>,
) -> Result<(f64, f64)> {
    // Try viewBox first
    if let Some(vb) = root.attribute("viewBox") {
        let parts: Vec<f64> = vb
            .split(|c: char| c == ' ' || c == ',')
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse().ok())
            .collect();
        if parts.len() == 4 {
            return Ok((parts[2], parts[3]));
        }
    }

    // Fall back to width/height attributes
    let w = root
        .attribute("width")
        .and_then(|s| parse_length(s))
        .unwrap_or(300.0);
    let h = root
        .attribute("height")
        .and_then(|s| parse_length(s))
        .unwrap_or(150.0);

    Ok((w, h))
}

/// Parse a CSS length value (e.g., "100", "100px", "72pt").
fn parse_length(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.ends_with("px") {
        s[..s.len() - 2].trim().parse().ok()
    } else if s.ends_with("pt") {
        s[..s.len() - 2].trim().parse().ok()
    } else if s.ends_with("in") {
        s[..s.len() - 2].trim().parse::<f64>().ok().map(|v| v * 72.0)
    } else if s.ends_with("mm") {
        s[..s.len() - 2]
            .trim()
            .parse::<f64>()
            .ok()
            .map(|v| v * 72.0 / 25.4)
    } else if s.ends_with("cm") {
        s[..s.len() - 2]
            .trim()
            .parse::<f64>()
            .ok()
            .map(|v| v * 72.0 / 2.54)
    } else if s.ends_with('%') {
        None // percentages need parent context, skip
    } else {
        s.parse().ok()
    }
}

/// Parse style attributes from an element node.
fn parse_style(node: &roxmltree::Node<'_, '_>) -> ElementStyle {
    let mut style = ElementStyle::default();

    if let Some(fill) = node.attribute("fill") {
        style.fill = parse_color(fill);
    }
    if let Some(stroke) = node.attribute("stroke") {
        style.stroke = parse_color(stroke);
    }
    if let Some(sw) = node.attribute("stroke-width") {
        if let Ok(v) = sw.parse::<f64>() {
            style.stroke_width = v;
        }
    }
    if let Some(op) = node.attribute("opacity") {
        if let Ok(v) = op.parse::<f64>() {
            style.opacity = v.clamp(0.0, 1.0);
        }
    }

    // Parse inline style attribute
    if let Some(css) = node.attribute("style") {
        for decl in css.split(';') {
            let decl = decl.trim();
            if let Some((prop, val)) = decl.split_once(':') {
                let prop = prop.trim();
                let val = val.trim();
                match prop {
                    "fill" => style.fill = parse_color(val),
                    "stroke" => style.stroke = parse_color(val),
                    "stroke-width" => {
                        if let Ok(v) = val.parse::<f64>() {
                            style.stroke_width = v;
                        }
                    }
                    "opacity" => {
                        if let Ok(v) = val.parse::<f64>() {
                            style.opacity = v.clamp(0.0, 1.0);
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    // Parse transform
    if let Some(t) = node.attribute("transform") {
        style.transform = parse_transform(t);
    }

    style
}

/// Parse a CSS color string into RGBA.
fn parse_color(s: &str) -> Option<(u8, u8, u8, u8)> {
    let s = s.trim();
    if s == "none" || s == "transparent" {
        return None;
    }
    if s.starts_with('#') {
        let hex = &s[1..];
        return match hex.len() {
            3 => {
                let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
                let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
                let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
                Some((r, g, b, 255))
            }
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                Some((r, g, b, 255))
            }
            _ => None,
        };
    }
    if let Some(rest) = s.strip_prefix("rgb(").and_then(|s| s.strip_suffix(')')) {
        let parts: Vec<&str> = rest.split(',').collect();
        if parts.len() == 3 {
            let r = parts[0].trim().parse().ok()?;
            let g = parts[1].trim().parse().ok()?;
            let b = parts[2].trim().parse().ok()?;
            return Some((r, g, b, 255));
        }
    }
    // Named colors (common subset)
    match s {
        "black" => Some((0, 0, 0, 255)),
        "white" => Some((255, 255, 255, 255)),
        "red" => Some((255, 0, 0, 255)),
        "green" => Some((0, 128, 0, 255)),
        "blue" => Some((0, 0, 255, 255)),
        "yellow" => Some((255, 255, 0, 255)),
        "orange" => Some((255, 165, 0, 255)),
        "purple" => Some((128, 0, 128, 255)),
        "gray" | "grey" => Some((128, 128, 128, 255)),
        "cyan" => Some((0, 255, 255, 255)),
        "magenta" => Some((255, 0, 255, 255)),
        _ => Some((0, 0, 0, 255)), // default to black for unknown
    }
}

/// Parse a transform attribute.
fn parse_transform(s: &str) -> Transform {
    let s = s.trim();
    let mut result = Transform::identity();

    // Simple parser: handle translate(), scale(), rotate(), matrix()
    let mut remaining = s;
    while !remaining.is_empty() {
        remaining = remaining.trim_start();
        if let Some(rest) = remaining.strip_prefix("translate(") {
            if let Some(end) = rest.find(')') {
                let args = &rest[..end];
                let vals: Vec<f64> = args
                    .split(|c: char| c == ',' || c == ' ')
                    .filter(|s| !s.is_empty())
                    .filter_map(|s| s.parse().ok())
                    .collect();
                let tx = vals.first().copied().unwrap_or(0.0);
                let ty = vals.get(1).copied().unwrap_or(0.0);
                result = result.multiply(&Transform::translate(tx, ty));
                remaining = &rest[end + 1..];
            } else {
                break;
            }
        } else if let Some(rest) = remaining.strip_prefix("scale(") {
            if let Some(end) = rest.find(')') {
                let args = &rest[..end];
                let vals: Vec<f64> = args
                    .split(|c: char| c == ',' || c == ' ')
                    .filter(|s| !s.is_empty())
                    .filter_map(|s| s.parse().ok())
                    .collect();
                let sx = vals.first().copied().unwrap_or(1.0);
                let sy = vals.get(1).copied().unwrap_or(sx);
                result = result.multiply(&Transform::scale(sx, sy));
                remaining = &rest[end + 1..];
            } else {
                break;
            }
        } else if let Some(rest) = remaining.strip_prefix("rotate(") {
            if let Some(end) = rest.find(')') {
                let args = &rest[..end];
                let vals: Vec<f64> = args
                    .split(|c: char| c == ',' || c == ' ')
                    .filter(|s| !s.is_empty())
                    .filter_map(|s| s.parse().ok())
                    .collect();
                let angle = vals.first().copied().unwrap_or(0.0);
                if vals.len() >= 3 {
                    let cx = vals[1];
                    let cy = vals[2];
                    result = result.multiply(&Transform::translate(cx, cy));
                    result = result.multiply(&Transform::rotate(angle));
                    result = result.multiply(&Transform::translate(-cx, -cy));
                } else {
                    result = result.multiply(&Transform::rotate(angle));
                }
                remaining = &rest[end + 1..];
            } else {
                break;
            }
        } else if let Some(rest) = remaining.strip_prefix("matrix(") {
            if let Some(end) = rest.find(')') {
                let args = &rest[..end];
                let vals: Vec<f64> = args
                    .split(|c: char| c == ',' || c == ' ')
                    .filter(|s| !s.is_empty())
                    .filter_map(|s| s.parse().ok())
                    .collect();
                if vals.len() == 6 {
                    result = result.multiply(&Transform {
                        a: vals[0],
                        b: vals[1],
                        c: vals[2],
                        d: vals[3],
                        e: vals[4],
                        f: vals[5],
                    });
                }
                remaining = &rest[end + 1..];
            } else {
                break;
            }
        } else {
            // Skip unknown content
            break;
        }
    }

    result
}

/// Recursively parse SVG nodes into elements.
fn parse_node(
    node: &roxmltree::Node<'_, '_>,
    parent_transform: &Transform,
    elements: &mut Vec<SvgElement>,
    text_parts: &mut Vec<String>,
) {
    if !node.is_element() {
        return;
    }

    let tag = node.tag_name().name();
    let style = parse_style(node);
    let current_transform = parent_transform.multiply(&style.transform);
    let mut elem_style = style;
    elem_style.transform = current_transform;

    match tag {
        "rect" => {
            let x = attr_f64(node, "x");
            let y = attr_f64(node, "y");
            let w = attr_f64(node, "width");
            let h = attr_f64(node, "height");
            if w > 0.0 && h > 0.0 {
                elements.push(SvgElement::Rect {
                    x,
                    y,
                    width: w,
                    height: h,
                    style: elem_style.clone(),
                });
            }
        }
        "circle" => {
            let cx = attr_f64(node, "cx");
            let cy = attr_f64(node, "cy");
            let r = attr_f64(node, "r");
            if r > 0.0 {
                elements.push(SvgElement::Circle {
                    cx,
                    cy,
                    r,
                    style: elem_style.clone(),
                });
            }
        }
        "ellipse" => {
            let cx = attr_f64(node, "cx");
            let cy = attr_f64(node, "cy");
            let rx = attr_f64(node, "rx");
            let ry = attr_f64(node, "ry");
            if rx > 0.0 && ry > 0.0 {
                elements.push(SvgElement::Ellipse {
                    cx,
                    cy,
                    rx,
                    ry,
                    style: elem_style.clone(),
                });
            }
        }
        "line" => {
            let x1 = attr_f64(node, "x1");
            let y1 = attr_f64(node, "y1");
            let x2 = attr_f64(node, "x2");
            let y2 = attr_f64(node, "y2");
            elements.push(SvgElement::Line {
                x1,
                y1,
                x2,
                y2,
                style: elem_style.clone(),
            });
        }
        "path" => {
            if let Some(d) = node.attribute("d") {
                let commands = parse_path_data(d);
                if !commands.is_empty() {
                    elements.push(SvgElement::Path {
                        commands,
                        style: elem_style.clone(),
                    });
                }
            }
        }
        "polygon" => {
            if let Some(pts) = node.attribute("points") {
                let points = parse_points(pts);
                if points.len() >= 2 {
                    elements.push(SvgElement::Polygon {
                        points,
                        style: elem_style.clone(),
                    });
                }
            }
        }
        "polyline" => {
            if let Some(pts) = node.attribute("points") {
                let points = parse_points(pts);
                if points.len() >= 2 {
                    elements.push(SvgElement::Polyline {
                        points,
                        style: elem_style.clone(),
                    });
                }
            }
        }
        "text" | "tspan" => {
            let x = attr_f64(node, "x");
            let y = attr_f64(node, "y");
            // Collect all text content from this element and its descendants
            let content = collect_text_content(node);
            if !content.is_empty() {
                text_parts.push(content.clone());
                elements.push(SvgElement::Text {
                    x,
                    y,
                    content,
                    style: elem_style.clone(),
                });
            }
            // Don't recurse into text children since we already collected them
            return;
        }
        "g" | "svg" => {
            // Group element: recurse with accumulated transform
            for child in node.children() {
                parse_node(&child, &current_transform, elements, text_parts);
            }
            return;
        }
        _ => {}
    }

    // Recurse for non-group elements that may have children
    for child in node.children() {
        parse_node(&child, &current_transform, elements, text_parts);
    }
}

/// Collect all text content from a node and its descendants.
fn collect_text_content(node: &roxmltree::Node<'_, '_>) -> String {
    let mut result = String::new();
    for child in node.children() {
        if child.is_text() {
            if let Some(t) = child.text() {
                result.push_str(t.trim());
            }
        } else if child.is_element() {
            result.push_str(&collect_text_content(&child));
        }
    }
    result
}

fn attr_f64(node: &roxmltree::Node<'_, '_>, name: &str) -> f64 {
    node.attribute(name)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.0)
}

/// Parse SVG points attribute "x1,y1 x2,y2 ..."
fn parse_points(s: &str) -> Vec<(f64, f64)> {
    let nums: Vec<f64> = s
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse().ok())
        .collect();
    nums.chunks(2)
        .filter(|c| c.len() == 2)
        .map(|c| (c[0], c[1]))
        .collect()
}

/// Parse SVG path `d` attribute data.
fn parse_path_data(d: &str) -> Vec<PathCommand> {
    let mut commands = Vec::new();
    let mut chars = d.chars().peekable();
    let mut current_x: f64 = 0.0;
    let mut current_y: f64 = 0.0;

    while chars.peek().is_some() {
        // Skip whitespace and commas
        while let Some(&c) = chars.peek() {
            if c.is_whitespace() || c == ',' {
                chars.next();
            } else {
                break;
            }
        }

        let Some(&cmd_char) = chars.peek() else {
            break;
        };

        if !cmd_char.is_ascii_alphabetic() {
            break;
        }
        chars.next();
        let is_relative = cmd_char.is_ascii_lowercase();

        match cmd_char.to_ascii_uppercase() {
            'M' => {
                if let Some((x, y)) = read_coord_pair(&mut chars) {
                    let (ax, ay) = if is_relative {
                        (current_x + x, current_y + y)
                    } else {
                        (x, y)
                    };
                    current_x = ax;
                    current_y = ay;
                    commands.push(PathCommand::MoveTo(ax, ay));
                    // Subsequent coordinate pairs are implicit LineTo
                    while let Some((x2, y2)) = try_read_coord_pair(&mut chars) {
                        let (ax2, ay2) = if is_relative {
                            (current_x + x2, current_y + y2)
                        } else {
                            (x2, y2)
                        };
                        current_x = ax2;
                        current_y = ay2;
                        commands.push(PathCommand::LineTo(ax2, ay2));
                    }
                }
            }
            'L' => {
                while let Some((x, y)) = try_read_coord_pair(&mut chars) {
                    let (ax, ay) = if is_relative {
                        (current_x + x, current_y + y)
                    } else {
                        (x, y)
                    };
                    current_x = ax;
                    current_y = ay;
                    commands.push(PathCommand::LineTo(ax, ay));
                }
            }
            'H' => {
                while let Some(x) = try_read_number(&mut chars) {
                    let ax = if is_relative { current_x + x } else { x };
                    current_x = ax;
                    commands.push(PathCommand::HorizTo(ax));
                }
            }
            'V' => {
                while let Some(y) = try_read_number(&mut chars) {
                    let ay = if is_relative { current_y + y } else { y };
                    current_y = ay;
                    commands.push(PathCommand::VertTo(ay));
                }
            }
            'C' => {
                while let Some(coords) = try_read_n_numbers(&mut chars, 6) {
                    let (ox, oy) = if is_relative {
                        (current_x, current_y)
                    } else {
                        (0.0, 0.0)
                    };
                    let x1 = coords[0] + ox;
                    let y1 = coords[1] + oy;
                    let x2 = coords[2] + ox;
                    let y2 = coords[3] + oy;
                    let x = coords[4] + ox;
                    let y = coords[5] + oy;
                    current_x = x;
                    current_y = y;
                    commands.push(PathCommand::CurveTo(x1, y1, x2, y2, x, y));
                }
            }
            'Q' => {
                while let Some(coords) = try_read_n_numbers(&mut chars, 4) {
                    let (ox, oy) = if is_relative {
                        (current_x, current_y)
                    } else {
                        (0.0, 0.0)
                    };
                    let x1 = coords[0] + ox;
                    let y1 = coords[1] + oy;
                    let x = coords[2] + ox;
                    let y = coords[3] + oy;
                    current_x = x;
                    current_y = y;
                    commands.push(PathCommand::QuadTo(x1, y1, x, y));
                }
            }
            'Z' => {
                commands.push(PathCommand::Close);
            }
            _ => {
                // Skip unknown commands
            }
        }
    }

    commands
}

fn skip_whitespace_and_commas(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() || c == ',' {
            chars.next();
        } else {
            break;
        }
    }
}

fn try_read_number(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Option<f64> {
    skip_whitespace_and_commas(chars);
    // Peek to see if we have a number
    let &c = chars.peek()?;
    if !c.is_ascii_digit() && c != '-' && c != '+' && c != '.' {
        return None;
    }
    let mut num = String::new();
    if c == '-' || c == '+' {
        num.push(c);
        chars.next();
    }
    let mut has_dot = false;
    while let Some(&c) = chars.peek() {
        if c.is_ascii_digit() {
            num.push(c);
            chars.next();
        } else if c == '.' && !has_dot {
            has_dot = true;
            num.push(c);
            chars.next();
        } else if c == 'e' || c == 'E' {
            num.push(c);
            chars.next();
            if let Some(&sign) = chars.peek() {
                if sign == '+' || sign == '-' {
                    num.push(sign);
                    chars.next();
                }
            }
        } else {
            break;
        }
    }
    num.parse().ok()
}

fn read_coord_pair(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
) -> Option<(f64, f64)> {
    let x = try_read_number(chars)?;
    let y = try_read_number(chars)?;
    Some((x, y))
}

fn try_read_coord_pair(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
) -> Option<(f64, f64)> {
    skip_whitespace_and_commas(chars);
    let &c = chars.peek()?;
    if c.is_ascii_alphabetic() {
        return None;
    }
    read_coord_pair(chars)
}

fn try_read_n_numbers(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    n: usize,
) -> Option<Vec<f64>> {
    skip_whitespace_and_commas(chars);
    let &c = chars.peek()?;
    if c.is_ascii_alphabetic() {
        return None;
    }
    let mut nums = Vec::with_capacity(n);
    for _ in 0..n {
        nums.push(try_read_number(chars)?);
    }
    Some(nums)
}

/// Simple pixel-level rendering of an SVG element.
fn render_element(elem: &SvgElement, pixels: &mut [u8], w: u32, h: u32, scale: f64) {
    match elem {
        SvgElement::Rect {
            x,
            y,
            width,
            height,
            style,
        } => {
            if let Some(fill) = style.fill {
                let (tx, ty) = style.transform.apply(*x * scale, *y * scale);
                let tw = *width * scale * style.transform.a.abs();
                let th = *height * scale * style.transform.d.abs();
                fill_rect_pixels(
                    pixels,
                    w,
                    h,
                    tx as i32,
                    ty as i32,
                    tw as i32,
                    th as i32,
                    fill,
                    style.opacity,
                );
            }
        }
        SvgElement::Circle { cx, cy, r, style } => {
            if let Some(fill) = style.fill {
                let (tcx, tcy) = style.transform.apply(*cx * scale, *cy * scale);
                let tr = *r * scale;
                fill_ellipse_pixels(
                    pixels,
                    w,
                    h,
                    tcx,
                    tcy,
                    tr,
                    tr,
                    fill,
                    style.opacity,
                );
            }
        }
        SvgElement::Ellipse {
            cx,
            cy,
            rx,
            ry,
            style,
        } => {
            if let Some(fill) = style.fill {
                let (tcx, tcy) = style.transform.apply(*cx * scale, *cy * scale);
                let trx = *rx * scale;
                let try_ = *ry * scale;
                fill_ellipse_pixels(
                    pixels,
                    w,
                    h,
                    tcx,
                    tcy,
                    trx,
                    try_,
                    fill,
                    style.opacity,
                );
            }
        }
        SvgElement::Line {
            x1,
            y1,
            x2,
            y2,
            style,
        } => {
            let color = style.stroke.unwrap_or((0, 0, 0, 255));
            let (tx1, ty1) = style.transform.apply(*x1 * scale, *y1 * scale);
            let (tx2, ty2) = style.transform.apply(*x2 * scale, *y2 * scale);
            draw_line_pixels(
                pixels,
                w,
                h,
                tx1 as i32,
                ty1 as i32,
                tx2 as i32,
                ty2 as i32,
                color,
                style.opacity,
            );
        }
        SvgElement::Polygon { points, style } => {
            if let Some(fill) = style.fill {
                let transformed: Vec<(f64, f64)> = points
                    .iter()
                    .map(|(x, y)| style.transform.apply(*x * scale, *y * scale))
                    .collect();
                fill_polygon_pixels(pixels, w, h, &transformed, fill, style.opacity);
            }
        }
        SvgElement::Polyline { points, style } => {
            let color = style.stroke.unwrap_or((0, 0, 0, 255));
            for pair in points.windows(2) {
                let (tx1, ty1) = style.transform.apply(pair[0].0 * scale, pair[0].1 * scale);
                let (tx2, ty2) = style.transform.apply(pair[1].0 * scale, pair[1].1 * scale);
                draw_line_pixels(
                    pixels,
                    w,
                    h,
                    tx1 as i32,
                    ty1 as i32,
                    tx2 as i32,
                    ty2 as i32,
                    color,
                    style.opacity,
                );
            }
        }
        SvgElement::Path {
            commands, style, ..
        } => {
            // Simplified path rendering: just draw line segments
            let color = style.stroke.unwrap_or_else(|| style.fill.unwrap_or((0, 0, 0, 255)));
            let mut cx = 0.0f64;
            let mut cy = 0.0f64;
            for cmd in commands {
                match cmd {
                    PathCommand::MoveTo(x, y) => {
                        cx = *x;
                        cy = *y;
                    }
                    PathCommand::LineTo(x, y) => {
                        let (tx1, ty1) =
                            style.transform.apply(cx * scale, cy * scale);
                        let (tx2, ty2) =
                            style.transform.apply(*x * scale, *y * scale);
                        draw_line_pixels(
                            pixels,
                            w,
                            h,
                            tx1 as i32,
                            ty1 as i32,
                            tx2 as i32,
                            ty2 as i32,
                            color,
                            style.opacity,
                        );
                        cx = *x;
                        cy = *y;
                    }
                    PathCommand::HorizTo(x) => {
                        let (tx1, ty1) =
                            style.transform.apply(cx * scale, cy * scale);
                        let (tx2, ty2) =
                            style.transform.apply(*x * scale, cy * scale);
                        draw_line_pixels(
                            pixels,
                            w,
                            h,
                            tx1 as i32,
                            ty1 as i32,
                            tx2 as i32,
                            ty2 as i32,
                            color,
                            style.opacity,
                        );
                        cx = *x;
                    }
                    PathCommand::VertTo(y) => {
                        let (tx1, ty1) =
                            style.transform.apply(cx * scale, cy * scale);
                        let (tx2, ty2) =
                            style.transform.apply(cx * scale, *y * scale);
                        draw_line_pixels(
                            pixels,
                            w,
                            h,
                            tx1 as i32,
                            ty1 as i32,
                            tx2 as i32,
                            ty2 as i32,
                            color,
                            style.opacity,
                        );
                        cy = *y;
                    }
                    PathCommand::CurveTo(_x1, _y1, _x2, _y2, x, y) => {
                        // Simplified: draw straight line to endpoint
                        let (tx1, ty1) =
                            style.transform.apply(cx * scale, cy * scale);
                        let (tx2, ty2) =
                            style.transform.apply(*x * scale, *y * scale);
                        draw_line_pixels(
                            pixels,
                            w,
                            h,
                            tx1 as i32,
                            ty1 as i32,
                            tx2 as i32,
                            ty2 as i32,
                            color,
                            style.opacity,
                        );
                        cx = *x;
                        cy = *y;
                    }
                    PathCommand::QuadTo(_x1, _y1, x, y) => {
                        let (tx1, ty1) =
                            style.transform.apply(cx * scale, cy * scale);
                        let (tx2, ty2) =
                            style.transform.apply(*x * scale, *y * scale);
                        draw_line_pixels(
                            pixels,
                            w,
                            h,
                            tx1 as i32,
                            ty1 as i32,
                            tx2 as i32,
                            ty2 as i32,
                            color,
                            style.opacity,
                        );
                        cx = *x;
                        cy = *y;
                    }
                    PathCommand::Close => {}
                }
            }
        }
        SvgElement::Text { x, y, style, .. } => {
            // Text rendering is complex; we just place a small marker
            if let Some(fill) = style.fill {
                let (tx, ty) = style.transform.apply(*x * scale, *y * scale);
                fill_rect_pixels(
                    pixels,
                    w,
                    h,
                    tx as i32,
                    (ty - 8.0) as i32,
                    4,
                    8,
                    fill,
                    style.opacity,
                );
            }
        }
    }
}

/// Fill a rectangle in the pixel buffer.
fn fill_rect_pixels(
    pixels: &mut [u8],
    buf_w: u32,
    buf_h: u32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    color: (u8, u8, u8, u8),
    opacity: f64,
) {
    let alpha = (color.3 as f64 * opacity) as u8;
    for py in y.max(0)..(y + h).min(buf_h as i32) {
        for px in x.max(0)..(x + w).min(buf_w as i32) {
            let idx = ((py as u32 * buf_w + px as u32) * 4) as usize;
            if idx + 3 < pixels.len() {
                blend_pixel(pixels, idx, color.0, color.1, color.2, alpha);
            }
        }
    }
}

/// Fill an ellipse in the pixel buffer.
fn fill_ellipse_pixels(
    pixels: &mut [u8],
    buf_w: u32,
    buf_h: u32,
    cx: f64,
    cy: f64,
    rx: f64,
    ry: f64,
    color: (u8, u8, u8, u8),
    opacity: f64,
) {
    let alpha = (color.3 as f64 * opacity) as u8;
    let min_x = (cx - rx).floor() as i32;
    let max_x = (cx + rx).ceil() as i32;
    let min_y = (cy - ry).floor() as i32;
    let max_y = (cy + ry).ceil() as i32;

    for py in min_y.max(0)..max_y.min(buf_h as i32) {
        for px in min_x.max(0)..max_x.min(buf_w as i32) {
            let dx = (px as f64 - cx) / rx;
            let dy = (py as f64 - cy) / ry;
            if dx * dx + dy * dy <= 1.0 {
                let idx = ((py as u32 * buf_w + px as u32) * 4) as usize;
                if idx + 3 < pixels.len() {
                    blend_pixel(pixels, idx, color.0, color.1, color.2, alpha);
                }
            }
        }
    }
}

/// Draw a line using Bresenham's algorithm.
fn draw_line_pixels(
    pixels: &mut [u8],
    buf_w: u32,
    buf_h: u32,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    color: (u8, u8, u8, u8),
    opacity: f64,
) {
    let alpha = (color.3 as f64 * opacity) as u8;
    let mut x0 = x0;
    let mut y0 = y0;
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    loop {
        if x0 >= 0 && x0 < buf_w as i32 && y0 >= 0 && y0 < buf_h as i32 {
            let idx = ((y0 as u32 * buf_w + x0 as u32) * 4) as usize;
            if idx + 3 < pixels.len() {
                blend_pixel(pixels, idx, color.0, color.1, color.2, alpha);
            }
        }
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

/// Simple scanline polygon fill.
fn fill_polygon_pixels(
    pixels: &mut [u8],
    buf_w: u32,
    buf_h: u32,
    points: &[(f64, f64)],
    color: (u8, u8, u8, u8),
    opacity: f64,
) {
    if points.len() < 3 {
        return;
    }
    let alpha = (color.3 as f64 * opacity) as u8;

    let min_y = points
        .iter()
        .map(|(_, y)| *y)
        .fold(f64::INFINITY, f64::min)
        .floor() as i32;
    let max_y = points
        .iter()
        .map(|(_, y)| *y)
        .fold(f64::NEG_INFINITY, f64::max)
        .ceil() as i32;

    for y in min_y.max(0)..max_y.min(buf_h as i32) {
        let yf = y as f64 + 0.5;
        let mut intersections = Vec::new();
        let n = points.len();
        for i in 0..n {
            let (x0, y0) = points[i];
            let (x1, y1) = points[(i + 1) % n];
            if (y0 <= yf && y1 > yf) || (y1 <= yf && y0 > yf) {
                let t = (yf - y0) / (y1 - y0);
                intersections.push(x0 + t * (x1 - x0));
            }
        }
        intersections.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        for pair in intersections.chunks(2) {
            if pair.len() == 2 {
                let x_start = pair[0].ceil() as i32;
                let x_end = pair[1].floor() as i32;
                for px in x_start.max(0)..=x_end.min(buf_w as i32 - 1) {
                    let idx = ((y as u32 * buf_w + px as u32) * 4) as usize;
                    if idx + 3 < pixels.len() {
                        blend_pixel(pixels, idx, color.0, color.1, color.2, alpha);
                    }
                }
            }
        }
    }
}

/// Alpha blend a pixel.
fn blend_pixel(pixels: &mut [u8], idx: usize, r: u8, g: u8, b: u8, a: u8) {
    if a == 255 {
        pixels[idx] = r;
        pixels[idx + 1] = g;
        pixels[idx + 2] = b;
        pixels[idx + 3] = 255;
    } else if a > 0 {
        let alpha = a as f64 / 255.0;
        let inv = 1.0 - alpha;
        pixels[idx] = (r as f64 * alpha + pixels[idx] as f64 * inv) as u8;
        pixels[idx + 1] = (g as f64 * alpha + pixels[idx + 1] as f64 * inv) as u8;
        pixels[idx + 2] = (b as f64 * alpha + pixels[idx + 2] as f64 * inv) as u8;
        pixels[idx + 3] = 255;
    }
}

impl FormatDocument for SvgDocument {
    fn metadata(&self) -> FormatMetadata {
        FormatMetadata {
            title: self.title.clone(),
            author: None,
            subject: None,
            creator: Some("justpdf-formats/svg".to_string()),
            page_count: 1,
        }
    }

    fn page_count(&self) -> usize {
        1
    }

    fn page(&self, index: usize) -> Result<FormatPage> {
        if index != 0 {
            return Err(FormatError::OutOfRange {
                index,
                count: 1,
            });
        }
        Ok(FormatPage {
            index: 0,
            width_pt: self.width,
            height_pt: self.height,
        })
    }

    fn page_text(&self, index: usize) -> Result<String> {
        if index != 0 {
            return Err(FormatError::OutOfRange {
                index,
                count: 1,
            });
        }
        Ok(self.text_content.clone())
    }

    fn render_page(&self, index: usize, dpi: f64) -> Result<RenderedPage> {
        if index != 0 {
            return Err(FormatError::OutOfRange {
                index,
                count: 1,
            });
        }
        let scale = dpi / 72.0;
        let (data, width, height) = self.render_rgba(scale)?;
        Ok(RenderedPage {
            data,
            width,
            height,
        })
    }

    fn render_page_png(&self, index: usize, dpi: f64) -> Result<Vec<u8>> {
        let rendered = self.render_page(index, dpi)?;
        let mut buf = Vec::new();
        let encoder = image::codecs::png::PngEncoder::new(&mut buf);
        image::ImageEncoder::write_image(
            encoder,
            &rendered.data,
            rendered.width,
            rendered.height,
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|e| FormatError::Format {
            detail: format!("PNG encode: {e}"),
        })?;
        Ok(buf)
    }

    fn to_pdf(&self) -> Result<Vec<u8>> {
        // Render to image then embed in PDF
        let rendered = self.render_page(0, 72.0)?;
        let mut builder = justpdf_core::writer::DocumentBuilder::new();

        let w = self.width;
        let h = self.height;
        let mut page = justpdf_core::writer::PageBuilder::new(w, h);

        // Convert RGBA to RGB for inline image
        let rgb_data: Vec<u8> = rendered
            .data
            .chunks(4)
            .flat_map(|px| [px[0], px[1], px[2]])
            .collect();

        page.draw_inline_image(rendered.width, rendered.height, 8, "DeviceRGB", &rgb_data);
        builder.add_page(page);
        Ok(builder.build()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_svg() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100">
            <rect x="10" y="10" width="80" height="40" fill="red"/>
            <circle cx="150" cy="50" r="30" fill="blue"/>
            <text x="50" y="80">Hello SVG</text>
        </svg>"#;

        let doc = SvgDocument::from_string(svg).unwrap();
        assert_eq!(doc.page_count(), 1);
        assert_eq!(doc.page(0).unwrap().width_pt, 200.0);
        assert_eq!(doc.page(0).unwrap().height_pt, 100.0);

        let text = doc.page_text(0).unwrap();
        assert!(text.contains("Hello SVG"));
    }

    #[test]
    fn test_parse_viewbox() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 400 300"></svg>"#;
        let doc = SvgDocument::from_string(svg).unwrap();
        assert_eq!(doc.page(0).unwrap().width_pt, 400.0);
        assert_eq!(doc.page(0).unwrap().height_pt, 300.0);
    }

    #[test]
    fn test_parse_path_data() {
        let commands = parse_path_data("M 10 20 L 30 40 H 50 V 60 Z");
        assert_eq!(commands.len(), 5);
        assert!(matches!(commands[0], PathCommand::MoveTo(10.0, 20.0)));
        assert!(matches!(commands[1], PathCommand::LineTo(30.0, 40.0)));
        assert!(matches!(commands[4], PathCommand::Close));
    }

    #[test]
    fn test_parse_relative_path() {
        let commands = parse_path_data("m 10 20 l 5 5");
        assert_eq!(commands.len(), 2);
        assert!(matches!(commands[0], PathCommand::MoveTo(10.0, 20.0)));
        assert!(matches!(commands[1], PathCommand::LineTo(15.0, 25.0)));
    }

    #[test]
    fn test_parse_color() {
        assert_eq!(parse_color("#ff0000"), Some((255, 0, 0, 255)));
        assert_eq!(parse_color("#f00"), Some((255, 0, 0, 255)));
        assert_eq!(parse_color("red"), Some((255, 0, 0, 255)));
        assert_eq!(parse_color("none"), None);
        assert_eq!(parse_color("rgb(0, 128, 255)"), Some((0, 128, 255, 255)));
    }

    #[test]
    fn test_parse_transform() {
        let t = parse_transform("translate(10, 20)");
        assert!((t.e - 10.0).abs() < 0.001);
        assert!((t.f - 20.0).abs() < 0.001);

        let t = parse_transform("scale(2)");
        assert!((t.a - 2.0).abs() < 0.001);
        assert!((t.d - 2.0).abs() < 0.001);
    }

    #[test]
    fn test_render_png() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="50" height="50">
            <rect x="0" y="0" width="50" height="50" fill="blue"/>
        </svg>"#;
        let doc = SvgDocument::from_string(svg).unwrap();
        let png = doc.render_page_png(0, 72.0).unwrap();
        assert_eq!(&png[..4], &[0x89, 0x50, 0x4E, 0x47]);
    }

    #[test]
    fn test_to_pdf() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
            <rect x="10" y="10" width="80" height="80" fill="green"/>
        </svg>"#;
        let doc = SvgDocument::from_string(svg).unwrap();
        let pdf = doc.to_pdf().unwrap();
        assert!(pdf.starts_with(b"%PDF"));
    }

    #[test]
    fn test_metadata() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
            <title>Test Image</title>
        </svg>"#;
        let doc = SvgDocument::from_string(svg).unwrap();
        let meta = doc.metadata();
        assert_eq!(meta.title.as_deref(), Some("Test Image"));
        assert_eq!(meta.page_count, 1);
    }

    #[test]
    fn test_page_out_of_range() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"></svg>"#;
        let doc = SvgDocument::from_string(svg).unwrap();
        assert!(doc.page(1).is_err());
        assert!(doc.page_text(1).is_err());
    }

    #[test]
    fn test_not_svg() {
        let result = SvgDocument::from_string("<html></html>");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_points() {
        let pts = parse_points("10,20 30,40 50,60");
        assert_eq!(pts.len(), 3);
        assert_eq!(pts[0], (10.0, 20.0));
        assert_eq!(pts[2], (50.0, 60.0));
    }

    #[test]
    fn test_polygon_element() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
            <polygon points="50,5 95,95 5,95" fill="red"/>
        </svg>"#;
        let doc = SvgDocument::from_string(svg).unwrap();
        assert_eq!(doc.page_count(), 1);
    }

    #[test]
    fn test_text_extraction_nested() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100">
            <text x="10" y="30">Hello <tspan>World</tspan></text>
        </svg>"#;
        let doc = SvgDocument::from_string(svg).unwrap();
        let text = doc.page_text(0).unwrap();
        assert!(text.contains("Hello"));
        assert!(text.contains("World"));
    }
}
