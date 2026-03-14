//! Folding ranges: #ifdef blocks, WGSL {} blocks, struct/function bodies.

use lsp_types::{FoldingRange, FoldingRangeKind};

/// Compute folding ranges for a document.
pub fn folding_ranges(source: &str) -> Vec<FoldingRange> {
    let mut ranges = Vec::new();

    // Track #ifdef/#if...#endif blocks
    let mut ifdef_stack: Vec<u32> = Vec::new();

    // Track { ... } blocks
    let mut brace_stack: Vec<u32> = Vec::new();

    for (line_num, line) in source.lines().enumerate() {
        let line_num = line_num as u32;
        let trimmed = line.trim();

        // Preprocessor folding
        if trimmed.starts_with("#ifdef")
            || trimmed.starts_with("#ifndef")
            || trimmed.starts_with("#if ")
        {
            ifdef_stack.push(line_num);
        } else if trimmed.starts_with("#endif") {
            if let Some(start) = ifdef_stack.pop() {
                if line_num > start {
                    ranges.push(FoldingRange {
                        start_line: start,
                        start_character: None,
                        end_line: line_num,
                        end_character: None,
                        kind: Some(FoldingRangeKind::Region),
                        collapsed_text: None,
                    });
                }
            }
        }

        // Brace folding
        for ch in line.chars() {
            if ch == '{' {
                brace_stack.push(line_num);
            } else if ch == '}' {
                if let Some(start) = brace_stack.pop() {
                    if line_num > start {
                        ranges.push(FoldingRange {
                            start_line: start,
                            start_character: None,
                            end_line: line_num,
                            end_character: None,
                            kind: None,
                            collapsed_text: None,
                        });
                    }
                }
            }
        }
    }

    ranges
}
