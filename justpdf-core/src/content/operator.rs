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
#[derive(Debug, Clone)]
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
}

impl std::fmt::Display for ContentOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, op) in self.operands.iter().enumerate() {
            if i > 0 {
                write!(f, " ")?;
            }
            write!(f, "{}", format_operand(op))?;
        }
        if !self.operands.is_empty() {
            write!(f, " ")?;
        }
        write!(f, "{}", self.operator_str())
    }
}

fn format_operand(op: &Operand) -> String {
    match op {
        Operand::Integer(v) => v.to_string(),
        Operand::Real(v) => format!("{v}"),
        Operand::Bool(v) => v.to_string(),
        Operand::Null => "null".into(),
        Operand::Name(n) => format!("/{}", std::str::from_utf8(n).unwrap_or("?")),
        Operand::String(s) => {
            match std::str::from_utf8(s) {
                Ok(text) => format!("({text})"),
                Err(_) => {
                    let hex: String = s.iter().map(|b| format!("{b:02X}")).collect();
                    format!("<{hex}>")
                }
            }
        }
        Operand::Array(items) => {
            let inner: Vec<String> = items.iter().map(format_operand).collect();
            format!("[{}]", inner.join(" "))
        }
        Operand::Dict(entries) => {
            let inner: Vec<String> = entries
                .iter()
                .map(|(k, v)| {
                    format!(
                        "/{} {}",
                        std::str::from_utf8(k).unwrap_or("?"),
                        format_operand(v)
                    )
                })
                .collect();
            format!("<< {} >>", inner.join(" "))
        }
        Operand::InlineImage { .. } => "<inline-image>".into(),
    }
}
