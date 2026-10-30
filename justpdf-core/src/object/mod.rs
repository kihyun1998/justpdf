mod types;

pub(crate) use types::{
    ByteSink, Number, name_syntax, real_syntax, string_syntax, write_name, write_real, write_string,
};
pub use types::{IndirectRef, PdfDict, PdfObject};

use crate::error::{JustPdfError, Result};
use crate::tokenizer::Tokenizer;
use crate::tokenizer::token::{Keyword, Token};

/// Parse a single PDF object from the tokenizer's current position.
/// Does NOT handle `N M obj ... endobj` wrappers — see `parse_indirect_object`.
pub fn parse_object(tokenizer: &mut Tokenizer<'_>) -> Result<PdfObject> {
    let offset = tokenizer.pos();
    let Some(token) = tokenizer.next_token()? else {
        return Err(JustPdfError::UnexpectedEof { offset });
    };

    match token {
        Token::Keyword(Keyword::Null) => Ok(PdfObject::Null),
        Token::Keyword(Keyword::True) => Ok(PdfObject::Bool(true)),
        Token::Keyword(Keyword::False) => Ok(PdfObject::Bool(false)),
        Token::Integer(v) => {
            // Peek ahead to check for "N M R" (indirect reference)
            let saved = tokenizer.pos();
            match tokenizer.next_token() {
                Ok(Some(Token::Integer(gen_val))) => match tokenizer.next_token() {
                    Ok(Some(Token::Keyword(Keyword::R))) => Ok(PdfObject::Reference(IndirectRef {
                        obj_num: v as u32,
                        gen_num: gen_val as u16,
                    })),
                    _ => {
                        tokenizer.seek(saved);
                        Ok(PdfObject::Integer(v))
                    }
                },
                _ => {
                    tokenizer.seek(saved);
                    Ok(PdfObject::Integer(v))
                }
            }
        }
        Token::Real(v) => Ok(PdfObject::Real(v)),
        Token::LiteralString(v) => Ok(PdfObject::String(v)),
        Token::HexString(v) => Ok(PdfObject::String(v)),
        Token::Name(v) => Ok(PdfObject::Name(v)),
        Token::ArrayBegin => {
            let mut arr = Vec::new();
            loop {
                let peek_pos = tokenizer.pos();
                match tokenizer.next_token()? {
                    Some(Token::ArrayEnd) => break,
                    Some(_tok) => {
                        tokenizer.seek(peek_pos);
                        arr.push(parse_object(tokenizer)?);
                    }
                    None => {
                        return Err(JustPdfError::UnexpectedEof { offset });
                    }
                }
            }
            Ok(PdfObject::Array(arr))
        }
        Token::DictBegin => {
            let dict = parse_dict_body(tokenizer, offset)?;
            Ok(PdfObject::Dict(dict))
        }
        _ => Err(JustPdfError::InvalidObject {
            offset,
            detail: format!("unexpected token: {token:?}"),
        }),
    }
}

/// Parse dict entries until `>>`. Assumes `<<` has already been consumed.
fn parse_dict_body(tokenizer: &mut Tokenizer<'_>, start: usize) -> Result<PdfDict> {
    let mut dict = PdfDict::new();
    loop {
        let peek_pos = tokenizer.pos();
        match tokenizer.next_token()? {
            Some(Token::DictEnd) => break,
            Some(Token::Name(key)) => {
                let value = parse_object(tokenizer)?;
                dict.insert(key, value);
            }
            Some(tok) => {
                return Err(JustPdfError::InvalidObject {
                    offset: peek_pos,
                    detail: format!("expected name or >> in dict, got: {tok:?}"),
                });
            }
            None => {
                return Err(JustPdfError::UnexpectedEof { offset: start });
            }
        }
    }
    Ok(dict)
}

/// Parse an indirect object: `N M obj <object> endobj`.
/// Returns (IndirectRef, PdfObject).
pub fn parse_indirect_object(tokenizer: &mut Tokenizer<'_>) -> Result<(IndirectRef, PdfObject)> {
    let offset = tokenizer.pos();

    let obj_num = match tokenizer.next_token()? {
        Some(Token::Integer(n)) => n as u32,
        _ => {
            return Err(JustPdfError::InvalidObject {
                offset,
                detail: "expected object number".into(),
            });
        }
    };

    let gen_num = match tokenizer.next_token()? {
        Some(Token::Integer(n)) => n as u16,
        _ => {
            return Err(JustPdfError::InvalidObject {
                offset,
                detail: "expected generation number".into(),
            });
        }
    };

    match tokenizer.next_token()? {
        Some(Token::Keyword(Keyword::Obj)) => {}
        _ => {
            return Err(JustPdfError::InvalidObject {
                offset,
                detail: "expected 'obj' keyword".into(),
            });
        }
    }

    let obj = parse_object(tokenizer)?;

    // Check for stream
    let saved = tokenizer.pos();
    let result = match tokenizer.next_token()? {
        Some(Token::Keyword(Keyword::Stream)) => {
            // Stream: the dict we just parsed must be the stream dict
            let dict = match obj {
                PdfObject::Dict(d) => d,
                _ => {
                    return Err(JustPdfError::InvalidObject {
                        offset,
                        detail: "stream must be preceded by a dictionary".into(),
                    });
                }
            };

            let stream_data = read_stream_data(tokenizer, &dict, offset)?;
            let stream_obj = PdfObject::Stream {
                dict,
                data: stream_data,
            };

            // Consume `endstream` when it follows the data.
            let saved2 = tokenizer.pos();
            if let Ok(Some(Token::Keyword(Keyword::EndStream))) = tokenizer.next_token() {
                // good
            } else {
                tokenizer.seek(saved2);
            }

            stream_obj
        }
        Some(Token::Keyword(Keyword::EndObj)) => {
            return Ok((IndirectRef { obj_num, gen_num }, obj));
        }
        _ => {
            tokenizer.seek(saved);
            obj
        }
    };

    // Consume endobj
    let saved = tokenizer.pos();
    if let Ok(Some(Token::Keyword(Keyword::EndObj))) = tokenizer.next_token() {
        // good
    } else {
        tokenizer.seek(saved);
    }

    Ok((IndirectRef { obj_num, gen_num }, result))
}

/// The raw data of a stream whose `stream` keyword the tokenizer has just
/// read: by a direct `/Length` when `endstream` follows it, else up to the
/// first `endstream`, else by an in-range `/Length`. Leaves the tokenizer at
/// the end of the data.
fn read_stream_data(
    tokenizer: &mut Tokenizer<'_>,
    dict: &PdfDict,
    start_offset: usize,
) -> Result<Vec<u8>> {
    // Skip the newline after 'stream' keyword
    let data = tokenizer.reader().data();
    let mut pos = tokenizer.pos();

    // PDF spec: stream keyword followed by \r\n or \n
    if pos < data.len() && data[pos] == b'\r' {
        pos += 1;
    }
    if pos < data.len() && data[pos] == b'\n' {
        pos += 1;
    }

    // The end a direct, non-negative /Length gives, when it is inside `data`.
    let declared_end = match dict.get(b"Length") {
        Some(PdfObject::Integer(n)) => usize::try_from(*n).ok(),
        _ => None,
    }
    .and_then(|length| pos.checked_add(length))
    .filter(|&end| end <= data.len());

    if let Some(end) = declared_end.filter(|&end| endstream_follows(data, end)) {
        tokenizer.seek(end);
        return Ok(data[pos..end].to_vec());
    }
    match find_stream_data_by_endstream(data, pos, start_offset) {
        Ok((stream_data, endstream_at)) => {
            tokenizer.seek(endstream_at);
            Ok(stream_data)
        }
        Err(err) => match declared_end {
            Some(end) => {
                tokenizer.seek(end);
                Ok(data[pos..end].to_vec())
            }
            None => Err(err),
        },
    }
}

/// Whether `endstream` is next in `data` at `pos`, after any whitespace.
fn endstream_follows(data: &[u8], pos: usize) -> bool {
    let rest = &data[pos..];
    let skip = rest
        .iter()
        .position(|&b| !crate::tokenizer::reader::is_pdf_whitespace(b))
        .unwrap_or(rest.len());
    rest[skip..].starts_with(b"endstream")
}

/// Whether a stream terminator starts at `data[0]`: `endstream`, or the
/// misspelled `endsteam` or truncated `endstrea` when whitespace follows.
fn is_stream_terminator(data: &[u8]) -> bool {
    let Some(rest) = data.strip_prefix(b"end") else {
        return false;
    };
    rest.starts_with(b"stream")
        || [&b"steam"[..], b"strea"].iter().any(|part| {
            rest.strip_prefix(*part)
                .and_then(|after| after.first())
                .is_some_and(|&b| crate::tokenizer::reader::is_pdf_whitespace(b))
        })
}

/// The stream data from `start` to the first stream terminator
/// (`is_stream_terminator`), without the one end-of-line marker before it,
/// and the offset of that terminator.
fn find_stream_data_by_endstream(
    data: &[u8],
    start: usize,
    err_offset: usize,
) -> Result<(Vec<u8>, usize)> {
    let Some(found) = (start..data.len()).find(|&i| is_stream_terminator(&data[i..])) else {
        return Err(JustPdfError::InvalidObject {
            offset: err_offset,
            detail: "could not find endstream".into(),
        });
    };
    let endstream_at = found;
    let mut end = endstream_at;
    if end > start && data[end - 1] == b'\n' {
        end -= 1;
    }
    if end > start && data[end - 1] == b'\r' {
        end -= 1;
    }
    Ok((data[start..end].to_vec(), endstream_at))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_null() {
        let mut t = Tokenizer::new(b"null");
        assert_eq!(parse_object(&mut t).unwrap(), PdfObject::Null);
    }

    #[test]
    fn test_parse_bool() {
        let mut t = Tokenizer::new(b"true");
        assert_eq!(parse_object(&mut t).unwrap(), PdfObject::Bool(true));

        let mut t = Tokenizer::new(b"false");
        assert_eq!(parse_object(&mut t).unwrap(), PdfObject::Bool(false));
    }

    #[test]
    fn test_parse_numbers() {
        let mut t = Tokenizer::new(b"42");
        assert_eq!(parse_object(&mut t).unwrap(), PdfObject::Integer(42));

        let mut t = Tokenizer::new(b"3.15");
        assert_eq!(parse_object(&mut t).unwrap(), PdfObject::Real(3.15));
    }

    #[test]
    fn test_parse_string() {
        let mut t = Tokenizer::new(b"(Hello)");
        assert_eq!(
            parse_object(&mut t).unwrap(),
            PdfObject::String(b"Hello".to_vec())
        );
    }

    #[test]
    fn test_parse_name() {
        let mut t = Tokenizer::new(b"/Type");
        assert_eq!(
            parse_object(&mut t).unwrap(),
            PdfObject::Name(b"Type".to_vec())
        );
    }

    #[test]
    fn test_parse_array() {
        let mut t = Tokenizer::new(b"[1 2 3]");
        assert_eq!(
            parse_object(&mut t).unwrap(),
            PdfObject::Array(vec![
                PdfObject::Integer(1),
                PdfObject::Integer(2),
                PdfObject::Integer(3),
            ])
        );
    }

    #[test]
    fn test_parse_dict() {
        let mut t = Tokenizer::new(b"<< /Type /Catalog /Pages 2 0 R >>");
        let obj = parse_object(&mut t).unwrap();
        match &obj {
            PdfObject::Dict(d) => {
                assert_eq!(d.get(b"Type"), Some(&PdfObject::Name(b"Catalog".to_vec())));
                assert_eq!(
                    d.get(b"Pages"),
                    Some(&PdfObject::Reference(IndirectRef {
                        obj_num: 2,
                        gen_num: 0
                    }))
                );
            }
            _ => panic!("expected dict, got {obj:?}"),
        }
    }

    #[test]
    fn test_parse_reference() {
        let mut t = Tokenizer::new(b"10 0 R");
        assert_eq!(
            parse_object(&mut t).unwrap(),
            PdfObject::Reference(IndirectRef {
                obj_num: 10,
                gen_num: 0
            })
        );
    }

    #[test]
    fn test_parse_indirect_object() {
        let input = b"1 0 obj\n<< /Type /Catalog >>\nendobj";
        let mut t = Tokenizer::new(input);
        let (iref, obj) = parse_indirect_object(&mut t).unwrap();
        assert_eq!(
            iref,
            IndirectRef {
                obj_num: 1,
                gen_num: 0
            }
        );
        assert!(matches!(obj, PdfObject::Dict(_)));
    }

    /// The data of `1 0 obj << /Length {length} >> stream\n{body}endobj`.
    fn stream_data(length: &str, body: &[u8]) -> Result<Vec<u8>> {
        let mut input = format!("1 0 obj\n<< /Length {length} >>\nstream\n").into_bytes();
        input.extend_from_slice(body);
        input.extend_from_slice(b"endobj");
        let mut t = Tokenizer::new(&input);
        let obj = parse_indirect_object(&mut t)?.1;
        assert_eq!(t.pos(), input.len(), "endstream and endobj consumed");
        match obj {
            PdfObject::Stream { data, .. } => Ok(data),
            other => panic!("expected a stream, got {other:?}"),
        }
    }

    #[test]
    fn a_stream_with_a_wrong_length_reads_up_to_endstream() {
        for length in ["-1", "-1000", "100000", "3", "7"] {
            assert_eq!(
                stream_data(length, b"BT ET\nendstream\n").unwrap(),
                b"BT ET",
                "/Length {length}"
            );
        }
    }

    #[test]
    fn a_stream_with_a_correct_length_is_read_by_length() {
        let body = b"ab endstream cd";
        let mut text = body.to_vec();
        text.extend_from_slice(b"\nendstream\n");
        assert_eq!(stream_data(&body.len().to_string(), &text).unwrap(), body);
        assert_eq!(stream_data("5", b"BT ET\r\nendstream\n").unwrap(), b"BT ET");
        assert_eq!(stream_data("5", b"BT ETendstream\n").unwrap(), b"BT ET");
    }

    #[test]
    fn a_misspelled_or_truncated_endstream_ends_the_data() {
        for terminator in ["endsteam", "endstrea"] {
            let mut body = b"BT ET\n".to_vec();
            body.extend_from_slice(terminator.as_bytes());
            body.extend_from_slice(b"\nendobj\n2 0 obj\n<< /Length 2 >>\nstream\nzz\nendstream\n");
            for length in ["5", "-1"] {
                assert_eq!(
                    stream_data_prefix(length, &body).unwrap(),
                    b"BT ET",
                    "{terminator}, /Length {length}"
                );
            }
        }
        // Without the whitespace after it, a near miss is not a terminator.
        let body = b"BT ET\nendsteamX\nendstream\n";
        assert_eq!(stream_data("-1", body).unwrap(), b"BT ET\nendsteamX");
    }

    /// The data of the first object in `1 0 obj << /Length {length} >>
    /// stream\n{body}`, where `body` may hold further objects.
    fn stream_data_prefix(length: &str, body: &[u8]) -> Result<Vec<u8>> {
        let mut input = format!("1 0 obj\n<< /Length {length} >>\nstream\n").into_bytes();
        input.extend_from_slice(body);
        let mut t = Tokenizer::new(&input);
        match parse_indirect_object(&mut t)?.1 {
            PdfObject::Stream { data, .. } => Ok(data),
            other => panic!("expected a stream, got {other:?}"),
        }
    }

    #[test]
    fn a_stream_without_endstream_is_an_invalid_object() {
        let err = stream_data("-1", b"BT ET\n").unwrap_err();
        assert!(matches!(err, JustPdfError::InvalidObject { .. }), "{err:?}");
    }

    #[test]
    fn a_stream_with_no_endstream_anywhere_keeps_an_in_range_length() {
        assert_eq!(stream_data("5", b"BT ET\n").unwrap(), b"BT ET");
    }

    #[test]
    fn the_endstream_scan_finds_one_ending_the_input() {
        let input = b"1 0 obj\n<< >>\nstream\nBT ET\nendstream";
        let mut t = Tokenizer::new(input);
        let (_, obj) = parse_indirect_object(&mut t).unwrap();
        assert!(matches!(obj, PdfObject::Stream { ref data, .. } if data == b"BT ET"));
    }

    #[test]
    fn test_parse_nested() {
        let input = b"<< /Kids [ 1 0 R 2 0 R ] /Count 2 >>";
        let mut t = Tokenizer::new(input);
        let obj = parse_object(&mut t).unwrap();
        match &obj {
            PdfObject::Dict(d) => {
                assert_eq!(d.get(b"Count"), Some(&PdfObject::Integer(2)));
                match d.get(b"Kids") {
                    Some(PdfObject::Array(arr)) => assert_eq!(arr.len(), 2),
                    _ => panic!("expected array"),
                }
            }
            _ => panic!("expected dict"),
        }
    }
}
