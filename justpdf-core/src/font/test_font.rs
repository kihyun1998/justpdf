//! A synthetic TrueType font for tests.

/// A `cmap` subtable: platform, encoding, and `(char code, glyph id)` pairs.
pub(crate) type CmapSubtable<'a> = (u16, u16, &'a [(u32, u16)]);

/// A minimal TrueType font with `num_glyphs` glyphs, the given `cmap`
/// subtables — `(platform, encoding, [(char code, glyph id)])`, written as
/// format 0 for platform 1 and format 4 otherwise — and, when `names` is
/// not empty, a version 2 `post` table naming glyph `i` `names[i]`.
pub(crate) fn font(num_glyphs: u16, cmaps: &[CmapSubtable], names: &[&str]) -> Vec<u8> {
    let be16 = |v: u16| v.to_be_bytes();
    let mut tables: Vec<([u8; 4], Vec<u8>)> = Vec::new();

    let mut head = vec![0u8; 54];
    head[0..4].copy_from_slice(&0x0001_0000u32.to_be_bytes());
    head[12..16].copy_from_slice(&0x5F0F_3CF5u32.to_be_bytes());
    head[18..20].copy_from_slice(&be16(1000));
    tables.push((*b"head", head));

    let mut hhea = vec![0u8; 36];
    hhea[0..4].copy_from_slice(&0x0001_0000u32.to_be_bytes());
    hhea[34..36].copy_from_slice(&be16(num_glyphs));
    tables.push((*b"hhea", hhea));

    let mut maxp = vec![0u8; 6];
    maxp[0..4].copy_from_slice(&0x0000_5000u32.to_be_bytes());
    maxp[4..6].copy_from_slice(&be16(num_glyphs));
    tables.push((*b"maxp", maxp));

    let mut hmtx = Vec::new();
    for _ in 0..num_glyphs {
        hmtx.extend_from_slice(&be16(500));
        hmtx.extend_from_slice(&be16(0));
    }
    tables.push((*b"hmtx", hmtx));

    let mut subtables: Vec<(u16, u16, Vec<u8>)> = Vec::new();
    for &(platform, encoding, map) in cmaps {
        let data = if platform == 1 {
            let mut glyphs = [0u8; 256];
            for &(c, g) in map {
                glyphs[c as usize] = g as u8;
            }
            let mut d = Vec::new();
            d.extend_from_slice(&be16(0));
            d.extend_from_slice(&be16(262));
            d.extend_from_slice(&be16(0));
            d.extend_from_slice(&glyphs);
            d
        } else {
            // one segment per mapping, then the 0xFFFF terminator
            let mut segs: Vec<(u16, u16)> = map.iter().map(|&(c, g)| (c as u16, g)).collect();
            segs.sort();
            segs.push((0xFFFF, 0));
            let n = segs.len() as u16;
            let mut d = Vec::new();
            d.extend_from_slice(&be16(4));
            d.extend_from_slice(&be16(16 + 8 * n));
            d.extend_from_slice(&be16(0));
            d.extend_from_slice(&be16(2 * n));
            d.extend_from_slice(&[0; 6]);
            for &(c, _) in &segs {
                d.extend_from_slice(&be16(c));
            }
            d.extend_from_slice(&be16(0));
            for &(c, _) in &segs {
                d.extend_from_slice(&be16(c));
            }
            for &(c, g) in &segs {
                let delta = if c == 0xFFFF { 1 } else { g.wrapping_sub(c) };
                d.extend_from_slice(&be16(delta));
            }
            for _ in &segs {
                d.extend_from_slice(&be16(0));
            }
            d
        };
        subtables.push((platform, encoding, data));
    }
    subtables.sort_by_key(|(p, e, _)| (*p, *e));
    let mut cmap = Vec::new();
    cmap.extend_from_slice(&be16(0));
    cmap.extend_from_slice(&be16(subtables.len() as u16));
    let mut offset = 4 + 8 * subtables.len() as u32;
    for (p, e, d) in &subtables {
        cmap.extend_from_slice(&be16(*p));
        cmap.extend_from_slice(&be16(*e));
        cmap.extend_from_slice(&offset.to_be_bytes());
        offset += d.len() as u32;
    }
    for (_, _, d) in &subtables {
        cmap.extend_from_slice(d);
    }
    tables.push((*b"cmap", cmap));

    if !names.is_empty() {
        let mut post = vec![0u8; 32];
        post[0..4].copy_from_slice(&0x0002_0000u32.to_be_bytes());
        post.extend_from_slice(&be16(num_glyphs));
        for i in 0..num_glyphs {
            post.extend_from_slice(&be16(258 + i));
        }
        for name in names {
            post.push(name.len() as u8);
            post.extend_from_slice(name.as_bytes());
        }
        tables.push((*b"post", post));
    }

    tables.sort_by_key(|(tag, _)| *tag);
    let mut out = Vec::new();
    out.extend_from_slice(&0x0001_0000u32.to_be_bytes());
    out.extend_from_slice(&be16(tables.len() as u16));
    out.extend_from_slice(&[0; 6]);
    let mut offset = 12 + 16 * tables.len() as u32;
    let mut body = Vec::new();
    for (tag, data) in &tables {
        out.extend_from_slice(tag);
        out.extend_from_slice(&[0; 4]);
        out.extend_from_slice(&offset.to_be_bytes());
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut padded = data.clone();
        while padded.len() % 4 != 0 {
            padded.push(0);
        }
        offset += padded.len() as u32;
        body.extend_from_slice(&padded);
    }
    out.extend_from_slice(&body);
    out
}
