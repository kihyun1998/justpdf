//! Character code to glyph ID for simple TrueType fonts (ISO 32000-2 §9.6.6.4).

use super::encoding_tables::{MAC_ROMAN_GLYPH_NAMES, STANDARD_GLYPH_NAMES, WIN_ANSI_GLYPH_NAMES};
use super::{Encoding, glyph_name_to_unicode, winansi_char};
use ttf_parser::cmap::Subtable;
use ttf_parser::{Face, GlyphId, PlatformId};

/// The glyph IDs a simple TrueType font's one-byte `code` can draw, most
/// preferred first and without duplicates.
///
/// `encoding` is the font's `/Encoding` — its base encoding, `None` when the
/// font dictionary has no `/Encoding` — and `differences` its `/Differences`.
/// `symbolic` is bit 3 of the font descriptor's `/Flags`.
///
/// First the §9.6.6.4 choice pdf.js makes (`src/core/fonts.js`): one `cmap`
/// subtable picked by the symbolic flag; for a non-symbolic font with an
/// `/Encoding` and a (3,1) or (1,0) subtable, the code's glyph name read as
/// Unicode or as a MacRoman code; for (3,0), the code in the `0xF0xx` range;
/// otherwise the code itself; and the `post` table by glyph name. Then the
/// fallbacks: every subtable with the code and its `0xF000`/`0xF100`/`0xF200`
/// prefixes, the glyph name through any Unicode subtable and `post`, the
/// code's WinAnsi character, and the code taken as a glyph ID.
pub fn truetype_glyph_candidates(
    face: &Face,
    code: u8,
    encoding: Option<Encoding>,
    differences: &[(u8, Vec<u8>)],
    symbolic: bool,
) -> Vec<GlyphId> {
    let mut out: Vec<GlyphId> = Vec::new();
    let mut push = |gid: Option<GlyphId>| {
        if let Some(gid) = gid
            && !out.contains(&gid)
        {
            out.push(gid);
        }
    };

    let subtables: Vec<Subtable> = face
        .tables()
        .cmap
        .map(|cmap| cmap.subtables.into_iter().collect())
        .unwrap_or_default();
    let chosen = preferred_subtable(&subtables, symbolic, encoding.is_some());
    let difference = encoding_glyph_name(code, None, differences);
    // pdf.js reads a base encoding's names only for MacRoman and WinAnsi.
    let mac_or_win =
        encoding.filter(|e| matches!(e, Encoding::MacRomanEncoding | Encoding::WinAnsiEncoding));
    let base_name = encoding_glyph_name(code, mac_or_win, &[]);
    // The name the §9.6.6.4 path reads: /Differences, else the base encoding, else Standard.
    let rule_name = difference
        .or(base_name)
        .or_else(|| encoding_glyph_name(code, Some(Encoding::StandardEncoding), &[]));
    // The name looked up in `post`: /Differences, else a MacRoman or WinAnsi base.
    let post_name = difference.or(base_name);
    let by_post = |name: Option<&str>| {
        name.and_then(|n| face.glyph_index_by_name(n))
            .filter(|g| g.0 > 0)
    };

    let mut force_post = false;
    let primary = chosen.and_then(|st| {
        let (platform, enc) = (st.platform_id, st.encoding_id);
        match (platform, enc) {
            (PlatformId::Windows, 1) | (PlatformId::Macintosh, 0)
                if encoding.is_some() && !symbolic =>
            {
                let name = rule_name?;
                let key = if platform == PlatformId::Windows {
                    single_char(name)? as u32
                } else {
                    MAC_ROMAN_GLYPH_NAMES.iter().position(|n| *n == name)? as u32
                };
                st.glyph_index(key)
            }
            (PlatformId::Unicode, _) => {
                force_post = true;
                st.glyph_index(u32::from(code))
            }
            (PlatformId::Windows, 0) => st
                .glyph_index(0xF000 | u32::from(code))
                .or_else(|| st.glyph_index(u32::from(code))),
            _ => st.glyph_index(u32::from(code)),
        }
    });
    if force_post {
        push(by_post(post_name));
    }
    push(primary);
    push(by_post(post_name));

    for st in &subtables {
        for prefix in [0u32, 0xF000, 0xF100, 0xF200] {
            push(st.glyph_index(prefix | u32::from(code)));
        }
    }
    for name in [difference, base_name, rule_name].into_iter().flatten() {
        push(single_char(name).and_then(|c| face.glyph_index(c)));
        push(by_post(Some(name)));
    }
    push(winansi_char(code).and_then(|c| face.glyph_index(c)));
    if u16::from(code) < face.number_of_glyphs() {
        push(Some(GlyphId(u16::from(code))));
    }
    out
}

/// The glyph name a simple font's one-byte `code` has in its encoding: the
/// `/Differences` name, else the name in `base` when that is StandardEncoding,
/// MacRomanEncoding or WinAnsiEncoding. `None` when neither names the code —
/// for an embedded font program, the code then goes through its built-in encoding.
pub fn encoding_glyph_name(
    code: u8,
    base: Option<Encoding>,
    differences: &[(u8, Vec<u8>)],
) -> Option<&str> {
    if let Some((_, name)) = differences.iter().rev().find(|(c, _)| *c == code) {
        return std::str::from_utf8(name).ok();
    }
    let names = match base? {
        Encoding::StandardEncoding => &STANDARD_GLYPH_NAMES,
        Encoding::MacRomanEncoding => &MAC_ROMAN_GLYPH_NAMES,
        Encoding::WinAnsiEncoding => &WIN_ANSI_GLYPH_NAMES,
        _ => return None,
    };
    Some(names[code as usize]).filter(|n| !n.is_empty())
}

/// The single character a glyph name stands for, by the Adobe Glyph List rules.
fn single_char(name: &str) -> Option<char> {
    let text = glyph_name_to_unicode(name.as_bytes())?;
    let mut chars = text.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => Some(c),
        _ => None,
    }
}

/// The `cmap` subtable pdf.js reads for a simple font: Unicode (0,0/1/3) and
/// (1,0) are candidates; (3,1) wins for a non-symbolic font that has an
/// `/Encoding` (or when nothing else was found); (3,0) wins for a symbolic one.
fn preferred_subtable<'a>(
    subtables: &'a [Subtable<'a>],
    symbolic: bool,
    has_encoding: bool,
) -> Option<&'a Subtable<'a>> {
    let mut potential: Option<&Subtable> = None;
    for st in subtables {
        let (platform, enc) = (st.platform_id, st.encoding_id);
        if potential.is_some_and(|p| p.platform_id == platform && p.encoding_id == enc) {
            continue;
        }
        let (use_table, can_break) = match (platform, enc) {
            (PlatformId::Unicode, 0 | 1 | 3) => (true, false),
            (PlatformId::Macintosh, 0) => (true, false),
            (PlatformId::Windows, 1) if has_encoding || potential.is_none() => (true, !symbolic),
            (PlatformId::Windows, 0) if symbolic => (true, true),
            _ => (false, false),
        };
        if use_table {
            potential = Some(st);
        }
        if can_break {
            break;
        }
    }
    potential
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::test_font::font;

    fn gids(v: Vec<GlyphId>) -> Vec<u16> {
        v.into_iter().map(|g| g.0).collect()
    }

    #[test]
    fn symbolic_three_zero_maps_through_the_f000_range() {
        let data = font(80, &[(3, 0, &[(0xF041, 7)])], &[]);
        let face = Face::parse(&data, 0).unwrap();
        assert_eq!(
            gids(truetype_glyph_candidates(&face, 0x41, None, &[], true))[0],
            7
        );
    }

    #[test]
    fn non_symbolic_one_zero_maps_through_the_mac_roman_code_of_the_glyph_name() {
        // WinAnsi 0xE9 is eacute; eacute is MacRoman 0x8E.
        let data = font(10, &[(1, 0, &[(0x8E, 5), (0xE9, 6)])], &[]);
        let face = Face::parse(&data, 0).unwrap();
        let c = truetype_glyph_candidates(&face, 0xE9, Some(Encoding::WinAnsiEncoding), &[], false);
        assert_eq!(gids(c)[0], 5);
    }

    #[test]
    fn non_symbolic_three_one_maps_through_the_unicode_of_the_glyph_name() {
        let data = font(10, &[(3, 1, &[(0x41, 1), (0x42, 2), (0xE9, 3)])], &[]);
        let face = Face::parse(&data, 0).unwrap();
        let differences = [(65u8, b"B".to_vec())];
        let c = truetype_glyph_candidates(
            &face,
            65,
            Some(Encoding::StandardEncoding),
            &differences,
            false,
        );
        assert_eq!(gids(c)[0], 2);
        // MacRoman 0x8E is eacute -> U+00E9
        let c =
            truetype_glyph_candidates(&face, 0x8E, Some(Encoding::MacRomanEncoding), &[], false);
        assert_eq!(gids(c)[0], 3);
    }

    #[test]
    fn a_name_only_in_post_resolves_through_post() {
        let names = [".notdef", "space", "zzcustom"];
        let data = font(3, &[(3, 1, &[(0x20, 1)])], &names);
        let face = Face::parse(&data, 0).unwrap();
        let differences = [(0x80u8, b"zzcustom".to_vec())];
        let c = truetype_glyph_candidates(
            &face,
            0x80,
            Some(Encoding::WinAnsiEncoding),
            &differences,
            false,
        );
        assert_eq!(gids(c)[0], 2);
    }

    #[test]
    fn a_base_encoding_name_only_in_post_resolves_through_post() {
        // WinAnsi 0x80 is Euro; the font has no Unicode entry for it, only a post name.
        let names = [".notdef", "Euro"];
        let data = font(2, &[(3, 1, &[(0x41, 0)])], &names);
        let face = Face::parse(&data, 0).unwrap();
        let c = truetype_glyph_candidates(&face, 0x80, Some(Encoding::WinAnsiEncoding), &[], false);
        assert_eq!(gids(c)[0], 1);
    }

    #[test]
    fn without_an_encoding_the_raw_code_is_looked_up() {
        // No /Encoding: the (3,1) table is read with the code itself.
        let data = font(10, &[(3, 1, &[(0x41, 4)])], &[]);
        let face = Face::parse(&data, 0).unwrap();
        assert_eq!(
            gids(truetype_glyph_candidates(&face, 0x41, None, &[], false))[0],
            4
        );
    }

    #[test]
    fn candidates_follow_the_rule_then_the_fallbacks_without_duplicates() {
        // Primary: name B -> U+0042 -> 2. Fallbacks: raw 0x41 in (3,1) -> 1, code as GID -> 65.
        let data = font(70, &[(3, 1, &[(0x41, 1), (0x42, 2)])], &[]);
        let face = Face::parse(&data, 0).unwrap();
        let differences = [(65u8, b"B".to_vec())];
        let c = gids(truetype_glyph_candidates(
            &face,
            65,
            Some(Encoding::WinAnsiEncoding),
            &differences,
            false,
        ));
        assert_eq!(c, vec![2, 1, 65]);
    }

    #[test]
    fn nothing_maps_gives_no_candidates() {
        let data = font(3, &[(3, 1, &[(0x41, 1)])], &[]);
        let face = Face::parse(&data, 0).unwrap();
        let differences = [(0x90u8, b"g123".to_vec())];
        let c = truetype_glyph_candidates(
            &face,
            0x90,
            Some(Encoding::WinAnsiEncoding),
            &differences,
            false,
        );
        assert!(c.is_empty(), "{c:?}");
    }
}
