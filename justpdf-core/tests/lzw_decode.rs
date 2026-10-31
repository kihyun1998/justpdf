//! LZWDecode through `decode_stream`.
//!
//! `lzw_early1.bin` is `lzw_plain.txt` encoded by pypdf's `LzwCodec` (early
//! change 1); `lzw_early0.bin` is the same text encoded with early change 0.
//! Both decode to `lzw_plain.txt` in MuPDF, the second with `/EarlyChange 0`.

use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use justpdf_core::object::{PdfDict, PdfObject};
use justpdf_core::stream::decode_stream;

fn fixture(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn lzw_dict(early_change: Option<i64>) -> PdfDict {
    let mut dict = PdfDict::new();
    dict.insert(b"Filter".to_vec(), PdfObject::Name(b"LZWDecode".to_vec()));
    if let Some(v) = early_change {
        let mut parms = PdfDict::new();
        parms.insert(b"EarlyChange".to_vec(), PdfObject::Integer(v));
        dict.insert(b"DecodeParms".to_vec(), PdfObject::Dict(parms));
    }
    dict
}

/// Decodes on another thread, failing after 10 s instead of hanging the run,
/// or at once if the decoder panics.
fn decode_lzw(data: &[u8], early_change: Option<i64>) -> Vec<u8> {
    let data = data.to_vec();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = tx.send(decode_stream(&data, &lzw_dict(early_change)));
    });
    rx.recv_timeout(Duration::from_secs(10))
        .expect("LZW decode panicked or did not finish within 10 s")
        .expect("LZW decode failed")
}

/// Packs `(code, width)` pairs MSB first.
fn pack(codes: &[(u32, u32)]) -> Vec<u8> {
    let mut out = Vec::new();
    let (mut acc, mut n) = (0u64, 0u32);
    for &(code, width) in codes {
        acc = (acc << width) | code as u64;
        n += width;
        while n >= 8 {
            n -= 8;
            out.push((acc >> n) as u8);
        }
    }
    if n > 0 {
        out.push((acc << (8 - n)) as u8);
    }
    out
}

#[test]
fn a_stream_without_an_end_code_stops_at_the_end_of_its_data() {
    // 8 bits hold no 9-bit code; 16 bits hold code 0 and 7 bits of padding.
    assert_eq!(decode_lzw(&[0x00], None), b"");
    assert_eq!(decode_lzw(&[0x00, 0x00], None), b"\x00");
}

#[test]
fn a_short_stream_decodes() {
    // Decodes to `-----A---B` in MuPDF.
    let data = [0x80, 0x0B, 0x60, 0x50, 0x22, 0x0C, 0x0C, 0x85, 0x01];
    assert_eq!(decode_lzw(&data, None), b"-----A---B");
}

#[test]
fn a_code_one_past_the_table_repeats_the_previous_string_and_its_first_byte() {
    // Codes 97, 98, 258 ("ab"), 260: 260 is not in the table yet, so it is
    // "ab" + "a". Decodes to `abababa` in MuPDF.
    let data = [0x80, 0x18, 0x4C, 0x50, 0x28, 0x24, 0x04];
    assert_eq!(decode_lzw(&data, None), b"abababa");
}

#[test]
fn a_code_past_the_table_is_an_error() {
    // 258 right after a clear, and 300 when the table ends at 259.
    for codes in [
        vec![(256, 9), (258, 9), (257, 9)],
        vec![(97, 9), (98, 9), (300, 9)],
    ] {
        assert!(
            decode_stream(&pack(&codes), &lzw_dict(None)).is_err(),
            "{codes:?}"
        );
    }
}

#[test]
fn a_standard_stream_decodes_through_every_code_width() {
    let plain = fixture("lzw_plain.txt");
    let encoded = fixture("lzw_early1.bin");
    assert_eq!(decode_lzw(&encoded, None), plain);
    assert_eq!(decode_lzw(&encoded, Some(1)), plain);
}

#[test]
fn early_change_0_widens_codes_one_entry_later() {
    let plain = fixture("lzw_plain.txt");
    assert_eq!(decode_lzw(&fixture("lzw_early0.bin"), Some(0)), plain);
    // The same bytes read with early change 1 are not the text.
    let as_early1 = decode_stream(&fixture("lzw_early0.bin"), &lzw_dict(None));
    assert!(as_early1.map_or(true, |out| out != plain));
}

#[test]
fn an_early_change_other_than_0_reads_as_1() {
    let plain = fixture("lzw_plain.txt");
    let encoded = fixture("lzw_early1.bin");
    assert_eq!(decode_lzw(&encoded, Some(2)), plain);
    assert_eq!(decode_lzw(&encoded, Some(-1)), plain);
}

#[test]
fn a_truncated_stream_returns_what_decoded_before_the_cut() {
    let plain = fixture("lzw_plain.txt");
    let encoded = fixture("lzw_early1.bin");
    // The bytes the whole codes of each prefix decode to.
    for (cut, len) in [(3, 1), (100, 97), (1000, 1219), (5000, 6889), (9999, 13810)] {
        assert_eq!(
            decode_lzw(&encoded[..cut], None),
            &plain[..len],
            "cut at {cut} bytes"
        );
    }
}

#[test]
fn a_full_table_takes_no_new_entries() {
    // Code 65 after a clear, then 65 again until the table holds 4096
    // entries (258..=4095 are all "AA"), ten more 65s, code 4095, and EOD.
    let mut codes = vec![(256, 9)];
    let mut width = 9;
    let mut next = 258u32;
    let mut first = true;
    let mut push = |code: u32, codes: &mut Vec<(u32, u32)>| {
        codes.push((code, width));
        if first {
            first = false;
        } else if next < 4096 {
            next += 1;
            if next + 1 >= 1 << width && width < 12 {
                width += 1;
            }
        }
    };
    for _ in 0..(4096 - 258 + 1) + 10 {
        push(65, &mut codes);
    }
    push(4095, &mut codes);
    codes.push((257, width));
    assert_eq!(width, 12);

    let mut want = vec![b'A'; 4096 - 258 + 1 + 10];
    want.extend_from_slice(b"AA");
    assert_eq!(decode_lzw(&pack(&codes), None), want);
}
