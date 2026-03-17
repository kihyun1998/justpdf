/// A byte-level reader over a PDF byte slice, with position tracking.
pub struct PdfReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> PdfReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    /// Create a reader starting at a specific offset.
    pub fn new_at(data: &'a [u8], pos: usize) -> Self {
        Self { data, pos }
    }

    /// Current byte offset in the data.
    #[inline]
    pub fn pos(&self) -> usize {
        self.pos
    }

    /// Total length of the underlying data.
    #[inline]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Whether the underlying data is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Whether we've reached the end.
    #[inline]
    pub fn is_eof(&self) -> bool {
        self.pos >= self.data.len()
    }

    /// Peek at the current byte without consuming.
    #[inline]
    pub fn peek(&self) -> Option<u8> {
        self.data.get(self.pos).copied()
    }

    /// Peek at the byte at offset `pos + n`.
    #[inline]
    pub fn peek_at(&self, n: usize) -> Option<u8> {
        self.data.get(self.pos + n).copied()
    }

    /// Consume and return the current byte.
    #[inline]
    pub fn next_byte(&mut self) -> Option<u8> {
        let b = self.data.get(self.pos).copied();
        if b.is_some() {
            self.pos += 1;
        }
        b
    }

    /// Advance position by `n` bytes.
    #[inline]
    pub fn advance(&mut self, n: usize) {
        self.pos = (self.pos + n).min(self.data.len());
    }

    /// Set position to an absolute offset.
    #[inline]
    pub fn seek(&mut self, pos: usize) {
        self.pos = pos.min(self.data.len());
    }

    /// Return a slice from the underlying data.
    pub fn slice(&self, start: usize, end: usize) -> &'a [u8] {
        let end = end.min(self.data.len());
        let start = start.min(end);
        &self.data[start..end]
    }

    /// Return remaining bytes from current position.
    pub fn remaining(&self) -> &'a [u8] {
        if self.pos >= self.data.len() {
            &[]
        } else {
            &self.data[self.pos..]
        }
    }

    /// The full underlying data.
    pub fn data(&self) -> &'a [u8] {
        self.data
    }

    /// Skip PDF whitespace characters: \0, \t, \n, \x0C, \r, \x20.
    pub fn skip_whitespace(&mut self) {
        while let Some(b) = self.peek() {
            if is_pdf_whitespace(b) {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    /// Skip whitespace and comments (% to end of line).
    pub fn skip_whitespace_and_comments(&mut self) {
        loop {
            self.skip_whitespace();
            if self.peek() == Some(b'%') {
                // Skip to end of line
                while let Some(b) = self.next_byte() {
                    if b == b'\n' || b == b'\r' {
                        break;
                    }
                }
            } else {
                break;
            }
        }
    }
}

/// Check if a byte is PDF whitespace.
#[inline]
pub fn is_pdf_whitespace(b: u8) -> bool {
    matches!(b, b'\0' | b'\t' | b'\n' | b'\x0C' | b'\r' | b' ')
}

/// Check if a byte is a PDF delimiter.
#[inline]
pub fn is_pdf_delimiter(b: u8) -> bool {
    matches!(
        b,
        b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%'
    )
}

/// Check if a byte is a regular character (not whitespace or delimiter).
#[inline]
pub fn is_pdf_regular(b: u8) -> bool {
    !is_pdf_whitespace(b) && !is_pdf_delimiter(b)
}
