//! OpenType layout table (GSUB/GPOS) parsing.
//!
//! Parses the most common GSUB and GPOS subtable types from raw TTF/OTF font data.
//! Implements parsing per the OpenType specification sections on GSUB and GPOS tables.

/// Parsed OpenType layout data.
#[derive(Debug, Clone)]
pub struct OpenTypeLayout {
    /// GSUB features (glyph substitution).
    pub gsub: Option<GsubTable>,
    /// GPOS features (glyph positioning).
    pub gpos: Option<GposTable>,
}

/// GSUB table - glyph substitution.
#[derive(Debug, Clone)]
pub struct GsubTable {
    /// Script list.
    pub scripts: Vec<ScriptRecord>,
    /// Feature list.
    pub features: Vec<FeatureRecord>,
    /// Lookups.
    pub lookups: Vec<GsubLookup>,
}

/// GPOS table - glyph positioning.
#[derive(Debug, Clone)]
pub struct GposTable {
    /// Script list.
    pub scripts: Vec<ScriptRecord>,
    /// Feature list.
    pub features: Vec<FeatureRecord>,
    /// Lookups.
    pub lookups: Vec<GposLookup>,
}

/// A script record from the Script List table.
#[derive(Debug, Clone)]
pub struct ScriptRecord {
    /// 4-byte script tag (e.g. b"latn").
    pub tag: [u8; 4],
    /// Default language system for this script.
    pub default_lang_sys: Option<LangSys>,
    /// Named language systems within this script.
    pub lang_sys: Vec<([u8; 4], LangSys)>,
}

/// Language system record.
#[derive(Debug, Clone)]
pub struct LangSys {
    /// Index of the required feature, if any (0xFFFF means none).
    pub required_feature_index: Option<u16>,
    /// Indices into the feature list.
    pub feature_indices: Vec<u16>,
}

/// Feature record from the Feature List table.
#[derive(Debug, Clone)]
pub struct FeatureRecord {
    /// 4-byte feature tag (e.g. b"liga", b"kern").
    pub tag: [u8; 4],
    /// Indices into the lookup list.
    pub lookup_indices: Vec<u16>,
}

/// GSUB lookup types.
#[derive(Debug, Clone)]
pub enum GsubLookup {
    /// Type 1: Single substitution (one-to-one).
    Single(Vec<(u16, u16)>),
    /// Type 2: Multiple substitution (one-to-many).
    Multiple(Vec<(u16, Vec<u16>)>),
    /// Type 3: Alternate substitution.
    Alternate(Vec<(u16, Vec<u16>)>),
    /// Type 4: Ligature substitution (many-to-one).
    Ligature(Vec<LigatureSet>),
    /// Unsupported lookup type.
    Unsupported { lookup_type: u16 },
}

/// A set of ligatures sharing the same first glyph.
#[derive(Debug, Clone)]
pub struct LigatureSet {
    /// First glyph ID that triggers this ligature set.
    pub first_glyph: u16,
    /// Ligature substitution rules.
    pub ligatures: Vec<Ligature>,
}

/// A single ligature rule.
#[derive(Debug, Clone)]
pub struct Ligature {
    /// Additional component glyph IDs (after the first).
    pub component_glyphs: Vec<u16>,
    /// The replacement ligature glyph ID.
    pub ligature_glyph: u16,
}

/// GPOS lookup types.
#[derive(Debug, Clone)]
pub enum GposLookup {
    /// Type 2: Pair adjustment (kerning).
    PairAdjustment(Vec<KerningPair>),
    /// Unsupported lookup type.
    Unsupported { lookup_type: u16 },
}

/// A kerning pair extracted from GPOS pair adjustment.
#[derive(Debug, Clone)]
pub struct KerningPair {
    /// First glyph ID.
    pub first: u16,
    /// Second glyph ID.
    pub second: u16,
    /// X advance adjustment (in font units).
    pub x_advance: i16,
}

// ---------------------------------------------------------------------------
// Binary reading helpers (big-endian)
// ---------------------------------------------------------------------------

fn read_u16(data: &[u8], offset: usize) -> Option<u16> {
    if offset + 2 > data.len() {
        return None;
    }
    Some(u16::from_be_bytes([data[offset], data[offset + 1]]))
}

fn read_i16(data: &[u8], offset: usize) -> Option<i16> {
    if offset + 2 > data.len() {
        return None;
    }
    Some(i16::from_be_bytes([data[offset], data[offset + 1]]))
}

fn read_u32(data: &[u8], offset: usize) -> Option<u32> {
    if offset + 4 > data.len() {
        return None;
    }
    Some(u32::from_be_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ]))
}

fn read_tag(data: &[u8], offset: usize) -> Option<[u8; 4]> {
    if offset + 4 > data.len() {
        return None;
    }
    Some([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]])
}

// ---------------------------------------------------------------------------
// Table directory
// ---------------------------------------------------------------------------

/// Find a table in the font data by its 4-byte tag.
/// Returns `(offset, length)` of the table within `font_data`.
fn find_table(font_data: &[u8], tag: &[u8; 4]) -> Option<(usize, usize)> {
    // TrueType / OpenType offset table:
    //   0: sfVersion (u32)
    //   4: numTables (u16)
    //   6: searchRange (u16)
    //   8: entrySelector (u16)
    //  10: rangeShift (u16)
    // Table records start at offset 12, each 16 bytes:
    //   0: tag (4 bytes)
    //   4: checkSum (u32)
    //   8: offset (u32)
    //  12: length (u32)
    let num_tables = read_u16(font_data, 4)? as usize;
    for i in 0..num_tables {
        let rec = 12 + i * 16;
        let t = read_tag(font_data, rec)?;
        if &t == tag {
            let offset = read_u32(font_data, rec + 8)? as usize;
            let length = read_u32(font_data, rec + 12)? as usize;
            return Some((offset, length));
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Coverage table
// ---------------------------------------------------------------------------

/// Parse a Coverage table. Returns a list of glyph IDs in coverage order.
///
/// - Format 1: array of glyph IDs.
/// - Format 2: array of ranges (startGlyphID, endGlyphID, startCoverageIndex).
fn parse_coverage(data: &[u8], offset: usize) -> Vec<u16> {
    let format = match read_u16(data, offset) {
        Some(f) => f,
        None => return Vec::new(),
    };
    match format {
        1 => {
            let count = match read_u16(data, offset + 2) {
                Some(c) => c as usize,
                None => return Vec::new(),
            };
            let mut glyphs = Vec::with_capacity(count);
            for i in 0..count {
                if let Some(gid) = read_u16(data, offset + 4 + i * 2) {
                    glyphs.push(gid);
                }
            }
            glyphs
        }
        2 => {
            let count = match read_u16(data, offset + 2) {
                Some(c) => c as usize,
                None => return Vec::new(),
            };
            let mut glyphs = Vec::new();
            for i in 0..count {
                let rec = offset + 4 + i * 6;
                let start = match read_u16(data, rec) {
                    Some(s) => s,
                    None => break,
                };
                let end = match read_u16(data, rec + 2) {
                    Some(e) => e,
                    None => break,
                };
                for gid in start..=end {
                    glyphs.push(gid);
                }
            }
            glyphs
        }
        _ => Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// Script List
// ---------------------------------------------------------------------------

/// Parse the Script List table.
fn parse_script_list(data: &[u8], offset: usize) -> Vec<ScriptRecord> {
    let count = match read_u16(data, offset) {
        Some(c) => c as usize,
        None => return Vec::new(),
    };
    let mut scripts = Vec::with_capacity(count);
    for i in 0..count {
        let rec = offset + 2 + i * 6;
        let tag = match read_tag(data, rec) {
            Some(t) => t,
            None => break,
        };
        let script_offset = match read_u16(data, rec + 4) {
            Some(o) => offset + o as usize,
            None => break,
        };
        let (default_lang_sys, lang_sys) = parse_script_table(data, script_offset);
        scripts.push(ScriptRecord {
            tag,
            default_lang_sys,
            lang_sys,
        });
    }
    scripts
}

fn parse_script_table(data: &[u8], offset: usize) -> (Option<LangSys>, Vec<([u8; 4], LangSys)>) {
    let default_offset = match read_u16(data, offset) {
        Some(o) => o,
        None => return (None, Vec::new()),
    };
    let default_lang_sys = if default_offset != 0 {
        parse_lang_sys(data, offset + default_offset as usize)
    } else {
        None
    };
    let lang_count = match read_u16(data, offset + 2) {
        Some(c) => c as usize,
        None => return (default_lang_sys, Vec::new()),
    };
    let mut lang_sys_list = Vec::with_capacity(lang_count);
    for i in 0..lang_count {
        let rec = offset + 4 + i * 6;
        let tag = match read_tag(data, rec) {
            Some(t) => t,
            None => break,
        };
        let lang_offset = match read_u16(data, rec + 4) {
            Some(o) => offset + o as usize,
            None => break,
        };
        if let Some(ls) = parse_lang_sys(data, lang_offset) {
            lang_sys_list.push((tag, ls));
        }
    }
    (default_lang_sys, lang_sys_list)
}

fn parse_lang_sys(data: &[u8], offset: usize) -> Option<LangSys> {
    // lookupOrder (u16, reserved), requiredFeatureIndex (u16), featureIndexCount (u16)
    let _lookup_order = read_u16(data, offset)?;
    let req = read_u16(data, offset + 2)?;
    let count = read_u16(data, offset + 4)? as usize;
    let required_feature_index = if req == 0xFFFF { None } else { Some(req) };
    let mut feature_indices = Vec::with_capacity(count);
    for i in 0..count {
        if let Some(idx) = read_u16(data, offset + 6 + i * 2) {
            feature_indices.push(idx);
        }
    }
    Some(LangSys {
        required_feature_index,
        feature_indices,
    })
}

// ---------------------------------------------------------------------------
// Feature List
// ---------------------------------------------------------------------------

/// Parse the Feature List table.
fn parse_feature_list(data: &[u8], offset: usize) -> Vec<FeatureRecord> {
    let count = match read_u16(data, offset) {
        Some(c) => c as usize,
        None => return Vec::new(),
    };
    let mut features = Vec::with_capacity(count);
    for i in 0..count {
        let rec = offset + 2 + i * 6;
        let tag = match read_tag(data, rec) {
            Some(t) => t,
            None => break,
        };
        let feat_offset = match read_u16(data, rec + 4) {
            Some(o) => offset + o as usize,
            None => break,
        };
        // Feature table: featureParams (u16), lookupIndexCount (u16), lookupListIndices[]
        let _params = match read_u16(data, feat_offset) {
            Some(p) => p,
            None => break,
        };
        let lookup_count = match read_u16(data, feat_offset + 2) {
            Some(c) => c as usize,
            None => break,
        };
        let mut lookup_indices = Vec::with_capacity(lookup_count);
        for j in 0..lookup_count {
            if let Some(idx) = read_u16(data, feat_offset + 4 + j * 2) {
                lookup_indices.push(idx);
            }
        }
        features.push(FeatureRecord {
            tag,
            lookup_indices,
        });
    }
    features
}

// ---------------------------------------------------------------------------
// GSUB Lookups
// ---------------------------------------------------------------------------

/// Parse the GSUB Lookup List table.
fn parse_gsub_lookups(data: &[u8], offset: usize) -> Vec<GsubLookup> {
    let count = match read_u16(data, offset) {
        Some(c) => c as usize,
        None => return Vec::new(),
    };
    let mut lookups = Vec::with_capacity(count);
    for i in 0..count {
        let lookup_offset = match read_u16(data, offset + 2 + i * 2) {
            Some(o) => offset + o as usize,
            None => break,
        };
        lookups.push(parse_gsub_lookup(data, lookup_offset));
    }
    lookups
}

fn parse_gsub_lookup(data: &[u8], offset: usize) -> GsubLookup {
    let lookup_type = match read_u16(data, offset) {
        Some(t) => t,
        None => return GsubLookup::Unsupported { lookup_type: 0 },
    };
    let _flags = read_u16(data, offset + 2);
    let subtable_count = match read_u16(data, offset + 4) {
        Some(c) => c as usize,
        None => return GsubLookup::Unsupported { lookup_type },
    };

    // Collect subtable offsets
    let mut subtable_offsets = Vec::with_capacity(subtable_count);
    for i in 0..subtable_count {
        if let Some(o) = read_u16(data, offset + 6 + i * 2) {
            subtable_offsets.push(offset + o as usize);
        }
    }

    match lookup_type {
        1 => parse_gsub_single(data, &subtable_offsets),
        2 => parse_gsub_multiple_or_alternate(data, &subtable_offsets, false),
        3 => parse_gsub_multiple_or_alternate(data, &subtable_offsets, true),
        4 => parse_gsub_ligature(data, &subtable_offsets),
        _ => GsubLookup::Unsupported { lookup_type },
    }
}

fn parse_gsub_single(data: &[u8], subtable_offsets: &[usize]) -> GsubLookup {
    let mut mappings = Vec::new();
    for &st in subtable_offsets {
        let format = match read_u16(data, st) {
            Some(f) => f,
            None => continue,
        };
        let cov_offset = match read_u16(data, st + 2) {
            Some(o) => st + o as usize,
            None => continue,
        };
        let coverage = parse_coverage(data, cov_offset);
        match format {
            1 => {
                // Format 1: delta
                let delta = match read_i16(data, st + 4) {
                    Some(d) => d,
                    None => continue,
                };
                for gid in coverage {
                    let to = (gid as i32 + delta as i32) as u16;
                    mappings.push((gid, to));
                }
            }
            2 => {
                // Format 2: substitute array
                let count = match read_u16(data, st + 4) {
                    Some(c) => c as usize,
                    None => continue,
                };
                for (i, &gid) in coverage.iter().enumerate() {
                    if i >= count {
                        break;
                    }
                    if let Some(sub) = read_u16(data, st + 6 + i * 2) {
                        mappings.push((gid, sub));
                    }
                }
            }
            _ => {}
        }
    }
    GsubLookup::Single(mappings)
}

fn parse_gsub_multiple_or_alternate(
    data: &[u8],
    subtable_offsets: &[usize],
    is_alternate: bool,
) -> GsubLookup {
    let mut mappings: Vec<(u16, Vec<u16>)> = Vec::new();
    for &st in subtable_offsets {
        let _format = match read_u16(data, st) {
            Some(f) => f,
            None => continue,
        };
        let cov_offset = match read_u16(data, st + 2) {
            Some(o) => st + o as usize,
            None => continue,
        };
        let coverage = parse_coverage(data, cov_offset);
        let seq_count = match read_u16(data, st + 4) {
            Some(c) => c as usize,
            None => continue,
        };
        for (i, &gid) in coverage.iter().enumerate() {
            if i >= seq_count {
                break;
            }
            let seq_offset = match read_u16(data, st + 6 + i * 2) {
                Some(o) => st + o as usize,
                None => continue,
            };
            let glyph_count = match read_u16(data, seq_offset) {
                Some(c) => c as usize,
                None => continue,
            };
            let mut glyphs = Vec::with_capacity(glyph_count);
            for j in 0..glyph_count {
                if let Some(g) = read_u16(data, seq_offset + 2 + j * 2) {
                    glyphs.push(g);
                }
            }
            mappings.push((gid, glyphs));
        }
    }
    if is_alternate {
        GsubLookup::Alternate(mappings)
    } else {
        GsubLookup::Multiple(mappings)
    }
}

fn parse_gsub_ligature(data: &[u8], subtable_offsets: &[usize]) -> GsubLookup {
    let mut lig_sets = Vec::new();
    for &st in subtable_offsets {
        let _format = match read_u16(data, st) {
            Some(f) => f,
            None => continue,
        };
        let cov_offset = match read_u16(data, st + 2) {
            Some(o) => st + o as usize,
            None => continue,
        };
        let coverage = parse_coverage(data, cov_offset);
        let set_count = match read_u16(data, st + 4) {
            Some(c) => c as usize,
            None => continue,
        };
        for (i, &first_glyph) in coverage.iter().enumerate() {
            if i >= set_count {
                break;
            }
            let set_offset = match read_u16(data, st + 6 + i * 2) {
                Some(o) => st + o as usize,
                None => continue,
            };
            let lig_count = match read_u16(data, set_offset) {
                Some(c) => c as usize,
                None => continue,
            };
            let mut ligatures = Vec::with_capacity(lig_count);
            for j in 0..lig_count {
                let lig_offset = match read_u16(data, set_offset + 2 + j * 2) {
                    Some(o) => set_offset + o as usize,
                    None => continue,
                };
                let lig_glyph = match read_u16(data, lig_offset) {
                    Some(g) => g,
                    None => continue,
                };
                let comp_count = match read_u16(data, lig_offset + 2) {
                    Some(c) => c as usize,
                    None => continue,
                };
                // comp_count includes the first glyph, so components = comp_count - 1
                let extra = if comp_count > 0 { comp_count - 1 } else { 0 };
                let mut component_glyphs = Vec::with_capacity(extra);
                for k in 0..extra {
                    if let Some(g) = read_u16(data, lig_offset + 4 + k * 2) {
                        component_glyphs.push(g);
                    }
                }
                ligatures.push(Ligature {
                    component_glyphs,
                    ligature_glyph: lig_glyph,
                });
            }
            lig_sets.push(LigatureSet {
                first_glyph,
                ligatures,
            });
        }
    }
    GsubLookup::Ligature(lig_sets)
}

// ---------------------------------------------------------------------------
// GPOS Lookups
// ---------------------------------------------------------------------------

/// Parse the GPOS Lookup List table.
fn parse_gpos_lookups(data: &[u8], offset: usize) -> Vec<GposLookup> {
    let count = match read_u16(data, offset) {
        Some(c) => c as usize,
        None => return Vec::new(),
    };
    let mut lookups = Vec::with_capacity(count);
    for i in 0..count {
        let lookup_offset = match read_u16(data, offset + 2 + i * 2) {
            Some(o) => offset + o as usize,
            None => break,
        };
        lookups.push(parse_gpos_lookup(data, lookup_offset));
    }
    lookups
}

fn parse_gpos_lookup(data: &[u8], offset: usize) -> GposLookup {
    let lookup_type = match read_u16(data, offset) {
        Some(t) => t,
        None => return GposLookup::Unsupported { lookup_type: 0 },
    };
    let _flags = read_u16(data, offset + 2);
    let subtable_count = match read_u16(data, offset + 4) {
        Some(c) => c as usize,
        None => return GposLookup::Unsupported { lookup_type },
    };

    let mut subtable_offsets = Vec::with_capacity(subtable_count);
    for i in 0..subtable_count {
        if let Some(o) = read_u16(data, offset + 6 + i * 2) {
            subtable_offsets.push(offset + o as usize);
        }
    }

    match lookup_type {
        2 => parse_gpos_pair(data, &subtable_offsets),
        _ => GposLookup::Unsupported { lookup_type },
    }
}

/// Compute the byte-size of a ValueRecord given the ValueFormat bitmask.
fn value_record_size(format: u16) -> usize {
    // Each set bit means one i16 field (2 bytes).
    (format.count_ones() as usize) * 2
}

/// Read the XAdvance field from a ValueRecord, if present.
/// The XAdvance is the 3rd field (bit 0x0004), preceded by XPlacement (0x0001)
/// and YPlacement (0x0002).
fn read_x_advance(data: &[u8], offset: usize, format: u16) -> i16 {
    if format & 0x0004 == 0 {
        return 0;
    }
    // Count how many i16 fields come before XAdvance.
    let mut skip = 0;
    if format & 0x0001 != 0 {
        skip += 1; // XPlacement
    }
    if format & 0x0002 != 0 {
        skip += 1; // YPlacement
    }
    read_i16(data, offset + skip * 2).unwrap_or(0)
}

fn parse_gpos_pair(data: &[u8], subtable_offsets: &[usize]) -> GposLookup {
    let mut pairs = Vec::new();
    for &st in subtable_offsets {
        let format = match read_u16(data, st) {
            Some(f) => f,
            None => continue,
        };
        match format {
            1 => {
                // Format 1: individual pair sets
                let cov_offset = match read_u16(data, st + 2) {
                    Some(o) => st + o as usize,
                    None => continue,
                };
                let val_fmt1 = match read_u16(data, st + 4) {
                    Some(f) => f,
                    None => continue,
                };
                let val_fmt2 = match read_u16(data, st + 6) {
                    Some(f) => f,
                    None => continue,
                };
                let pair_set_count = match read_u16(data, st + 8) {
                    Some(c) => c as usize,
                    None => continue,
                };
                let coverage = parse_coverage(data, cov_offset);
                let vr1_size = value_record_size(val_fmt1);
                let vr2_size = value_record_size(val_fmt2);
                let pvr_size = 2 + vr1_size + vr2_size; // secondGlyph + value1 + value2

                for (i, &first_glyph) in coverage.iter().enumerate() {
                    if i >= pair_set_count {
                        break;
                    }
                    let ps_offset = match read_u16(data, st + 10 + i * 2) {
                        Some(o) => st + o as usize,
                        None => continue,
                    };
                    let pvr_count = match read_u16(data, ps_offset) {
                        Some(c) => c as usize,
                        None => continue,
                    };
                    for j in 0..pvr_count {
                        let rec = ps_offset + 2 + j * pvr_size;
                        let second = match read_u16(data, rec) {
                            Some(s) => s,
                            None => break,
                        };
                        let x_adv = read_x_advance(data, rec + 2, val_fmt1);
                        if x_adv != 0 {
                            pairs.push(KerningPair {
                                first: first_glyph,
                                second,
                                x_advance: x_adv,
                            });
                        }
                    }
                }
            }
            2 => {
                // Format 2: class-based - skip for now, store nothing
                // (class-based kerning is complex; we only handle format 1)
            }
            _ => {}
        }
    }
    GposLookup::PairAdjustment(pairs)
}

// ---------------------------------------------------------------------------
// Top-level layout table parser
// ---------------------------------------------------------------------------

/// Parse a GSUB or GPOS table given the table data slice.
/// Returns (scripts, features, lookup_list_offset).
fn parse_layout_header(
    data: &[u8],
    table_offset: usize,
) -> Option<(usize, usize, usize)> {
    let version = read_u32(data, table_offset)?;
    if version != 0x00010000 {
        return None;
    }
    let script_off = read_u16(data, table_offset + 4)? as usize;
    let feature_off = read_u16(data, table_offset + 6)? as usize;
    let lookup_off = read_u16(data, table_offset + 8)? as usize;
    Some((
        table_offset + script_off,
        table_offset + feature_off,
        table_offset + lookup_off,
    ))
}

/// Parse OpenType layout tables from raw font data.
///
/// `font_data` is the complete TTF/OTF binary. Returns `None` if neither
/// GSUB nor GPOS tables are present.
pub fn parse_opentype_layout(font_data: &[u8]) -> Option<OpenTypeLayout> {
    let gsub = find_table(font_data, b"GSUB").and_then(|(offset, _len)| {
        let (script_off, feature_off, lookup_off) = parse_layout_header(font_data, offset)?;
        Some(GsubTable {
            scripts: parse_script_list(font_data, script_off),
            features: parse_feature_list(font_data, feature_off),
            lookups: parse_gsub_lookups(font_data, lookup_off),
        })
    });

    let gpos = find_table(font_data, b"GPOS").and_then(|(offset, _len)| {
        let (script_off, feature_off, lookup_off) = parse_layout_header(font_data, offset)?;
        Some(GposTable {
            scripts: parse_script_list(font_data, script_off),
            features: parse_feature_list(font_data, feature_off),
            lookups: parse_gpos_lookups(font_data, lookup_off),
        })
    });

    if gsub.is_none() && gpos.is_none() {
        return None;
    }

    Some(OpenTypeLayout { gsub, gpos })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to build big-endian u16.
    fn be16(v: u16) -> [u8; 2] {
        v.to_be_bytes()
    }

    /// Helper to build big-endian u32.
    fn be32(v: u32) -> [u8; 4] {
        v.to_be_bytes()
    }

    /// Helper to build big-endian i16.
    fn bei16(v: i16) -> [u8; 2] {
        v.to_be_bytes()
    }

    // -- Coverage tests --

    #[test]
    fn test_coverage_format1() {
        // Format 1: glyph list
        let mut data = Vec::new();
        data.extend_from_slice(&be16(1)); // format
        data.extend_from_slice(&be16(3)); // count
        data.extend_from_slice(&be16(10)); // glyph 10
        data.extend_from_slice(&be16(20)); // glyph 20
        data.extend_from_slice(&be16(30)); // glyph 30

        let cov = parse_coverage(&data, 0);
        assert_eq!(cov, vec![10, 20, 30]);
    }

    #[test]
    fn test_coverage_format2() {
        // Format 2: range list
        let mut data = Vec::new();
        data.extend_from_slice(&be16(2)); // format
        data.extend_from_slice(&be16(2)); // range count
        // Range 1: 5..=7
        data.extend_from_slice(&be16(5));
        data.extend_from_slice(&be16(7));
        data.extend_from_slice(&be16(0)); // startCoverageIndex
        // Range 2: 100..=102
        data.extend_from_slice(&be16(100));
        data.extend_from_slice(&be16(102));
        data.extend_from_slice(&be16(3)); // startCoverageIndex

        let cov = parse_coverage(&data, 0);
        assert_eq!(cov, vec![5, 6, 7, 100, 101, 102]);
    }

    #[test]
    fn test_coverage_empty() {
        let cov = parse_coverage(&[], 0);
        assert!(cov.is_empty());
    }

    #[test]
    fn test_coverage_unknown_format() {
        let mut data = Vec::new();
        data.extend_from_slice(&be16(99)); // unknown format
        data.extend_from_slice(&be16(0));
        let cov = parse_coverage(&data, 0);
        assert!(cov.is_empty());
    }

    // -- Script List tests --

    #[test]
    fn test_parse_script_list() {
        // Build a minimal script list with one script, one default LangSys
        let mut data = vec![0u8; 256];
        let base = 0;

        // ScriptList at base:
        // scriptCount = 1
        data[base..base + 2].copy_from_slice(&be16(1));
        // ScriptRecord[0]: tag = "latn", offset
        data[base + 2..base + 6].copy_from_slice(b"latn");
        let script_table_off = 8u16;
        data[base + 6..base + 8].copy_from_slice(&be16(script_table_off));

        // Script table at base + 8:
        let st = base + script_table_off as usize;
        // defaultLangSysOffset = 4 (relative to script table)
        data[st..st + 2].copy_from_slice(&be16(4));
        // langSysCount = 0
        data[st + 2..st + 4].copy_from_slice(&be16(0));

        // LangSys at st + 4:
        let ls = st + 4;
        data[ls..ls + 2].copy_from_slice(&be16(0)); // lookupOrder (reserved)
        data[ls + 2..ls + 4].copy_from_slice(&be16(0xFFFF)); // no required feature
        data[ls + 4..ls + 6].copy_from_slice(&be16(2)); // 2 feature indices
        data[ls + 6..ls + 8].copy_from_slice(&be16(0)); // feature 0
        data[ls + 8..ls + 10].copy_from_slice(&be16(1)); // feature 1

        let scripts = parse_script_list(&data, base);
        assert_eq!(scripts.len(), 1);
        assert_eq!(&scripts[0].tag, b"latn");
        let def_ls = scripts[0].default_lang_sys.as_ref().unwrap();
        assert!(def_ls.required_feature_index.is_none());
        assert_eq!(def_ls.feature_indices, vec![0, 1]);
        assert!(scripts[0].lang_sys.is_empty());
    }

    // -- Feature List tests --

    #[test]
    fn test_parse_feature_list() {
        let mut data = vec![0u8; 256];
        let base = 0;

        // featureCount = 2
        data[base..base + 2].copy_from_slice(&be16(2));

        // Feature record 0: "liga", offset 14
        data[base + 2..base + 6].copy_from_slice(b"liga");
        data[base + 6..base + 8].copy_from_slice(&be16(14));

        // Feature record 1: "kern", offset 22
        data[base + 8..base + 12].copy_from_slice(b"kern");
        data[base + 12..base + 14].copy_from_slice(&be16(22));

        // Feature table 0 at base + 14:
        let f0 = base + 14;
        data[f0..f0 + 2].copy_from_slice(&be16(0)); // featureParams
        data[f0 + 2..f0 + 4].copy_from_slice(&be16(1)); // lookupIndexCount
        data[f0 + 4..f0 + 6].copy_from_slice(&be16(0)); // lookup 0

        // Feature table 1 at base + 22:
        let f1 = base + 22;
        data[f1..f1 + 2].copy_from_slice(&be16(0));
        data[f1 + 2..f1 + 4].copy_from_slice(&be16(2));
        data[f1 + 4..f1 + 6].copy_from_slice(&be16(1));
        data[f1 + 6..f1 + 8].copy_from_slice(&be16(2));

        let features = parse_feature_list(&data, base);
        assert_eq!(features.len(), 2);
        assert_eq!(&features[0].tag, b"liga");
        assert_eq!(features[0].lookup_indices, vec![0]);
        assert_eq!(&features[1].tag, b"kern");
        assert_eq!(features[1].lookup_indices, vec![1, 2]);
    }

    // -- GSUB Single Substitution tests --

    #[test]
    fn test_gsub_single_format1() {
        // Build a minimal GSUB single sub format 1 (delta)
        let mut data = vec![0u8; 256];

        // Coverage at offset 0: format 1, 2 glyphs
        data[0..2].copy_from_slice(&be16(1)); // format
        data[2..4].copy_from_slice(&be16(2)); // count
        data[4..6].copy_from_slice(&be16(10)); // glyph 10
        data[6..8].copy_from_slice(&be16(20)); // glyph 20

        // Subtable at offset 50: format 1
        let st = 50;
        data[st..st + 2].copy_from_slice(&be16(1)); // format 1
        // Coverage offset relative to subtable: 50 -> 0 means offset = -50? No, it's relative.
        // coverageOffset is relative to the subtable start.
        // We need coverage at st + cov_off = 0, so cov_off = -50. That won't work.
        // Let's put coverage after the subtable.
        // Subtable at 50, coverage at 56.
        data[st + 2..st + 4].copy_from_slice(&be16(6)); // coverage at st + 6 = 56
        data[st + 4..st + 6].copy_from_slice(&bei16(5)); // delta = +5

        // Coverage at 56
        let cov = 56;
        data[cov..cov + 2].copy_from_slice(&be16(1));
        data[cov + 2..cov + 4].copy_from_slice(&be16(2));
        data[cov + 4..cov + 6].copy_from_slice(&be16(10));
        data[cov + 6..cov + 8].copy_from_slice(&be16(20));

        let result = parse_gsub_single(&data, &[st]);
        match result {
            GsubLookup::Single(mappings) => {
                assert_eq!(mappings.len(), 2);
                assert_eq!(mappings[0], (10, 15));
                assert_eq!(mappings[1], (20, 25));
            }
            _ => panic!("expected Single"),
        }
    }

    #[test]
    fn test_gsub_single_format2() {
        let mut data = vec![0u8; 256];
        let st = 0;

        // Subtable format 2
        data[st..st + 2].copy_from_slice(&be16(2)); // format 2
        data[st + 2..st + 4].copy_from_slice(&be16(10)); // coverage at st + 10
        data[st + 4..st + 6].copy_from_slice(&be16(2)); // substitute count
        data[st + 6..st + 8].copy_from_slice(&be16(100)); // substitute for glyph 0
        data[st + 8..st + 10].copy_from_slice(&be16(200)); // substitute for glyph 1

        // Coverage at st + 10
        let cov = st + 10;
        data[cov..cov + 2].copy_from_slice(&be16(1));
        data[cov + 2..cov + 4].copy_from_slice(&be16(2));
        data[cov + 4..cov + 6].copy_from_slice(&be16(5));
        data[cov + 6..cov + 8].copy_from_slice(&be16(15));

        let result = parse_gsub_single(&data, &[st]);
        match result {
            GsubLookup::Single(mappings) => {
                assert_eq!(mappings.len(), 2);
                assert_eq!(mappings[0], (5, 100));
                assert_eq!(mappings[1], (15, 200));
            }
            _ => panic!("expected Single"),
        }
    }

    // -- GSUB Ligature tests --

    #[test]
    fn test_gsub_ligature() {
        let mut data = vec![0u8; 512];
        let st = 0;

        // Subtable: format 1
        data[st..st + 2].copy_from_slice(&be16(1)); // format
        data[st + 2..st + 4].copy_from_slice(&be16(100)); // coverage at st + 100
        data[st + 4..st + 6].copy_from_slice(&be16(1)); // ligSetCount = 1
        data[st + 6..st + 8].copy_from_slice(&be16(50)); // ligSetOffset[0] at st + 50

        // LigatureSet at st + 50
        let ls = st + 50;
        data[ls..ls + 2].copy_from_slice(&be16(1)); // ligCount = 1
        data[ls + 2..ls + 4].copy_from_slice(&be16(10)); // ligOffset[0] at ls + 10 = 60

        // Ligature at 60
        let lig = 60;
        data[lig..lig + 2].copy_from_slice(&be16(500)); // ligature glyph
        data[lig + 2..lig + 4].copy_from_slice(&be16(3)); // compCount = 3 (first + 2 more)
        data[lig + 4..lig + 6].copy_from_slice(&be16(20)); // component 2
        data[lig + 6..lig + 8].copy_from_slice(&be16(30)); // component 3

        // Coverage at st + 100
        let cov = 100;
        data[cov..cov + 2].copy_from_slice(&be16(1));
        data[cov + 2..cov + 4].copy_from_slice(&be16(1));
        data[cov + 4..cov + 6].copy_from_slice(&be16(10)); // first glyph

        let result = parse_gsub_ligature(&data, &[st]);
        match result {
            GsubLookup::Ligature(sets) => {
                assert_eq!(sets.len(), 1);
                assert_eq!(sets[0].first_glyph, 10);
                assert_eq!(sets[0].ligatures.len(), 1);
                assert_eq!(sets[0].ligatures[0].ligature_glyph, 500);
                assert_eq!(sets[0].ligatures[0].component_glyphs, vec![20, 30]);
            }
            _ => panic!("expected Ligature"),
        }
    }

    // -- GPOS Pair Adjustment tests --

    #[test]
    fn test_gpos_pair_format1() {
        let mut data = vec![0u8; 512];
        let st = 0;

        // PairPos format 1
        data[st..st + 2].copy_from_slice(&be16(1)); // format
        data[st + 2..st + 4].copy_from_slice(&be16(200)); // coverage at st + 200
        data[st + 4..st + 6].copy_from_slice(&be16(0x0004)); // valueFormat1: XAdvance only
        data[st + 6..st + 8].copy_from_slice(&be16(0)); // valueFormat2: nothing
        data[st + 8..st + 10].copy_from_slice(&be16(1)); // pairSetCount = 1
        data[st + 10..st + 12].copy_from_slice(&be16(100)); // pairSetOffset[0] at st + 100

        // PairSet at st + 100
        let ps = 100;
        data[ps..ps + 2].copy_from_slice(&be16(2)); // pairValueCount = 2
        // PairValueRecord size = 2 (secondGlyph) + 2 (xAdvance) + 0 = 4
        // Record 0: second=20, xAdvance=-50
        data[ps + 2..ps + 4].copy_from_slice(&be16(20));
        data[ps + 4..ps + 6].copy_from_slice(&bei16(-50));
        // Record 1: second=30, xAdvance=-80
        data[ps + 6..ps + 8].copy_from_slice(&be16(30));
        data[ps + 8..ps + 10].copy_from_slice(&bei16(-80));

        // Coverage at st + 200
        let cov = 200;
        data[cov..cov + 2].copy_from_slice(&be16(1));
        data[cov + 2..cov + 4].copy_from_slice(&be16(1));
        data[cov + 4..cov + 6].copy_from_slice(&be16(10)); // first glyph

        let result = parse_gpos_pair(&data, &[st]);
        match result {
            GposLookup::PairAdjustment(pairs) => {
                assert_eq!(pairs.len(), 2);
                assert_eq!(pairs[0].first, 10);
                assert_eq!(pairs[0].second, 20);
                assert_eq!(pairs[0].x_advance, -50);
                assert_eq!(pairs[1].first, 10);
                assert_eq!(pairs[1].second, 30);
                assert_eq!(pairs[1].x_advance, -80);
            }
            _ => panic!("expected PairAdjustment"),
        }
    }

    // -- Empty / missing table tests --

    #[test]
    fn test_parse_empty_data() {
        assert!(parse_opentype_layout(&[]).is_none());
    }

    #[test]
    fn test_parse_no_layout_tables() {
        // Minimal font header with 0 tables
        let mut data = vec![0u8; 12];
        data[0..4].copy_from_slice(&be32(0x00010000)); // sfVersion
        data[4..6].copy_from_slice(&be16(0)); // numTables = 0
        assert!(parse_opentype_layout(&data).is_none());
    }

    #[test]
    fn test_find_table_not_found() {
        let mut data = vec![0u8; 12];
        data[0..4].copy_from_slice(&be32(0x00010000));
        data[4..6].copy_from_slice(&be16(0));
        assert!(find_table(&data, b"GSUB").is_none());
    }

    #[test]
    fn test_invalid_version() {
        // Build a font with a GSUB table that has wrong version
        let mut data = vec![0u8; 128];
        // Font header
        data[0..4].copy_from_slice(&be32(0x00010000));
        data[4..6].copy_from_slice(&be16(1)); // 1 table
        // Table record for GSUB
        data[12..16].copy_from_slice(b"GSUB");
        data[16..20].copy_from_slice(&be32(0)); // checksum
        data[20..24].copy_from_slice(&be32(50)); // offset
        data[24..28].copy_from_slice(&be32(20)); // length
        // GSUB table at offset 50 with wrong version
        data[50..54].copy_from_slice(&be32(0x00020000)); // version 2 (unsupported)

        assert!(parse_opentype_layout(&data).is_none());
    }

    #[test]
    fn test_script_list_with_lang_sys() {
        let mut data = vec![0u8; 256];
        let base = 0;

        // scriptCount = 1
        data[base..base + 2].copy_from_slice(&be16(1));
        data[base + 2..base + 6].copy_from_slice(b"latn");
        data[base + 6..base + 8].copy_from_slice(&be16(8)); // script table at base + 8

        let st = base + 8;
        // defaultLangSysOffset = 0 (no default)
        data[st..st + 2].copy_from_slice(&be16(0));
        // langSysCount = 1
        data[st + 2..st + 4].copy_from_slice(&be16(1));
        // LangSysRecord: tag "DEU ", offset 10 (relative to script table)
        data[st + 4..st + 8].copy_from_slice(b"DEU ");
        data[st + 8..st + 10].copy_from_slice(&be16(10));

        // LangSys at st + 10
        let ls = st + 10;
        data[ls..ls + 2].copy_from_slice(&be16(0)); // lookupOrder
        data[ls + 2..ls + 4].copy_from_slice(&be16(3)); // required feature = 3
        data[ls + 4..ls + 6].copy_from_slice(&be16(1)); // 1 feature index
        data[ls + 6..ls + 8].copy_from_slice(&be16(5)); // feature 5

        let scripts = parse_script_list(&data, base);
        assert_eq!(scripts.len(), 1);
        assert!(scripts[0].default_lang_sys.is_none());
        assert_eq!(scripts[0].lang_sys.len(), 1);
        assert_eq!(&scripts[0].lang_sys[0].0, b"DEU ");
        assert_eq!(scripts[0].lang_sys[0].1.required_feature_index, Some(3));
        assert_eq!(scripts[0].lang_sys[0].1.feature_indices, vec![5]);
    }

    #[test]
    fn test_gpos_pair_zero_advance_filtered() {
        // Pairs with zero x_advance should not appear in results
        let mut data = vec![0u8; 512];
        let st = 0;

        data[st..st + 2].copy_from_slice(&be16(1)); // format 1
        data[st + 2..st + 4].copy_from_slice(&be16(200)); // coverage
        data[st + 4..st + 6].copy_from_slice(&be16(0x0004)); // valueFormat1: XAdvance
        data[st + 6..st + 8].copy_from_slice(&be16(0)); // valueFormat2: none
        data[st + 8..st + 10].copy_from_slice(&be16(1)); // pairSetCount
        data[st + 10..st + 12].copy_from_slice(&be16(100)); // pairSetOffset

        let ps = 100;
        data[ps..ps + 2].copy_from_slice(&be16(1)); // 1 pair
        data[ps + 2..ps + 4].copy_from_slice(&be16(20)); // second glyph
        data[ps + 4..ps + 6].copy_from_slice(&bei16(0)); // x_advance = 0

        let cov = 200;
        data[cov..cov + 2].copy_from_slice(&be16(1));
        data[cov + 2..cov + 4].copy_from_slice(&be16(1));
        data[cov + 4..cov + 6].copy_from_slice(&be16(10));

        let result = parse_gpos_pair(&data, &[st]);
        match result {
            GposLookup::PairAdjustment(pairs) => {
                assert!(pairs.is_empty());
            }
            _ => panic!("expected PairAdjustment"),
        }
    }
}
