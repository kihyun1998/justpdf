//! Text to the codes of a font a document already holds, such as a form's
//! `/DR` font: the inverse of how text extraction decodes that font's codes.

use std::collections::HashMap;

use ttf_parser::{Face, GlyphId};

use super::standard14::strip_subset_prefix;
use super::{
    EncodingCMap, ToUnicodeCMap, descendant_font, parse_font_info, resolve_font_entries,
    truetype_glyph_candidates, type0_encoding_cmap,
};
use crate::object::{IndirectRef, PdfDict, PdfObject};

/// The codes of one font, by the character each decodes to.
#[derive(Debug, Clone)]
pub(crate) struct FontEncoder {
    codes: HashMap<char, Vec<u8>>,
}

impl FontEncoder {
    /// The encoder for the font dictionary `font`. `resolve` resolves
    /// references; `decode` returns a stream's decoded data.
    ///
    /// A character gets the code text extraction decodes to it: through the
    /// ToUnicode CMap, else (simple fonts) the `/Differences` glyph name or the
    /// base encoding. Where the font embeds a TrueType program (`FontFile2`), a
    /// code counts only if the glyph it draws is in the program.
    ///
    /// `None` when nothing can be encoded: a Type0 font without ToUnicode or
    /// with a named CMap other than `Identity-H`/`Identity-V`, or a subset font
    /// (`/BaseFont` with a `ABCDEF+` tag) whose program is not TrueType.
    pub(crate) fn new(
        font: &PdfDict,
        mut resolve: impl FnMut(&IndirectRef) -> Option<PdfObject>,
        mut decode: impl FnMut(&PdfDict, &[u8]) -> Option<Vec<u8>>,
    ) -> Option<FontEncoder> {
        let font = resolve_font_entries(font, &mut resolve);
        let info = parse_font_info(&font);
        let to_unicode = to_unicode(&font, &mut resolve, &mut decode);
        let subset = strip_subset_prefix(&info.base_font).len() < info.base_font.len();
        let mut codes = HashMap::new();
        let mut insert = |text: Option<String>, bytes: Vec<u8>| {
            let mut chars = text.as_deref().unwrap_or("").chars();
            if let (Some(c), None) = (chars.next(), chars.next()) {
                codes.entry(c).or_insert(bytes);
            }
        };

        if font.get_name(b"Subtype") == Some(b"Type0") {
            match direct(font.get(b"Encoding"), &mut resolve) {
                Some(PdfObject::Name(n)) if n != b"Identity-H" && n != b"Identity-V" => {
                    return None;
                }
                _ => {}
            }
            let to_unicode = to_unicode?;
            let cmap = type0_encoding_cmap(&font, &mut resolve, &mut decode);
            let descendant = descendant_font(&font, &mut resolve)
                .map(|d| resolve_font_entries(&d, &mut resolve))?;
            let program = font_file2(&descendant, &mut resolve, &mut decode);
            if subset && program.is_none() {
                return None;
            }
            let face = match &program {
                Some(data) => Some(Face::parse(data, 0).ok()?),
                None => None,
            };
            let cid_to_gid = cid_to_gid_map(&descendant, &mut resolve, &mut decode);
            for (code, text) in to_unicode.entries() {
                let Some(bytes) = code_bytes(&cmap, code) else {
                    continue;
                };
                if let Some(face) = &face {
                    let cid = cmap.next_code(&bytes).cid;
                    let gid = match &cid_to_gid {
                        Some(map) => usize::try_from(cid)
                            .ok()
                            .and_then(|cid| cid.checked_mul(2))
                            .and_then(|at| map.get(at..at + 2))
                            .map_or(0, |b| u16::from_be_bytes([b[0], b[1]])),
                        None => u16::try_from(cid).unwrap_or(0),
                    };
                    if !has_glyph(face, gid) {
                        continue;
                    }
                }
                insert(Some(text), bytes);
            }
        } else {
            let program = font_file2(&font, &mut resolve, &mut decode);
            if subset && program.is_none() {
                return None;
            }
            let face = match &program {
                Some(data) => Some(Face::parse(data, 0).ok()?),
                None => None,
            };
            let encoding = font.get(b"Encoding").map(|_| info.encoding);
            let symbolic = info.descriptor.as_ref().is_some_and(|d| d.is_symbolic());
            for code in 0..=255u8 {
                if let Some(face) = &face {
                    let candidates = truetype_glyph_candidates(
                        face,
                        code,
                        encoding,
                        &info.differences,
                        symbolic,
                    );
                    if !candidates.first().is_some_and(|g| has_glyph(face, g.0)) {
                        continue;
                    }
                }
                let text = to_unicode
                    .as_ref()
                    .and_then(|t| t.lookup(u32::from(code)))
                    .unwrap_or_else(|| info.decode_simple_code(code));
                insert(Some(text), vec![code]);
            }
        }
        Some(FontEncoder { codes })
    }

    /// `text` in this font's codes, `None` when a character has none.
    pub(crate) fn encode(&self, text: &str) -> Option<Vec<u8>> {
        let mut out = Vec::new();
        for c in text.chars() {
            out.extend_from_slice(self.codes.get(&c)?);
        }
        Some(out)
    }
}

/// `obj`, resolved when it is a reference.
fn direct(
    obj: Option<&PdfObject>,
    resolve: &mut impl FnMut(&IndirectRef) -> Option<PdfObject>,
) -> Option<PdfObject> {
    match obj? {
        PdfObject::Reference(r) => resolve(r),
        other => Some(other.clone()),
    }
}

/// The decoded data of the stream `obj` is, or refers to.
fn stream_data(
    obj: Option<&PdfObject>,
    resolve: &mut impl FnMut(&IndirectRef) -> Option<PdfObject>,
    decode: &mut impl FnMut(&PdfDict, &[u8]) -> Option<Vec<u8>>,
) -> Option<Vec<u8>> {
    match direct(obj, resolve)? {
        PdfObject::Stream { dict, data } => decode(&dict, &data),
        _ => None,
    }
}

fn to_unicode(
    font: &PdfDict,
    resolve: &mut impl FnMut(&IndirectRef) -> Option<PdfObject>,
    decode: &mut impl FnMut(&PdfDict, &[u8]) -> Option<Vec<u8>>,
) -> Option<ToUnicodeCMap> {
    stream_data(font.get(b"ToUnicode"), resolve, decode).map(|d| ToUnicodeCMap::parse(&d))
}

/// The `FontFile2` program of `font`'s descriptor.
fn font_file2(
    font: &PdfDict,
    resolve: &mut impl FnMut(&IndirectRef) -> Option<PdfObject>,
    decode: &mut impl FnMut(&PdfDict, &[u8]) -> Option<Vec<u8>>,
) -> Option<Vec<u8>> {
    let descriptor = match direct(font.get(b"FontDescriptor"), resolve)? {
        PdfObject::Dict(d) => d,
        _ => return None,
    };
    stream_data(descriptor.get(b"FontFile2"), resolve, decode)
}

/// A CID font's `/CIDToGIDMap` stream data; `None` for `/Identity` or none.
fn cid_to_gid_map(
    cid_font: &PdfDict,
    resolve: &mut impl FnMut(&IndirectRef) -> Option<PdfObject>,
    decode: &mut impl FnMut(&PdfDict, &[u8]) -> Option<Vec<u8>>,
) -> Option<Vec<u8>> {
    stream_data(cid_font.get(b"CIDToGIDMap"), resolve, decode)
}

/// The bytes of `code` as `cmap` reads it: the length whose bytes are one
/// whole code of a codespace range.
fn code_bytes(cmap: &EncodingCMap, code: u32) -> Option<Vec<u8>> {
    (1..=4usize).find_map(|len| {
        if len < 4 && code >> (8 * len) != 0 {
            return None;
        }
        let bytes = code.to_be_bytes()[4 - len..].to_vec();
        cmap.is_whole_code(&bytes).then_some(bytes)
    })
}

/// Whether glyph `gid` is drawn from `face`: not `.notdef`, and with an
/// outline or an advance.
fn has_glyph(face: &Face, gid: u16) -> bool {
    let glyph = GlyphId(gid);
    gid != 0
        && gid < face.number_of_glyphs()
        && (face.glyph_bounding_box(glyph).is_some()
            || face.glyph_hor_advance(glyph).unwrap_or(0) > 0)
}
