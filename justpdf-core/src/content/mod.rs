mod operator;

pub(crate) use operator::write_content;
pub use operator::{ContentOp, Operand};

use crate::error::Result;
use crate::tokenizer::reader::{is_pdf_delimiter, is_pdf_regular, is_pdf_whitespace};

/// Parse a content stream into a sequence of operations.
/// Each operation is a list of operands followed by an operator.
pub fn parse_content_stream(data: &[u8]) -> Result<Vec<ContentOp>> {
    let mut ops = Vec::new();
    let mut parser = ContentParser::new(data);

    while let Some(op) = parser.next_op()? {
        ops.push(op);
    }

    Ok(ops)
}

/// Parse a content stream using an arena allocator for reduced allocation overhead.
///
/// This function uses `bumpalo::Bump` for temporary allocations during parsing,
/// which can significantly reduce allocation pressure for complex content streams
/// with many operations. The final `ContentOp` values are still heap-allocated
/// (owned), but intermediate parsing buffers use the arena.
///
/// Requires the `arena` feature.
#[cfg(feature = "arena")]
pub fn parse_content_stream_arena(data: &[u8]) -> Result<Vec<ContentOp>> {
    let arena = bumpalo::Bump::new();
    parse_content_stream_with_arena(data, &arena)
}

#[cfg(feature = "arena")]
fn parse_content_stream_with_arena(
    data: &[u8],
    arena: &bumpalo::Bump,
) -> Result<Vec<ContentOp>> {
    let mut ops = Vec::new();
    let mut parser = ArenaContentParser::new(data, arena);

    while let Some(op) = parser.next_op()? {
        ops.push(op);
    }

    Ok(ops)
}

/// Arena-backed content stream parser that uses bumpalo for intermediate buffers.
#[cfg(feature = "arena")]
struct ArenaContentParser<'a> {
    data: &'a [u8],
    pos: usize,
    arena: &'a bumpalo::Bump,
}

#[cfg(feature = "arena")]
impl<'a> ArenaContentParser<'a> {
    fn new(data: &'a [u8], arena: &'a bumpalo::Bump) -> Self {
        Self { data, pos: 0, arena }
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            self.skip_whitespace();
            if self.pos < self.data.len() && self.data[self.pos] == b'%' {
                while self.pos < self.data.len()
                    && self.data[self.pos] != b'\n'
                    && self.data[self.pos] != b'\r'
                {
                    self.pos += 1;
                }
            } else {
                break;
            }
        }
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.data.len() && is_pdf_whitespace(self.data[self.pos]) {
            self.pos += 1;
        }
    }

    /// Parse the next operation, using the arena for the operand accumulator.
    fn next_op(&mut self) -> Result<Option<ContentOp>> {
        let mut operands = bumpalo::collections::Vec::new_in(self.arena);

        loop {
            self.skip_whitespace_and_comments();
            if self.pos >= self.data.len() {
                if operands.is_empty() {
                    return Ok(None);
                }
                return Ok(None);
            }

            let b = self.data[self.pos];

            match b {
                b'0'..=b'9' | b'+' | b'-' | b'.' => {
                    operands.push(self.read_number()?);
                }
                b'(' => {
                    operands.push(Operand::String(self.read_literal_string()?));
                }
                b'<' => {
                    if self.pos + 1 < self.data.len() && self.data[self.pos + 1] == b'<' {
                        operands.push(Operand::Dict(self.read_inline_dict()?));
                    } else {
                        operands.push(Operand::String(self.read_hex_string()?));
                    }
                }
                b'/' => {
                    operands.push(Operand::Name(self.read_name()));
                }
                b'[' => {
                    operands.push(self.read_array()?);
                }
                _ if is_pdf_regular(b) => {
                    let word = self.read_word();
                    match word.as_slice() {
                        b"true" => operands.push(Operand::Bool(true)),
                        b"false" => operands.push(Operand::Bool(false)),
                        b"null" => operands.push(Operand::Null),
                        b"BI" => {
                            let (dict_operands, image_data) = self.read_inline_image()?;
                            return Ok(Some(ContentOp {
                                operator: b"BI".to_vec(),
                                operands: vec![Operand::InlineImage {
                                    dict: dict_operands,
                                    data: image_data,
                                }],
                            }));
                        }
                        _ => {
                            // Collect operands from arena vec into owned vec
                            let owned_operands: Vec<Operand> = operands.drain(..).collect();
                            return Ok(Some(ContentOp {
                                operator: word,
                                operands: owned_operands,
                            }));
                        }
                    }
                }
                _ => {
                    self.pos += 1;
                }
            }
        }
    }

    fn read_number(&mut self) -> Result<Operand> {
        let start = self.pos;
        let mut has_dot = false;

        if self.pos < self.data.len()
            && (self.data[self.pos] == b'+' || self.data[self.pos] == b'-')
        {
            self.pos += 1;
        }

        while self.pos < self.data.len() {
            match self.data[self.pos] {
                b'0'..=b'9' => self.pos += 1,
                b'.' if !has_dot => {
                    has_dot = true;
                    self.pos += 1;
                }
                _ => break,
            }
        }

        let s = std::str::from_utf8(&self.data[start..self.pos]).unwrap_or("0");
        if has_dot {
            Ok(Operand::Real(s.parse().unwrap_or(0.0)))
        } else {
            Ok(Operand::Integer(s.parse().unwrap_or(0)))
        }
    }

    fn read_literal_string(&mut self) -> Result<Vec<u8>> {
        self.pos += 1;
        let mut result = bumpalo::collections::Vec::new_in(self.arena);
        let mut depth: u32 = 1;

        while self.pos < self.data.len() {
            let b = self.data[self.pos];
            self.pos += 1;
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
                b'\\' if self.pos < self.data.len() => {
                    let esc = self.data[self.pos];
                    self.pos += 1;
                    match esc {
                        b'n' => result.push(b'\n'),
                        b'r' => result.push(b'\r'),
                        b't' => result.push(b'\t'),
                        b'b' => result.push(0x08),
                        b'f' => result.push(0x0C),
                        b'(' => result.push(b'('),
                        b')' => result.push(b')'),
                        b'\\' => result.push(b'\\'),
                        b'0'..=b'7' => {
                            let mut val = esc - b'0';
                            if self.pos < self.data.len()
                                && (b'0'..=b'7').contains(&self.data[self.pos])
                            {
                                val = val * 8 + (self.data[self.pos] - b'0');
                                self.pos += 1;
                                if self.pos < self.data.len()
                                    && (b'0'..=b'7').contains(&self.data[self.pos])
                                {
                                    val = val * 8 + (self.data[self.pos] - b'0');
                                    self.pos += 1;
                                }
                            }
                            result.push(val);
                        }
                        b'\r' => {
                            if self.pos < self.data.len() && self.data[self.pos] == b'\n' {
                                self.pos += 1;
                            }
                        }
                        b'\n' => {}
                        _ => result.push(esc),
                    }
                }
                _ => result.push(b),
            }
        }

        Ok(result.into_iter().collect())
    }

    fn read_hex_string(&mut self) -> Result<Vec<u8>> {
        self.pos += 1;
        let mut hex = bumpalo::collections::Vec::new_in(self.arena);

        while self.pos < self.data.len() {
            let b = self.data[self.pos];
            self.pos += 1;
            match b {
                b'>' => break,
                _ if b.is_ascii_hexdigit() => hex.push(b),
                _ if is_pdf_whitespace(b) => {}
                _ => {}
            }
        }

        if hex.len() % 2 != 0 {
            hex.push(b'0');
        }

        let mut result = Vec::with_capacity(hex.len() / 2);
        for pair in hex.chunks(2) {
            result.push((hex_val(pair[0]) << 4) | hex_val(pair[1]));
        }

        Ok(result)
    }

    fn read_name(&mut self) -> Vec<u8> {
        self.pos += 1;
        let mut name = bumpalo::collections::Vec::new_in(self.arena);

        while self.pos < self.data.len() {
            let b = self.data[self.pos];
            if is_pdf_whitespace(b) || is_pdf_delimiter(b) {
                break;
            }
            self.pos += 1;
            if b == b'#' && self.pos + 1 < self.data.len() {
                let h1 = self.data[self.pos];
                let h2 = self.data[self.pos + 1];
                if h1.is_ascii_hexdigit() && h2.is_ascii_hexdigit() {
                    name.push((hex_val(h1) << 4) | hex_val(h2));
                    self.pos += 2;
                    continue;
                }
            }
            name.push(b);
        }

        name.into_iter().collect()
    }

    fn read_word(&mut self) -> Vec<u8> {
        let start = self.pos;
        while self.pos < self.data.len() && is_pdf_regular(self.data[self.pos]) {
            self.pos += 1;
        }
        self.data[start..self.pos].to_vec()
    }

    fn read_array(&mut self) -> Result<Operand> {
        self.pos += 1;
        let mut items = bumpalo::collections::Vec::new_in(self.arena);

        loop {
            self.skip_whitespace_and_comments();
            if self.pos >= self.data.len() {
                break;
            }
            if self.data[self.pos] == b']' {
                self.pos += 1;
                break;
            }

            let b = self.data[self.pos];
            match b {
                b'0'..=b'9' | b'+' | b'-' | b'.' => items.push(self.read_number()?),
                b'(' => items.push(Operand::String(self.read_literal_string()?)),
                b'<' => {
                    if self.pos + 1 < self.data.len() && self.data[self.pos + 1] == b'<' {
                        items.push(Operand::Dict(self.read_inline_dict()?));
                    } else {
                        items.push(Operand::String(self.read_hex_string()?));
                    }
                }
                b'/' => items.push(Operand::Name(self.read_name())),
                b'[' => items.push(self.read_array()?),
                _ if is_pdf_regular(b) => {
                    let word = self.read_word();
                    match word.as_slice() {
                        b"true" => items.push(Operand::Bool(true)),
                        b"false" => items.push(Operand::Bool(false)),
                        b"null" => items.push(Operand::Null),
                        _ => items.push(Operand::Name(word)),
                    }
                }
                _ => {
                    self.pos += 1;
                }
            }
        }

        Ok(Operand::Array(items.into_iter().collect()))
    }

    fn read_inline_dict(&mut self) -> Result<Vec<(Vec<u8>, Operand)>> {
        self.pos += 2;
        let mut entries = bumpalo::collections::Vec::new_in(self.arena);

        loop {
            self.skip_whitespace_and_comments();
            if self.pos + 1 < self.data.len()
                && self.data[self.pos] == b'>'
                && self.data[self.pos + 1] == b'>'
            {
                self.pos += 2;
                break;
            }
            if self.pos >= self.data.len() {
                break;
            }

            if self.data[self.pos] != b'/' {
                break;
            }
            let key = self.read_name();

            self.skip_whitespace_and_comments();
            if self.pos >= self.data.len() {
                break;
            }

            let b = self.data[self.pos];
            let value = match b {
                b'0'..=b'9' | b'+' | b'-' | b'.' => self.read_number()?,
                b'(' => Operand::String(self.read_literal_string()?),
                b'<' => {
                    if self.pos + 1 < self.data.len() && self.data[self.pos + 1] == b'<' {
                        Operand::Dict(self.read_inline_dict()?)
                    } else {
                        Operand::String(self.read_hex_string()?)
                    }
                }
                b'/' => Operand::Name(self.read_name()),
                b'[' => self.read_array()?,
                _ if is_pdf_regular(b) => {
                    let word = self.read_word();
                    match word.as_slice() {
                        b"true" => Operand::Bool(true),
                        b"false" => Operand::Bool(false),
                        b"null" => Operand::Null,
                        _ => Operand::Name(word),
                    }
                }
                _ => {
                    self.pos += 1;
                    continue;
                }
            };

            entries.push((key, value));
        }

        Ok(entries.into_iter().collect())
    }

    #[allow(clippy::type_complexity)]
    fn read_inline_image(&mut self) -> Result<(Vec<(Vec<u8>, Operand)>, Vec<u8>)> {
        let mut dict = bumpalo::collections::Vec::new_in(self.arena);

        loop {
            self.skip_whitespace_and_comments();
            if self.pos >= self.data.len() {
                break;
            }

            if self.pos + 2 <= self.data.len() {
                let maybe_id = &self.data[self.pos..self.pos + 2];
                if maybe_id == b"ID"
                    && (self.pos + 2 >= self.data.len()
                        || is_pdf_whitespace(self.data[self.pos + 2]))
                {
                    self.pos += 2;
                    if self.pos < self.data.len() && is_pdf_whitespace(self.data[self.pos]) {
                        self.pos += 1;
                    }
                    break;
                }
            }

            if self.data[self.pos] != b'/' {
                self.pos += 1;
                continue;
            }

            let key = self.read_name();
            self.skip_whitespace_and_comments();
            if self.pos >= self.data.len() {
                break;
            }

            let b = self.data[self.pos];
            let value = match b {
                b'0'..=b'9' | b'+' | b'-' | b'.' => self.read_number()?,
                b'/' => Operand::Name(self.read_name()),
                b'(' => Operand::String(self.read_literal_string()?),
                b'<' => {
                    if self.pos + 1 < self.data.len() && self.data[self.pos + 1] == b'<' {
                        Operand::Dict(self.read_inline_dict()?)
                    } else {
                        Operand::String(self.read_hex_string()?)
                    }
                }
                b'[' => self.read_array()?,
                _ if is_pdf_regular(b) => {
                    let word = self.read_word();
                    match word.as_slice() {
                        b"true" => Operand::Bool(true),
                        b"false" => Operand::Bool(false),
                        b"null" => Operand::Null,
                        _ => Operand::Name(word),
                    }
                }
                _ => {
                    self.pos += 1;
                    continue;
                }
            };

            dict.push((key, value));
        }

        let data_start = self.pos;
        let mut data_end = self.pos;

        while self.pos < self.data.len() {
            if self.data[self.pos] == b'E'
                && self.pos + 1 < self.data.len()
                && self.data[self.pos + 1] == b'I'
                && (self.pos + 2 >= self.data.len()
                    || is_pdf_whitespace(self.data[self.pos + 2])
                    || is_pdf_delimiter(self.data[self.pos + 2]))
                && self.pos > data_start
                && is_pdf_whitespace(self.data[self.pos - 1])
            {
                data_end = self.pos - 1;
                self.pos += 2;
                break;
            }
            self.pos += 1;
        }

        let image_data = self.data[data_start..data_end].to_vec();
        Ok((dict.into_iter().collect(), image_data))
    }
}

/// Low-level content stream parser.
struct ContentParser<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> ContentParser<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            self.skip_whitespace();
            if self.pos < self.data.len() && self.data[self.pos] == b'%' {
                while self.pos < self.data.len()
                    && self.data[self.pos] != b'\n'
                    && self.data[self.pos] != b'\r'
                {
                    self.pos += 1;
                }
            } else {
                break;
            }
        }
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.data.len() && is_pdf_whitespace(self.data[self.pos]) {
            self.pos += 1;
        }
    }

    /// Parse the next operation (operands + operator).
    fn next_op(&mut self) -> Result<Option<ContentOp>> {
        let mut operands = Vec::new();

        loop {
            self.skip_whitespace_and_comments();
            if self.pos >= self.data.len() {
                if operands.is_empty() {
                    return Ok(None);
                }
                // Leftover operands without operator — malformed, but don't crash
                return Ok(None);
            }

            let b = self.data[self.pos];

            match b {
                // Number
                b'0'..=b'9' | b'+' | b'-' | b'.' => {
                    operands.push(self.read_number()?);
                }
                // Literal string
                b'(' => {
                    operands.push(Operand::String(self.read_literal_string()?));
                }
                // Hex string or dict
                b'<' => {
                    if self.pos + 1 < self.data.len() && self.data[self.pos + 1] == b'<' {
                        operands.push(Operand::Dict(self.read_inline_dict()?));
                    } else {
                        operands.push(Operand::String(self.read_hex_string()?));
                    }
                }
                // Name
                b'/' => {
                    operands.push(Operand::Name(self.read_name()));
                }
                // Array
                b'[' => {
                    operands.push(self.read_array()?);
                }
                // true/false/null or operator keyword
                _ if is_pdf_regular(b) => {
                    let word = self.read_word();
                    match word.as_slice() {
                        b"true" => operands.push(Operand::Bool(true)),
                        b"false" => operands.push(Operand::Bool(false)),
                        b"null" => operands.push(Operand::Null),
                        b"BI" => {
                            // Inline image: BI <dict> ID <data> EI
                            let (dict_operands, image_data) = self.read_inline_image()?;
                            return Ok(Some(ContentOp {
                                operator: b"BI".to_vec(),
                                operands: vec![Operand::InlineImage {
                                    dict: dict_operands,
                                    data: image_data,
                                }],
                            }));
                        }
                        _ => {
                            // This is the operator
                            return Ok(Some(ContentOp {
                                operator: word,
                                operands,
                            }));
                        }
                    }
                }
                _ => {
                    self.pos += 1; // skip unknown byte
                }
            }
        }
    }

    fn read_number(&mut self) -> Result<Operand> {
        let start = self.pos;
        let mut has_dot = false;

        if self.pos < self.data.len()
            && (self.data[self.pos] == b'+' || self.data[self.pos] == b'-')
        {
            self.pos += 1;
        }

        while self.pos < self.data.len() {
            match self.data[self.pos] {
                b'0'..=b'9' => self.pos += 1,
                b'.' if !has_dot => {
                    has_dot = true;
                    self.pos += 1;
                }
                _ => break,
            }
        }

        let s = std::str::from_utf8(&self.data[start..self.pos]).unwrap_or("0");
        if has_dot {
            Ok(Operand::Real(s.parse().unwrap_or(0.0)))
        } else {
            Ok(Operand::Integer(s.parse().unwrap_or(0)))
        }
    }

    fn read_literal_string(&mut self) -> Result<Vec<u8>> {
        self.pos += 1; // skip '('
        let mut result = Vec::new();
        let mut depth: u32 = 1;

        while self.pos < self.data.len() {
            let b = self.data[self.pos];
            self.pos += 1;
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
                b'\\' if self.pos < self.data.len() => {
                    let esc = self.data[self.pos];
                    self.pos += 1;
                    match esc {
                        b'n' => result.push(b'\n'),
                        b'r' => result.push(b'\r'),
                        b't' => result.push(b'\t'),
                        b'b' => result.push(0x08),
                        b'f' => result.push(0x0C),
                        b'(' => result.push(b'('),
                        b')' => result.push(b')'),
                        b'\\' => result.push(b'\\'),
                        b'0'..=b'7' => {
                            let mut val = esc - b'0';
                            if self.pos < self.data.len()
                                && (b'0'..=b'7').contains(&self.data[self.pos])
                            {
                                val = val * 8 + (self.data[self.pos] - b'0');
                                self.pos += 1;
                                if self.pos < self.data.len()
                                    && (b'0'..=b'7').contains(&self.data[self.pos])
                                {
                                    val = val * 8 + (self.data[self.pos] - b'0');
                                    self.pos += 1;
                                }
                            }
                            result.push(val);
                        }
                        b'\r' => {
                            if self.pos < self.data.len() && self.data[self.pos] == b'\n' {
                                self.pos += 1;
                            }
                        }
                        b'\n' => {}
                        _ => result.push(esc),
                    }
                }
                _ => result.push(b),
            }
        }

        Ok(result)
    }

    fn read_hex_string(&mut self) -> Result<Vec<u8>> {
        self.pos += 1; // skip '<'
        let mut hex = Vec::new();

        while self.pos < self.data.len() {
            let b = self.data[self.pos];
            self.pos += 1;
            match b {
                b'>' => break,
                _ if b.is_ascii_hexdigit() => hex.push(b),
                _ if is_pdf_whitespace(b) => {}
                _ => {}
            }
        }

        if hex.len() % 2 != 0 {
            hex.push(b'0');
        }

        let mut result = Vec::with_capacity(hex.len() / 2);
        for pair in hex.chunks(2) {
            result.push((hex_val(pair[0]) << 4) | hex_val(pair[1]));
        }

        Ok(result)
    }

    fn read_name(&mut self) -> Vec<u8> {
        self.pos += 1; // skip '/'
        let mut name = Vec::new();

        while self.pos < self.data.len() {
            let b = self.data[self.pos];
            if is_pdf_whitespace(b) || is_pdf_delimiter(b) {
                break;
            }
            self.pos += 1;
            if b == b'#' && self.pos + 1 < self.data.len() {
                let h1 = self.data[self.pos];
                let h2 = self.data[self.pos + 1];
                if h1.is_ascii_hexdigit() && h2.is_ascii_hexdigit() {
                    name.push((hex_val(h1) << 4) | hex_val(h2));
                    self.pos += 2;
                    continue;
                }
            }
            name.push(b);
        }

        name
    }

    fn read_word(&mut self) -> Vec<u8> {
        let start = self.pos;
        while self.pos < self.data.len() && is_pdf_regular(self.data[self.pos]) {
            self.pos += 1;
        }
        self.data[start..self.pos].to_vec()
    }

    fn read_array(&mut self) -> Result<Operand> {
        self.pos += 1; // skip '['
        let mut items = Vec::new();

        loop {
            self.skip_whitespace_and_comments();
            if self.pos >= self.data.len() {
                break;
            }
            if self.data[self.pos] == b']' {
                self.pos += 1;
                break;
            }

            let b = self.data[self.pos];
            match b {
                b'0'..=b'9' | b'+' | b'-' | b'.' => items.push(self.read_number()?),
                b'(' => items.push(Operand::String(self.read_literal_string()?)),
                b'<' => {
                    if self.pos + 1 < self.data.len() && self.data[self.pos + 1] == b'<' {
                        items.push(Operand::Dict(self.read_inline_dict()?));
                    } else {
                        items.push(Operand::String(self.read_hex_string()?));
                    }
                }
                b'/' => items.push(Operand::Name(self.read_name())),
                b'[' => items.push(self.read_array()?),
                _ if is_pdf_regular(b) => {
                    let word = self.read_word();
                    match word.as_slice() {
                        b"true" => items.push(Operand::Bool(true)),
                        b"false" => items.push(Operand::Bool(false)),
                        b"null" => items.push(Operand::Null),
                        _ => items.push(Operand::Name(word)), // treat as name-like
                    }
                }
                _ => {
                    self.pos += 1;
                }
            }
        }

        Ok(Operand::Array(items))
    }

    fn read_inline_dict(&mut self) -> Result<Vec<(Vec<u8>, Operand)>> {
        self.pos += 2; // skip '<<'
        let mut entries = Vec::new();

        loop {
            self.skip_whitespace_and_comments();
            if self.pos + 1 < self.data.len()
                && self.data[self.pos] == b'>'
                && self.data[self.pos + 1] == b'>'
            {
                self.pos += 2;
                break;
            }
            if self.pos >= self.data.len() {
                break;
            }

            // Key must be a name
            if self.data[self.pos] != b'/' {
                break;
            }
            let key = self.read_name();

            self.skip_whitespace_and_comments();
            if self.pos >= self.data.len() {
                break;
            }

            // Value
            let b = self.data[self.pos];
            let value = match b {
                b'0'..=b'9' | b'+' | b'-' | b'.' => self.read_number()?,
                b'(' => Operand::String(self.read_literal_string()?),
                b'<' => {
                    if self.pos + 1 < self.data.len() && self.data[self.pos + 1] == b'<' {
                        Operand::Dict(self.read_inline_dict()?)
                    } else {
                        Operand::String(self.read_hex_string()?)
                    }
                }
                b'/' => Operand::Name(self.read_name()),
                b'[' => self.read_array()?,
                _ if is_pdf_regular(b) => {
                    let word = self.read_word();
                    match word.as_slice() {
                        b"true" => Operand::Bool(true),
                        b"false" => Operand::Bool(false),
                        b"null" => Operand::Null,
                        _ => Operand::Name(word),
                    }
                }
                _ => {
                    self.pos += 1;
                    continue;
                }
            };

            entries.push((key, value));
        }

        Ok(entries)
    }

    /// Read an inline image: after BI, read key/value pairs until ID,
    /// then read raw image data until EI.
    #[allow(clippy::type_complexity)]
    fn read_inline_image(&mut self) -> Result<(Vec<(Vec<u8>, Operand)>, Vec<u8>)> {
        // Read dict entries until "ID"
        let mut dict = Vec::new();

        loop {
            self.skip_whitespace_and_comments();
            if self.pos >= self.data.len() {
                break;
            }

            // Check for "ID" keyword
            if self.pos + 2 <= self.data.len() {
                let maybe_id = &self.data[self.pos..self.pos + 2];
                if maybe_id == b"ID"
                    && (self.pos + 2 >= self.data.len()
                        || is_pdf_whitespace(self.data[self.pos + 2]))
                {
                    self.pos += 2;
                    // Skip single whitespace byte after ID
                    if self.pos < self.data.len() && is_pdf_whitespace(self.data[self.pos]) {
                        self.pos += 1;
                    }
                    break;
                }
            }

            if self.data[self.pos] != b'/' {
                // Unexpected, skip
                self.pos += 1;
                continue;
            }

            let key = self.read_name();
            self.skip_whitespace_and_comments();
            if self.pos >= self.data.len() {
                break;
            }

            let b = self.data[self.pos];
            let value = match b {
                b'0'..=b'9' | b'+' | b'-' | b'.' => self.read_number()?,
                b'/' => Operand::Name(self.read_name()),
                b'(' => Operand::String(self.read_literal_string()?),
                b'<' => {
                    if self.pos + 1 < self.data.len() && self.data[self.pos + 1] == b'<' {
                        Operand::Dict(self.read_inline_dict()?)
                    } else {
                        Operand::String(self.read_hex_string()?)
                    }
                }
                b'[' => self.read_array()?,
                _ if is_pdf_regular(b) => {
                    let word = self.read_word();
                    match word.as_slice() {
                        b"true" => Operand::Bool(true),
                        b"false" => Operand::Bool(false),
                        b"null" => Operand::Null,
                        _ => Operand::Name(word),
                    }
                }
                _ => {
                    self.pos += 1;
                    continue;
                }
            };

            dict.push((key, value));
        }

        // Read raw image data until "\nEI" or "\r\nEI" or " EI"
        let data_start = self.pos;
        let mut data_end = self.pos;

        while self.pos < self.data.len() {
            // Look for EI preceded by whitespace
            if self.data[self.pos] == b'E'
                && self.pos + 1 < self.data.len()
                && self.data[self.pos + 1] == b'I'
                && (self.pos + 2 >= self.data.len()
                    || is_pdf_whitespace(self.data[self.pos + 2])
                    || is_pdf_delimiter(self.data[self.pos + 2]))
                && self.pos > data_start
                && is_pdf_whitespace(self.data[self.pos - 1])
            {
                data_end = self.pos - 1; // exclude the whitespace before EI
                self.pos += 2; // skip "EI"
                break;
            }
            self.pos += 1;
        }

        let image_data = self.data[data_start..data_end].to_vec();
        Ok((dict, image_data))
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

    #[test]
    fn test_simple_ops() {
        let data = b"1 0 0 1 72 720 cm";
        let ops = parse_content_stream(data).unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].operator, b"cm");
        assert_eq!(ops[0].operands.len(), 6);
    }

    #[test]
    fn test_text_ops() {
        let data = b"BT /F1 12 Tf 72 720 Td (Hello World) Tj ET";
        let ops = parse_content_stream(data).unwrap();

        let op_names: Vec<&[u8]> = ops.iter().map(|o| o.operator.as_slice()).collect();
        assert_eq!(op_names, vec![b"BT", b"Tf", b"Td", b"Tj", b"ET"]);

        // Check Tj operand is the string
        let tj = &ops[3];
        assert!(matches!(&tj.operands[0], Operand::String(s) if s == b"Hello World"));
    }

    #[test]
    fn test_array_operand() {
        let data = b"[1 2 3] TJ";
        let ops = parse_content_stream(data).unwrap();
        assert_eq!(ops.len(), 1);
        assert!(matches!(&ops[0].operands[0], Operand::Array(_)));
    }

    #[test]
    fn test_color_ops() {
        let data = b"0.5 0.2 0.8 rg 1 0 0 RG";
        let ops = parse_content_stream(data).unwrap();
        assert_eq!(ops.len(), 2);
        assert_eq!(ops[0].operator, b"rg");
        assert_eq!(ops[0].operands.len(), 3);
        assert_eq!(ops[1].operator, b"RG");
        assert_eq!(ops[1].operands.len(), 3);
    }

    #[test]
    fn test_graphics_state() {
        let data = b"q 1 0 0 1 0 0 cm Q";
        let ops = parse_content_stream(data).unwrap();
        let op_names: Vec<&[u8]> = ops.iter().map(|o| o.operator.as_slice()).collect();
        assert_eq!(op_names, vec![b"q".as_slice(), b"cm", b"Q"]);
    }

    #[test]
    fn test_path_ops() {
        let data = b"100 200 m 300 400 l 100 200 300 400 500 600 c h S";
        let ops = parse_content_stream(data).unwrap();
        let op_names: Vec<&[u8]> = ops.iter().map(|o| o.operator.as_slice()).collect();
        assert_eq!(op_names, vec![b"m", b"l", b"c", b"h", b"S"]);
    }

    #[test]
    fn test_name_operand() {
        let data = b"/CS0 cs";
        let ops = parse_content_stream(data).unwrap();
        assert_eq!(ops.len(), 1);
        assert!(matches!(&ops[0].operands[0], Operand::Name(n) if n == b"CS0"));
    }

    #[test]
    fn test_hex_string_operand() {
        let data = b"<48656C6C6F> Tj";
        let ops = parse_content_stream(data).unwrap();
        assert!(matches!(&ops[0].operands[0], Operand::String(s) if s == b"Hello"));
    }

    #[test]
    fn test_comment_in_stream() {
        let data = b"1 0 0 1 0 0 cm % this is a comment\nBT ET";
        let ops = parse_content_stream(data).unwrap();
        assert_eq!(ops.len(), 3);
    }

    #[test]
    fn test_empty_stream() {
        let ops = parse_content_stream(b"").unwrap();
        assert!(ops.is_empty());
    }

    #[test]
    fn test_tj_array() {
        let data = b"[(Hello) -10 (World)] TJ";
        let ops = parse_content_stream(data).unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].operator, b"TJ");
        if let Operand::Array(items) = &ops[0].operands[0] {
            assert_eq!(items.len(), 3);
        } else {
            panic!("expected array");
        }
    }

    #[test]
    fn test_do_xobject() {
        let data = b"/Im0 Do";
        let ops = parse_content_stream(data).unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].operator, b"Do");
        assert!(matches!(&ops[0].operands[0], Operand::Name(n) if n == b"Im0"));
    }

    #[test]
    fn test_marked_content() {
        let data = b"/OC BMC (text) Tj EMC";
        let ops = parse_content_stream(data).unwrap();
        let op_names: Vec<&[u8]> = ops.iter().map(|o| o.operator.as_slice()).collect();
        assert_eq!(op_names, vec![b"BMC".as_slice(), b"Tj", b"EMC"]);
    }

    fn op(operator: &[u8], operands: Vec<Operand>) -> ContentOp {
        ContentOp {
            operator: operator.to_vec(),
            operands,
        }
    }

    #[test]
    fn test_written_ops_read_back_unchanged() {
        let ops = vec![
            // Strings: parentheses (balanced and not), backslash, line ends, high bytes
            op(b"Tj", vec![Operand::String(b"1)".to_vec())]),
            op(b"Tj", vec![Operand::String(b"(".to_vec())]),
            op(b"Tj", vec![Operand::String(b"f(x)".to_vec())]),
            op(b"Tj", vec![Operand::String(b"a\\b".to_vec())]),
            op(b"Tj", vec![Operand::String(b"a\rb\r\nc\nd\te".to_vec())]),
            op(b"Tj", vec![Operand::String(vec![0x80, 0xE9, 0xFF, 0x00])]),
            op(b"Tj", vec![Operand::String(Vec::new())]),
            // Names: space, '#', delimiter, high byte
            op(
                b"Tf",
                vec![Operand::Name(b"F 1".to_vec()), Operand::Integer(12)],
            ),
            op(
                b"Tf",
                vec![Operand::Name(b"F#1".to_vec()), Operand::Integer(12)],
            ),
            op(b"Do", vec![Operand::Name(b"A/B(C)".to_vec())]),
            op(
                b"Tf",
                vec![Operand::Name(vec![b'F', 0xE9, b'1']), Operand::Real(9.5)],
            ),
            // Numbers
            op(
                b"cm",
                vec![
                    Operand::Real(1e20),
                    Operand::Real(1.0),
                    Operand::Real(-0.0),
                    Operand::Real(0.1 + 0.2),
                    Operand::Real(1e-10),
                    Operand::Real(f64::from(f32::MAX)),
                ],
            ),
            op(
                b"xx",
                vec![
                    Operand::Integer(i64::MIN),
                    Operand::Integer(i64::MAX),
                    Operand::Integer(0),
                ],
            ),
            op(
                b"xx",
                vec![Operand::Bool(true), Operand::Bool(false), Operand::Null],
            ),
            // Nested array and dictionary, dictionary key order kept
            op(
                b"TJ",
                vec![Operand::Array(vec![
                    Operand::String(b"A)".to_vec()),
                    Operand::Integer(-120),
                    Operand::Real(2.5),
                    Operand::Array(vec![Operand::Name(b"N M".to_vec())]),
                ])],
            ),
            op(
                b"BDC",
                vec![
                    Operand::Name(b"Span".to_vec()),
                    Operand::Dict(vec![
                        (b"Zeta".to_vec(), Operand::Integer(0)),
                        (b"Actual Text".to_vec(), Operand::String(b"x)".to_vec())),
                        (b"Alpha".to_vec(), Operand::Array(vec![Operand::Real(1.0)])),
                    ]),
                ],
            ),
            // Inline images: binary data kept byte for byte
            op(b"q", vec![]),
            op(
                b"BI",
                vec![Operand::InlineImage {
                    dict: vec![
                        (b"W".to_vec(), Operand::Integer(2)),
                        (b"H".to_vec(), Operand::Integer(1)),
                        (b"BPC".to_vec(), Operand::Integer(8)),
                        (b"CS".to_vec(), Operand::Name(b"G".to_vec())),
                        (
                            b"D".to_vec(),
                            Operand::Array(vec![Operand::Real(1.0), Operand::Integer(0)]),
                        ),
                    ],
                    data: vec![b' ', 0x80, b'E', b'I', 0x0D, 0x0A, 0xFF, b'\n'],
                }],
            ),
            op(
                b"BI",
                vec![Operand::InlineImage {
                    dict: vec![(b"W".to_vec(), Operand::Integer(1))],
                    data: Vec::new(),
                }],
            ),
            op(b"Q", vec![]),
        ];

        let written = write_content(&ops);
        let read_back = parse_content_stream(&written).unwrap();
        assert_eq!(
            read_back,
            ops,
            "written: {:?}",
            String::from_utf8_lossy(&written)
        );
    }

    const INLINE_IMAGE_WITH_DICT_VALUES: &[u8] =
        b"BI /W 1 /DP << /Predictor 15 /Columns 2 >> /X null ID \x80 EI";

    fn assert_inline_image_dict_values(ops: &[ContentOp]) {
        let [Operand::InlineImage { dict, .. }] = &ops[0].operands[..] else {
            panic!("expected an inline image: {ops:?}")
        };
        let params = vec![
            (b"Predictor".to_vec(), Operand::Integer(15)),
            (b"Columns".to_vec(), Operand::Integer(2)),
        ];
        assert_eq!(dict[1], (b"DP".to_vec(), Operand::Dict(params)));
        assert_eq!(dict[2], (b"X".to_vec(), Operand::Null));
        assert_eq!(parse_content_stream(&write_content(ops)).unwrap(), ops);
    }

    #[test]
    fn test_inline_image_dict_values_read_like_other_dicts() {
        assert_inline_image_dict_values(
            &parse_content_stream(INLINE_IMAGE_WITH_DICT_VALUES).unwrap(),
        );
    }

    #[cfg(feature = "arena")]
    #[test]
    fn test_inline_image_dict_values_read_like_other_dicts_arena() {
        assert_inline_image_dict_values(
            &parse_content_stream_arena(INLINE_IMAGE_WITH_DICT_VALUES).unwrap(),
        );
    }

    #[test]
    fn test_inline_image_under_another_operator_keeps_the_operator() {
        let image = Operand::InlineImage {
            dict: vec![(b"W".to_vec(), Operand::Integer(1))],
            data: vec![0x80],
        };
        let mut written = Vec::new();
        op(b"Do", vec![image]).write_to(&mut written);
        assert!(
            written.ends_with(b" Do"),
            "{:?}",
            String::from_utf8_lossy(&written)
        );
    }

    #[test]
    fn test_non_finite_operands_are_written_as_finite_reals() {
        let ops = vec![op(
            b"cm",
            vec![
                Operand::Real(f64::NAN),
                Operand::Real(f64::INFINITY),
                Operand::Real(f64::NEG_INFINITY),
            ],
        )];
        let read_back = parse_content_stream(&write_content(&ops)).unwrap();
        let max = f64::from(f32::MAX);
        assert_eq!(
            read_back[0].operands,
            vec![Operand::Real(0.0), Operand::Real(max), Operand::Real(-max)]
        );
    }
}
