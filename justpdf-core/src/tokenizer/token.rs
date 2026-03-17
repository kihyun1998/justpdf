/// A keyword recognized by the PDF tokenizer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keyword {
    True,
    False,
    Null,
    Obj,
    EndObj,
    Stream,
    EndStream,
    Xref,
    Trailer,
    StartXref,
    R,
}

impl Keyword {
    pub fn from_bytes(b: &[u8]) -> Option<Self> {
        match b {
            b"true" => Some(Self::True),
            b"false" => Some(Self::False),
            b"null" => Some(Self::Null),
            b"obj" => Some(Self::Obj),
            b"endobj" => Some(Self::EndObj),
            b"stream" => Some(Self::Stream),
            b"endstream" => Some(Self::EndStream),
            b"xref" => Some(Self::Xref),
            b"trailer" => Some(Self::Trailer),
            b"startxref" => Some(Self::StartXref),
            b"R" => Some(Self::R),
            _ => None,
        }
    }
}

/// A single token produced by the PDF tokenizer.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    /// Integer number.
    Integer(i64),
    /// Real (floating-point) number.
    Real(f64),
    /// Literal string `(...)`, stored as raw decoded bytes.
    LiteralString(Vec<u8>),
    /// Hex string `<...>`, stored as decoded bytes.
    HexString(Vec<u8>),
    /// Name object (without leading `/`).
    Name(Vec<u8>),
    /// A recognized PDF keyword.
    Keyword(Keyword),
    /// `[`
    ArrayBegin,
    /// `]`
    ArrayEnd,
    /// `<<`
    DictBegin,
    /// `>>`
    DictEnd,
}

impl Token {
    pub fn is_keyword(&self, kw: Keyword) -> bool {
        matches!(self, Token::Keyword(k) if *k == kw)
    }
}
