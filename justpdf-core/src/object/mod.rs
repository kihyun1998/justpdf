mod types;

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

            // Consume endstream
            // (read_stream_data positions us after the data, now skip to endobj)
            // The 'endstream' keyword may have been consumed by position-based reading
            // Try to consume endstream if present
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

/// Read raw stream data based on /Length in the dict.
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

    // Get length from dict
    let length = match dict.get(b"Length") {
        Some(PdfObject::Integer(n)) => *n as usize,
        _ => {
            // Try to find endstream by scanning
            return find_stream_data_by_endstream(data, pos, start_offset);
        }
    };

    if pos + length > data.len() {
        return Err(JustPdfError::UnexpectedEof { offset: pos });
    }

    let stream_data = data[pos..pos + length].to_vec();
    tokenizer.seek(pos + length);

    Ok(stream_data)
}

/// Fallback: scan for 'endstream' to determine stream length.
fn find_stream_data_by_endstream(data: &[u8], start: usize, err_offset: usize) -> Result<Vec<u8>> {
    let needle = b"endstream";
    for i in start..data.len().saturating_sub(needle.len()) {
        if &data[i..i + needle.len()] == needle {
            // Remove trailing \r\n or \n before endstream
            let mut end = i;
            if end > start && data[end - 1] == b'\n' {
                end -= 1;
            }
            if end > start && data[end - 1] == b'\r' {
                end -= 1;
            }
            return Ok(data[start..end].to_vec());
        }
    }
    Err(JustPdfError::InvalidObject {
        offset: err_offset,
        detail: "could not find endstream".into(),
    })
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
