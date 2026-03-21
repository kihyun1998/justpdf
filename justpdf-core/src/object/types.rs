use std::collections::BTreeMap;
use std::fmt;

/// A reference to an indirect PDF object: `N M R`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IndirectRef {
    pub obj_num: u32,
    pub gen_num: u16,
}

impl fmt::Display for IndirectRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {} R", self.obj_num, self.gen_num)
    }
}

/// An ordered dictionary of PDF objects, keyed by name bytes.
#[derive(Debug, Clone, PartialEq)]
pub struct PdfDict(BTreeMap<Vec<u8>, PdfObject>);

impl PdfDict {
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }

    pub fn get(&self, key: &[u8]) -> Option<&PdfObject> {
        self.0.get(key)
    }

    pub fn insert(&mut self, key: Vec<u8>, value: PdfObject) {
        self.0.insert(key, value);
    }

    pub fn remove(&mut self, key: &[u8]) -> Option<PdfObject> {
        self.0.remove(key)
    }

    pub fn contains_key(&self, key: &[u8]) -> bool {
        self.0.contains_key(key)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Vec<u8>, &PdfObject)> {
        self.0.iter()
    }

    pub fn keys(&self) -> impl Iterator<Item = &Vec<u8>> {
        self.0.keys()
    }

    /// Get a value as i64, returning None if not an integer.
    pub fn get_i64(&self, key: &[u8]) -> Option<i64> {
        match self.get(key) {
            Some(PdfObject::Integer(v)) => Some(*v),
            _ => None,
        }
    }

    /// Get a value as a name (byte slice), returning None if not a name.
    pub fn get_name(&self, key: &[u8]) -> Option<&[u8]> {
        match self.get(key) {
            Some(PdfObject::Name(v)) => Some(v.as_slice()),
            _ => None,
        }
    }

    /// Get a value as array ref.
    pub fn get_array(&self, key: &[u8]) -> Option<&[PdfObject]> {
        match self.get(key) {
            Some(PdfObject::Array(v)) => Some(v.as_slice()),
            _ => None,
        }
    }

    /// Get a value as dict ref.
    pub fn get_dict(&self, key: &[u8]) -> Option<&PdfDict> {
        match self.get(key) {
            Some(PdfObject::Dict(d)) => Some(d),
            _ => None,
        }
    }

    /// Get a value as an indirect reference.
    pub fn get_ref(&self, key: &[u8]) -> Option<&IndirectRef> {
        match self.get(key) {
            Some(PdfObject::Reference(r)) => Some(r),
            _ => None,
        }
    }

    /// Get a value as a string (byte slice), returning None if not a string.
    pub fn get_string(&self, key: &[u8]) -> Option<&[u8]> {
        match self.get(key) {
            Some(PdfObject::String(v)) => Some(v.as_slice()),
            _ => None,
        }
    }

    /// Get a value as bool, returning None if not a boolean.
    pub fn get_bool(&self, key: &[u8]) -> Option<bool> {
        match self.get(key) {
            Some(PdfObject::Bool(v)) => Some(*v),
            _ => None,
        }
    }

    /// Get a value as f64, returning None if not numeric.
    pub fn get_f64(&self, key: &[u8]) -> Option<f64> {
        match self.get(key) {
            Some(PdfObject::Integer(v)) => Some(*v as f64),
            Some(PdfObject::Real(v)) => Some(*v),
            _ => None,
        }
    }
}

impl Default for PdfDict {
    fn default() -> Self {
        Self::new()
    }
}

/// A PDF object value.
#[derive(Debug, Clone, PartialEq)]
pub enum PdfObject {
    Null,
    Bool(bool),
    Integer(i64),
    Real(f64),
    /// Name object (without leading `/`). Stored as raw bytes.
    Name(Vec<u8>),
    /// String object (literal or hex). Stored as raw bytes.
    String(Vec<u8>),
    /// Array of PDF objects.
    Array(Vec<PdfObject>),
    /// Dictionary of PDF objects.
    Dict(PdfDict),
    /// Stream: a dictionary plus raw (still-encoded) data.
    Stream {
        dict: PdfDict,
        data: Vec<u8>,
    },
    /// Indirect reference: `N M R`.
    Reference(IndirectRef),
}

impl PdfObject {
    // --- Type checking ---

    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    pub fn is_bool(&self) -> bool {
        matches!(self, Self::Bool(_))
    }

    pub fn is_integer(&self) -> bool {
        matches!(self, Self::Integer(_))
    }

    pub fn is_real(&self) -> bool {
        matches!(self, Self::Real(_))
    }

    pub fn is_number(&self) -> bool {
        matches!(self, Self::Integer(_) | Self::Real(_))
    }

    pub fn is_name(&self) -> bool {
        matches!(self, Self::Name(_))
    }

    pub fn is_string(&self) -> bool {
        matches!(self, Self::String(_))
    }

    pub fn is_array(&self) -> bool {
        matches!(self, Self::Array(_))
    }

    pub fn is_dict(&self) -> bool {
        matches!(self, Self::Dict(_))
    }

    pub fn is_stream(&self) -> bool {
        matches!(self, Self::Stream { .. })
    }

    pub fn is_reference(&self) -> bool {
        matches!(self, Self::Reference(_))
    }

    // --- Accessors ---

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(v) => Some(*v),
            _ => None,
        }
    }

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

    pub fn as_array(&self) -> Option<&[PdfObject]> {
        match self {
            Self::Array(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_dict(&self) -> Option<&PdfDict> {
        match self {
            Self::Dict(d) => Some(d),
            _ => None,
        }
    }

    pub fn as_reference(&self) -> Option<&IndirectRef> {
        match self {
            Self::Reference(r) => Some(r),
            _ => None,
        }
    }

    pub fn as_stream(&self) -> Option<(&PdfDict, &[u8])> {
        match self {
            Self::Stream { dict, data } => Some((dict, data)),
            _ => None,
        }
    }

    /// Get type name for display purposes.
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Null => "Null",
            Self::Bool(_) => "Bool",
            Self::Integer(_) => "Integer",
            Self::Real(_) => "Real",
            Self::Name(_) => "Name",
            Self::String(_) => "String",
            Self::Array(_) => "Array",
            Self::Dict(_) => "Dict",
            Self::Stream { .. } => "Stream",
            Self::Reference(_) => "Reference",
        }
    }
}

impl fmt::Display for PdfObject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Null => write!(f, "null"),
            Self::Bool(v) => write!(f, "{v}"),
            Self::Integer(v) => write!(f, "{v}"),
            Self::Real(v) => write!(f, "{v}"),
            Self::Name(v) => {
                write!(f, "/")?;
                for &byte in v {
                    // PDF spec: Name objects must escape delimiters, whitespace,
                    // and '#' using #XX hex notation
                    if byte == b'#'
                        || byte == b'/'
                        || byte == b'('
                        || byte == b')'
                        || byte == b'<'
                        || byte == b'>'
                        || byte == b'['
                        || byte == b']'
                        || byte == b'{'
                        || byte == b'}'
                        || byte == b'%'
                        || byte <= b' '
                        || byte >= 127
                    {
                        write!(f, "#{:02X}", byte)?;
                    } else {
                        write!(f, "{}", byte as char)?;
                    }
                }
                Ok(())
            }
            Self::String(v) => {
                // Use literal string with proper escaping if ASCII-safe,
                // hex string otherwise
                let needs_hex = v.iter().any(|&b| b == 0 || (b < 32 && b != b'\n' && b != b'\r' && b != b'\t'));
                if needs_hex {
                    write!(f, "<")?;
                    for b in v {
                        write!(f, "{b:02X}")?;
                    }
                    write!(f, ">")
                } else {
                    write!(f, "(")?;
                    for &b in v {
                        match b {
                            b'(' => write!(f, "\\(")?,
                            b')' => write!(f, "\\)")?,
                            b'\\' => write!(f, "\\\\")?,
                            _ => write!(f, "{}", b as char)?,
                        }
                    }
                    write!(f, ")")
                }
            }
            Self::Array(v) => {
                write!(f, "[")?;
                for (i, item) in v.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    write!(f, "{item}")?;
                }
                write!(f, "]")
            }
            Self::Dict(d) => {
                write!(f, "<< ")?;
                for (key, val) in d.iter() {
                    write!(f, "/")?;
                    for &byte in key {
                        if byte == b'#' || byte == b'/' || byte == b'(' || byte == b')'
                            || byte == b'<' || byte == b'>' || byte == b'['
                            || byte == b']' || byte == b'{' || byte == b'}'
                            || byte == b'%' || byte <= b' ' || byte >= 127
                        {
                            write!(f, "#{:02X}", byte)?;
                        } else {
                            write!(f, "{}", byte as char)?;
                        }
                    }
                    write!(f, " {val} ")?;
                }
                write!(f, ">>")
            }
            Self::Stream { dict, data } => {
                write!(f, "<stream dict={dict:?} len={}>", data.len())
            }
            Self::Reference(r) => write!(f, "{r}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_indirect_ref_display() {
        let r = IndirectRef {
            obj_num: 10,
            gen_num: 0,
        };
        assert_eq!(r.to_string(), "10 0 R");
    }

    #[test]
    fn test_pdf_dict_basic() {
        let mut d = PdfDict::new();
        d.insert(b"Type".to_vec(), PdfObject::Name(b"Catalog".to_vec()));
        d.insert(b"Count".to_vec(), PdfObject::Integer(5));

        assert_eq!(d.len(), 2);
        assert_eq!(d.get_name(b"Type"), Some(b"Catalog".as_slice()));
        assert_eq!(d.get_i64(b"Count"), Some(5));
        assert!(d.get(b"Missing").is_none());
    }

    #[test]
    fn test_object_accessors() {
        assert_eq!(PdfObject::Bool(true).as_bool(), Some(true));
        assert_eq!(PdfObject::Integer(42).as_i64(), Some(42));
        assert_eq!(PdfObject::Integer(42).as_f64(), Some(42.0));
        assert_eq!(PdfObject::Real(3.15).as_f64(), Some(3.15));
        assert_eq!(
            PdfObject::Name(b"Test".to_vec()).as_name(),
            Some(b"Test".as_slice())
        );
        assert_eq!(PdfObject::Null.as_bool(), None);
    }

    #[test]
    fn test_object_type_checks() {
        assert!(PdfObject::Null.is_null());
        assert!(PdfObject::Bool(true).is_bool());
        assert!(PdfObject::Integer(1).is_integer());
        assert!(PdfObject::Integer(1).is_number());
        assert!(PdfObject::Real(1.0).is_number());
        assert!(!PdfObject::Null.is_number());
    }

    #[test]
    fn test_object_display() {
        assert_eq!(PdfObject::Null.to_string(), "null");
        assert_eq!(PdfObject::Integer(42).to_string(), "42");
        assert_eq!(PdfObject::Name(b"Type".to_vec()).to_string(), "/Type");
        assert_eq!(PdfObject::String(b"Hello".to_vec()).to_string(), "(Hello)");
    }

    // ── Name escape tests ───────────────────────────────────────────

    #[test]
    fn test_name_display_with_space() {
        // Font names like "Pretendard Black" must escape the space
        let name = PdfObject::Name(b"Pretendard Black".to_vec());
        let s = name.to_string();
        assert_eq!(s, "/Pretendard#20Black");
        assert!(!s.contains(' '), "Name must not contain raw spaces");
    }

    #[test]
    fn test_name_display_with_hash() {
        let name = PdfObject::Name(b"Font#1".to_vec());
        assert_eq!(name.to_string(), "/Font#231");
    }

    #[test]
    fn test_name_display_with_delimiters() {
        let name = PdfObject::Name(b"A(B)C".to_vec());
        let s = name.to_string();
        assert!(s.contains("#28"), "( should be escaped to #28");
        assert!(s.contains("#29"), ") should be escaped to #29");
    }

    #[test]
    fn test_name_display_with_non_ascii() {
        let name = PdfObject::Name(vec![0xC0, 0xAF]); // non-ASCII bytes
        let s = name.to_string();
        assert!(s.contains("#C0"));
        assert!(s.contains("#AF"));
    }

    #[test]
    fn test_name_display_simple_no_escape() {
        // Normal names should not be escaped
        let name = PdfObject::Name(b"BaseFont".to_vec());
        assert_eq!(name.to_string(), "/BaseFont");
    }

    #[test]
    fn test_name_display_with_slash() {
        let name = PdfObject::Name(b"A/B".to_vec());
        assert_eq!(name.to_string(), "/A#2FB");
    }

    #[test]
    fn test_name_display_with_percent() {
        let name = PdfObject::Name(b"50%".to_vec());
        assert_eq!(name.to_string(), "/50#25");
    }

    // ── String escape tests ─────────────────────────────────────────

    #[test]
    fn test_string_display_with_parens() {
        let s = PdfObject::String(b"hello(world)".to_vec());
        let display = s.to_string();
        assert!(display.contains("\\(") && display.contains("\\)"),
            "Parentheses must be escaped: {}", display);
    }

    #[test]
    fn test_string_display_with_backslash() {
        let s = PdfObject::String(b"path\\to".to_vec());
        let display = s.to_string();
        assert!(display.contains("\\\\"), "Backslash must be escaped: {}", display);
    }

    #[test]
    fn test_string_display_binary_uses_hex() {
        // Binary data with null bytes should use hex encoding
        let s = PdfObject::String(vec![0x00, 0xFF, 0x42]);
        let display = s.to_string();
        assert!(display.starts_with('<') && display.ends_with('>'),
            "Binary strings should use hex: {}", display);
        assert_eq!(display, "<00FF42>");
    }

    #[test]
    fn test_string_display_normal_ascii() {
        let s = PdfObject::String(b"Normal text".to_vec());
        assert_eq!(s.to_string(), "(Normal text)");
    }

    // ── Dict display with special name keys ─────────────────────────

    #[test]
    fn test_dict_display_key_with_space() {
        let mut d = PdfDict::new();
        d.insert(b"Base Font".to_vec(), PdfObject::Name(b"Helvetica".to_vec()));
        let s = format!("{}", PdfObject::Dict(d));
        assert!(s.contains("/Base#20Font"), "Dict key with space must be escaped: {}", s);
    }

    #[test]
    fn test_object_clone_eq() {
        let obj = PdfObject::Array(vec![PdfObject::Integer(1), PdfObject::Integer(2)]);
        let cloned = obj.clone();
        assert_eq!(obj, cloned);
    }
}
