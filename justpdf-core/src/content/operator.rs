use std::fmt::Write;

use crate::object::{ByteSink, write_name, write_real, write_string};

/// A single operand value in a content stream.
#[derive(Debug, Clone, PartialEq)]
pub enum Operand {
    Integer(i64),
    Real(f64),
    Bool(bool),
    Null,
    Name(Vec<u8>),
    String(Vec<u8>),
    Array(Vec<Operand>),
    Dict(Vec<(Vec<u8>, Operand)>),
    /// Inline image data (from BI ... ID ... EI).
    InlineImage {
        dict: Vec<(Vec<u8>, Operand)>,
        data: Vec<u8>,
    },
}

impl Operand {
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Integer(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Real(v) => Some(*v),
            Self::Integer(v) => Some(*v as f64),
            _ => None,
        }
    }

    pub fn as_name(&self) -> Option<&[u8]> {
        match self {
            Self::Name(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&[u8]> {
        match self {
            Self::String(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Operand]> {
        match self {
            Self::Array(v) => Some(v),
            _ => None,
        }
    }
}

/// A single content stream operation: operands followed by an operator.
#[derive(Debug, Clone, PartialEq)]
pub struct ContentOp {
    /// The operator keyword (e.g., "cm", "Tf", "Tj", "q", "Q").
    pub operator: Vec<u8>,
    /// Operands that precede the operator.
    pub operands: Vec<Operand>,
}

impl ContentOp {
    /// Get the operator as a string.
    pub fn operator_str(&self) -> &str {
        std::str::from_utf8(&self.operator).unwrap_or("?")
    }

    /// Append this operation as content stream syntax, without a trailing
    /// end-of-line. A `BI` operation holding an inline image is written as
    /// `BI … ID … EI`.
    pub(crate) fn write_to(&self, buf: &mut Vec<u8>) {
        if let (b"BI", [image @ Operand::InlineImage { .. }]) =
            (self.operator.as_slice(), self.operands.as_slice())
        {
            image.write_to(buf);
            return;
        }
        for op in &self.operands {
            op.write_to(buf);
            buf.push(b' ');
        }
        buf.extend_from_slice(&self.operator);
    }
}

impl Operand {
    /// Append this operand as content stream syntax.
    pub(crate) fn write_to(&self, buf: &mut Vec<u8>) {
        let mut sink = ByteSink(buf);
        let _ = match self {
            Operand::Integer(v) => write!(sink, "{v}"),
            Operand::Real(v) => write_real(&mut sink, *v),
            Operand::Bool(v) => write!(sink, "{v}"),
            Operand::Null => write!(sink, "null"),
            Operand::Name(n) => write_name(&mut sink, n),
            Operand::String(s) => write_string(&mut sink, s),
            Operand::Array(items) => {
                buf.push(b'[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        buf.push(b' ');
                    }
                    item.write_to(buf);
                }
                buf.push(b']');
                Ok(())
            }
            Operand::Dict(entries) => {
                buf.extend_from_slice(b"<<");
                write_entries(buf, entries);
                buf.extend_from_slice(b" >>");
                Ok(())
            }
            Operand::InlineImage { dict, data } => {
                buf.extend_from_slice(b"BI");
                write_entries(buf, dict);
                buf.extend_from_slice(b" ID ");
                buf.extend_from_slice(data);
                buf.extend_from_slice(b" EI");
                Ok(())
            }
        };
    }
}

fn write_entries(buf: &mut Vec<u8>, entries: &[(Vec<u8>, Operand)]) {
    for (key, value) in entries {
        buf.push(b' ');
        let _ = write_name(&mut ByteSink(buf), key);
        buf.push(b' ');
        value.write_to(buf);
    }
}

/// Write operations as a content stream, one per line.
pub(crate) fn write_content(ops: &[ContentOp]) -> Vec<u8> {
    let mut buf = Vec::new();
    for op in ops {
        op.write_to(&mut buf);
        buf.push(b'\n');
    }
    buf
}

impl std::fmt::Display for ContentOp {
    /// Content stream syntax; inline image data that is not UTF-8 is shown lossily.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut buf = Vec::new();
        self.write_to(&mut buf);
        f.write_str(&String::from_utf8_lossy(&buf))
    }
}
