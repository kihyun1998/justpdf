//! ICC profile parsing and color transformation (ICC.1:2004 / ISO 15076-1).
//!
//! Supports matrix/TRC-based RGB profiles, gray profiles, and provides a
//! fallback path for CMYK profiles.  The primary goal is converting ICC-based
//! colors into sRGB for rendering.

use super::cmyk_to_rgb;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Parsed ICC profile header and essential tag data.
#[derive(Debug, Clone, PartialEq)]
pub struct IccProfile {
    /// ICC version (major, minor).
    pub version: (u8, u8),
    /// Profile/device class.
    pub profile_class: ProfileClass,
    /// Data (device) color space.
    pub color_space: IccColorSpace,
    /// Profile Connection Space (PCS), usually XYZ or Lab.
    pub pcs: IccColorSpace,
    /// Number of components in the data color space.
    pub num_components: u32,
    /// Rendering intent stored in the profile header.
    pub rendering_intent: RenderingIntent,

    // Matrix/TRC data (present in matrix-based RGB profiles)
    pub red_matrix_column: Option<[f64; 3]>,
    pub green_matrix_column: Option<[f64; 3]>,
    pub blue_matrix_column: Option<[f64; 3]>,
    pub red_trc: Option<ToneCurve>,
    pub green_trc: Option<ToneCurve>,
    pub blue_trc: Option<ToneCurve>,
    pub gray_trc: Option<ToneCurve>,

    /// Media white point (from 'wtpt' tag, or D50 default).
    pub white_point: [f64; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileClass {
    Input,
    Display,
    Output,
    ColorSpace,
    DeviceLink,
    Abstract,
    NamedColor,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IccColorSpace {
    RGB,
    CMYK,
    Gray,
    Lab,
    XYZ,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderingIntent {
    Perceptual,
    RelativeColorimetric,
    Saturation,
    AbsoluteColorimetric,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ToneCurve {
    /// Simple gamma curve: y = x^gamma
    Gamma(f64),
    /// Parametric curve (ICC type 0-4 parameters).
    Parametric(Vec<f64>),
    /// 1-D lookup table (16-bit entries, normalised to 0..65535).
    Table(Vec<u16>),
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// D50 illuminant XYZ (ICC PCS white point).
const D50: [f64; 3] = [0.9642, 1.0, 0.8249];

/// D65 illuminant XYZ (sRGB white point).
const D65: [f64; 3] = [0.9505, 1.0, 1.0890];

/// XYZ-to-linear-sRGB matrix (from IEC 61966-2-1).
const XYZ_TO_SRGB: [[f64; 3]; 3] = [
    [3.2404542, -1.5371385, -0.4985314],
    [-0.9692660, 1.8760108, 0.0415560],
    [0.0556434, -0.2040259, 1.0572252],
];

/// Bradford chromatic adaptation matrix (M).
const BRADFORD: [[f64; 3]; 3] = [
    [0.8951, 0.2664, -0.1614],
    [-0.7502, 1.7135, 0.0367],
    [0.0389, -0.0685, 1.0296],
];

/// Inverse Bradford matrix (M^-1).
const BRADFORD_INV: [[f64; 3]; 3] = [
    [0.9869929, -0.1470543, 0.1599627],
    [0.4323053, 0.5183603, 0.0492912],
    [-0.0085287, 0.0400428, 0.9684867],
];

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Parse an ICC profile from raw binary data.
///
/// Returns `None` if the data is too short or structurally invalid.
pub fn parse_icc_profile(data: &[u8]) -> Option<IccProfile> {
    if data.len() < 132 {
        return None; // 128 header + at least 4 bytes for tag count
    }

    // --- Header ---
    let _profile_size = u32_be(data, 0);
    let version_major = data[8];
    let version_minor = data[9] >> 4;

    let profile_class = match &data[12..16] {
        b"mntr" => ProfileClass::Display,
        b"scnr" => ProfileClass::Input,
        b"prtr" => ProfileClass::Output,
        b"spac" => ProfileClass::ColorSpace,
        b"link" => ProfileClass::DeviceLink,
        b"abst" => ProfileClass::Abstract,
        b"nmcl" => ProfileClass::NamedColor,
        _ => ProfileClass::Unknown,
    };

    let color_space = parse_color_space_sig(&data[16..20]);
    let pcs = parse_color_space_sig(&data[20..24]);

    let rendering_intent = match u32_be(data, 64) & 0xFFFF {
        0 => RenderingIntent::Perceptual,
        1 => RenderingIntent::RelativeColorimetric,
        2 => RenderingIntent::Saturation,
        3 => RenderingIntent::AbsoluteColorimetric,
        _ => RenderingIntent::Perceptual,
    };

    let num_components = match color_space {
        IccColorSpace::Gray => 1,
        IccColorSpace::RGB | IccColorSpace::Lab | IccColorSpace::XYZ => 3,
        IccColorSpace::CMYK => 4,
        IccColorSpace::Unknown => 3, // fallback
    };

    // --- Tag table ---
    let tag_count = u32_be(data, 128) as usize;
    if data.len() < 132 + tag_count * 12 {
        return None;
    }

    let mut red_matrix_column = None;
    let mut green_matrix_column = None;
    let mut blue_matrix_column = None;
    let mut red_trc = None;
    let mut green_trc = None;
    let mut blue_trc = None;
    let mut gray_trc = None;
    let mut white_point = D50; // ICC default PCS illuminant

    for i in 0..tag_count {
        let base = 132 + i * 12;
        if base + 12 > data.len() {
            break;
        }
        let sig = &data[base..base + 4];
        let offset = u32_be(data, base + 4) as usize;
        let size = u32_be(data, base + 8) as usize;

        if offset + size > data.len() {
            continue; // skip broken tags
        }
        let tag_data = &data[offset..offset + size];

        match sig {
            b"rXYZ" => red_matrix_column = parse_xyz_tag(tag_data),
            b"gXYZ" => green_matrix_column = parse_xyz_tag(tag_data),
            b"bXYZ" => blue_matrix_column = parse_xyz_tag(tag_data),
            b"rTRC" => red_trc = parse_trc_tag(tag_data),
            b"gTRC" => green_trc = parse_trc_tag(tag_data),
            b"bTRC" => blue_trc = parse_trc_tag(tag_data),
            b"kTRC" => gray_trc = parse_trc_tag(tag_data),
            b"wtpt" => {
                if let Some(xyz) = parse_xyz_tag(tag_data) {
                    white_point = xyz;
                }
            }
            _ => {}
        }
    }

    Some(IccProfile {
        version: (version_major, version_minor),
        profile_class,
        color_space,
        pcs,
        num_components,
        rendering_intent,
        red_matrix_column,
        green_matrix_column,
        blue_matrix_column,
        red_trc,
        green_trc,
        blue_trc,
        gray_trc,
        white_point,
    })
}

// ---------------------------------------------------------------------------
// Tag parsers
// ---------------------------------------------------------------------------

/// Parse an XYZ tag – type signature 'XYZ ' followed by 4 reserved bytes then
/// one or more XYZNumber (each 12 bytes of s15Fixed16Number).
fn parse_xyz_tag(data: &[u8]) -> Option<[f64; 3]> {
    if data.len() < 20 {
        return None;
    }
    // Tag type signature should be 'XYZ '
    if &data[0..4] != b"XYZ " {
        return None;
    }
    Some([
        s15fixed16(data, 8),
        s15fixed16(data, 12),
        s15fixed16(data, 16),
    ])
}

/// Parse a TRC (Tone Response Curve) tag.
///
/// Handles:
///  - 'curv' with count=0 -> identity (gamma 1.0)
///  - 'curv' with count=1 -> simple gamma encoded as u8Fixed8Number
///  - 'curv' with count>1 -> 1-D LUT
///  - 'para' -> parametric curve
fn parse_trc_tag(data: &[u8]) -> Option<ToneCurve> {
    if data.len() < 8 {
        return None;
    }
    let type_sig = &data[0..4];
    match type_sig {
        b"curv" => {
            let count = u32_be(data, 8) as usize;
            if count == 0 {
                Some(ToneCurve::Gamma(1.0))
            } else if count == 1 {
                if data.len() < 14 {
                    return None;
                }
                let raw = u16_be(data, 12);
                let gamma = raw as f64 / 256.0; // u8Fixed8Number
                Some(ToneCurve::Gamma(gamma))
            } else {
                if data.len() < 12 + count * 2 {
                    return None;
                }
                let mut table = Vec::with_capacity(count);
                for j in 0..count {
                    table.push(u16_be(data, 12 + j * 2));
                }
                Some(ToneCurve::Table(table))
            }
        }
        b"para" => {
            if data.len() < 12 {
                return None;
            }
            let func_type = u16_be(data, 8) as usize;
            let param_counts = [1, 3, 4, 5, 7];
            let n = *param_counts.get(func_type).unwrap_or(&1);
            if data.len() < 12 + n * 4 {
                return None;
            }
            let mut params = Vec::with_capacity(n);
            for j in 0..n {
                params.push(s15fixed16(data, 12 + j * 4));
            }
            Some(ToneCurve::Parametric(params))
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Color transform
// ---------------------------------------------------------------------------

/// Transform a color from an ICC profile's device space to sRGB [0..1]^3.
///
/// Supports:
///  - Matrix/TRC RGB profiles (three-component)
///  - Gray TRC profiles
///  - CMYK fallback (simple subtractive model)
///
/// Components should be normalised to [0..1] for RGB/Gray, or device values
/// for CMYK.
pub fn icc_to_srgb(profile: &IccProfile, components: &[f64]) -> [f64; 3] {
    match profile.color_space {
        IccColorSpace::RGB => icc_rgb_to_srgb(profile, components),
        IccColorSpace::Gray => icc_gray_to_srgb(profile, components),
        IccColorSpace::CMYK => {
            let c = components.first().copied().unwrap_or(0.0);
            let m = components.get(1).copied().unwrap_or(0.0);
            let y = components.get(2).copied().unwrap_or(0.0);
            let k = components.get(3).copied().unwrap_or(0.0);
            cmyk_to_rgb(c, m, y, k)
        }
        _ => [0.0, 0.0, 0.0],
    }
}

/// RGB matrix/TRC profile -> sRGB.
fn icc_rgb_to_srgb(profile: &IccProfile, components: &[f64]) -> [f64; 3] {
    let r_in = components.first().copied().unwrap_or(0.0).clamp(0.0, 1.0);
    let g_in = components.get(1).copied().unwrap_or(0.0).clamp(0.0, 1.0);
    let b_in = components.get(2).copied().unwrap_or(0.0).clamp(0.0, 1.0);

    // 1. Apply TRC to linearise
    let r_lin = apply_trc(profile.red_trc.as_ref(), r_in);
    let g_lin = apply_trc(profile.green_trc.as_ref(), g_in);
    let b_lin = apply_trc(profile.blue_trc.as_ref(), b_in);

    // 2. Convert to XYZ via the profile's 3x3 matrix
    let rm = profile.red_matrix_column.unwrap_or([1.0, 0.0, 0.0]);
    let gm = profile.green_matrix_column.unwrap_or([0.0, 1.0, 0.0]);
    let bm = profile.blue_matrix_column.unwrap_or([0.0, 0.0, 1.0]);

    let x = rm[0] * r_lin + gm[0] * g_lin + bm[0] * b_lin;
    let y = rm[1] * r_lin + gm[1] * g_lin + bm[1] * b_lin;
    let z = rm[2] * r_lin + gm[2] * g_lin + bm[2] * b_lin;

    let mut xyz = [x, y, z];

    // 3. Chromatic adaptation from profile white point to D65
    xyz = chromatic_adapt(&xyz, &profile.white_point, &D65);

    // 4. XYZ -> linear sRGB -> sRGB gamma
    xyz_to_srgb(&xyz)
}

/// Gray TRC profile -> sRGB (uniform gray).
fn icc_gray_to_srgb(profile: &IccProfile, components: &[f64]) -> [f64; 3] {
    let g_in = components.first().copied().unwrap_or(0.0).clamp(0.0, 1.0);
    let linear = apply_trc(profile.gray_trc.as_ref(), g_in);

    // For a gray profile, the linearised value maps directly to a Y luminance.
    // Construct an XYZ proportional to the profile white point.
    let x = profile.white_point[0] * linear;
    let y = profile.white_point[1] * linear;
    let z = profile.white_point[2] * linear;

    let xyz = chromatic_adapt(&[x, y, z], &profile.white_point, &D65);
    xyz_to_srgb(&xyz)
}

// ---------------------------------------------------------------------------
// TRC evaluation
// ---------------------------------------------------------------------------

/// Apply a tone response curve to a normalised [0..1] input value.
pub(crate) fn apply_trc(trc: Option<&ToneCurve>, v: f64) -> f64 {
    let v = v.clamp(0.0, 1.0);
    match trc {
        None => v, // identity
        Some(ToneCurve::Gamma(g)) => v.powf(*g),
        Some(ToneCurve::Table(table)) => evaluate_table(table, v),
        Some(ToneCurve::Parametric(params)) => evaluate_parametric(params, v),
    }
}

/// Evaluate a 1-D lookup table with linear interpolation.
fn evaluate_table(table: &[u16], v: f64) -> f64 {
    if table.is_empty() {
        return v;
    }
    if table.len() == 1 {
        return table[0] as f64 / 65535.0;
    }
    let max_idx = (table.len() - 1) as f64;
    let pos = v * max_idx;
    let lo = pos.floor() as usize;
    let hi = (lo + 1).min(table.len() - 1);
    let frac = pos - lo as f64;
    let a = table[lo] as f64 / 65535.0;
    let b = table[hi] as f64 / 65535.0;
    a + (b - a) * frac
}

/// Evaluate a parametric curve (ICC types 0-4).
fn evaluate_parametric(params: &[f64], x: f64) -> f64 {
    let x = x.clamp(0.0, 1.0);
    match params.len() {
        // Type 0: y = x^g
        1 => x.powf(params[0]),
        // Type 1: y = (a*x + b)^g  if x >= -b/a, else 0
        3 => {
            let (g, a, b) = (params[0], params[1], params[2]);
            let threshold = if a.abs() > 1e-10 { -b / a } else { 0.0 };
            if x >= threshold {
                (a * x + b).max(0.0).powf(g)
            } else {
                0.0
            }
        }
        // Type 2: y = (a*x + b)^g + c  if x >= -b/a, else c
        4 => {
            let (g, a, b, c) = (params[0], params[1], params[2], params[3]);
            let threshold = if a.abs() > 1e-10 { -b / a } else { 0.0 };
            if x >= threshold {
                (a * x + b).max(0.0).powf(g) + c
            } else {
                c
            }
        }
        // Type 3: y = (a*x + b)^g  if x >= d, else c*x
        5 => {
            let (g, a, b, c, d) = (params[0], params[1], params[2], params[3], params[4]);
            if x >= d {
                (a * x + b).max(0.0).powf(g)
            } else {
                c * x
            }
        }
        // Type 4: y = (a*x + b)^g + e  if x >= d, else c*x + f
        7 => {
            let (g, a, b, c, d, e, f) =
                (params[0], params[1], params[2], params[3], params[4], params[5], params[6]);
            if x >= d {
                (a * x + b).max(0.0).powf(g) + e
            } else {
                c * x + f
            }
        }
        _ => x.powf(params.first().copied().unwrap_or(1.0)),
    }
}

// ---------------------------------------------------------------------------
// Chromatic adaptation (Bradford)
// ---------------------------------------------------------------------------

/// Adapt XYZ from source white point to destination white point using the
/// Bradford transform.
fn chromatic_adapt(xyz: &[f64; 3], src_wp: &[f64; 3], dst_wp: &[f64; 3]) -> [f64; 3] {
    // If white points are essentially the same, skip.
    if (src_wp[0] - dst_wp[0]).abs() < 1e-4
        && (src_wp[1] - dst_wp[1]).abs() < 1e-4
        && (src_wp[2] - dst_wp[2]).abs() < 1e-4
    {
        return *xyz;
    }

    // cone-response of source and destination white points
    let src_lms = mat3_mul_vec(&BRADFORD, src_wp);
    let dst_lms = mat3_mul_vec(&BRADFORD, dst_wp);

    // Diagonal scaling matrix in LMS space
    let scale = [
        if src_lms[0].abs() > 1e-10 { dst_lms[0] / src_lms[0] } else { 1.0 },
        if src_lms[1].abs() > 1e-10 { dst_lms[1] / src_lms[1] } else { 1.0 },
        if src_lms[2].abs() > 1e-10 { dst_lms[2] / src_lms[2] } else { 1.0 },
    ];

    // M^-1 * diag(scale) * M
    let lms = mat3_mul_vec(&BRADFORD, xyz);
    let scaled = [lms[0] * scale[0], lms[1] * scale[1], lms[2] * scale[2]];
    mat3_mul_vec(&BRADFORD_INV, &scaled)
}

// ---------------------------------------------------------------------------
// XYZ -> sRGB
// ---------------------------------------------------------------------------

/// Convert CIE XYZ to sRGB [0..1]^3 (clamped).
fn xyz_to_srgb(xyz: &[f64; 3]) -> [f64; 3] {
    let r_lin = XYZ_TO_SRGB[0][0] * xyz[0] + XYZ_TO_SRGB[0][1] * xyz[1] + XYZ_TO_SRGB[0][2] * xyz[2];
    let g_lin = XYZ_TO_SRGB[1][0] * xyz[0] + XYZ_TO_SRGB[1][1] * xyz[1] + XYZ_TO_SRGB[1][2] * xyz[2];
    let b_lin = XYZ_TO_SRGB[2][0] * xyz[0] + XYZ_TO_SRGB[2][1] * xyz[1] + XYZ_TO_SRGB[2][2] * xyz[2];

    [
        srgb_gamma(r_lin).clamp(0.0, 1.0),
        srgb_gamma(g_lin).clamp(0.0, 1.0),
        srgb_gamma(b_lin).clamp(0.0, 1.0),
    ]
}

/// Apply sRGB companding (linear -> gamma-encoded).
fn srgb_gamma(v: f64) -> f64 {
    if v <= 0.0031308 {
        12.92 * v
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

// ---------------------------------------------------------------------------
// Numeric helpers
// ---------------------------------------------------------------------------

fn u32_be(data: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}

fn u16_be(data: &[u8], offset: usize) -> u16 {
    u16::from_be_bytes([data[offset], data[offset + 1]])
}

/// Read a s15Fixed16Number (signed 32-bit, 16.16 fixed-point, big-endian).
fn s15fixed16(data: &[u8], offset: usize) -> f64 {
    let raw = i32::from_be_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ]);
    raw as f64 / 65536.0
}

fn parse_color_space_sig(sig: &[u8]) -> IccColorSpace {
    match sig {
        b"RGB " => IccColorSpace::RGB,
        b"CMYK" => IccColorSpace::CMYK,
        b"GRAY" => IccColorSpace::Gray,
        b"Lab " => IccColorSpace::Lab,
        b"XYZ " => IccColorSpace::XYZ,
        _ => IccColorSpace::Unknown,
    }
}

/// Multiply a 3x3 matrix by a 3-vector.
fn mat3_mul_vec(m: &[[f64; 3]; 3], v: &[f64; 3]) -> [f64; 3] {
    [
        m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
        m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
        m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
    ]
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // Helpers to build minimal ICC binary data ---

    fn put_u32_be(buf: &mut Vec<u8>, offset: usize, val: u32) {
        let bytes = val.to_be_bytes();
        buf[offset..offset + 4].copy_from_slice(&bytes);
    }

    fn put_u16_be(buf: &mut Vec<u8>, offset: usize, val: u16) {
        let bytes = val.to_be_bytes();
        buf[offset..offset + 2].copy_from_slice(&bytes);
    }

    fn put_s15fixed16(buf: &mut Vec<u8>, offset: usize, val: f64) {
        let raw = (val * 65536.0).round() as i32;
        let bytes = raw.to_be_bytes();
        buf[offset..offset + 4].copy_from_slice(&bytes);
    }

    /// Build a minimal ICC profile for an sRGB-like matrix/TRC RGB display profile.
    fn build_srgb_like_profile() -> Vec<u8> {
        // We need header (128) + tag table + tag data
        // Tags: rXYZ, gXYZ, bXYZ, rTRC, gTRC, bTRC, wtpt = 7 tags

        let tag_count: usize = 7;
        let tag_table_size = 4 + tag_count * 12; // count + entries
        let tag_table_start = 128;

        // Each XYZ tag: 20 bytes (4 sig + 4 reserved + 12 data)
        // Each curv gamma tag: 14 bytes (4 sig + 4 reserved + 4 count + 2 gamma)
        // wtpt: 20 bytes
        let xyz_tag_size = 20;
        let curv_tag_size = 14;

        let data_start = tag_table_start + tag_table_size;
        // Layout: rXYZ, gXYZ, bXYZ, wtpt, rTRC, gTRC, bTRC
        let rxyz_off = data_start;
        let gxyz_off = rxyz_off + xyz_tag_size;
        let bxyz_off = gxyz_off + xyz_tag_size;
        let wtpt_off = bxyz_off + xyz_tag_size;
        let rtrc_off = wtpt_off + xyz_tag_size;
        let gtrc_off = rtrc_off + curv_tag_size;
        let btrc_off = gtrc_off + curv_tag_size;
        let total_size = btrc_off + curv_tag_size;

        let mut buf = vec![0u8; total_size];

        // -- Header --
        put_u32_be(&mut buf, 0, total_size as u32); // profile size
        // preferred CMM: 0
        buf[8] = 2; // version major = 2
        buf[9] = 0x40; // version minor = 4, bugfix = 0
        buf[12..16].copy_from_slice(b"mntr"); // Display profile
        buf[16..20].copy_from_slice(b"RGB "); // Data color space
        buf[20..24].copy_from_slice(b"XYZ "); // PCS
        // rendering intent = 0 (perceptual) at offset 64
        put_u32_be(&mut buf, 64, 0);

        // -- Tag table --
        put_u32_be(&mut buf, tag_table_start, tag_count as u32);

        let mut write_tag_entry = |idx: usize, sig: &[u8; 4], offset: usize, size: usize| {
            let base = tag_table_start + 4 + idx * 12;
            buf[base..base + 4].copy_from_slice(sig);
            put_u32_be(&mut buf, base + 4, offset as u32);
            put_u32_be(&mut buf, base + 8, size as u32);
        };

        write_tag_entry(0, b"rXYZ", rxyz_off, xyz_tag_size);
        write_tag_entry(1, b"gXYZ", gxyz_off, xyz_tag_size);
        write_tag_entry(2, b"bXYZ", bxyz_off, xyz_tag_size);
        write_tag_entry(3, b"wtpt", wtpt_off, xyz_tag_size);
        write_tag_entry(4, b"rTRC", rtrc_off, curv_tag_size);
        write_tag_entry(5, b"gTRC", gtrc_off, curv_tag_size);
        write_tag_entry(6, b"bTRC", btrc_off, curv_tag_size);

        // -- Tag data --

        // sRGB-ish matrix columns (IEC 61966-2-1 reference, D50-adapted)
        let write_xyz_tag = |buf: &mut Vec<u8>, off: usize, x: f64, y: f64, z: f64| {
            buf[off..off + 4].copy_from_slice(b"XYZ ");
            // 4 reserved bytes already zero
            put_s15fixed16(buf, off + 8, x);
            put_s15fixed16(buf, off + 12, y);
            put_s15fixed16(buf, off + 16, z);
        };

        // sRGB primaries (D50 adapted, from ICC sRGB profile)
        write_xyz_tag(&mut buf, rxyz_off, 0.4361, 0.2225, 0.0139);
        write_xyz_tag(&mut buf, gxyz_off, 0.3851, 0.7169, 0.0971);
        write_xyz_tag(&mut buf, bxyz_off, 0.1431, 0.0606, 0.7141);
        // D50 white point
        write_xyz_tag(&mut buf, wtpt_off, 0.9642, 1.0, 0.8249);

        // TRC: gamma 2.2 (approximate sRGB)
        let write_curv_gamma = |buf: &mut Vec<u8>, off: usize, gamma: f64| {
            buf[off..off + 4].copy_from_slice(b"curv");
            put_u32_be(buf, off + 8, 1); // count=1 -> gamma
            let g_u8f8 = (gamma * 256.0).round() as u16;
            put_u16_be(buf, off + 12, g_u8f8);
        };

        write_curv_gamma(&mut buf, rtrc_off, 2.2);
        write_curv_gamma(&mut buf, gtrc_off, 2.2);
        write_curv_gamma(&mut buf, btrc_off, 2.2);

        buf
    }

    /// Build a minimal gray profile.
    fn build_gray_profile() -> Vec<u8> {
        let tag_count: usize = 2; // wtpt, kTRC
        let tag_table_size = 4 + tag_count * 12;
        let tag_table_start = 128;

        let xyz_tag_size = 20;
        let curv_tag_size = 14;

        let data_start = tag_table_start + tag_table_size;
        let wtpt_off = data_start;
        let ktrc_off = wtpt_off + xyz_tag_size;
        let total_size = ktrc_off + curv_tag_size;

        let mut buf = vec![0u8; total_size];

        put_u32_be(&mut buf, 0, total_size as u32);
        buf[8] = 2;
        buf[9] = 0x20;
        buf[12..16].copy_from_slice(b"mntr");
        buf[16..20].copy_from_slice(b"GRAY");
        buf[20..24].copy_from_slice(b"XYZ ");
        put_u32_be(&mut buf, 64, 0);

        put_u32_be(&mut buf, tag_table_start, tag_count as u32);

        let mut base = tag_table_start + 4;
        buf[base..base + 4].copy_from_slice(b"wtpt");
        put_u32_be(&mut buf, base + 4, wtpt_off as u32);
        put_u32_be(&mut buf, base + 8, xyz_tag_size as u32);

        base += 12;
        buf[base..base + 4].copy_from_slice(b"kTRC");
        put_u32_be(&mut buf, base + 4, ktrc_off as u32);
        put_u32_be(&mut buf, base + 8, curv_tag_size as u32);

        // wtpt = D50
        buf[wtpt_off..wtpt_off + 4].copy_from_slice(b"XYZ ");
        put_s15fixed16(&mut buf, wtpt_off + 8, 0.9642);
        put_s15fixed16(&mut buf, wtpt_off + 12, 1.0);
        put_s15fixed16(&mut buf, wtpt_off + 16, 0.8249);

        // kTRC gamma 2.2
        buf[ktrc_off..ktrc_off + 4].copy_from_slice(b"curv");
        put_u32_be(&mut buf, ktrc_off + 8, 1);
        let g = (2.2f64 * 256.0).round() as u16;
        put_u16_be(&mut buf, ktrc_off + 12, g);

        buf
    }

    // --- Test cases ---

    #[test]
    fn test_invalid_short_data() {
        assert!(parse_icc_profile(&[]).is_none());
        assert!(parse_icc_profile(&[0u8; 100]).is_none());
        assert!(parse_icc_profile(&[0u8; 131]).is_none());
    }

    #[test]
    fn test_parse_srgb_header() {
        let data = build_srgb_like_profile();
        let profile = parse_icc_profile(&data).expect("should parse");

        assert_eq!(profile.version, (2, 4));
        assert_eq!(profile.profile_class, ProfileClass::Display);
        assert_eq!(profile.color_space, IccColorSpace::RGB);
        assert_eq!(profile.pcs, IccColorSpace::XYZ);
        assert_eq!(profile.num_components, 3);
        assert_eq!(profile.rendering_intent, RenderingIntent::Perceptual);
    }

    #[test]
    fn test_parse_gray_profile() {
        let data = build_gray_profile();
        let profile = parse_icc_profile(&data).expect("should parse");

        assert_eq!(profile.version, (2, 2));
        assert_eq!(profile.color_space, IccColorSpace::Gray);
        assert_eq!(profile.num_components, 1);
        assert!(profile.gray_trc.is_some());
    }

    #[test]
    fn test_parse_matrix_columns() {
        let data = build_srgb_like_profile();
        let profile = parse_icc_profile(&data).unwrap();

        let rm = profile.red_matrix_column.unwrap();
        // Check approximate sRGB red primary X
        assert!((rm[0] - 0.4361).abs() < 0.001, "red X = {}", rm[0]);
        assert!((rm[1] - 0.2225).abs() < 0.001, "red Y = {}", rm[1]);

        let gm = profile.green_matrix_column.unwrap();
        assert!((gm[0] - 0.3851).abs() < 0.001);

        let bm = profile.blue_matrix_column.unwrap();
        assert!((bm[2] - 0.7141).abs() < 0.001);
    }

    #[test]
    fn test_parse_white_point() {
        let data = build_srgb_like_profile();
        let profile = parse_icc_profile(&data).unwrap();

        assert!((profile.white_point[0] - 0.9642).abs() < 0.001);
        assert!((profile.white_point[1] - 1.0).abs() < 0.001);
        assert!((profile.white_point[2] - 0.8249).abs() < 0.001);
    }

    #[test]
    fn test_trc_gamma() {
        let trc = ToneCurve::Gamma(2.2);
        let out = apply_trc(Some(&trc), 0.5);
        let expected = 0.5_f64.powf(2.2);
        assert!((out - expected).abs() < 1e-6, "got {out}, expected {expected}");
    }

    #[test]
    fn test_trc_identity() {
        let trc = ToneCurve::Gamma(1.0);
        assert!((apply_trc(Some(&trc), 0.3) - 0.3).abs() < 1e-9);
        assert!((apply_trc(None, 0.7) - 0.7).abs() < 1e-9);
    }

    #[test]
    fn test_trc_table() {
        // Simple 3-entry table: 0, 32768, 65535
        let trc = ToneCurve::Table(vec![0, 32768, 65535]);
        let mid = apply_trc(Some(&trc), 0.5);
        assert!((mid - 0.5).abs() < 0.01, "table mid = {mid}");

        let end = apply_trc(Some(&trc), 1.0);
        assert!((end - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_trc_parametric_type0() {
        // Type 0: y = x^g
        let trc = ToneCurve::Parametric(vec![2.4]);
        let out = apply_trc(Some(&trc), 0.5);
        let expected = 0.5_f64.powf(2.4);
        assert!((out - expected).abs() < 1e-6);
    }

    #[test]
    fn test_srgb_gamma_function() {
        // Below linear threshold
        assert!((srgb_gamma(0.001) - 0.001 * 12.92).abs() < 1e-6);
        // At white
        assert!((srgb_gamma(1.0) - 1.0).abs() < 1e-6);
        // Known mid value
        let mid = srgb_gamma(0.5);
        assert!(mid > 0.7 && mid < 0.8, "srgb_gamma(0.5) = {mid}");
    }

    #[test]
    fn test_xyz_to_srgb_d65_white() {
        // D65 white in XYZ should map to sRGB (1,1,1)
        let rgb = xyz_to_srgb(&D65);
        assert!((rgb[0] - 1.0).abs() < 0.02, "R={}", rgb[0]);
        assert!((rgb[1] - 1.0).abs() < 0.02, "G={}", rgb[1]);
        assert!((rgb[2] - 1.0).abs() < 0.02, "B={}", rgb[2]);
    }

    #[test]
    fn test_xyz_to_srgb_black() {
        let rgb = xyz_to_srgb(&[0.0, 0.0, 0.0]);
        assert!((rgb[0]).abs() < 1e-6);
        assert!((rgb[1]).abs() < 1e-6);
        assert!((rgb[2]).abs() < 1e-6);
    }

    #[test]
    fn test_chromatic_adapt_identity() {
        // Same white point -> no change
        let xyz = [0.5, 0.4, 0.3];
        let result = chromatic_adapt(&xyz, &D65, &D65);
        assert!((result[0] - xyz[0]).abs() < 1e-10);
        assert!((result[1] - xyz[1]).abs() < 1e-10);
        assert!((result[2] - xyz[2]).abs() < 1e-10);
    }

    #[test]
    fn test_chromatic_adapt_d50_to_d65() {
        // D50 white should adapt to D65 white
        let result = chromatic_adapt(&D50, &D50, &D65);
        assert!((result[0] - D65[0]).abs() < 0.01, "X={}", result[0]);
        assert!((result[1] - D65[1]).abs() < 0.01, "Y={}", result[1]);
        assert!((result[2] - D65[2]).abs() < 0.01, "Z={}", result[2]);
    }

    #[test]
    fn test_full_pipeline_rgb_white() {
        let data = build_srgb_like_profile();
        let profile = parse_icc_profile(&data).unwrap();

        // Full white input through sRGB-like profile should yield approximately (1,1,1)
        let rgb = icc_to_srgb(&profile, &[1.0, 1.0, 1.0]);
        assert!((rgb[0] - 1.0).abs() < 0.05, "R={}", rgb[0]);
        assert!((rgb[1] - 1.0).abs() < 0.05, "G={}", rgb[1]);
        assert!((rgb[2] - 1.0).abs() < 0.05, "B={}", rgb[2]);
    }

    #[test]
    fn test_full_pipeline_rgb_black() {
        let data = build_srgb_like_profile();
        let profile = parse_icc_profile(&data).unwrap();

        let rgb = icc_to_srgb(&profile, &[0.0, 0.0, 0.0]);
        assert!((rgb[0]).abs() < 0.01);
        assert!((rgb[1]).abs() < 0.01);
        assert!((rgb[2]).abs() < 0.01);
    }

    #[test]
    fn test_full_pipeline_rgb_red() {
        let data = build_srgb_like_profile();
        let profile = parse_icc_profile(&data).unwrap();

        // Pure red through an sRGB-like profile should be predominantly red
        let rgb = icc_to_srgb(&profile, &[1.0, 0.0, 0.0]);
        assert!(rgb[0] > 0.8, "R should be high: {}", rgb[0]);
        assert!(rgb[1] < 0.3, "G should be low: {}", rgb[1]);
        assert!(rgb[2] < 0.3, "B should be low: {}", rgb[2]);
    }

    #[test]
    fn test_full_pipeline_gray() {
        let data = build_gray_profile();
        let profile = parse_icc_profile(&data).unwrap();

        // Mid-gray
        let rgb = icc_to_srgb(&profile, &[0.5]);
        // All channels should be approximately equal
        assert!((rgb[0] - rgb[1]).abs() < 0.05, "R={} G={}", rgb[0], rgb[1]);
        assert!((rgb[1] - rgb[2]).abs() < 0.05, "G={} B={}", rgb[1], rgb[2]);
        // Should be between 0 and 1
        assert!(rgb[0] > 0.1 && rgb[0] < 0.9, "gray 0.5 -> {}", rgb[0]);
    }

    #[test]
    fn test_full_pipeline_gray_extremes() {
        let data = build_gray_profile();
        let profile = parse_icc_profile(&data).unwrap();

        let white = icc_to_srgb(&profile, &[1.0]);
        assert!((white[0] - 1.0).abs() < 0.05);

        let black = icc_to_srgb(&profile, &[0.0]);
        assert!(black[0].abs() < 0.01);
    }

    #[test]
    fn test_cmyk_fallback() {
        let profile = IccProfile {
            version: (2, 0),
            profile_class: ProfileClass::Output,
            color_space: IccColorSpace::CMYK,
            pcs: IccColorSpace::Lab,
            num_components: 4,
            rendering_intent: RenderingIntent::Perceptual,
            red_matrix_column: None,
            green_matrix_column: None,
            blue_matrix_column: None,
            red_trc: None,
            green_trc: None,
            blue_trc: None,
            gray_trc: None,
            white_point: D50,
        };

        let rgb = icc_to_srgb(&profile, &[0.0, 0.0, 0.0, 0.0]);
        assert!((rgb[0] - 1.0).abs() < 0.01); // white
    }

    #[test]
    fn test_curv_tag_count_zero_identity() {
        // Build a profile where the TRC has count=0 (identity curve)
        let data = build_srgb_like_profile();
        let _ = data;

        // Test the parse_trc_tag function directly with a count=0 identity curve.
        let identity_curv = {
            let mut d = vec![0u8; 12];
            d[0..4].copy_from_slice(b"curv");
            // count = 0 already (zeroed)
            d
        };
        let trc = parse_trc_tag(&identity_curv);
        match trc {
            Some(ToneCurve::Gamma(g)) => assert!((g - 1.0).abs() < 1e-9),
            _ => panic!("expected Gamma(1.0) for identity curve, got {:?}", trc),
        }

        // Verify it works as identity
        assert!((apply_trc(trc.as_ref(), 0.42) - 0.42).abs() < 1e-9);
    }
}
