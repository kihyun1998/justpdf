pub mod reader;
pub mod token;

use crate::error::{JustPdfError, Result};
use reader::{PdfReader, is_pdf_delimiter, is_pdf_regular, is_pdf_whitespace};
use token::{Keyword, Token};

/// PDF tokenizer: consumes bytes from a `PdfReader` and yields `Token` values.
pub struct Tokenizer<'a> {
    reader: PdfReader<'a>,
}

impl<'a> Tokenizer<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            reader: PdfReader::new(data),
        }
    }

    pub fn new_at(data: &'a [u8], pos: usize) -> Self {
        Self {
            reader: PdfReader::new_at(data, pos),
        }
    }

    /// Current byte offset.
    pub fn pos(&self) -> usize {
        self.reader.pos()
    }

    /// Set position.
    pub fn seek(&mut self, pos: usize) {
        self.reader.seek(pos);
    }

    pub fn is_eof(&self) -> bool {
        self.reader.is_eof()
    }

    /// Access the underlying reader.
    pub fn reader(&self) -> &PdfReader<'a> {
        &self.reader
    }

    /// Read the next token, skipping whitespace and comments.
    /// Returns `None` at EOF.
    pub fn next_token(&mut self) -> Result<Option<Token>> {
        self.reader.skip_whitespace_and_comments();
        if self.reader.is_eof() {
            return Ok(None);
        }

        let offset = self.reader.pos();
        let b = self.reader.peek().unwrap();

        match b {
            // Literal string
            b'(' => self.read_literal_string(),
            // Hex string or dict delimiter
            b'<' => {
                if self.reader.peek_at(1) == Some(b'<') {
                    self.reader.advance(2);
                    Ok(Some(Token::DictBegin))
                } else {
                    self.read_hex_string()
                }
            }
            // Dict end or unexpected >
            b'>' => {
                if self.reader.peek_at(1) == Some(b'>') {
                    self.reader.advance(2);
                    Ok(Some(Token::DictEnd))
                } else {
                    self.reader.advance(1);
                    Err(JustPdfError::InvalidToken {
                        offset,
                        detail: "unexpected '>'".into(),
                    })
                }
            }
            b'[' => {
                self.reader.advance(1);
                Ok(Some(Token::ArrayBegin))
            }
            b']' => {
                self.reader.advance(1);
                Ok(Some(Token::ArrayEnd))
            }
            // Name
            b'/' => self.read_name(),
            // Number or keyword starting with +/-
            b'+' | b'-' => self.read_number_or_keyword(),
            b'0'..=b'9' | b'.' => self.read_number_or_keyword(),
            // Regular character → keyword or unknown
            _ if is_pdf_regular(b) => self.read_keyword(),
            _ => {
                self.reader.advance(1);
                Err(JustPdfError::InvalidToken {
                    offset,
                    detail: format!("unexpected byte 0x{b:02X}"),
                })
            }
        }
    }

    /// Read a literal string `(...)` with escape handling and balanced parentheses.
    fn read_literal_string(&mut self) -> Result<Option<Token>> {
        let start = self.reader.pos();
        self.reader.advance(1); // skip '('
        let mut result = Vec::new();
        let mut depth: u32 = 1;

        loop {
            let Some(b) = self.reader.next_byte() else {
                return Err(JustPdfError::UnexpectedEof { offset: start });
            };
            match b {
                b'(' => {
                    depth += 1;
                    result.push(b'(');
                }
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                    result.push(b')');
                }
                b'\\' => {
                    let Some(esc) = self.reader.next_byte() else {
                        return Err(JustPdfError::UnexpectedEof { offset: start });
                    };
                    match esc {
                        b'n' => result.push(b'\n'),
                        b'r' => result.push(b'\r'),
                        b't' => result.push(b'\t'),
                        b'b' => result.push(0x08),
                        b'f' => result.push(0x0C),
                        b'(' => result.push(b'('),
                        b')' => result.push(b')'),
                        b'\\' => result.push(b'\\'),
                        b'\r' => {
                            // Line continuation: \<CR> or \<CR><LF>
                            if self.reader.peek() == Some(b'\n') {
                                self.reader.advance(1);
                            }
                        }
                        b'\n' => {
                            // Line continuation: \<LF>
                        }
                        b'0'..=b'7' => {
                            // Octal escape: 1-3 digits
                            let mut val = esc - b'0';
                            if let Some(d) = self.reader.peek()
                                && (b'0'..=b'7').contains(&d)
                            {
                                self.reader.advance(1);
                                val = val * 8 + (d - b'0');
                                if let Some(d2) = self.reader.peek()
                                    && (b'0'..=b'7').contains(&d2)
                                {
                                    self.reader.advance(1);
                                    val = val * 8 + (d2 - b'0');
                                }
                            }
                            result.push(val);
                        }
                        // Unknown escape: ignore the backslash
                        _ => result.push(esc),
                    }
                }
                // Normalize line endings to \n
                b'\r' => {
                    result.push(b'\n');
                    if self.reader.peek() == Some(b'\n') {
                        self.reader.advance(1);
                    }
                }
                _ => result.push(b),
            }
        }

        Ok(Some(Token::LiteralString(result)))
    }

    /// Read a hex string `<...>`.
    fn read_hex_string(&mut self) -> Result<Option<Token>> {
        let start = self.reader.pos();
        self.reader.advance(1); // skip '<'
        let mut hex_chars = Vec::new();

        loop {
            let Some(b) = self.reader.next_byte() else {
                return Err(JustPdfError::UnexpectedEof { offset: start });
            };
            match b {
                b'>' => break,
                _ if is_pdf_whitespace(b) => continue,
                _ if b.is_ascii_hexdigit() => hex_chars.push(b),
                _ => {
                    return Err(JustPdfError::InvalidToken {
                        offset: self.reader.pos() - 1,
                        detail: format!("invalid hex digit 0x{b:02X}"),
                    });
                }
            }
        }

        // Pad with trailing 0 if odd number of hex chars
        if hex_chars.len() % 2 != 0 {
            hex_chars.push(b'0');
        }

        let mut result = Vec::with_capacity(hex_chars.len() / 2);
        for pair in hex_chars.chunks(2) {
            let hi = hex_val(pair[0]);
            let lo = hex_val(pair[1]);
            result.push((hi << 4) | lo);
        }

        Ok(Some(Token::HexString(result)))
    }

    /// Read a name `/...`.
    fn read_name(&mut self) -> Result<Option<Token>> {
        self.reader.advance(1); // skip '/'
        let mut name = Vec::new();

        while let Some(b) = self.reader.peek() {
            if is_pdf_whitespace(b) || is_pdf_delimiter(b) {
                break;
            }
            self.reader.advance(1);
            if b == b'#' {
                // #XX hex escape in name
                let h1 = self.reader.next_byte();
                let h2 = self.reader.next_byte();
                match (h1, h2) {
                    (Some(a), Some(b)) if a.is_ascii_hexdigit() && b.is_ascii_hexdigit() => {
                        name.push((hex_val(a) << 4) | hex_val(b));
                    }
                    _ => {
                        return Err(JustPdfError::InvalidToken {
                            offset: self.reader.pos() - 2,
                            detail: "invalid hex escape in name".into(),
                        });
                    }
                }
            } else {
                name.push(b);
            }
        }

        Ok(Some(Token::Name(name)))
    }

    /// Read a number (integer or real) or a keyword starting with +/-.
    fn read_number_or_keyword(&mut self) -> Result<Option<Token>> {
        let start = self.reader.pos();
        let mut buf = Vec::new();
        let mut has_dot = false;

        while let Some(b) = self.reader.peek() {
            match b {
                b'0'..=b'9' | b'+' | b'-' => {
                    buf.push(b);
                    self.reader.advance(1);
                }
                b'.' => {
                    has_dot = true;
                    buf.push(b);
                    self.reader.advance(1);
                }
                _ if is_pdf_whitespace(b) || is_pdf_delimiter(b) => break,
                _ if is_pdf_regular(b) => {
                    // Not a number, fall back to keyword reading
                    buf.push(b);
                    self.reader.advance(1);
                    while let Some(b) = self.reader.peek() {
                        if !is_pdf_regular(b) {
                            break;
                        }
                        buf.push(b);
                        self.reader.advance(1);
                    }
                    return self.classify_keyword(&buf, start);
                }
                _ => break,
            }
        }

        if has_dot {
            let s = std::str::from_utf8(&buf).unwrap_or("?");
            match s.parse::<f64>() {
                Ok(v) => Ok(Some(Token::Real(v))),
                Err(_) => Err(JustPdfError::InvalidToken {
                    offset: start,
                    detail: format!("invalid real number: {s}"),
                }),
            }
        } else {
            let s = std::str::from_utf8(&buf).unwrap_or("?");
            match s.parse::<i64>() {
                Ok(v) => Ok(Some(Token::Integer(v))),
                Err(_) => Err(JustPdfError::InvalidToken {
                    offset: start,
                    detail: format!("invalid integer: {s}"),
                }),
            }
        }
    }

    /// Read a keyword (sequence of regular characters).
    fn read_keyword(&mut self) -> Result<Option<Token>> {
        let start = self.reader.pos();
        let mut buf = Vec::new();

        while let Some(b) = self.reader.peek() {
            if !is_pdf_regular(b) {
                break;
            }
            buf.push(b);
            self.reader.advance(1);
        }

        self.classify_keyword(&buf, start)
    }

    fn classify_keyword(&self, buf: &[u8], offset: usize) -> Result<Option<Token>> {
        if let Some(kw) = Keyword::from_bytes(buf) {
            Ok(Some(Token::Keyword(kw)))
        } else {
            Err(JustPdfError::InvalidToken {
                offset,
                detail: format!(
                    "unknown keyword: {}",
                    std::str::from_utf8(buf).unwrap_or("<non-utf8>")
                ),
            })
        }
    }
}

#[inline]
fn hex_val(b: u8) -> u8 {
    match b {
        b'0'..=b'9' => b - b'0',
        b'a'..=b'f' => b - b'a' + 10,
        b'A'..=b'F' => b - b'A' + 10,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokenize(input: &[u8]) -> Vec<Token> {
        let mut t = Tokenizer::new(input);
        let mut tokens = Vec::new();
        while let Ok(Some(tok)) = t.next_token() {
            tokens.push(tok);
        }
        tokens
    }

    #[test]
    fn test_integer() {
        assert_eq!(tokenize(b"42"), vec![Token::Integer(42)]);
        assert_eq!(tokenize(b"-17"), vec![Token::Integer(-17)]);
        assert_eq!(tokenize(b"+5"), vec![Token::Integer(5)]);
        assert_eq!(tokenize(b"0"), vec![Token::Integer(0)]);
    }

    #[test]
    fn test_real() {
        assert_eq!(tokenize(b"3.15"), vec![Token::Real(3.15)]);
        assert_eq!(tokenize(b"-0.5"), vec![Token::Real(-0.5)]);
        assert_eq!(tokenize(b".25"), vec![Token::Real(0.25)]);
    }

    #[test]
    fn test_literal_string() {
        assert_eq!(
            tokenize(b"(Hello)"),
            vec![Token::LiteralString(b"Hello".to_vec())]
        );
        assert_eq!(
            tokenize(b"(Hello\\nWorld)"),
            vec![Token::LiteralString(b"Hello\nWorld".to_vec())]
        );
        // Balanced parens
        assert_eq!(
            tokenize(b"(a(b)c)"),
            vec![Token::LiteralString(b"a(b)c".to_vec())]
        );
        // Octal escape
        assert_eq!(
            tokenize(b"(\\101)"),
            vec![Token::LiteralString(b"A".to_vec())]
        );
    }

    #[test]
    fn test_hex_string() {
        assert_eq!(
            tokenize(b"<48656C6C6F>"),
            vec![Token::HexString(b"Hello".to_vec())]
        );
        // Odd number of hex digits → trailing 0
        assert_eq!(tokenize(b"<ABC>"), vec![Token::HexString(vec![0xAB, 0xC0])]);
        // Whitespace in hex string
        assert_eq!(
            tokenize(b"<48 65 6C 6C 6F>"),
            vec![Token::HexString(b"Hello".to_vec())]
        );
    }

    #[test]
    fn test_name() {
        assert_eq!(tokenize(b"/Type"), vec![Token::Name(b"Type".to_vec())]);
        assert_eq!(tokenize(b"/A#42C"), vec![Token::Name(b"ABC".to_vec())]);
        // Empty name
        assert_eq!(tokenize(b"/ "), vec![Token::Name(b"".to_vec())]);
    }

    #[test]
    fn test_keywords() {
        assert_eq!(
            tokenize(b"true false null"),
            vec![
                Token::Keyword(Keyword::True),
                Token::Keyword(Keyword::False),
                Token::Keyword(Keyword::Null),
            ]
        );
    }

    #[test]
    fn test_array_dict_delimiters() {
        assert_eq!(tokenize(b"[ ]"), vec![Token::ArrayBegin, Token::ArrayEnd]);
        assert_eq!(tokenize(b"<< >>"), vec![Token::DictBegin, Token::DictEnd]);
    }

    #[test]
    fn test_comment_skipping() {
        assert_eq!(
            tokenize(b"42 % this is a comment\n17"),
            vec![Token::Integer(42), Token::Integer(17)]
        );
    }

    #[test]
    fn test_mixed_tokens() {
        let input = b"/Type /Catalog /Pages 2 0 R";
        let tokens = tokenize(input);
        assert_eq!(
            tokens,
            vec![
                Token::Name(b"Type".to_vec()),
                Token::Name(b"Catalog".to_vec()),
                Token::Name(b"Pages".to_vec()),
                Token::Integer(2),
                Token::Integer(0),
                Token::Keyword(Keyword::R),
            ]
        );
    }

    #[test]
    fn test_empty_input() {
        assert_eq!(tokenize(b""), Vec::<Token>::new());
    }

    #[test]
    fn test_whitespace_only() {
        assert_eq!(tokenize(b"   \t\n\r  "), Vec::<Token>::new());
    }
}
