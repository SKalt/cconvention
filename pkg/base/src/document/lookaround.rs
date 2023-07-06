use crop::Rope;

/// given a line/column position in the text, return the the byte offset of the position
pub(crate) fn find_byte_offset(text: &Rope, pos: lsp_types::Position) -> usize {
    let mut byte_offset: usize = 0;
    // only do the conversions once
    let line_index = pos.line as usize;
    let char_index = pos.character as usize;
    for (i, line) in text.raw_lines().enumerate() {
        // includes line breaks
        if i < line_index {
            byte_offset += line.byte_len();
            continue;
        } else {
            let line = line.to_string();
            for (i, c) in line.chars().enumerate() {
                if i >= char_index {
                    // don't include the target char in the byte-offset
                    return byte_offset;
                } else {
                    byte_offset += c.len_utf8();
                }
            }
        }
    }
    byte_offset
}

/// transform a line/column position into a tree-sitter Point struct
pub(crate) fn to_point(p: lsp_types::Position) -> tree_sitter::Point {
    tree_sitter::Point {
        row: p.line as usize,
        column: p.character as usize,
    }
}
