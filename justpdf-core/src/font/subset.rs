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
    /// Mapping from old glyph IDs to new glyph IDs, one entry per kept glyph.
    /// Glyph IDs are kept, so every entry maps a glyph ID to itself.
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
/// Every glyph keeps its glyph ID: `numGlyphs` is unchanged, glyphs that are
/// not kept become empty, and composite references still point at the same
/// glyphs. `cmap` and `post` keep only the entries of kept glyphs.
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
    let find_table =
        |tag: &[u8; 4]| -> Option<&TableRecord> { tables.iter().find(|t| &t.tag == tag) };

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

    let gid_map: HashMap<u16, u16> = keep_gids.iter().map(|&gid| (gid, gid)).collect();

    // Build new glyf table holding only kept glyphs; every glyph ID keeps a loca entry.
    let mut new_glyf: Vec<u8> = Vec::new();
    let mut new_loca_offsets: Vec<u32> = Vec::with_capacity(total_glyphs + 1);

    for gid in 0..total_glyphs {
        new_loca_offsets.push(new_glyf.len() as u32);
        if !keep_gids.contains(&(gid as u16)) {
            continue;
        }
        let start = glyph_offsets[gid];
        let end = glyph_offsets[gid + 1];
        if start >= end {
            continue; // empty glyph
        }
        new_glyf.extend_from_slice(glyf_data.get(start..end)?);
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
    let new_hmtx = build_subset_hmtx(hmtx_data, &keep_gids, num_h_metrics, total_glyphs)?;

    // Patch head table: update indexToLocFormat and zero out checksumAdjustment.
    let mut new_head = head_data.to_vec();
    // Zero checksumAdjustment (offset 8, 4 bytes) — we fix it later.
    write_u32(&mut new_head, 8, 0);
    // Set indexToLocFormat to 1 (long).
    write_i16(&mut new_head, 50, new_index_to_loc_format);

    // Collect all table data for output.
    struct TableEntry {
        tag: [u8; 4],
        data: Vec<u8>,
    }

    let mut out_tables: Vec<TableEntry> = Vec::new();

    for kept_tag in KEPT_TABLES {
        let data: Vec<u8> = match kept_tag {
            b"head" => new_head.clone(),
            b"loca" => new_loca.clone(),
            b"glyf" => new_glyf.clone(),
            b"hmtx" => new_hmtx.clone(),
            b"cmap" | b"post" => {
                let Some(rec) = find_table(kept_tag) else {
                    continue;
                };
                let original = table_data(font_data, rec)?;
                let subset = if kept_tag == b"cmap" {
                    subset_cmap(original, &keep_gids)
                } else {
                    subset_post(original, &keep_gids)
                };
                subset.unwrap_or_else(|| original.to_vec())
            }
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

    Some(SubsetResult {
        data: output,
        gid_map,
    })
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

/// Build the hmtx table for the subset: the original layout, with the metrics
/// of glyphs that are not kept set to zero.
///
/// The advance width of the last long metric is kept regardless, since every
/// glyph after `num_h_metrics` shares it.
fn build_subset_hmtx(
    hmtx_data: &[u8],
    keep_gids: &BTreeSet<u16>,
    num_h_metrics: usize,
    total_glyphs: usize,
) -> Option<Vec<u8>> {
    if num_h_metrics == 0 || num_h_metrics > total_glyphs {
        return None;
    }
    // hmtx structure:
    //   numOfLongHorMetrics entries of (advanceWidth: u16, lsb: i16) = 4 bytes each
    //   Remaining glyphs: just lsb (i16) = 2 bytes each, using the last advanceWidth.
    let len = num_h_metrics * 4 + (total_glyphs - num_h_metrics) * 2;
    let mut new_hmtx = vec![0u8; len];
    let available = hmtx_data.len().min(len);
    new_hmtx[..available].copy_from_slice(&hmtx_data[..available]);

    for gid in 0..total_glyphs {
        if keep_gids.contains(&(gid as u16)) {
            continue;
        }
        if gid < num_h_metrics {
            let offset = gid * 4;
            if gid + 1 < num_h_metrics {
                write_u16(&mut new_hmtx, offset, 0);
            }
            write_u16(&mut new_hmtx, offset + 2, 0);
        } else {
            write_u16(
                &mut new_hmtx,
                num_h_metrics * 4 + (gid - num_h_metrics) * 2,
                0,
            );
        }
    }

    Some(new_hmtx)
}

/// Build the `cmap` table for the subset: every encoding record in its original
/// order, each subtable mapping only to kept glyphs.
///
/// Formats 0 and 6 keep their layout with dropped glyphs set to 0; formats 4
/// and 12 are re-encoded from the kept mappings; other formats are copied
/// unchanged. Records that share a subtable still share it.
///
/// Returns `None` if the table cannot be parsed.
fn subset_cmap(cmap: &[u8], keep_gids: &BTreeSet<u16>) -> Option<Vec<u8>> {
    let num_records = read_u16(cmap, 2)? as usize;
    let table = ttf_parser::cmap::Table::parse(cmap)?;
    let header_len = 4 + num_records * 8;

    let mut header = Vec::with_capacity(header_len);
    header.extend_from_slice(&0u16.to_be_bytes());
    header.extend_from_slice(&(num_records as u16).to_be_bytes());
    let mut body: Vec<u8> = Vec::new();
    let mut new_offsets: HashMap<u32, u32> = HashMap::new();

    for i in 0..num_records {
        let rec = 4 + i * 8;
        let offset = read_u32(cmap, rec + 4)?;
        let new_offset = match new_offsets.get(&offset) {
            Some(&new_offset) => new_offset,
            None => {
                let data = cmap_subtable_bytes(cmap, offset as usize)?;
                let subset = table
                    .subtables
                    .get(i as u16)
                    .and_then(|subtable| subset_cmap_subtable(&subtable, data, keep_gids));
                let new_offset = (header_len + body.len()) as u32;
                body.extend_from_slice(subset.as_deref().unwrap_or(data));
                new_offsets.insert(offset, new_offset);
                new_offset
            }
        };
        header.extend_from_slice(cmap.get(rec..rec + 4)?);
        header.extend_from_slice(&new_offset.to_be_bytes());
    }

    header.extend_from_slice(&body);
    Some(header)
}

/// The bytes of the `cmap` subtable at `offset`, as long as its format declares.
fn cmap_subtable_bytes(cmap: &[u8], offset: usize) -> Option<&[u8]> {
    let len = match read_u16(cmap, offset)? {
        0 | 2 | 4 | 6 => read_u16(cmap, offset + 2)? as usize,
        8 | 10 | 12 | 13 => read_u32(cmap, offset + 4)? as usize,
        14 => read_u32(cmap, offset + 2)? as usize,
        _ => return None,
    };
    cmap.get(offset..offset.checked_add(len)?)
}

/// One `cmap` subtable restricted to kept glyphs, or `None` to keep it as is.
fn subset_cmap_subtable(
    subtable: &ttf_parser::cmap::Subtable,
    data: &[u8],
    keep_gids: &BTreeSet<u16>,
) -> Option<Vec<u8>> {
    match read_u16(data, 0)? {
        0 => {
            let mut out = data.to_vec();
            for gid in out.get_mut(6..6 + 256)? {
                if !keep_gids.contains(&u16::from(*gid)) {
                    *gid = 0;
                }
            }
            Some(out)
        }
        6 => {
            let mut out = data.to_vec();
            let entry_count = read_u16(data, 8)? as usize;
            for i in 0..entry_count {
                let offset = 10 + i * 2;
                if !keep_gids.contains(&read_u16(data, offset)?) {
                    write_u16(&mut out, offset, 0);
                }
            }
            Some(out)
        }
        4 => {
            let mappings = (0..0xFFFF)
                .filter_map(|code| Some((code, subtable.glyph_index(code)?.0)))
                .filter(|&(_, gid)| gid != 0 && keep_gids.contains(&gid))
                .collect();
            encode_cmap_format_4(read_u16(data, 4)?, &cmap_runs(mappings))
        }
        12 => Some(encode_cmap_format_12(
            read_u32(data, 8)?,
            &cmap_runs(format_12_kept_mappings(data, keep_gids)?),
        )),
        _ => None,
    }
}

/// The (code, glyph) mappings of a format 12 subtable that reach kept glyphs
/// other than 0, read group by group through the kept glyphs each group covers.
///
/// Returns `None` if the groups are truncated or yield more mappings than
/// there are Unicode code points.
fn format_12_kept_mappings(data: &[u8], keep_gids: &BTreeSet<u16>) -> Option<Vec<(u32, u16)>> {
    let num_groups = read_u32(data, 12)? as usize;
    let mut mappings: Vec<(u32, u16)> = Vec::new();
    for i in 0..num_groups {
        let group = 16 + i.checked_mul(12)?;
        let (start, end) = (read_u32(data, group)?, read_u32(data, group + 4)?);
        let Ok(start_gid) = u16::try_from(read_u32(data, group + 8)?) else {
            continue;
        };
        let Some(span) = end.checked_sub(start) else {
            continue;
        };
        for &gid in keep_gids.range(start_gid.max(1)..) {
            let offset = u32::from(gid - start_gid);
            if offset > span {
                break;
            }
            mappings.push((start + offset, gid));
        }
        if mappings.len() > 0x110000 {
            return None;
        }
    }
    Some(mappings)
}

/// A run of consecutive codes mapped to consecutive glyphs:
/// (first code, last code, glyph of the first code).
type CmapRun = (u32, u32, u16);

/// (code, glyph) mappings as runs; a code mapped twice keeps its lowest glyph.
fn cmap_runs(mut mappings: Vec<(u32, u16)>) -> Vec<CmapRun> {
    mappings.sort_unstable();
    mappings.dedup_by_key(|&mut (code, _)| code);

    let mut runs: Vec<CmapRun> = Vec::new();
    for (code, gid) in mappings {
        match runs.last_mut() {
            Some((first, last, first_gid))
                if code == *last + 1
                    && u32::from(gid) == u32::from(*first_gid) + (code - *first) =>
            {
                *last = code;
            }
            _ => runs.push((code, code, gid)),
        }
    }
    runs
}

/// A format 4 subtable holding `runs` (codes below 0xFFFF), one segment each
/// with an `idDelta`, closed by the 0xFFFF segment.
///
/// Returns `None` if the subtable would not fit its 16-bit length.
fn encode_cmap_format_4(language: u16, runs: &[CmapRun]) -> Option<Vec<u8>> {
    let mut segments: Vec<(u16, u16, u16)> = runs
        .iter()
        .map(|&(first, last, gid)| (first as u16, last as u16, gid.wrapping_sub(first as u16)))
        .collect();
    segments.push((0xFFFF, 0xFFFF, 1));

    let seg_count = segments.len();
    let length = 16 + seg_count * 8;
    if length > usize::from(u16::MAX) {
        return None;
    }
    let entry_selector = seg_count.ilog2() as u16;
    let search_range = 2u16 << entry_selector;
    let range_shift = (seg_count * 2) as u16 - search_range;

    let mut out = Vec::with_capacity(length);
    for field in [
        4,
        length as u16,
        language,
        (seg_count * 2) as u16,
        search_range,
        entry_selector,
        range_shift,
    ] {
        out.extend_from_slice(&field.to_be_bytes());
    }
    for &(_, last, _) in &segments {
        out.extend_from_slice(&last.to_be_bytes());
    }
    out.extend_from_slice(&0u16.to_be_bytes()); // reservedPad
    for &(first, _, _) in &segments {
        out.extend_from_slice(&first.to_be_bytes());
    }
    for &(_, _, delta) in &segments {
        out.extend_from_slice(&delta.to_be_bytes());
    }
    out.resize(length, 0); // idRangeOffset: all 0
    Some(out)
}

/// A format 12 subtable holding `runs`, one sequential map group each.
fn encode_cmap_format_12(language: u32, runs: &[CmapRun]) -> Vec<u8> {
    let length = 16 + runs.len() * 12;
    let mut out = Vec::with_capacity(length);
    out.extend_from_slice(&12u16.to_be_bytes());
    out.extend_from_slice(&0u16.to_be_bytes()); // reserved
    out.extend_from_slice(&(length as u32).to_be_bytes());
    out.extend_from_slice(&language.to_be_bytes());
    out.extend_from_slice(&(runs.len() as u32).to_be_bytes());
    for &(first, last, gid) in runs {
        out.extend_from_slice(&first.to_be_bytes());
        out.extend_from_slice(&last.to_be_bytes());
        out.extend_from_slice(&u32::from(gid).to_be_bytes());
    }
    out
}

/// Build the `post` table for the subset: format 2 with names kept for kept
/// glyphs only, every other glyph pointing at `.notdef`.
///
/// Returns `None` for other formats, if the table cannot be parsed, or if the
/// kept names would need an index from 32768 up (reserved).
fn subset_post(post: &[u8], keep_gids: &BTreeSet<u16>) -> Option<Vec<u8>> {
    if read_u32(post, 0)? != 0x00020000 {
        return None;
    }
    let num_glyphs = read_u16(post, 32)? as usize;
    let names_start = 34 + num_glyphs * 2;

    let mut names: Vec<&[u8]> = Vec::new();
    let mut pos = names_start;
    while pos < post.len() {
        let len = post[pos] as usize;
        names.push(post.get(pos + 1..pos + 1 + len)?);
        pos += 1 + len;
    }

    let mut out = post.get(..names_start)?.to_vec();
    let mut new_names: Vec<&[u8]> = Vec::new();
    for gid in 0..num_glyphs {
        let offset = 34 + gid * 2;
        let index = read_u16(post, offset)?;
        let new_index = if !keep_gids.contains(&(gid as u16)) {
            0
        } else if index < 258 {
            index
        } else {
            new_names.push(names.get(usize::from(index) - 258)?);
            u16::try_from(257 + new_names.len())
                .ok()
                .filter(|&new_index| new_index < 32768)?
        };
        write_u16(&mut out, offset, new_index);
    }
    for name in new_names {
        out.push(name.len() as u8);
        out.extend_from_slice(name);
    }
    Some(out)
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

    /// Noto Sans Regular (SIL OFL 1.1), the fixture pinned in `writer::compress` tests.
    const NOTO_SANS_REGULAR: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/NotoSans-Regular.ttf"
    ));

    /// Raw `glyf` bytes of `gid` in `font`, located through `loca`.
    fn glyph_bytes(font: &[u8], gid: u16) -> Vec<u8> {
        let num_tables = read_u16(font, 4).unwrap() as usize;
        let table = |tag: &[u8; 4]| -> &[u8] {
            (0..num_tables)
                .map(|i| 12 + i * 16)
                .find(|&rec| &font[rec..rec + 4] == tag)
                .map(|rec| {
                    let off = read_u32(font, rec + 8).unwrap() as usize;
                    let len = read_u32(font, rec + 12).unwrap() as usize;
                    &font[off..off + len]
                })
                .unwrap()
        };
        let format = read_i16(table(b"head"), 50).unwrap();
        let num_glyphs = read_u16(table(b"maxp"), 4).unwrap() as usize;
        let offsets = parse_loca(table(b"loca"), format, num_glyphs).unwrap();
        let (start, end) = (offsets[gid as usize], offsets[gid as usize + 1]);
        table(b"glyf")[start..end].to_vec()
    }

    /// Assert that `gid` in `subset` holds the same glyph data as in `original`,
    /// allowing the zero padding the subset adds after each glyph.
    fn assert_same_glyph(subset: &[u8], original: &[u8], gid: u16) {
        let (out, orig) = (glyph_bytes(subset, gid), glyph_bytes(original, gid));
        assert!(out.len() >= orig.len(), "glyph {gid} shorter than original");
        assert_eq!(&out[..orig.len()], &orig[..], "glyph {gid} data changed");
        assert!(
            out[orig.len()..].iter().all(|&b| b == 0),
            "glyph {gid} padding"
        );
    }

    #[test]
    fn test_subset_keeps_glyph_ids() {
        let font = build_test_font_with_glyphs(5, &[], false);
        let result = subset_font(&font, &[3]).expect("subsetting should succeed");

        let face = ttf_parser::Face::parse(&result.data, 0).unwrap();
        assert_eq!(face.number_of_glyphs(), 5);
        assert!(!glyph_bytes(&font, 3).is_empty());
        assert_same_glyph(&result.data, &font, 3);
        for dropped in [1, 2, 4] {
            assert!(
                glyph_bytes(&result.data, dropped).is_empty(),
                "glyph {dropped} kept"
            );
        }
    }

    #[test]
    fn test_subset_keeps_advance_widths_of_kept_glyphs() {
        let font = build_test_font_with_glyphs(5, &[], false);
        let result = subset_font(&font, &[3]).expect("subsetting should succeed");

        let original = ttf_parser::Face::parse(&font, 0).unwrap();
        let subset = ttf_parser::Face::parse(&result.data, 0).unwrap();
        let gid = ttf_parser::GlyphId(3);
        assert_eq!(original.glyph_hor_advance(gid), Some(800));
        assert_eq!(subset.glyph_hor_advance(gid), Some(800));
    }

    #[test]
    fn test_subset_composite_glyph_keeps_component_ids() {
        let font = build_test_font_with_glyphs(5, &[(4, vec![1, 3])], false);
        let result = subset_font(&font, &[4]).expect("subsetting should succeed");

        assert_same_glyph(&result.data, &font, 4);
        assert_same_glyph(&result.data, &font, 1);
        assert_same_glyph(&result.data, &font, 3);
        assert!(glyph_bytes(&result.data, 2).is_empty());
    }

    #[test]
    fn test_subset_real_font_cmap_still_reaches_kept_glyph() {
        let original = ttf_parser::Face::parse(NOTO_SANS_REGULAR, 0).unwrap();
        let h = original.glyph_index('H').unwrap();
        let result = subset_font(NOTO_SANS_REGULAR, &[h.0]).expect("subsetting should succeed");

        let subset = ttf_parser::Face::parse(&result.data, 0).unwrap();
        assert_eq!(subset.number_of_glyphs(), original.number_of_glyphs());
        assert_eq!(subset.glyph_index('H'), Some(h));
        assert!(subset.glyph_bounding_box(h).is_some());
        assert_eq!(subset.glyph_bounding_box(h), original.glyph_bounding_box(h));
        let o = original.glyph_index('o').unwrap();
        assert!(
            subset.glyph_bounding_box(o).is_none(),
            "unrequested glyph kept"
        );
        assert!(result.data.len() < NOTO_SANS_REGULAR.len());
    }

    /// Raw bytes of table `tag` in `font`.
    fn table_bytes<'a>(font: &'a [u8], tag: &[u8; 4]) -> &'a [u8] {
        let num_tables = read_u16(font, 4).unwrap() as usize;
        (0..num_tables)
            .map(|i| 12 + i * 16)
            .find(|&rec| &font[rec..rec + 4] == tag)
            .map(|rec| {
                let off = read_u32(font, rec + 8).unwrap() as usize;
                let len = read_u32(font, rec + 12).unwrap() as usize;
                &font[off..off + len]
            })
            .unwrap()
    }

    /// The Noto Sans subset keeping the glyphs of `text`.
    fn noto_subset(text: &str) -> SubsetResult {
        let face = ttf_parser::Face::parse(NOTO_SANS_REGULAR, 0).unwrap();
        let gids: Vec<u16> = text
            .chars()
            .map(|c| face.glyph_index(c).unwrap().0)
            .collect();
        subset_font(NOTO_SANS_REGULAR, &gids).expect("subsetting should succeed")
    }

    fn noto_hello_subset() -> SubsetResult {
        noto_subset("Hello")
    }

    #[test]
    fn test_subset_real_font_cmap_keeps_only_kept_glyph_entries() {
        let result = noto_hello_subset();
        let original =
            ttf_parser::cmap::Table::parse(table_bytes(NOTO_SANS_REGULAR, b"cmap")).unwrap();
        let subset = ttf_parser::cmap::Table::parse(table_bytes(&result.data, b"cmap")).unwrap();

        assert_eq!(subset.subtables.len(), original.subtables.len());
        let mut kept_entries = 0;
        for (orig, sub) in original.subtables.into_iter().zip(subset.subtables) {
            assert_eq!(
                (sub.platform_id, sub.encoding_id),
                (orig.platform_id, orig.encoding_id)
            );
            orig.codepoints(|code| {
                let expected = orig
                    .glyph_index(code)
                    .filter(|gid| result.gid_map.contains_key(&gid.0));
                assert_eq!(sub.glyph_index(code), expected, "code {code:#x}");
                kept_entries += usize::from(expected.is_some());
            });
        }
        // "Hello" is four distinct characters in each of four subtables.
        assert_eq!(kept_entries, 16);
        assert!(table_bytes(&result.data, b"cmap").len() < 1024);
        // Noto's (0,3)/(3,1) and (0,4)/(3,10) records each share one subtable.
        let offsets = |cmap: &[u8]| -> Vec<u32> {
            (0..read_u16(cmap, 2).unwrap() as usize)
                .map(|i| read_u32(cmap, 4 + i * 8 + 4).unwrap())
                .collect()
        };
        let out_offsets = offsets(table_bytes(&result.data, b"cmap"));
        assert_eq!(out_offsets[0], out_offsets[2]);
        assert_eq!(out_offsets[1], out_offsets[3]);
        assert_ne!(out_offsets[0], out_offsets[1]);
        assert_eq!(offsets(table_bytes(NOTO_SANS_REGULAR, b"cmap"))[0], 36);
    }

    #[test]
    fn test_subset_real_font_post_keeps_only_kept_glyph_names() {
        // Amacron and eng have names outside the standard Macintosh set.
        let result = noto_subset("HelloĀŋ");
        let original = ttf_parser::Face::parse(NOTO_SANS_REGULAR, 0).unwrap();
        let subset = ttf_parser::Face::parse(&result.data, 0).unwrap();

        let post = table_bytes(NOTO_SANS_REGULAR, b"post");
        let custom_names = result
            .gid_map
            .keys()
            .filter(|&&gid| read_u16(post, 34 + 2 * gid as usize).unwrap() >= 258)
            .count();
        assert!(
            custom_names >= 2,
            "{custom_names} kept glyphs with custom names"
        );

        for gid in 0..original.number_of_glyphs() {
            let gid = ttf_parser::GlyphId(gid);
            let name = original.glyph_name(gid).unwrap();
            if result.gid_map.contains_key(&gid.0) {
                assert_eq!(subset.glyph_name(gid), Some(name), "glyph {}", gid.0);
                assert_eq!(subset.glyph_index_by_name(name), Some(gid), "name {name}");
            } else {
                assert_eq!(subset.glyph_name(gid), Some(".notdef"), "glyph {}", gid.0);
            }
        }
        assert!(table_bytes(&result.data, b"post").len() < 32 + 2 + 2 * 3884 + 64);
    }

    /// A `cmap` table holding the given (platformID, encodingID, subtable) records.
    fn build_cmap(records: &[(u16, u16, Vec<u8>)]) -> Vec<u8> {
        let mut cmap = Vec::new();
        cmap.extend_from_slice(&0u16.to_be_bytes());
        cmap.extend_from_slice(&(records.len() as u16).to_be_bytes());
        let mut offset = 4 + records.len() * 8;
        for (platform, encoding, subtable) in records {
            cmap.extend_from_slice(&platform.to_be_bytes());
            cmap.extend_from_slice(&encoding.to_be_bytes());
            cmap.extend_from_slice(&(offset as u32).to_be_bytes());
            offset += subtable.len();
        }
        for (_, _, subtable) in records {
            cmap.extend_from_slice(subtable);
        }
        cmap
    }

    #[test]
    fn test_subset_cmap_formats_0_and_6_drop_entries_and_unknown_formats_are_copied() {
        let mut format_0 = vec![0u8; 6 + 256];
        write_u16(&mut format_0, 2, 262);
        format_0[6 + 65] = 1;
        format_0[6 + 66] = 2;
        // Format 6: codes 0x41 and 0x42 map to glyphs 1 and 2.
        let format_6 = [0, 6, 0, 14, 0, 0, 0, 0x41, 0, 2, 0, 1, 0, 2].to_vec();
        // Format 14 with no variation selector records.
        let format_14 = [0, 14, 0, 0, 0, 10, 0, 0, 0, 0].to_vec();
        let cmap = build_cmap(&[
            (1, 0, format_0),
            (3, 0, format_6),
            (0, 5, format_14.clone()),
        ]);

        let keep: BTreeSet<u16> = [0, 1].into();
        let out = subset_cmap(&cmap, &keep).expect("cmap should be rewritten");

        let table = ttf_parser::cmap::Table::parse(&out).unwrap();
        for i in 0..2 {
            let subtable = table.subtables.get(i).unwrap();
            assert_eq!(
                subtable.glyph_index(0x41),
                Some(ttf_parser::GlyphId(1)),
                "subtable {i}"
            );
            // ttf-parser reports glyph 0 as `None` for format 0 and as `Some(0)` for format 6.
            let dropped = subtable.glyph_index(0x42).filter(|gid| gid.0 != 0);
            assert_eq!(dropped, None, "subtable {i}");
        }
        let offset = read_u32(&out, 4 + 2 * 8 + 4).unwrap() as usize;
        assert_eq!(&out[offset..offset + format_14.len()], &format_14[..]);
    }

    #[test]
    fn test_subset_cmap_format_4_with_many_segments() {
        // Every other code, each to its own glyph: one segment per code.
        let runs: Vec<CmapRun> = (0..5000u32).map(|i| (i * 2, i * 2, i as u16 + 1)).collect();
        let subtable = encode_cmap_format_4(0, &runs).expect("fits in 16 bits");

        let cmap = build_cmap(&[(3, 1, subtable)]);
        let table = ttf_parser::cmap::Table::parse(&cmap).unwrap();
        let subtable = table.subtables.get(0).unwrap();
        assert_eq!(subtable.glyph_index(0), Some(ttf_parser::GlyphId(1)));
        assert_eq!(subtable.glyph_index(1), None);
        assert_eq!(subtable.glyph_index(9998), Some(ttf_parser::GlyphId(5000)));
        let seg_count_x2 = read_u16(&cmap, 12 + 6).unwrap();
        assert_eq!(seg_count_x2, 5001 * 2);
        // searchRange = 2 × 2^floor(log2(5001)), entrySelector, rangeShift.
        assert_eq!(read_u16(&cmap, 12 + 8).unwrap(), 8192);
        assert_eq!(read_u16(&cmap, 12 + 10).unwrap(), 12);
        assert_eq!(read_u16(&cmap, 12 + 12).unwrap(), 10002 - 8192);
    }

    #[test]
    fn test_subset_cmap_format_12_reads_only_kept_glyphs_of_a_huge_group() {
        // One group mapping every 32-bit code to glyph (code - 0) … far past numGlyphs.
        let mut format_12 = Vec::new();
        for field in [12u16, 0] {
            format_12.extend_from_slice(&field.to_be_bytes());
        }
        for field in [28u32, 0, 1, 0, u32::MAX, 0] {
            format_12.extend_from_slice(&field.to_be_bytes());
        }
        let cmap = build_cmap(&[(3, 10, format_12)]);

        let start = std::time::Instant::now();
        let out = subset_cmap(&cmap, &[0, 5, 70].into()).expect("cmap should be rewritten");
        assert!(start.elapsed() < std::time::Duration::from_secs(1));

        let table = ttf_parser::cmap::Table::parse(&out).unwrap();
        let subtable = table.subtables.get(0).unwrap();
        assert_eq!(subtable.glyph_index(5), Some(ttf_parser::GlyphId(5)));
        assert_eq!(subtable.glyph_index(70), Some(ttf_parser::GlyphId(70)));
        assert_eq!(subtable.glyph_index(6), None);
        // Two groups (5 and 70); glyph 0 is not mapped.
        assert_eq!(read_u32(&out, 12 + 12).unwrap(), 2);
    }

    #[test]
    fn test_subset_post_refuses_more_custom_names_than_indices_allow() {
        // 32,511 glyphs, each with its own one-letter custom name.
        let num_glyphs = 32_511usize;
        let mut post = vec![0u8; 32];
        write_u32(&mut post, 0, 0x00020000);
        post.extend_from_slice(&(num_glyphs as u16).to_be_bytes());
        for gid in 0..num_glyphs {
            post.extend_from_slice(&(258 + gid as u16).to_be_bytes());
        }
        for _ in 0..num_glyphs {
            post.extend_from_slice(&[1, b'a']);
        }
        let keep: BTreeSet<u16> = (0..num_glyphs as u16).collect();
        assert!(subset_post(&post, &keep).is_none());
        let keep: BTreeSet<u16> = (0..num_glyphs as u16 - 1).collect();
        assert!(subset_post(&post, &keep).is_some());
    }

    #[test]
    fn test_subset_post_leaves_other_formats_alone() {
        // Long enough to parse as format 2 with one glyph.
        let mut post = vec![0u8; 36];
        write_u32(&mut post, 0, 0x00030000);
        write_u16(&mut post, 32, 1);
        assert!(subset_post(&post, &[0].into()).is_none());
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
        // Verify maxp in output keeps numGlyphs = 3.
        let num_tables = read_u16(&result.data, 4).unwrap() as usize;
        let mut found_maxp = false;
        for i in 0..num_tables {
            let rec_off = 12 + i * 16;
            let tag = &result.data[rec_off..rec_off + 4];
            if tag == b"maxp" {
                let offset = read_u32(&result.data, rec_off + 8).unwrap() as usize;
                let num_glyphs = read_u16(&result.data, offset + 4).unwrap();
                assert_eq!(num_glyphs, 3);
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
        assert_eq!(result.gid_map[&0], 0);
        assert_eq!(result.gid_map[&2], 2);
    }

    #[test]
    fn test_subset_multiple_glyphs_ordering() {
        let font = build_test_font_with_glyphs(5, &[], false);
        let result = subset_font(&font, &[3, 1, 4]).expect("subsetting should succeed");
        // Should keep glyphs 0, 1, 3, 4 at their own IDs.
        assert_eq!(result.gid_map.len(), 4);
        assert_eq!(result.gid_map[&0], 0);
        assert_eq!(result.gid_map[&1], 1);
        assert_eq!(result.gid_map[&3], 3);
        assert_eq!(result.gid_map[&4], 4);
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
