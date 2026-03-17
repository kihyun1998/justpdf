//! Output formats for extracted text: plain text, HTML, JSON, Markdown.

use super::PageText;

/// Output format enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    PlainText,
    Html,
    Json,
    Markdown,
}

/// Format a single page's text in the specified format.
pub fn format_page(page: &PageText, format: OutputFormat) -> String {
    match format {
        OutputFormat::PlainText => format_plain(page),
        OutputFormat::Html => format_html(page),
        OutputFormat::Json => format_json(page),
        OutputFormat::Markdown => format_markdown(page),
    }
}

/// Format multiple pages.
pub fn format_pages(pages: &[PageText], format: OutputFormat) -> String {
    match format {
        OutputFormat::PlainText => {
            pages
                .iter()
                .map(format_plain)
                .collect::<Vec<_>>()
                .join("\n\n")
        }
        OutputFormat::Html => format_html_multi(pages),
        OutputFormat::Json => format_json_multi(pages),
        OutputFormat::Markdown => {
            pages
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    let mut s = format!("## Page {}\n\n", i + 1);
                    s.push_str(&format_markdown(p));
                    s
                })
                .collect::<Vec<_>>()
                .join("\n\n---\n\n")
        }
    }
}

// ---------------------------------------------------------------------------
// Plain text
// ---------------------------------------------------------------------------

fn format_plain(page: &PageText) -> String {
    page.plain_text()
}

// ---------------------------------------------------------------------------
// HTML
// ---------------------------------------------------------------------------

fn format_html(page: &PageText) -> String {
    let mut html = String::new();
    html.push_str("<div class=\"page\">\n");

    for block in &page.blocks {
        html.push_str("  <p>");
        for (i, line) in block.lines.iter().enumerate() {
            if i > 0 {
                html.push_str("<br/>\n    ");
            }
            html.push_str(&html_escape(&line.text));
        }
        html.push_str("</p>\n");
    }

    html.push_str("</div>");
    html
}

fn format_html_multi(pages: &[PageText]) -> String {
    let mut html = String::new();
    html.push_str("<!DOCTYPE html>\n<html>\n<head>\n");
    html.push_str("  <meta charset=\"utf-8\">\n");
    html.push_str("  <title>Extracted Text</title>\n");
    html.push_str("  <style>\n");
    html.push_str("    .page { margin-bottom: 2em; padding-bottom: 1em; border-bottom: 1px solid #ccc; }\n");
    html.push_str("    p { margin: 0.5em 0; }\n");
    html.push_str("  </style>\n");
    html.push_str("</head>\n<body>\n");

    for (i, page) in pages.iter().enumerate() {
        html.push_str(&format!("<h2>Page {}</h2>\n", i + 1));
        html.push_str(&format_html(page));
        html.push('\n');
    }

    html.push_str("</body>\n</html>");
    html
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

// ---------------------------------------------------------------------------
// JSON
// ---------------------------------------------------------------------------

fn format_json(page: &PageText) -> String {
    let mut json = String::new();
    json.push_str("{\n");
    json.push_str(&format!("  \"page_index\": {},\n", page.page_index));

    // Blocks
    if page.blocks.is_empty() {
        json.push_str("  \"blocks\": [],\n");
    } else {
        json.push_str("  \"blocks\": [\n");
        for (bi, block) in page.blocks.iter().enumerate() {
            json.push_str("    {\n");
            json.push_str(&format!("      \"text\": {},\n", json_string(&block.text)));

            // Lines
            json.push_str("      \"lines\": [\n");
            for (li, line) in block.lines.iter().enumerate() {
                json.push_str("        {\n");
                json.push_str(&format!("          \"text\": {},\n", json_string(&line.text)));
                json.push_str(&format!("          \"x\": {:.2},\n", line.x));
                json.push_str(&format!("          \"y\": {:.2},\n", line.y));

                // Words
                json.push_str("          \"words\": [\n");
                for (wi, word) in line.words.iter().enumerate() {
                    json.push_str("            {\n");
                    json.push_str(&format!("              \"text\": {},\n", json_string(&word.text)));
                    json.push_str(&format!("              \"x\": {:.2},\n", word.x));
                    json.push_str(&format!("              \"y\": {:.2},\n", word.y));
                    json.push_str(&format!("              \"width\": {:.2},\n", word.width));
                    json.push_str(&format!("              \"font_size\": {:.2}\n", word.font_size));
                    json.push_str("            }");
                    if wi + 1 < line.words.len() {
                        json.push(',');
                    }
                    json.push('\n');
                }
                json.push_str("          ]\n");

                json.push_str("        }");
                if li + 1 < block.lines.len() {
                    json.push(',');
                }
                json.push('\n');
            }
            json.push_str("      ]\n");

            json.push_str("    }");
            if bi + 1 < page.blocks.len() {
                json.push(',');
            }
            json.push('\n');
        }
        json.push_str("  ],\n");
    }

    // Characters (compact)
    json.push_str(&format!("  \"char_count\": {}\n", page.chars.len()));

    json.push('}');
    json
}

fn format_json_multi(pages: &[PageText]) -> String {
    let mut json = String::new();
    json.push_str("{\n  \"pages\": [\n");

    for (i, page) in pages.iter().enumerate() {
        json.push_str("    ");
        // Indent the page JSON
        let page_json = format_json(page);
        for (li, line) in page_json.lines().enumerate() {
            if li > 0 {
                json.push_str("\n    ");
            }
            json.push_str(line);
        }
        if i + 1 < pages.len() {
            json.push(',');
        }
        json.push('\n');
    }

    json.push_str("  ]\n}");
    json
}

fn json_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len() + 2);
    result.push('"');
    for c in s.chars() {
        match c {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            c if c < '\x20' => {
                result.push_str(&format!("\\u{:04x}", c as u32));
            }
            _ => result.push(c),
        }
    }
    result.push('"');
    result
}

// ---------------------------------------------------------------------------
// Markdown
// ---------------------------------------------------------------------------

fn format_markdown(page: &PageText) -> String {
    let mut md = String::new();

    for (i, block) in page.blocks.iter().enumerate() {
        if i > 0 {
            md.push_str("\n\n");
        }
        md.push_str(&block.text);
    }

    md
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::{TextBlock, TextChar, TextLine, TextWord};

    fn make_test_page() -> PageText {
        PageText {
            page_index: 0,
            chars: vec![
                TextChar {
                    unicode: "H".into(),
                    x: 72.0,
                    y: 720.0,
                    font_size: 12.0,
                    font_name: "F1".into(),
                    width: 7.0,
                },
                TextChar {
                    unicode: "i".into(),
                    x: 79.0,
                    y: 720.0,
                    font_size: 12.0,
                    font_name: "F1".into(),
                    width: 3.0,
                },
            ],
            lines: vec![TextLine {
                text: "Hi there".into(),
                words: vec![
                    TextWord {
                        text: "Hi".into(),
                        x: 72.0,
                        y: 720.0,
                        width: 10.0,
                        font_size: 12.0,
                    },
                    TextWord {
                        text: "there".into(),
                        x: 90.0,
                        y: 720.0,
                        width: 28.0,
                        font_size: 12.0,
                    },
                ],
                x: 72.0,
                y: 720.0,
            }],
            blocks: vec![TextBlock {
                text: "Hi there".into(),
                lines: vec![TextLine {
                    text: "Hi there".into(),
                    words: vec![
                        TextWord {
                            text: "Hi".into(),
                            x: 72.0,
                            y: 720.0,
                            width: 10.0,
                            font_size: 12.0,
                        },
                        TextWord {
                            text: "there".into(),
                            x: 90.0,
                            y: 720.0,
                            width: 28.0,
                            font_size: 12.0,
                        },
                    ],
                    x: 72.0,
                    y: 720.0,
                }],
            }],
        }
    }

    #[test]
    fn test_plain_text() {
        let page = make_test_page();
        let text = format_page(&page, OutputFormat::PlainText);
        assert_eq!(text, "Hi there");
    }

    #[test]
    fn test_html_output() {
        let page = make_test_page();
        let html = format_page(&page, OutputFormat::Html);
        assert!(html.contains("<div class=\"page\">"));
        assert!(html.contains("<p>Hi there</p>"));
        assert!(html.contains("</div>"));
    }

    #[test]
    fn test_html_escaping() {
        let page = PageText {
            page_index: 0,
            chars: Vec::new(),
            lines: Vec::new(),
            blocks: vec![TextBlock {
                text: "a < b & c > d".into(),
                lines: vec![TextLine {
                    text: "a < b & c > d".into(),
                    words: Vec::new(),
                    x: 0.0,
                    y: 0.0,
                }],
            }],
        };
        let html = format_page(&page, OutputFormat::Html);
        assert!(html.contains("a &lt; b &amp; c &gt; d"));
    }

    #[test]
    fn test_json_output() {
        let page = make_test_page();
        let json = format_page(&page, OutputFormat::Json);
        assert!(json.contains("\"page_index\": 0"));
        assert!(json.contains("\"text\": \"Hi there\""));
        assert!(json.contains("\"blocks\""));
        assert!(json.contains("\"words\""));
    }

    #[test]
    fn test_json_string_escaping() {
        assert_eq!(json_string("hello"), "\"hello\"");
        assert_eq!(json_string("a\"b"), "\"a\\\"b\"");
        assert_eq!(json_string("a\\b"), "\"a\\\\b\"");
        assert_eq!(json_string("a\nb"), "\"a\\nb\"");
    }

    #[test]
    fn test_markdown_output() {
        let page = make_test_page();
        let md = format_page(&page, OutputFormat::Markdown);
        assert_eq!(md, "Hi there");
    }

    #[test]
    fn test_multi_page_html() {
        let pages = vec![make_test_page(), make_test_page()];
        let html = format_pages(&pages, OutputFormat::Html);
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("Page 1"));
        assert!(html.contains("Page 2"));
    }

    #[test]
    fn test_multi_page_json() {
        let pages = vec![make_test_page()];
        let json = format_pages(&pages, OutputFormat::Json);
        assert!(json.contains("\"pages\""));
        assert!(json.contains("\"page_index\": 0"));
    }

    #[test]
    fn test_multi_page_markdown() {
        let pages = vec![make_test_page(), make_test_page()];
        let md = format_pages(&pages, OutputFormat::Markdown);
        assert!(md.contains("## Page 1"));
        assert!(md.contains("## Page 2"));
        assert!(md.contains("---"));
    }

    #[test]
    fn test_empty_page() {
        let page = PageText {
            page_index: 0,
            chars: Vec::new(),
            lines: Vec::new(),
            blocks: Vec::new(),
        };
        assert_eq!(format_page(&page, OutputFormat::PlainText), "");
        assert!(format_page(&page, OutputFormat::Html).contains("<div class=\"page\">"));
        assert!(format_page(&page, OutputFormat::Json).contains("\"blocks\": []"));
    }
}
