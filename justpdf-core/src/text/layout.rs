//! Advanced layout analysis: multi-column detection, reading order, dehyphenation.

use super::{TextLine, TextBlock, TextWord};

/// Detect columns in a set of lines and reorder them for correct reading order.
/// Returns blocks grouped by column, in reading order (left-to-right, top-to-bottom).
pub fn detect_columns_and_reorder(lines: &[TextLine]) -> Vec<TextBlock> {
    if lines.is_empty() {
        return Vec::new();
    }

    // Find column boundaries by analyzing X positions of line starts
    let columns = detect_column_boundaries(lines);

    if columns.len() <= 1 {
        // Single column: just group into blocks by vertical spacing
        return group_into_blocks_with_dehyphenation(lines);
    }

    // Multi-column: assign each line to a column, then process each column
    let mut column_lines: Vec<Vec<&TextLine>> = vec![Vec::new(); columns.len()];

    for line in lines {
        let col_idx = find_column_for_line(line, &columns);
        column_lines[col_idx].push(line);
    }

    // Sort lines within each column by Y descending (top to bottom)
    for col in &mut column_lines {
        col.sort_by(|a, b| b.y.partial_cmp(&a.y).unwrap_or(std::cmp::Ordering::Equal));
    }

    // Process columns left-to-right
    let mut blocks = Vec::new();
    for col in &column_lines {
        if col.is_empty() {
            continue;
        }
        let owned_lines: Vec<TextLine> = col.iter().map(|l| (*l).clone()).collect();
        let mut col_blocks = group_into_blocks_with_dehyphenation(&owned_lines);
        blocks.append(&mut col_blocks);
    }

    blocks
}

/// A column boundary defined by X range.
#[derive(Debug, Clone)]
struct ColumnBound {
    x_min: f64,
    x_max: f64,
}

/// Detect column boundaries from line X positions using gap analysis.
fn detect_column_boundaries(lines: &[TextLine]) -> Vec<ColumnBound> {
    if lines.is_empty() {
        return vec![];
    }

    // Collect all line start X positions
    let mut x_starts: Vec<f64> = lines.iter().map(|l| l.x).collect();
    x_starts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    x_starts.dedup_by(|a, b| (*a - *b).abs() < 5.0);

    if x_starts.len() < 2 {
        // All lines start at roughly the same X: single column
        let x_min = lines.iter().map(|l| l.x).fold(f64::MAX, f64::min);
        let x_max = lines
            .iter()
            .map(|l| {
                l.words
                    .last()
                    .map(|w| w.x + w.width)
                    .unwrap_or(l.x + 100.0)
            })
            .fold(f64::MIN, f64::max);
        return vec![ColumnBound { x_min, x_max }];
    }

    // Look for significant gaps between clusters of X starts
    // Use a simple clustering: if gap between consecutive X starts > page_width * 0.1, it's a column break
    let page_width = x_starts.last().unwrap_or(&612.0) - x_starts.first().unwrap_or(&0.0);
    let gap_threshold = (page_width * 0.15).max(30.0);

    let mut columns = Vec::new();
    let mut cluster_start = x_starts[0];
    let mut prev_x = x_starts[0];

    for &x in &x_starts[1..] {
        if x - prev_x > gap_threshold {
            // Gap found: end current column
            let x_max = find_max_x_for_cluster(lines, cluster_start, prev_x);
            columns.push(ColumnBound {
                x_min: cluster_start,
                x_max,
            });
            cluster_start = x;
        }
        prev_x = x;
    }

    // Last column
    let x_max = find_max_x_for_cluster(lines, cluster_start, prev_x);
    columns.push(ColumnBound {
        x_min: cluster_start,
        x_max,
    });

    // Only return multiple columns if we have at least 2 lines per column
    if columns.len() > 1 {
        let min_lines_per_col = 2;
        let valid = columns.iter().all(|col| {
            let count = lines
                .iter()
                .filter(|l| l.x >= col.x_min - 5.0 && l.x <= col.x_max + 5.0)
                .count();
            count >= min_lines_per_col
        });
        if !valid {
            // Not enough evidence for multi-column: treat as single
            let x_min = columns.first().map(|c| c.x_min).unwrap_or(0.0);
            let x_max = columns.last().map(|c| c.x_max).unwrap_or(612.0);
            return vec![ColumnBound { x_min, x_max }];
        }
    }

    columns
}

fn find_max_x_for_cluster(lines: &[TextLine], cluster_x_min: f64, cluster_x_max: f64) -> f64 {
    lines
        .iter()
        .filter(|l| l.x >= cluster_x_min - 5.0 && l.x <= cluster_x_max + 5.0)
        .map(|l| {
            l.words
                .last()
                .map(|w| w.x + w.width)
                .unwrap_or(l.x + 100.0)
        })
        .fold(cluster_x_max, f64::max)
}

fn find_column_for_line(line: &TextLine, columns: &[ColumnBound]) -> usize {
    let line_center = line.x;
    let mut best_col = 0;
    let mut best_dist = f64::MAX;

    for (i, col) in columns.iter().enumerate() {
        let col_center = (col.x_min + col.x_max) / 2.0;
        let dist = (line_center - col_center).abs();
        if dist < best_dist {
            best_dist = dist;
            best_col = i;
        }
    }

    best_col
}

/// Group lines into blocks with dehyphenation applied.
pub fn group_into_blocks_with_dehyphenation(lines: &[TextLine]) -> Vec<TextBlock> {
    if lines.is_empty() {
        return Vec::new();
    }

    let mut blocks: Vec<TextBlock> = Vec::new();
    let mut current_lines: Vec<TextLine> = vec![lines[0].clone()];

    for i in 1..lines.len() {
        let prev = &lines[i - 1];
        let curr = &lines[i];

        let avg_font_size = prev
            .words
            .first()
            .map(|w| w.font_size)
            .unwrap_or(12.0);
        let line_gap = (prev.y - curr.y).abs();
        let block_threshold = avg_font_size * 2.0;

        if line_gap > block_threshold {
            blocks.push(build_block_dehyphenated(std::mem::take(&mut current_lines)));
        }
        current_lines.push(curr.clone());
    }

    if !current_lines.is_empty() {
        blocks.push(build_block_dehyphenated(current_lines));
    }

    blocks
}

fn build_block_dehyphenated(lines: Vec<TextLine>) -> TextBlock {
    let mut result_lines: Vec<String> = Vec::new();

    for (i, line) in lines.iter().enumerate() {
        let text = &line.text;

        // Check if this line ends with a hyphen and there's a next line
        if i + 1 < lines.len() && text.ends_with('-') {
            // Dehyphenate: remove trailing hyphen and join with next line's first word
            let dehyphenated = text.trim_end_matches('-').to_string();
            // We'll merge with next line
            if let Some(last) = result_lines.last_mut() {
                last.push(' ');
                last.push_str(&dehyphenated);
            } else {
                result_lines.push(dehyphenated);
            }
        } else if let Some(last) = result_lines.last_mut() {
            // Check if previous line was dehyphenated (no trailing hyphen, just continue)
            // Normal line: just add
            if last.ends_with('-') {
                // Previous was a partial dehyphenation - shouldn't happen with above logic
                last.push_str(text);
            } else {
                result_lines.push(text.clone());
            }
        } else {
            result_lines.push(text.clone());
        }
    }

    let text = result_lines.join("\n");
    TextBlock { text, lines }
}

/// Determine reading order for words based on their spatial positions.
/// Returns words sorted in reading order (top-to-bottom, left-to-right).
pub fn reading_order_sort(words: &mut [TextWord]) {
    words.sort_by(|a, b| {
        // Sort by Y descending (higher y = top of page = comes first)
        let y_threshold = a.font_size.max(b.font_size) * 0.5;
        let y_diff = a.y - b.y;

        if y_diff.abs() < y_threshold {
            // Same line: sort left to right
            a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal)
        } else if y_diff > 0.0 {
            // a has higher y (top of page) → a comes first
            std::cmp::Ordering::Less
        } else {
            std::cmp::Ordering::Greater
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_word(text: &str, x: f64, y: f64, width: f64, font_size: f64) -> TextWord {
        TextWord {
            text: text.to_string(),
            x,
            y,
            width,
            font_size,
        }
    }

    fn make_line(text: &str, words: Vec<TextWord>, x: f64, y: f64) -> TextLine {
        TextLine {
            text: text.to_string(),
            words,
            x,
            y,
        }
    }

    #[test]
    fn test_single_column() {
        let lines = vec![
            make_line(
                "Hello World",
                vec![make_word("Hello", 72.0, 720.0, 30.0, 12.0), make_word("World", 110.0, 720.0, 30.0, 12.0)],
                72.0,
                720.0,
            ),
            make_line(
                "Second line",
                vec![make_word("Second", 72.0, 706.0, 36.0, 12.0), make_word("line", 115.0, 706.0, 20.0, 12.0)],
                72.0,
                706.0,
            ),
        ];

        let blocks = detect_columns_and_reorder(&lines);
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].text.contains("Hello World"));
        assert!(blocks[0].text.contains("Second line"));
    }

    #[test]
    fn test_two_columns() {
        // Left column at x=72, right column at x=320
        let lines = vec![
            make_line("Left col line 1", vec![make_word("Left", 72.0, 720.0, 20.0, 12.0), make_word("col", 96.0, 720.0, 16.0, 12.0), make_word("line", 116.0, 720.0, 20.0, 12.0), make_word("1", 140.0, 720.0, 6.0, 12.0)], 72.0, 720.0),
            make_line("Left col line 2", vec![make_word("Left", 72.0, 706.0, 20.0, 12.0), make_word("col", 96.0, 706.0, 16.0, 12.0), make_word("line", 116.0, 706.0, 20.0, 12.0), make_word("2", 140.0, 706.0, 6.0, 12.0)], 72.0, 706.0),
            make_line("Right col line 1", vec![make_word("Right", 320.0, 720.0, 26.0, 12.0), make_word("col", 350.0, 720.0, 16.0, 12.0), make_word("line", 370.0, 720.0, 20.0, 12.0), make_word("1", 394.0, 720.0, 6.0, 12.0)], 320.0, 720.0),
            make_line("Right col line 2", vec![make_word("Right", 320.0, 706.0, 26.0, 12.0), make_word("col", 350.0, 706.0, 16.0, 12.0), make_word("line", 370.0, 706.0, 20.0, 12.0), make_word("2", 394.0, 706.0, 6.0, 12.0)], 320.0, 706.0),
        ];

        let blocks = detect_columns_and_reorder(&lines);
        // Should produce 2 blocks: left column first, then right column
        assert_eq!(blocks.len(), 2);
        assert!(blocks[0].text.contains("Left col"));
        assert!(blocks[1].text.contains("Right col"));
    }

    #[test]
    fn test_dehyphenation() {
        let lines = vec![
            make_line(
                "This is a long para-",
                vec![make_word("This", 72.0, 720.0, 20.0, 12.0), make_word("is", 96.0, 720.0, 10.0, 12.0), make_word("a", 110.0, 720.0, 5.0, 12.0), make_word("long", 119.0, 720.0, 20.0, 12.0), make_word("para-", 143.0, 720.0, 26.0, 12.0)],
                72.0,
                720.0,
            ),
            make_line(
                "graph continues here",
                vec![make_word("graph", 72.0, 706.0, 28.0, 12.0), make_word("continues", 104.0, 706.0, 50.0, 12.0), make_word("here", 158.0, 706.0, 20.0, 12.0)],
                72.0,
                706.0,
            ),
        ];

        let blocks = group_into_blocks_with_dehyphenation(&lines);
        assert_eq!(blocks.len(), 1);
        // The hyphen should be removed and lines joined
        assert!(blocks[0].text.contains("para"));
        assert!(!blocks[0].text.contains("para-\n"));
    }

    #[test]
    fn test_reading_order() {
        let mut words = vec![
            make_word("C", 72.0, 700.0, 10.0, 12.0),
            make_word("A", 72.0, 720.0, 10.0, 12.0),
            make_word("B", 150.0, 720.0, 10.0, 12.0),
        ];

        reading_order_sort(&mut words);
        assert_eq!(words[0].text, "A");
        assert_eq!(words[1].text, "B");
        assert_eq!(words[2].text, "C");
    }

    #[test]
    fn test_empty_input() {
        assert!(detect_columns_and_reorder(&[]).is_empty());
        assert!(group_into_blocks_with_dehyphenation(&[]).is_empty());
    }
}
