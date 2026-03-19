//! TrueType font subsetting.
//!
//! Takes a TrueType font binary and a set of glyph IDs, and produces a new
//! font binary containing only those glyphs. This is used when embedding fonts
//! in PDF to reduce file size.

use std::collections::{BTreeSet, HashMap};

/// Result of font subsetting.
#[derive(Debug)]
pub struct SubsetResult {
    /// The subsetted font binary data.
    pub data: Vec<u8>,
    /// Mapping from old glyph IDs to new glyph IDs.
    pub gid_map: HashMap<u16, u16>,
}

/// A parsed TrueType table record.
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct TableRecord {
    tag: [u8; 4],
    checksum: u32,
    offset: u32,
    length: u32,
}

/// Tables we keep in the subset font (in recommended order).
const KEPT_TABLES: &[[u8; 4]] = &[
    *b"head", *b"hhea", *b"maxp", *b"OS/2", *b"name", *b"cmap", *b"loca", *b"glyf", *b"hmtx",
    *b"post", *b"cvt ", *b"fpgm", *b"prep",
];

// Composite glyph flags.
const ARG_1_AND_2_ARE_WORDS: u16 = 0x0001;
const WE_HAVE_A_SCALE: u16 = 0x0008;
const MORE_COMPONENTS: u16 = 0x0020;
const WE_HAVE_AN_X_AND_Y_SCALE: u16 = 0x0040;
const WE_HAVE_A_TWO_BY_TWO: u16 = 0x0080;

/// Subset a TrueType font to include only the specified glyph IDs.
///
/// `font_data` is the raw TTF binary.
/// `glyph_ids` is the set of glyph IDs to keep.
/// Returns the subsetted font data and a mapping from old to new glyph IDs.
///
/// Returns `None` if the font data is invalid, too short, or uses CFF outlines.
pub fn subset_font(font_data: &[u8], glyph_ids: &[u16]) -> Option<SubsetResult> {
    if font_data.len() < 12 {
        return None;
    }

    // Parse offset table.
    let sf_version = read_u32(font_data, 0)?;
    // Reject CFF/OpenType fonts (tag 'OTTO' = 0x4F54544F).
    if sf_version == 0x4F54544F {
        return None;
    }
    // Accept TrueType: 0x00010000 or 'true' (0x74727565).
    if sf_version != 0x00010000 && sf_version != 0x74727565 {
        return None;
    }

    let num_tables = read_u16(font_data, 4)? as usize;

    // Parse table records.
    let mut tables: Vec<TableRecord> = Vec::with_capacity(num_tables);
    for i in 0..num_tables {
        let rec_offset = 12 + i * 16;
        if rec_offset + 16 > font_data.len() {
            return None;
        }
        let mut tag = [0u8; 4];
        tag.copy_from_slice(&font_data[rec_offset..rec_offset + 4]);
        tables.push(TableRecord {
            tag,
            checksum: read_u32(font_data, rec_offset + 4)?,
            offset: read_u32(font_data, rec_offset + 8)?,
            length: read_u32(font_data, rec_offset + 12)?,
        });
    }

    // Look up essential tables.
    let find_table = |tag: &[u8; 4]| -> Option<&TableRecord> {
        tables.iter().find(|t| &t.tag == tag)
    };

    let head_rec = find_table(b"head")?;
    let maxp_rec = find_table(b"maxp")?;
    let loca_rec = find_table(b"loca")?;
    let glyf_rec = find_table(b"glyf")?;
    let hhea_rec = find_table(b"hhea")?;
    let hmtx_rec = find_table(b"hmtx")?;

    // Read head.indexToLocFormat (offset 50 within the head table).
    let head_data = table_data(font_data, head_rec)?;
    if head_data.len() < 54 {
        return None;
    }
    let index_to_loc_format = read_i16(head_data, 50)?;

    // Read maxp.numGlyphs (offset 4 within maxp).
    let maxp_data = table_data(font_data, maxp_rec)?;
    if maxp_data.len() < 6 {
        return None;
    }
    let total_glyphs = read_u16(maxp_data, 4)? as usize;

    // Read number of long horizontal metrics from hhea (offset 34).
    let hhea_data = table_data(font_data, hhea_rec)?;
    if hhea_data.len() < 36 {
        return None;
    }
    let num_h_metrics = read_u16(hhea_data, 34)? as usize;

    // Parse loca table to get glyph offsets.
    let loca_data = table_data(font_data, loca_rec)?;
    let glyf_data = table_data(font_data, glyf_rec)?;

    let glyph_offsets = parse_loca(loca_data, index_to_loc_format, total_glyphs)?;

    // Build the set of glyphs to keep: always include glyph 0, plus requested glyphs,
    // plus any components referenced by composite glyphs.
    let mut keep_gids: BTreeSet<u16> = BTreeSet::new();
    keep_gids.insert(0); // .notdef
    for &gid in glyph_ids {
        if (gid as usize) < total_glyphs {
            keep_gids.insert(gid);
        }
    }

    // Recursively find composite glyph components.
    let mut work: Vec<u16> = keep_gids.iter().copied().collect();
    while let Some(gid) = work.pop() {
        let start = glyph_offsets[gid as usize];
        let end = glyph_offsets[gid as usize + 1];
        if start >= end {
            continue; // empty glyph
        }
        let glyph_slice = glyf_data.get(start..end)?;
        if glyph_slice.len() < 2 {
            continue;
        }
        let num_contours = read_i16(glyph_slice, 0)?;
        if num_contours >= 0 {
            continue; // simple glyph
        }
        // Composite glyph: extract component glyph IDs.
        let components = parse_composite_glyph_components(glyph_slice)?;
        for comp_gid in components {
            if (comp_gid as usize) < total_glyphs && keep_gids.insert(comp_gid) {
                work.push(comp_gid);
            }
        }
    }

    // Build old-to-new GID mapping (sorted, sequential).
    let sorted_gids: Vec<u16> = keep_gids.iter().copied().collect();
    let mut gid_map: HashMap<u16, u16> = HashMap::new();
    for (new_gid, &old_gid) in sorted_gids.iter().enumerate() {
        gid_map.insert(old_gid, new_gid as u16);
    }
    let new_num_glyphs = sorted_gids.len() as u16;

    // Build new glyf table, updating composite glyph references.
    let mut new_glyf: Vec<u8> = Vec::new();
    let mut new_loca_offsets: Vec<u32> = Vec::with_capacity(sorted_gids.len() + 1);

    for &old_gid in &sorted_gids {
        new_loca_offsets.push(new_glyf.len() as u32);
        let start = glyph_offsets[old_gid as usize];
        let end = glyph_offsets[old_gid as usize + 1];
        if start >= end {
            continue; // empty glyph, offset stays the same
        }
        let glyph_slice = glyf_data.get(start..end)?;
        let num_contours = read_i16(glyph_slice, 0)?;
        if num_contours >= 0 {
            // Simple glyph: copy as-is.
            new_glyf.extend_from_slice(glyph_slice);
        } else {
            // Composite glyph: rewrite component GIDs.
            let mut patched = glyph_slice.to_vec();
            rewrite_composite_glyph_ids(&mut patched, &gid_map)?;
            new_glyf.extend_from_slice(&patched);
        }
        // Pad to 4-byte boundary.
        while new_glyf.len() % 4 != 0 {
            new_glyf.push(0);
        }
    }
    // Final loca entry: end of last glyph.
    new_loca_offsets.push(new_glyf.len() as u32);

    // Build new loca table (use long format for simplicity).
    let new_index_to_loc_format: i16 = 1; // long format
    let mut new_loca: Vec<u8> = Vec::with_capacity(new_loca_offsets.len() * 4);
    for &off in &new_loca_offsets {
        new_loca.extend_from_slice(&off.to_be_bytes());
    }

    // Build new hmtx table.
    let hmtx_data = table_data(font_data, hmtx_rec)?;
    let new_hmtx = build_subset_hmtx(hmtx_data, &sorted_gids, num_h_metrics, total_glyphs)?;

    // Patch head table: update indexToLocFormat and zero out checksumAdjustment.
    let mut new_head = head_data.to_vec();
    // Zero checksumAdjustment (offset 8, 4 bytes) — we fix it later.
    write_u32(&mut new_head, 8, 0);
    // Set indexToLocFormat to 1 (long).
    write_i16(&mut new_head, 50, new_index_to_loc_format);

    // Patch maxp table: update numGlyphs.
    let mut new_maxp = maxp_data.to_vec();
    write_u16(&mut new_maxp, 4, new_num_glyphs);

    // Patch hhea table: update numberOfHMetrics to new_num_glyphs.
    let mut new_hhea = hhea_data.to_vec();
    write_u16(&mut new_hhea, 34, new_num_glyphs);

    // Collect all table data for output.
    struct TableEntry {
        tag: [u8; 4],
        data: Vec<u8>,
    }

    let mut out_tables: Vec<TableEntry> = Vec::new();

    for kept_tag in KEPT_TABLES {
        let data: Vec<u8> = match kept_tag {
            b"head" => new_head.clone(),
            b"hhea" => new_hhea.clone(),
            b"maxp" => new_maxp.clone(),
            b"loca" => new_loca.clone(),
            b"glyf" => new_glyf.clone(),
            b"hmtx" => new_hmtx.clone(),
            _ => {
                // Copy the original table if it exists; skip if not.
                match find_table(kept_tag) {
                    Some(rec) => table_data(font_data, rec)?.to_vec(),
                    None => continue,
                }
            }
        };
        out_tables.push(TableEntry {
            tag: *kept_tag,
            data,
        });
    }

    // Assemble the final font binary.
    let num_out_tables = out_tables.len() as u16;
    let (search_range, entry_selector, range_shift) = calc_table_search_params(num_out_tables);

    // Offset table: 12 bytes.
    // Table records: 16 bytes each.
    let header_size = 12 + (num_out_tables as usize) * 16;
    let mut output: Vec<u8> = Vec::new();

    // Write offset table.
    output.extend_from_slice(&0x00010000u32.to_be_bytes()); // sfVersion
    output.extend_from_slice(&num_out_tables.to_be_bytes());
    output.extend_from_slice(&search_range.to_be_bytes());
    output.extend_from_slice(&entry_selector.to_be_bytes());
    output.extend_from_slice(&range_shift.to_be_bytes());

    // We need to compute the offsets for each table data block.
    // Table data starts right after the header.
    let mut data_offset = header_size;
    // Pad each table to 4-byte boundary.
    struct TableOut {
        tag: [u8; 4],
        checksum: u32,
        offset: u32,
        padded_data: Vec<u8>,
    }

    let mut table_outs: Vec<TableOut> = Vec::new();
    for entry in &out_tables {
        let mut padded = entry.data.clone();
        while padded.len() % 4 != 0 {
            padded.push(0);
        }
        let cs = calc_checksum(&padded);
        table_outs.push(TableOut {
            tag: entry.tag,
            checksum: cs,
            offset: data_offset as u32,
            padded_data: padded.clone(),
        });
        data_offset += padded.len();
    }

    // Write table records.
    for t in &table_outs {
        output.extend_from_slice(&t.tag);
        output.extend_from_slice(&t.checksum.to_be_bytes());
        output.extend_from_slice(&t.offset.to_be_bytes());
        // Length is the unpadded length.
        let unpadded_len = out_tables
            .iter()
            .find(|e| e.tag == t.tag)
            .map(|e| e.data.len() as u32)
            .unwrap_or(t.padded_data.len() as u32);
        output.extend_from_slice(&unpadded_len.to_be_bytes());
    }

    // Write table data.
    for t in &table_outs {
        output.extend_from_slice(&t.padded_data);
    }

    // Fix head.checksumAdjustment.
    // The adjustment is: 0xB1B0AFBA - checksum_of_entire_file.
    let file_checksum = calc_checksum(&output);
    let adjustment = 0xB1B0AFBAu32.wrapping_sub(file_checksum);

    // Find the head table offset in output and write the adjustment at offset 8.
    if let Some(head_out) = table_outs.iter().find(|t| &t.tag == b"head") {
        let adj_offset = head_out.offset as usize + 8;
        if adj_offset + 4 <= output.len() {
            write_u32(&mut output, adj_offset, adjustment);
        }
    }

    Some(SubsetResult { data: output, gid_map })
}

/// Parse the `loca` table into a vector of byte offsets into the `glyf` table.
/// Returns `total_glyphs + 1` entries (the last entry marks the end of the last glyph).
fn parse_loca(loca_data: &[u8], format: i16, num_glyphs: usize) -> Option<Vec<usize>> {
    let count = num_glyphs + 1;
    let mut offsets = Vec::with_capacity(count);
    match format {
        0 => {
            // Short format: u16 values, actual offset = value * 2.
            if loca_data.len() < count * 2 {
                return None;
            }
            for i in 0..count {
                let val = read_u16(loca_data, i * 2)? as usize;
                offsets.push(val * 2);
            }
        }
        1 => {
            // Long format: u32 values.
            if loca_data.len() < count * 4 {
                return None;
            }
            for i in 0..count {
                let val = read_u32(loca_data, i * 4)? as usize;
                offsets.push(val);
            }
        }
        _ => return None,
    }
    Some(offsets)
}

/// Parse a composite glyph to extract the component glyph IDs it references.
fn parse_composite_glyph_components(glyph_data: &[u8]) -> Option<Vec<u16>> {
    let mut components = Vec::new();
    // Skip the glyph header: numberOfContours (i16) + xMin, yMin, xMax, yMax (4 x i16) = 10 bytes.
    let mut pos = 10;

    loop {
        if pos + 4 > glyph_data.len() {
            return None;
        }
        let flags = read_u16(glyph_data, pos)?;
        let component_gid = read_u16(glyph_data, pos + 2)?;
        components.push(component_gid);
        pos += 4;

        // Skip arguments.
        if flags & ARG_1_AND_2_ARE_WORDS != 0 {
            pos += 4; // two i16 args
        } else {
            pos += 2; // two i8 args
        }

        // Skip transform data.
        if flags & WE_HAVE_A_SCALE != 0 {
            pos += 2; // one F2Dot14
        } else if flags & WE_HAVE_AN_X_AND_Y_SCALE != 0 {
            pos += 4; // two F2Dot14
        } else if flags & WE_HAVE_A_TWO_BY_TWO != 0 {
            pos += 8; // four F2Dot14
        }

        if flags & MORE_COMPONENTS == 0 {
            break;
        }
    }

    Some(components)
}

/// Rewrite component glyph IDs in a composite glyph using the gid_map.
fn rewrite_composite_glyph_ids(glyph_data: &mut [u8], gid_map: &HashMap<u16, u16>) -> Option<()> {
    let mut pos = 10; // skip glyph header

    loop {
        if pos + 4 > glyph_data.len() {
            return None;
        }
        let flags = read_u16(glyph_data, pos)?;
        let old_gid = read_u16(glyph_data, pos + 2)?;
        let new_gid = *gid_map.get(&old_gid)?;
        write_u16(glyph_data, pos + 2, new_gid);
        pos += 4;

        // Skip arguments.
        if flags & ARG_1_AND_2_ARE_WORDS != 0 {
            pos += 4;
        } else {
            pos += 2;
        }

        // Skip transform data.
        if flags & WE_HAVE_A_SCALE != 0 {
            pos += 2;
        } else if flags & WE_HAVE_AN_X_AND_Y_SCALE != 0 {
            pos += 4;
        } else if flags & WE_HAVE_A_TWO_BY_TWO != 0 {
            pos += 8;
        }

        if flags & MORE_COMPONENTS == 0 {
            break;
        }
    }

    Some(())
}

/// Build a new hmtx table for the subset glyphs.
fn build_subset_hmtx(
    hmtx_data: &[u8],
    sorted_gids: &[u16],
    num_h_metrics: usize,
    _total_glyphs: usize,
) -> Option<Vec<u8>> {
    // hmtx structure:
    //   numOfLongHorMetrics entries of (advanceWidth: u16, lsb: i16) = 4 bytes each
    //   Remaining glyphs: just lsb (i16) = 2 bytes each, using the last advanceWidth.
    let mut new_hmtx = Vec::new();

    for &old_gid in sorted_gids {
        let gid = old_gid as usize;
        if gid < num_h_metrics {
            // Full metric entry.
            let offset = gid * 4;
            if offset + 4 > hmtx_data.len() {
                // Fallback: write zeros.
                new_hmtx.extend_from_slice(&[0u8; 4]);
            } else {
                new_hmtx.extend_from_slice(&hmtx_data[offset..offset + 4]);
            }
        } else {
            // Glyph beyond numOfLongHorMetrics: use last advance width + per-glyph lsb.
            let last_aw_offset = (num_h_metrics - 1) * 4;
            let advance_width = if last_aw_offset + 2 <= hmtx_data.len() {
                &hmtx_data[last_aw_offset..last_aw_offset + 2]
            } else {
                &[0u8, 0]
            };
            let lsb_base = num_h_metrics * 4;
            let lsb_idx = gid - num_h_metrics;
            let lsb_offset = lsb_base + lsb_idx * 2;
            let lsb = if lsb_offset + 2 <= hmtx_data.len() {
                &hmtx_data[lsb_offset..lsb_offset + 2]
            } else {
                &[0u8, 0]
            };
            new_hmtx.extend_from_slice(advance_width);
            new_hmtx.extend_from_slice(lsb);
        }
    }

    Some(new_hmtx)
}

/// Calculate searchRange, entrySelector, rangeShift for the offset table.
fn calc_table_search_params(num_tables: u16) -> (u16, u16, u16) {
    let mut power = 1u16;
    let mut log2 = 0u16;
    while power * 2 <= num_tables {
        power *= 2;
        log2 += 1;
    }
    let search_range = power * 16;
    let entry_selector = log2;
    let range_shift = num_tables * 16 - search_range;
    (search_range, entry_selector, range_shift)
}

/// Calculate the checksum of a block of data (interpreted as big-endian u32 words).
fn calc_checksum(data: &[u8]) -> u32 {
    let mut sum: u32 = 0;
    let mut i = 0;
    while i + 4 <= data.len() {
        let word = u32::from_be_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]);
        sum = sum.wrapping_add(word);
        i += 4;
    }
    // Handle trailing bytes (data should be padded, but just in case).
    if i < data.len() {
        let mut last = [0u8; 4];
        for (j, &b) in data[i..].iter().enumerate() {
            last[j] = b;
        }
        sum = sum.wrapping_add(u32::from_be_bytes(last));
    }
    sum
}

/// Get the raw data slice for a table.
fn table_data<'a>(font_data: &'a [u8], rec: &TableRecord) -> Option<&'a [u8]> {
    let start = rec.offset as usize;
    let end = start + rec.length as usize;
    font_data.get(start..end)
}

// --- Binary read/write helpers ---

fn read_u32(data: &[u8], offset: usize) -> Option<u32> {
    let bytes = data.get(offset..offset + 4)?;
    Some(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn read_u16(data: &[u8], offset: usize) -> Option<u16> {
    let bytes = data.get(offset..offset + 2)?;
    Some(u16::from_be_bytes([bytes[0], bytes[1]]))
}

fn read_i16(data: &[u8], offset: usize) -> Option<i16> {
    let bytes = data.get(offset..offset + 2)?;
    Some(i16::from_be_bytes([bytes[0], bytes[1]]))
}

fn write_u32(data: &mut [u8], offset: usize, val: u32) {
    let bytes = val.to_be_bytes();
    data[offset..offset + 4].copy_from_slice(&bytes);
}

fn write_u16(data: &mut [u8], offset: usize, val: u16) {
    let bytes = val.to_be_bytes();
    data[offset..offset + 2].copy_from_slice(&bytes);
}

fn write_i16(data: &mut [u8], offset: usize, val: i16) {
    let bytes = val.to_be_bytes();
    data[offset..offset + 2].copy_from_slice(&bytes);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal valid TrueType font binary for testing.
    ///
    /// Creates a font with the required tables: head, hhea, maxp, loca, glyf, hmtx.
    /// Contains 3 glyphs:
    ///   - GID 0: .notdef (empty glyph, 0 bytes in glyf)
    ///   - GID 1: simple glyph (10 bytes of dummy contour data)
    ///   - GID 2: simple glyph (10 bytes of dummy contour data)
    fn build_test_font() -> Vec<u8> {
        build_test_font_with_glyphs(3, &[], false)
    }

    fn build_test_font_with_glyphs(
        num_glyphs: usize,
        composite_refs: &[(usize, Vec<u16>)], // (glyph_index, component_gids)
        _use_long_loca: bool,
    ) -> Vec<u8> {
        // We always use short loca format for test fonts.
        let index_to_loc_format: i16 = 0; // short

        // Build glyf table: glyph 0 is empty, rest are simple 12-byte glyphs
        // (or composite if specified).
        let mut glyf_entries: Vec<Vec<u8>> = Vec::new();
        for gid in 0..num_glyphs {
            if gid == 0 {
                // .notdef: empty
                glyf_entries.push(Vec::new());
                continue;
            }
            // Check if this glyph is composite.
            if let Some((_, components)) = composite_refs.iter().find(|(idx, _)| *idx == gid) {
                // Build composite glyph data.
                let mut data = Vec::new();
                // numberOfContours = -1 (composite)
                data.extend_from_slice(&(-1i16).to_be_bytes());
                // xMin, yMin, xMax, yMax
                data.extend_from_slice(&0i16.to_be_bytes());
                data.extend_from_slice(&0i16.to_be_bytes());
                data.extend_from_slice(&100i16.to_be_bytes());
                data.extend_from_slice(&100i16.to_be_bytes());
                // Component entries.
                for (i, &comp_gid) in components.iter().enumerate() {
                    let is_last = i == components.len() - 1;
                    let flags: u16 = if is_last { 0 } else { MORE_COMPONENTS };
                    data.extend_from_slice(&flags.to_be_bytes());
                    data.extend_from_slice(&comp_gid.to_be_bytes());
                    // Two i8 args (ARG_1_AND_2_ARE_WORDS not set).
                    data.push(0);
                    data.push(0);
                }
                // Pad to even length for short loca.
                while data.len() % 2 != 0 {
                    data.push(0);
                }
                glyf_entries.push(data);
            } else {
                // Simple glyph: 12 bytes (numberOfContours=1 + bbox + minimal data).
                let mut data = Vec::new();
                data.extend_from_slice(&1i16.to_be_bytes()); // numberOfContours
                data.extend_from_slice(&0i16.to_be_bytes()); // xMin
                data.extend_from_slice(&0i16.to_be_bytes()); // yMin
                data.extend_from_slice(&(100i16).to_be_bytes()); // xMax
                data.extend_from_slice(&(100i16).to_be_bytes()); // yMax
                // endPtsOfContours[0] = 0
                data.extend_from_slice(&0u16.to_be_bytes());
                glyf_entries.push(data);
            }
        }

        // Compute glyf table (concatenation of all glyph data).
        let mut glyf_table = Vec::new();
        let mut glyf_offsets: Vec<usize> = Vec::new();
        for entry in &glyf_entries {
            glyf_offsets.push(glyf_table.len());
            glyf_table.extend_from_slice(entry);
            // Pad individual glyph entries to 2-byte boundary (for short loca).
            while glyf_table.len() % 2 != 0 {
                glyf_table.push(0);
            }
        }
        glyf_offsets.push(glyf_table.len());

        // Build loca table (short format: offset / 2 as u16).
        let mut loca_table = Vec::new();
        for &off in &glyf_offsets {
            loca_table.extend_from_slice(&((off / 2) as u16).to_be_bytes());
        }

        // Build head table (54 bytes minimum).
        let mut head_table = vec![0u8; 54];
        // version = 1.0
        write_u32(&mut head_table, 0, 0x00010000);
        // magicNumber at offset 12
        write_u32(&mut head_table, 12, 0x5F0F3CF5);
        // flags at offset 16
        write_u16(&mut head_table, 16, 0x000B);
        // unitsPerEm at offset 18
        write_u16(&mut head_table, 18, 1000);
        // indexToLocFormat at offset 50
        write_i16(&mut head_table, 50, index_to_loc_format);

        // Build maxp table (6 bytes minimum: version + numGlyphs).
        let mut maxp_table = vec![0u8; 6];
        write_u32(&mut maxp_table, 0, 0x00010000);
        write_u16(&mut maxp_table, 4, num_glyphs as u16);

        // Build hhea table (36 bytes).
        let mut hhea_table = vec![0u8; 36];
        write_u32(&mut hhea_table, 0, 0x00010000);
        write_u16(&mut hhea_table, 34, num_glyphs as u16); // numberOfHMetrics

        // Build hmtx table (4 bytes per glyph: advanceWidth + lsb).
        let mut hmtx_table = Vec::new();
        for gid in 0..num_glyphs {
            let aw = (500 + gid * 100) as u16;
            let lsb = 10i16;
            hmtx_table.extend_from_slice(&aw.to_be_bytes());
            hmtx_table.extend_from_slice(&lsb.to_be_bytes());
        }

        // Assemble the font.
        let table_list: Vec<(&[u8; 4], &[u8])> = vec![
            (b"head", &head_table),
            (b"hhea", &hhea_table),
            (b"maxp", &maxp_table),
            (b"loca", &loca_table),
            (b"glyf", &glyf_table),
            (b"hmtx", &hmtx_table),
        ];

        let num_tables = table_list.len() as u16;
        let (sr, es, rs) = calc_table_search_params(num_tables);
        let header_size = 12 + (num_tables as usize) * 16;

        let mut font = Vec::new();
        // Offset table.
        font.extend_from_slice(&0x00010000u32.to_be_bytes());
        font.extend_from_slice(&num_tables.to_be_bytes());
        font.extend_from_slice(&sr.to_be_bytes());
        font.extend_from_slice(&es.to_be_bytes());
        font.extend_from_slice(&rs.to_be_bytes());

        // Compute table offsets.
        let mut data_offset = header_size;
        let mut table_records: Vec<(usize, usize)> = Vec::new(); // (offset, padded_len)
        for (_, data) in &table_list {
            let padded = (data.len() + 3) & !3;
            table_records.push((data_offset, padded));
            data_offset += padded;
        }

        // Write table records.
        for (i, (tag, data)) in table_list.iter().enumerate() {
            font.extend_from_slice(*tag);
            let mut padded_data = data.to_vec();
            while padded_data.len() % 4 != 0 {
                padded_data.push(0);
            }
            let cs = calc_checksum(&padded_data);
            font.extend_from_slice(&cs.to_be_bytes());
            font.extend_from_slice(&(table_records[i].0 as u32).to_be_bytes());
            font.extend_from_slice(&(data.len() as u32).to_be_bytes());
        }

        // Write table data.
        for (_, data) in &table_list {
            font.extend_from_slice(data);
            while font.len() % 4 != 0 {
                font.push(0);
            }
        }

        font
    }

    #[test]
    fn test_parse_offset_table_header() {
        let font = build_test_font();
        assert!(font.len() >= 12);
        let sf_version = read_u32(&font, 0).unwrap();
        assert_eq!(sf_version, 0x00010000);
        let num_tables = read_u16(&font, 4).unwrap();
        assert_eq!(num_tables, 6);
    }

    #[test]
    fn test_subset_empty_glyph_set() {
        // Subsetting with no glyph IDs should still include glyph 0 (.notdef).
        let font = build_test_font();
        let result = subset_font(&font, &[]).expect("subsetting should succeed");
        assert!(result.gid_map.contains_key(&0));
        assert_eq!(result.gid_map[&0], 0);
        assert_eq!(result.gid_map.len(), 1);
        // The output should be a valid font.
        assert!(result.data.len() > 12);
        let sf_version = read_u32(&result.data, 0).unwrap();
        assert_eq!(sf_version, 0x00010000);
    }

    #[test]
    fn test_subset_single_glyph() {
        let font = build_test_font();
        let result = subset_font(&font, &[1]).expect("subsetting should succeed");
        // Should have glyph 0 and glyph 1.
        assert_eq!(result.gid_map.len(), 2);
        assert_eq!(result.gid_map[&0], 0);
        assert_eq!(result.gid_map[&1], 1);
        // Output should parse back.
        let sf_version = read_u32(&result.data, 0).unwrap();
        assert_eq!(sf_version, 0x00010000);
        // Verify maxp in output has numGlyphs = 2.
        let num_tables = read_u16(&result.data, 4).unwrap() as usize;
        let mut found_maxp = false;
        for i in 0..num_tables {
            let rec_off = 12 + i * 16;
            let tag = &result.data[rec_off..rec_off + 4];
            if tag == b"maxp" {
                let offset = read_u32(&result.data, rec_off + 8).unwrap() as usize;
                let num_glyphs = read_u16(&result.data, offset + 4).unwrap();
                assert_eq!(num_glyphs, 2);
                found_maxp = true;
                break;
            }
        }
        assert!(found_maxp, "maxp table should be present");
    }

    #[test]
    fn test_gid_mapping_correctness() {
        let font = build_test_font(); // 3 glyphs: 0, 1, 2
        // Keep only glyph 2 (plus glyph 0 is always included).
        let result = subset_font(&font, &[2]).expect("subsetting should succeed");
        assert_eq!(result.gid_map.len(), 2);
        assert_eq!(result.gid_map[&0], 0); // .notdef stays at 0
        assert_eq!(result.gid_map[&2], 1); // old GID 2 -> new GID 1
    }

    #[test]
    fn test_subset_multiple_glyphs_ordering() {
        let font = build_test_font_with_glyphs(5, &[], false);
        let result = subset_font(&font, &[3, 1, 4]).expect("subsetting should succeed");
        // Should have glyphs 0, 1, 3, 4 -> new IDs 0, 1, 2, 3.
        assert_eq!(result.gid_map.len(), 4);
        assert_eq!(result.gid_map[&0], 0);
        assert_eq!(result.gid_map[&1], 1);
        assert_eq!(result.gid_map[&3], 2);
        assert_eq!(result.gid_map[&4], 3);
    }

    #[test]
    fn test_subset_composite_glyph_includes_components() {
        // Build a font with 4 glyphs:
        //   0: .notdef (empty)
        //   1: simple
        //   2: simple
        //   3: composite referencing GIDs 1 and 2
        let font = build_test_font_with_glyphs(4, &[(3, vec![1, 2])], false);
        // Request only glyph 3 (composite).
        let result = subset_font(&font, &[3]).expect("subsetting should succeed");
        // Should include 0, 1, 2, 3 (components pulled in automatically).
        assert_eq!(result.gid_map.len(), 4);
        assert!(result.gid_map.contains_key(&0));
        assert!(result.gid_map.contains_key(&1));
        assert!(result.gid_map.contains_key(&2));
        assert!(result.gid_map.contains_key(&3));
    }

    #[test]
    fn test_reject_cff_font() {
        // Build a minimal CFF/OpenType header.
        let mut data = vec![0u8; 64];
        data[0..4].copy_from_slice(b"OTTO"); // CFF signature
        assert!(subset_font(&data, &[1]).is_none());
    }

    #[test]
    fn test_reject_too_short() {
        assert!(subset_font(&[], &[]).is_none());
        assert!(subset_font(&[0; 8], &[]).is_none());
    }

    #[test]
    fn test_calc_table_search_params() {
        let (sr, es, rs) = calc_table_search_params(6);
        assert_eq!(sr, 64); // 4 * 16
        assert_eq!(es, 2); // log2(4)
        assert_eq!(rs, 32); // 6*16 - 64

        let (sr, es, rs) = calc_table_search_params(1);
        assert_eq!(sr, 16);
        assert_eq!(es, 0);
        assert_eq!(rs, 0);

        let (sr, es, rs) = calc_table_search_params(8);
        assert_eq!(sr, 128);
        assert_eq!(es, 3);
        assert_eq!(rs, 0);
    }

    #[test]
    fn test_calc_checksum() {
        // Four bytes: should be a single u32 word.
        let data = 0x01020304u32.to_be_bytes();
        assert_eq!(calc_checksum(&data), 0x01020304);

        // Eight bytes: sum of two words.
        let mut data = Vec::new();
        data.extend_from_slice(&0x00000001u32.to_be_bytes());
        data.extend_from_slice(&0x00000002u32.to_be_bytes());
        assert_eq!(calc_checksum(&data), 3);
    }

    #[test]
    fn test_subset_out_of_range_gid_ignored() {
        let font = build_test_font(); // 3 glyphs
        // Request a glyph ID that is out of range.
        let result = subset_font(&font, &[999]).expect("subsetting should succeed");
        // Only glyph 0 should be present.
        assert_eq!(result.gid_map.len(), 1);
        assert_eq!(result.gid_map[&0], 0);
    }

    #[test]
    fn test_subset_duplicate_glyph_ids() {
        let font = build_test_font();
        let result = subset_font(&font, &[1, 1, 1]).expect("subsetting should succeed");
        assert_eq!(result.gid_map.len(), 2);
        assert_eq!(result.gid_map[&0], 0);
        assert_eq!(result.gid_map[&1], 1);
    }

    #[test]
    fn test_parse_loca_short_format() {
        // Short format: offsets are u16, actual = value * 2.
        let mut loca = Vec::new();
        loca.extend_from_slice(&0u16.to_be_bytes()); // glyph 0 start
        loca.extend_from_slice(&6u16.to_be_bytes()); // glyph 0 end / glyph 1 start (actual: 12)
        loca.extend_from_slice(&10u16.to_be_bytes()); // glyph 1 end (actual: 20)

        let offsets = parse_loca(&loca, 0, 2).unwrap();
        assert_eq!(offsets, vec![0, 12, 20]);
    }

    #[test]
    fn test_parse_loca_long_format() {
        let mut loca = Vec::new();
        loca.extend_from_slice(&0u32.to_be_bytes());
        loca.extend_from_slice(&100u32.to_be_bytes());
        loca.extend_from_slice(&250u32.to_be_bytes());

        let offsets = parse_loca(&loca, 1, 2).unwrap();
        assert_eq!(offsets, vec![0, 100, 250]);
    }
}
