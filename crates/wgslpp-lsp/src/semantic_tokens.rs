//! Semantic token provider — highlights types, functions, variables, constants,
//! preprocessor directives, and inactive #ifdef regions.

use lsp_types::{
    SemanticToken, SemanticTokenType, SemanticTokensLegend,
};

/// Our semantic token types.
pub const TOKEN_TYPES: &[SemanticTokenType] = &[
    SemanticTokenType::FUNCTION,       // 0
    SemanticTokenType::TYPE,           // 1
    SemanticTokenType::VARIABLE,       // 2
    SemanticTokenType::PARAMETER,      // 3
    SemanticTokenType::KEYWORD,        // 4
    SemanticTokenType::COMMENT,        // 5
    SemanticTokenType::MACRO,          // 6 - preprocessor directives
    SemanticTokenType::STRING,         // 7 - include paths
    SemanticTokenType::NUMBER,         // 8
    SemanticTokenType::DECORATOR,      // 9 - attributes (@vertex, @group, etc.)
];

pub fn legend() -> SemanticTokensLegend {
    SemanticTokensLegend {
        token_types: TOKEN_TYPES.to_vec(),
        token_modifiers: Vec::new(),
    }
}

/// Compute semantic tokens for a document.
pub fn semantic_tokens(source: &str) -> Vec<SemanticToken> {
    let mut tokens = Vec::new();
    let mut prev_line = 0u32;
    let mut prev_start = 0u32;

    for (line_num, line) in source.lines().enumerate() {
        let line_num = line_num as u32;
        let trimmed = line.trim();

        // Preprocessor directives
        if trimmed.starts_with('#') {
            let start = line.find('#').unwrap_or(0) as u32;
            // Find end of directive keyword
            let directive_end = trimmed[1..]
                .find(|c: char| c == ' ' || c == '\t' || c == '(')
                .map(|i| i + 1)
                .unwrap_or(trimmed.len());

            push_token(
                &mut tokens,
                &mut prev_line,
                &mut prev_start,
                line_num,
                start,
                directive_end as u32,
                6, // MACRO
            );

            // If #include, highlight the path as STRING
            if trimmed.starts_with("#include") {
                if let Some(path_start) = line.find('"').or_else(|| line.find('<')) {
                    let path_end = if line.as_bytes()[path_start] == b'"' {
                        line[path_start + 1..].find('"').map(|i| path_start + 2 + i)
                    } else {
                        line[path_start + 1..].find('>').map(|i| path_start + 2 + i)
                    };
                    if let Some(end) = path_end {
                        push_token(
                            &mut tokens,
                            &mut prev_line,
                            &mut prev_start,
                            line_num,
                            path_start as u32,
                            (end - path_start) as u32,
                            7, // STRING
                        );
                    }
                }
            }
            continue;
        }

        // Attributes (@vertex, @fragment, @group, @binding, @location, @builtin)
        let mut pos = 0;
        while pos < line.len() {
            if line.as_bytes()[pos] == b'@' {
                let attr_start = pos;
                pos += 1;
                while pos < line.len()
                    && (line.as_bytes()[pos].is_ascii_alphanumeric()
                        || line.as_bytes()[pos] == b'_')
                {
                    pos += 1;
                }
                let attr_len = pos - attr_start;
                if attr_len > 1 {
                    push_token(
                        &mut tokens,
                        &mut prev_line,
                        &mut prev_start,
                        line_num,
                        attr_start as u32,
                        attr_len as u32,
                        9, // DECORATOR
                    );
                }
            } else {
                pos += 1;
            }
        }
    }

    tokens
}

fn push_token(
    tokens: &mut Vec<SemanticToken>,
    prev_line: &mut u32,
    prev_start: &mut u32,
    line: u32,
    start: u32,
    length: u32,
    token_type: u32,
) {
    let delta_line = line - *prev_line;
    let delta_start = if delta_line == 0 {
        start - *prev_start
    } else {
        start
    };

    tokens.push(SemanticToken {
        delta_line,
        delta_start,
        length,
        token_type,
        token_modifiers_bitset: 0,
    });

    *prev_line = line;
    *prev_start = start;
}
