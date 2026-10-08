//! A Type0 font's encoding CMap: character codes to CIDs (ISO 32000-2
//! §9.7.5, §9.7.6).

use crate::content::{Operand, parse_content_stream};
use crate::object::{IndirectRef, PdfDict, PdfObject};

/// A codespace range: codes of `len` bytes whose every byte lies between
/// the corresponding bytes of `low` and `high`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Codespace {
    len: usize,
    low: [u8; 4],
    high: [u8; 4],
}

impl Codespace {
    fn new(low: &[u8], high: &[u8]) -> Option<Self> {
        if low.len() != high.len() || low.is_empty() || low.len() > 4 {
            return None;
        }
        let mut range = Codespace {
            len: low.len(),
            low: [0; 4],
            high: [0; 4],
        };
        range.low[..low.len()].copy_from_slice(low);
        range.high[..high.len()].copy_from_slice(high);
        Some(range)
    }

    /// Whether `bytes` is a whole code of this range.
    fn matches(&self, bytes: &[u8]) -> bool {
        bytes.len() == self.len && self.matches_prefix(bytes)
    }

    /// Whether `bytes` begins a code of this range.
    fn matches_prefix(&self, bytes: &[u8]) -> bool {
        bytes.len() <= self.len
            && bytes
                .iter()
                .enumerate()
                .all(|(i, &b)| self.low[i] <= b && b <= self.high[i])
    }
}

/// Codes `low..=high` of `len` bytes map to CIDs from `cid` (`offset`: a
/// CID range) or all to `cid` (a notdef range).
#[derive(Debug, Clone, Copy)]
struct CidRange {
    len: usize,
    low: u32,
    high: u32,
    cid: u64,
    offset: bool,
}

impl CidRange {
    /// The CID of `code`, which lies in this range; 0 past the CID limit.
    fn cid_of(&self, code: u32) -> u32 {
        let cid = if self.offset {
            self.cid + u64::from(code - self.low)
        } else {
            self.cid
        };
        u32::try_from(cid).unwrap_or(0)
    }

    /// This range cut down to `low..=high`.
    fn slice(&self, low: u32, high: u32) -> CidRange {
        let cid = if self.offset {
            self.cid + u64::from(low - self.low)
        } else {
            self.cid
        };
        CidRange {
            low,
            high,
            cid,
            ..*self
        }
    }
}

/// Ranges sorted by code length and then by first code, never overlapping.
#[derive(Debug, Clone, Default)]
struct Ranges(Vec<CidRange>);

impl Ranges {
    /// Add `new`, which replaces whatever earlier ranges map of its codes.
    fn insert(&mut self, new: CidRange) {
        let ranges = &mut self.0;
        let start = ranges.partition_point(|r| (r.len, r.high) < (new.len, new.low));
        let mut end = start;
        while end < ranges.len() && ranges[end].len == new.len && ranges[end].low <= new.high {
            end += 1;
        }
        let mut replacement = Vec::with_capacity(3);
        let mut tail = None;
        for r in &ranges[start..end] {
            if r.low < new.low {
                replacement.push(r.slice(r.low, new.low - 1));
            }
            if r.high > new.high {
                tail = Some(r.slice(new.high + 1, r.high));
            }
        }
        replacement.push(new);
        replacement.extend(tail);
        ranges.splice(start..end, replacement);
    }

    /// The range holding the `len`-byte `code`.
    fn find(&self, len: usize, code: u32) -> Option<&CidRange> {
        let i = self.0.partition_point(|r| (r.len, r.low) <= (len, code));
        let range = self.0.get(i.checked_sub(1)?)?;
        (range.len == len && code <= range.high).then_some(range)
    }
}

/// The mappings of one CMap.
#[derive(Debug, Clone, Default)]
struct Mappings {
    cids: Ranges,
    notdefs: Ranges,
}

/// One character code read from a string shown in a Type0 font.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CMapCode {
    /// The code's bytes as a big-endian number.
    pub code: u32,
    /// How many bytes of the string the code took.
    pub len: usize,
    /// The CID the code selects.
    pub cid: u32,
}

/// A Type0 font's encoding CMap.
#[derive(Debug, Clone)]
pub struct EncodingCMap {
    codespace: Vec<Codespace>,
    /// This CMap's mappings, then those of each CMap it uses in turn.
    layers: Vec<Mappings>,
    wmode: u8,
}

impl EncodingCMap {
    /// `Identity-H` (`wmode` 0) or `Identity-V` (`wmode` 1): two-byte codes,
    /// each its own CID.
    pub fn identity(wmode: u8) -> Self {
        let mut layer = Mappings::default();
        layer.cids.insert(CidRange {
            len: 2,
            low: 0,
            high: 0xFFFF,
            cid: 0,
            offset: true,
        });
        EncodingCMap {
            codespace: vec![Codespace::new(&[0, 0], &[0xFF, 0xFF]).unwrap()],
            layers: vec![layer],
            wmode,
        }
    }

    /// The writing mode: 0 horizontal, 1 vertical.
    pub fn wmode(&self) -> u8 {
        self.wmode
    }

    /// The first character code of `bytes`, which must not be empty
    /// (§9.7.6.2): the shortest whole code matching a codespace range; for
    /// bytes matching none, the length the best partial match gives
    /// (§9.7.6.3).
    pub fn next_code(&self, bytes: &[u8]) -> CMapCode {
        for len in 1..=bytes.len().min(4) {
            let candidate = &bytes[..len];
            if self.codespace.iter().any(|c| c.matches(candidate)) {
                let code = be_number(candidate);
                return CMapCode {
                    code,
                    len,
                    cid: self.lookup(len, code),
                };
            }
        }

        let len = self.invalid_code_len(bytes).min(bytes.len());
        let code = be_number(&bytes[..len]);
        CMapCode {
            code,
            len,
            cid: self.lookup_notdef(len, code),
        }
    }

    /// Every character code of `bytes`, in order.
    pub fn decode(&self, bytes: &[u8]) -> Vec<CMapCode> {
        let mut codes = Vec::new();
        let mut rest = bytes;
        while !rest.is_empty() {
            let code = self.next_code(rest);
            rest = &rest[code.len..];
            codes.push(code);
        }
        codes
    }

    /// The bytes an invalid code takes (§9.7.6.3): the shortest codespace
    /// length when the first byte begins no range; otherwise the length of
    /// the range with the longest partial match, the shortest such range on
    /// a tie.
    fn invalid_code_len(&self, bytes: &[u8]) -> usize {
        let shortest = self.codespace.iter().map(|c| c.len).min().unwrap_or(1);
        let mut best: Option<(usize, usize)> = None; // (matched bytes, range length)
        for range in &self.codespace {
            let matched = (1..=range.len.min(bytes.len()))
                .rev()
                .find(|&k| range.matches_prefix(&bytes[..k]));
            if let Some(matched) = matched {
                let better = match best {
                    None => true,
                    Some((m, l)) => matched > m || (matched == m && range.len < l),
                };
                if better {
                    best = Some((matched, range.len));
                }
            }
        }
        best.map_or(shortest, |(_, len)| len)
    }

    /// The CID of a valid code: its character mapping, else its notdef
    /// mapping, else 0.
    fn lookup(&self, len: usize, code: u32) -> u32 {
        self.layers
            .iter()
            .find_map(|l| l.cids.find(len, code))
            .map(|r| r.cid_of(code))
            .unwrap_or_else(|| self.lookup_notdef(len, code))
    }

    fn lookup_notdef(&self, len: usize, code: u32) -> u32 {
        self.layers
            .iter()
            .find_map(|l| l.notdefs.find(len, code))
            .map_or(0, |r| r.cid_of(code))
    }

    /// Parse an embedded CMap stream's decoded data.
    fn parse(data: &[u8]) -> Option<Parsed> {
        let ops = parse_content_stream(data).ok()?;
        let mut codespace = Vec::new();
        let mut layer = Mappings::default();
        let mut wmode = None;
        let mut use_cmap = None;

        let code = |operand: &Operand| -> Option<(usize, u32)> {
            let bytes = operand.as_str()?;
            (!bytes.is_empty() && bytes.len() <= 4).then(|| (bytes.len(), be_number(bytes)))
        };
        let cid = |operand: &Operand| u64::try_from(operand.as_i64()?).ok();
        let range = |chunk: &[Operand], offset: bool| -> Option<CidRange> {
            let ((len, low), (high_len, high)) = (code(&chunk[0])?, code(&chunk[1])?);
            (len == high_len && low <= high).then_some(())?;
            Some(CidRange {
                len,
                low,
                high,
                cid: cid(&chunk[2])?,
                offset,
            })
        };
        let single = |chunk: &[Operand], offset: bool| -> Option<CidRange> {
            let (len, value) = code(&chunk[0])?;
            Some(CidRange {
                len,
                low: value,
                high: value,
                cid: cid(&chunk[1])?,
                offset,
            })
        };

        for op in &ops {
            let operands = op.operands.as_slice();
            let (target, chunk, offset) = match op.operator.as_slice() {
                b"endcidrange" => (&mut layer.cids, 3, true),
                b"endcidchar" => (&mut layer.cids, 2, true),
                b"endnotdefrange" => (&mut layer.notdefs, 3, false),
                b"endnotdefchar" => (&mut layer.notdefs, 2, false),
                b"endcodespacerange" => {
                    for pair in operands.chunks_exact(2) {
                        if let (Some(low), Some(high)) = (pair[0].as_str(), pair[1].as_str())
                            && let Some(range) = Codespace::new(low, high)
                        {
                            codespace.push(range);
                        }
                    }
                    continue;
                }
                b"usecmap" => {
                    if let Some(name) = operands.last().and_then(|o| o.as_name()) {
                        use_cmap = Some(name.to_vec());
                    }
                    continue;
                }
                b"def" => {
                    if let [Operand::Name(key), value] = operands
                        && key == b"WMode"
                    {
                        wmode = value.as_i64().and_then(|v| u8::try_from(v).ok());
                    }
                    continue;
                }
                _ => continue,
            };
            for entry in operands.chunks_exact(chunk) {
                let parsed = if chunk == 3 {
                    range(entry, offset)
                } else {
                    single(entry, offset)
                };
                if let Some(r) = parsed {
                    target.insert(r);
                }
            }
        }

        Some(Parsed {
            cmap: EncodingCMap {
                codespace,
                layers: vec![layer],
                wmode: wmode.unwrap_or(0),
            },
            use_cmap,
            wmode,
        })
    }

    /// Take `parent`'s mappings after this CMap's own, and its codespace
    /// when this CMap has none.
    fn use_cmap(&mut self, parent: EncodingCMap) {
        if self.codespace.is_empty() {
            self.codespace = parent.codespace;
        }
        self.layers.extend(parent.layers);
    }
}

/// What parsing an embedded CMap stream gives.
struct Parsed {
    cmap: EncodingCMap,
    /// The name the `usecmap` operator names.
    use_cmap: Option<Vec<u8>>,
    /// `/WMode` as the stream defines it.
    wmode: Option<u8>,
}

fn be_number(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0, |acc, &b| (acc << 8) | b as u32)
}

/// The CMap a predefined name stands for. Only `Identity-H` and
/// `Identity-V` are known.
fn named_cmap(name: &[u8]) -> Option<EncodingCMap> {
    match name {
        b"Identity-H" => Some(EncodingCMap::identity(0)),
        b"Identity-V" => Some(EncodingCMap::identity(1)),
        _ => None,
    }
}

/// `usecmap` chains are followed at most this deep.
const MAX_USE_CMAP_DEPTH: usize = 8;

/// The encoding CMap of a Type0 font dictionary, from its `/Encoding`:
/// `Identity-H`/`-V`, or an embedded CMap stream with the embedded or
/// Identity CMaps it uses. A missing, unreadable or other named encoding
/// gives `Identity-H`. `resolve` resolves references; `decode` returns a
/// stream's decoded data.
pub fn type0_encoding_cmap(
    font: &PdfDict,
    mut resolve: impl FnMut(&IndirectRef) -> Option<PdfObject>,
    mut decode: impl FnMut(&PdfDict, &[u8]) -> Option<Vec<u8>>,
) -> EncodingCMap {
    let encoding = match font.get(b"Encoding") {
        Some(PdfObject::Reference(r)) => resolve(r),
        other => other.cloned(),
    };
    let cmap = match encoding {
        Some(PdfObject::Name(name)) => named_cmap(&name),
        Some(PdfObject::Stream { dict, data }) => {
            let mut visited = Vec::new();
            embedded_cmap(&dict, &data, &mut resolve, &mut decode, &mut visited)
        }
        _ => None,
    };
    cmap.unwrap_or_else(|| EncodingCMap::identity(0))
}

fn embedded_cmap(
    dict: &PdfDict,
    data: &[u8],
    resolve: &mut impl FnMut(&IndirectRef) -> Option<PdfObject>,
    decode: &mut impl FnMut(&PdfDict, &[u8]) -> Option<Vec<u8>>,
    visited: &mut Vec<IndirectRef>,
) -> Option<EncodingCMap> {
    let decoded = decode(dict, data)?;
    let Parsed {
        mut cmap,
        use_cmap: use_cmap_name,
        wmode,
    } = EncodingCMap::parse(&decoded)?;
    // The stream dictionary's /WMode wins over the one the stream defines.
    cmap.wmode = dict
        .get_i64(b"WMode")
        .and_then(|v| u8::try_from(v).ok())
        .or(wmode)
        .unwrap_or(0);

    let parent = match dict.get(b"UseCMap") {
        Some(PdfObject::Reference(r))
            if !visited.contains(r) && visited.len() < MAX_USE_CMAP_DEPTH =>
        {
            visited.push(r.clone());
            match resolve(r) {
                Some(PdfObject::Stream { dict, data }) => {
                    embedded_cmap(&dict, &data, resolve, decode, visited)
                }
                _ => None,
            }
        }
        Some(PdfObject::Name(name)) => named_cmap(name),
        _ => use_cmap_name.as_deref().and_then(named_cmap),
    };
    if let Some(parent) = parent {
        cmap.use_cmap(parent);
    }
    if cmap.codespace.is_empty() {
        cmap.codespace = EncodingCMap::identity(0).codespace;
    }
    Some(cmap)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmap(body: &str) -> EncodingCMap {
        let data = format!(
            "/CIDInit /ProcSet findresource begin 12 dict begin begincmap \
             /CIDSystemInfo << /Registry (Adobe) /Ordering (Test) /Supplement 0 >> def \
             /CMapName /Test def {body} endcmap CMapName currentdict /CMap defineresource pop end end"
        );
        EncodingCMap::parse(data.as_bytes()).unwrap().cmap
    }

    fn codes(cmap: &EncodingCMap, bytes: &[u8]) -> Vec<(u32, usize, u32)> {
        cmap.decode(bytes)
            .into_iter()
            .map(|c| (c.code, c.len, c.cid))
            .collect()
    }

    const MIXED: &str = "2 begincodespacerange <00> <7F> <8140> <9FFC> endcodespacerange \
         2 begincidrange <20> <7E> 1 <8140> <817E> 633 endcidrange \
         1 begincidchar <8151> 9000 endcidchar";

    #[test]
    fn identity_maps_each_two_byte_code_to_itself() {
        let id = EncodingCMap::identity(1);
        assert_eq!(id.wmode(), 1);
        assert_eq!(
            codes(&id, &[0x00, 0x41, 0x12, 0x34]),
            vec![(0x41, 2, 0x41), (0x1234, 2, 0x1234)]
        );
    }

    #[test]
    fn codes_are_split_by_the_codespace_and_mapped_by_cid_ranges_and_chars() {
        let c = cmap(MIXED);
        assert_eq!(
            codes(&c, b"A\x81\x40B\x81\x51"),
            vec![
                (0x41, 1, 0x41 - 0x20 + 1),
                (0x8140, 2, 633),
                (0x42, 1, 0x42 - 0x20 + 1),
                (0x8151, 2, 9000),
            ]
        );
    }

    #[test]
    fn a_codespace_compares_byte_by_byte_not_as_one_number() {
        // 0x8200 lies between 0x8140 and 0x9FFC as a number, but its second
        // byte is below 0x40, so it is not a code of that range: the CID range
        // covering it does not apply.
        let c = cmap(
            "1 begincodespacerange <8140> <9FFC> endcodespacerange \
             1 begincidrange <8140> <9FFC> 1000 endcidrange",
        );
        assert_eq!(codes(&c, b"\x82\x00"), vec![(0x8200, 2, 0)]);
        assert_eq!(codes(&c, b"\x82\x40"), vec![(0x8240, 2, 1000 + 0x100)]);
    }

    #[test]
    fn a_later_mapping_replaces_the_part_of_an_earlier_one_it_overlaps() {
        // A broad range with single-code exceptions after it, then a range
        // over part of an exception's neighbours.
        let c = cmap(
            "1 begincodespacerange <0000> <FFFF> endcodespacerange \
             1 begincidrange <0000> <FFFF> 0 endcidrange \
             1 begincidchar <0041> 300 endcidchar \
             1 begincidrange <0050> <0052> 900 endcidrange \
             1 begincidchar <0051> 5 endcidchar",
        );
        assert_eq!(
            codes(
                &c,
                b"\x00\x40\x00\x41\x00\x42\x00\x50\x00\x51\x00\x52\x01\x00"
            ),
            vec![
                (0x40, 2, 0x40),
                (0x41, 2, 300),
                (0x42, 2, 0x42),
                (0x50, 2, 900),
                (0x51, 2, 5),
                (0x52, 2, 902),
                (0x100, 2, 0x100),
            ]
        );
        // The other order: the broad range, defined last, wins everywhere.
        let c = cmap(
            "1 begincodespacerange <0000> <FFFF> endcodespacerange \
             1 begincidchar <0041> 300 endcidchar \
             1 begincidrange <0000> <FFFF> 0 endcidrange",
        );
        assert_eq!(codes(&c, b"\x00\x41"), vec![(0x41, 2, 0x41)]);
    }

    #[test]
    fn a_cid_past_the_cid_limit_is_0_instead_of_overflowing() {
        // Code 2 would be CID 2^32 + 1, which wraps to 1 as a u32.
        let c = cmap(
            "1 begincodespacerange <00> <FF> endcodespacerange \
             1 begincidrange <00> <FF> 4294967295 endcidrange",
        );
        assert_eq!(
            codes(&c, b"\x00\x01\x02"),
            vec![(0, 1, 4294967295), (1, 1, 0), (2, 1, 0)]
        );
    }

    #[test]
    fn a_one_byte_code_is_matched_before_a_longer_one() {
        // The codespace ranges overlap (not allowed, but seen): `A` is read
        // as a one-byte code before `AA` can be read as a two-byte one.
        let c = cmap(
            "2 begincodespacerange <00> <7F> <4100> <41FF> endcodespacerange \
             1 begincidrange <4100> <41FF> 500 endcidrange",
        );
        assert_eq!(codes(&c, b"AA"), vec![(0x41, 1, 0), (0x41, 1, 0)]);
    }

    #[test]
    fn an_unmapped_valid_code_takes_its_notdef_mapping_or_cid_0() {
        let c = cmap(
            "1 begincodespacerange <0000> <FFFF> endcodespacerange \
             1 begincidrange <0100> <01FF> 500 endcidrange \
             1 beginnotdefrange <0200> <02FF> 7 endnotdefrange \
             1 beginnotdefchar <0300> 8 endnotdefchar",
        );
        assert_eq!(
            codes(&c, b"\x01\x05\x02\x50\x03\x00\x04\x00"),
            vec![
                (0x0105, 2, 505),
                (0x0250, 2, 7),
                (0x0300, 2, 8),
                (0x0400, 2, 0),
            ]
        );
    }

    #[test]
    fn invalid_bytes_take_the_length_of_the_best_partial_match() {
        let c = cmap(MIXED);
        // 0xA0 begins no range: the shortest range length, 1 byte.
        assert_eq!(codes(&c, b"\xA0A")[0], (0xA0, 1, 0));
        // 0x81 0x20 partly matches the 2-byte range: 2 bytes, CID 0.
        assert_eq!(
            codes(&c, b"\x81\x20A"),
            vec![(0x8120, 2, 0), (0x41, 1, 0x41 - 0x20 + 1)]
        );
        // A partial match cut short by the end of the string takes what is left.
        assert_eq!(codes(&c, b"\x81"), vec![(0x81, 1, 0)]);
    }

    #[test]
    fn invalid_bytes_take_the_longest_partial_match_and_the_shortest_range_on_a_tie() {
        let c = cmap("2 begincodespacerange <8140> <81FF> <814000> <81FFFF> endcodespacerange");
        // 0x81 0x30: both ranges match one byte; the shorter range wins.
        assert_eq!(codes(&c, b"\x81\x30\x41\x42")[0], (0x8130, 2, 0));
        let c = cmap("2 begincodespacerange <8140> <81FF> <813040> <8130FF> endcodespacerange");
        // 0x81 0x30 0x20: the 3-byte range matches two bytes, the 2-byte one
        // one, and neither a whole code: three bytes go.
        assert_eq!(codes(&c, b"\x81\x30\x20\x42")[0], (0x813020, 3, 0));
    }

    #[test]
    fn wmode_and_usecmap_names_are_read() {
        let Parsed {
            cmap: c,
            use_cmap: used,
            ..
        } = EncodingCMap::parse(
            b"/WMode 1 def /Identity-H usecmap 1 begincodespacerange <00> <FF> endcodespacerange",
        )
        .unwrap();
        assert_eq!(c.wmode(), 1);
        assert_eq!(used.as_deref(), Some(&b"Identity-H"[..]));
    }

    #[test]
    fn the_stream_dictionarys_wmode_wins_over_the_streams() {
        let mut vertical = PdfDict::new();
        vertical.insert(b"WMode".to_vec(), PdfObject::Integer(1));
        let streams = vec![
            // The dictionary says 1, the stream 0.
            (
                10,
                vertical,
                "/WMode 0 def 1 begincidchar <0041> 300 endcidchar",
            ),
            // A comment names WMode; the stream then defines 1.
            (11, PdfDict::new(), "% WMode\n/WMode 1 def"),
            (12, PdfDict::new(), "% no WMode def here"),
        ];
        let wmode = |n| {
            let font = font_with_encoding(reference(n));
            type0_encoding_cmap(&font, resolver(&streams), |_, d| Some(d.to_vec())).wmode()
        };
        assert_eq!((wmode(10), wmode(11), wmode(12)), (1, 1, 0));
    }

    #[test]
    fn a_cmap_with_no_codespace_reads_two_byte_codes() {
        let font = font_with_encoding(PdfObject::Reference(IndirectRef {
            obj_num: 10,
            gen_num: 0,
        }));
        let streams = vec![(10, PdfDict::new(), "1 begincidchar <0041> 300 endcidchar")];
        let c = type0_encoding_cmap(&font, resolver(&streams), |_, d| Some(d.to_vec()));
        assert_eq!(codes(&c, b"\x00\x41"), vec![(0x41, 2, 300)]);
    }

    fn font_with_encoding(encoding: PdfObject) -> PdfDict {
        let mut font = PdfDict::new();
        font.insert(b"Encoding".to_vec(), encoding);
        font
    }

    fn reference(obj_num: u32) -> PdfObject {
        PdfObject::Reference(IndirectRef {
            obj_num,
            gen_num: 0,
        })
    }

    /// Resolves object `n` to a stream with dictionary `dict` and data `body`.
    fn resolver(streams: &[(u32, PdfDict, &str)]) -> impl FnMut(&IndirectRef) -> Option<PdfObject> {
        let streams: Vec<_> = streams
            .iter()
            .map(|(n, d, b)| (*n, d.clone(), b.as_bytes().to_vec()))
            .collect();
        move |r| {
            streams
                .iter()
                .find(|(n, _, _)| *n == r.obj_num)
                .map(|(_, dict, data)| PdfObject::Stream {
                    dict: dict.clone(),
                    data: data.clone(),
                })
        }
    }

    fn use_cmap(obj_num: u32) -> PdfDict {
        let mut dict = PdfDict::new();
        dict.insert(b"UseCMap".to_vec(), reference(obj_num));
        dict
    }

    #[test]
    fn an_embedded_cmap_inherits_the_embedded_cmap_it_uses() {
        // 10 maps <41> and uses 11, which has the one-byte codespace, maps
        // <41> too (10's mapping wins) and maps <42>.
        let streams = vec![
            (10, use_cmap(11), "1 begincidchar <41> 300 endcidchar"),
            (
                11,
                PdfDict::new(),
                "1 begincodespacerange <00> <FF> endcodespacerange \
                 2 begincidchar <41> 1 <42> 301 endcidchar",
            ),
        ];
        let font = font_with_encoding(reference(10));
        let c = type0_encoding_cmap(&font, resolver(&streams), |_, d| Some(d.to_vec()));
        assert_eq!(codes(&c, b"AB"), vec![(0x41, 1, 300), (0x42, 1, 301)]);
    }

    #[test]
    fn a_usecmap_cycle_terminates() {
        let streams = vec![
            (10, use_cmap(11), "1 begincidchar <0041> 300 endcidchar"),
            (11, use_cmap(10), "1 begincidchar <0042> 301 endcidchar"),
        ];
        let font = font_with_encoding(reference(10));
        let c = type0_encoding_cmap(&font, resolver(&streams), |_, d| Some(d.to_vec()));
        assert_eq!(
            codes(&c, b"\x00\x41\x00\x42"),
            vec![(0x41, 2, 300), (0x42, 2, 301)]
        );
    }

    #[test]
    fn named_and_unreadable_encodings_fall_back_to_identity() {
        for encoding in [
            PdfObject::Name(b"Identity-H".to_vec()),
            PdfObject::Name(b"UniJIS-UCS2-H".to_vec()),
            reference(99),
        ] {
            let font = font_with_encoding(encoding);
            let c = type0_encoding_cmap(&font, resolver(&[]), |_, d| Some(d.to_vec()));
            assert_eq!(codes(&c, b"\x01\x2C"), vec![(300, 2, 300)]);
        }
        let font = font_with_encoding(PdfObject::Name(b"Identity-V".to_vec()));
        let c = type0_encoding_cmap(&font, resolver(&[]), |_, d| Some(d.to_vec()));
        assert_eq!(c.wmode(), 1);
    }
}
