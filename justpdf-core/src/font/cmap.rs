use std::collections::HashMap;

/// A parsed ToUnicode CMap: maps character codes to Unicode strings.
#[derive(Debug, Clone)]
pub struct ToUnicodeCMap {
    /// Single char code → Unicode string.
    mappings: HashMap<u32, String>,
    /// Range mappings: (start_code, end_code, start_unicode).
    ranges: Vec<(u32, u32, u32)>,
}

impl ToUnicodeCMap {
    /// Parse a ToUnicode CMap from its raw stream data.
    pub fn parse(data: &[u8]) -> Self {
        let mut cmap = Self {
            mappings: HashMap::new(),
            ranges: Vec::new(),
        };

        let text = String::from_utf8_lossy(data);

        // Parse "beginbfchar" sections: <src> <dst>
        let mut pos = 0;
        while let Some(start) = text[pos..].find("beginbfchar") {
            let section_start = pos + start + "beginbfchar".len();
            let section_end = text[section_start..]
                .find("endbfchar")
                .map(|i| section_start + i)
                .unwrap_or(text.len());

            let section = &text[section_start..section_end];
            parse_bfchar_section(section, &mut cmap.mappings);

            pos = section_end;
        }

        // Parse "beginbfrange" sections: <start> <end> <dst>
        pos = 0;
        while let Some(start) = text[pos..].find("beginbfrange") {
            let section_start = pos + start + "beginbfrange".len();
            let section_end = text[section_start..]
                .find("endbfrange")
                .map(|i| section_start + i)
                .unwrap_or(text.len());

            let section = &text[section_start..section_end];
            parse_bfrange_section(section, &mut cmap.mappings, &mut cmap.ranges);

            pos = section_end;
        }

        cmap
    }

    /// Look up a character code and return the Unicode string.
    pub fn lookup(&self, code: u32) -> Option<String> {
        // Check direct mappings first
        if let Some(s) = self.mappings.get(&code) {
            return Some(s.clone());
        }

        // Check range mappings
        for &(start, end, start_unicode) in &self.ranges {
            if code >= start && code <= end {
                let offset = code - start;
                if let Some(c) = char::from_u32(start_unicode + offset) {
                    return Some(c.to_string());
                }
            }
        }

        None
    }

    /// Number of mappings.
    pub fn len(&self) -> usize {
        self.mappings.len() + self.ranges.len()
    }

    pub fn is_empty(&self) -> bool {
        self.mappings.is_empty() && self.ranges.is_empty()
    }
}

/// Parse a bfchar section: pairs of <srcCode> <dstString>.
fn parse_bfchar_section(section: &str, mappings: &mut HashMap<u32, String>) {
    let hex_values = extract_hex_values(section);
    for pair in hex_values.chunks(2) {
        if pair.len() == 2 {
            let src_code = u32::from_str_radix(&pair[0], 16).unwrap_or(0);
            let dst_str = hex_to_unicode_string(&pair[1]);
            mappings.insert(src_code, dst_str);
        }
    }
}

/// Parse a bfrange section: triples of <startCode> <endCode> <dstStartOrArray>.
fn parse_bfrange_section(
    section: &str,
    mappings: &mut HashMap<u32, String>,
    ranges: &mut Vec<(u32, u32, u32)>,
) {
    let mut chars = section.chars().peekable();
    loop {
        // Skip to next '<'
        skip_until(&mut chars, '<');
        let start_hex = read_hex_token(&mut chars);
        if start_hex.is_empty() {
            break;
        }

        skip_until(&mut chars, '<');
        let end_hex = read_hex_token(&mut chars);
        if end_hex.is_empty() {
            break;
        }

        let start_code = u32::from_str_radix(&start_hex, 16).unwrap_or(0);
        let end_code = u32::from_str_radix(&end_hex, 16).unwrap_or(0);

        // Next could be <hex> or [ <hex> <hex> ... ]
        skip_whitespace(&mut chars);
        match chars.peek() {
            Some('<') => {
                chars.next(); // skip '<'
                let dst_hex = read_hex_token(&mut chars);
                let dst_code = u32::from_str_radix(&dst_hex, 16).unwrap_or(0);
                ranges.push((start_code, end_code, dst_code));
            }
            Some('[') => {
                chars.next(); // skip '['
                let mut code = start_code;
                loop {
                    skip_whitespace(&mut chars);
                    match chars.peek() {
                        Some(']') => {
                            chars.next();
                            break;
                        }
                        Some('<') => {
                            chars.next();
                            let hex = read_hex_token(&mut chars);
                            let dst_str = hex_to_unicode_string(&hex);
                            mappings.insert(code, dst_str);
                            code += 1;
                        }
                        None => break,
                        _ => {
                            chars.next();
                        }
                    }
                }
            }
            _ => break,
        }
    }
}

/// Extract all hex values enclosed in <...> from text.
fn extract_hex_values(text: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut in_hex = false;
    let mut current = String::new();

    for c in text.chars() {
        match c {
            '<' => {
                in_hex = true;
                current.clear();
            }
            '>' => {
                if in_hex {
                    values.push(current.clone());
                    in_hex = false;
                }
            }
            _ if in_hex && c.is_ascii_hexdigit() => {
                current.push(c);
            }
            _ => {}
        }
    }

    values
}

/// Convert a hex string to a Unicode string.
/// Each pair of hex digits is a byte; interpreted as UTF-16BE.
fn hex_to_unicode_string(hex: &str) -> String {
    let bytes: Vec<u8> = (0..hex.len())
        .step_by(2)
        .filter_map(|i| {
            if i + 2 <= hex.len() {
                u8::from_str_radix(&hex[i..i + 2], 16).ok()
            } else {
                None
            }
        })
        .collect();

    // If 2 bytes, interpret as single UTF-16BE codepoint
    if bytes.len() == 2 {
        let code = ((bytes[0] as u32) << 8) | bytes[1] as u32;
        if let Some(c) = char::from_u32(code) {
            return c.to_string();
        }
    }

    // If 4 bytes, could be surrogate pair or two codepoints
    if bytes.len() == 4 {
        let hi = ((bytes[0] as u16) << 8) | bytes[1] as u16;
        let lo = ((bytes[2] as u16) << 8) | bytes[3] as u16;

        // Check for surrogate pair
        if (0xD800..=0xDBFF).contains(&hi) && (0xDC00..=0xDFFF).contains(&lo) {
            let cp = 0x10000 + ((hi as u32 - 0xD800) << 10) + (lo as u32 - 0xDC00);
            if let Some(c) = char::from_u32(cp) {
                return c.to_string();
            }
        }

        // Two separate codepoints
        let mut s = String::new();
        if let Some(c) = char::from_u32(hi as u32) {
            s.push(c);
        }
        if let Some(c) = char::from_u32(lo as u32) {
            s.push(c);
        }
        return s;
    }

    // Fallback: try pairs as UTF-16BE
    let mut s = String::new();
    for chunk in bytes.chunks(2) {
        if chunk.len() == 2 {
            let code = ((chunk[0] as u32) << 8) | chunk[1] as u32;
            if let Some(c) = char::from_u32(code) {
                s.push(c);
            }
        }
    }
    s
}

fn skip_until(chars: &mut std::iter::Peekable<std::str::Chars<'_>>, target: char) {
    while let Some(&c) = chars.peek() {
        if c == target {
            chars.next(); // consume the target
            return;
        }
        chars.next();
    }
}

fn skip_whitespace(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
    while let Some(&c) = chars.peek() {
        if c.is_ascii_whitespace() {
            chars.next();
        } else {
            break;
        }
    }
}

fn read_hex_token(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut hex = String::new();
    while let Some(&c) = chars.peek() {
        if c == '>' {
            chars.next(); // consume '>'
            break;
        }
        if c.is_ascii_hexdigit() {
            hex.push(c);
        }
        chars.next();
    }
    hex
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_bfchar() {
        let data = br#"
/CIDInit /ProcSet findresource begin
1 begincodespacerange
<0000> <FFFF>
endcodespacerange
3 beginbfchar
<0003> <0020>
<0011> <002E>
<0024> <0041>
endbfchar
endcmap
"#;
        let cmap = ToUnicodeCMap::parse(data);
        assert_eq!(cmap.lookup(0x0003), Some(" ".into()));
        assert_eq!(cmap.lookup(0x0011), Some(".".into()));
        assert_eq!(cmap.lookup(0x0024), Some("A".into()));
        assert_eq!(cmap.lookup(0x9999), None);
    }

    #[test]
    fn test_parse_bfrange() {
        let data = br#"
1 begincodespacerange
<00> <FF>
endcodespacerange
1 beginbfrange
<41> <5A> <0041>
endbfrange
"#;
        let cmap = ToUnicodeCMap::parse(data);
        assert_eq!(cmap.lookup(0x41), Some("A".into()));
        assert_eq!(cmap.lookup(0x42), Some("B".into()));
        assert_eq!(cmap.lookup(0x5A), Some("Z".into()));
        assert_eq!(cmap.lookup(0x40), None); // before range
    }

    #[test]
    fn test_parse_bfrange_with_array() {
        let data = br#"
1 beginbfrange
<01> <03> [<0041> <0042> <0043>]
endbfrange
"#;
        let cmap = ToUnicodeCMap::parse(data);
        assert_eq!(cmap.lookup(0x01), Some("A".into()));
        assert_eq!(cmap.lookup(0x02), Some("B".into()));
        assert_eq!(cmap.lookup(0x03), Some("C".into()));
    }

    #[test]
    fn test_empty_cmap() {
        let cmap = ToUnicodeCMap::parse(b"");
        assert!(cmap.is_empty());
    }

    #[test]
    fn test_hex_to_unicode() {
        assert_eq!(hex_to_unicode_string("0041"), "A");
        assert_eq!(hex_to_unicode_string("0048"), "H");
        assert_eq!(hex_to_unicode_string("AC00"), "가"); // Korean
    }

    #[test]
    fn test_multibyte_unicode() {
        // Two codepoints
        assert_eq!(hex_to_unicode_string("00480069"), "Hi");
    }
}
