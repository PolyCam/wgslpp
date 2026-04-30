use std::collections::HashMap;

/// A preprocessor macro definition.
#[derive(Debug, Clone)]
pub enum MacroDef {
    /// Simple flag or text replacement: `#define NAME` or `#define NAME value`
    Object(String),
    /// Function-like macro: `#define NAME(a, b) body`
    Function { params: Vec<String>, body: String },
}

/// Parse a `#define` directive body (everything after `#define `).
/// Returns (name, MacroDef).
pub fn parse_define(rest: &str) -> Result<(String, MacroDef), String> {
    let rest = rest.trim();
    if rest.is_empty() {
        return Err("#define requires a name".into());
    }

    // Extract identifier
    let mut chars = rest.chars();
    let mut name = String::new();
    for ch in chars.by_ref() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            name.push(ch);
        } else {
            // Put back by using the remaining
            break;
        }
    }

    if name.is_empty() {
        return Err("#define requires a valid identifier".into());
    }

    let remaining: String = chars.collect();
    let after_name = &rest[name.len()..];

    // Check if function-like (parenthesis immediately after name, no space)
    if after_name.starts_with('(') {
        // Function-like macro
        let close = after_name
            .find(')')
            .ok_or_else(|| "unclosed parenthesis in #define".to_string())?;
        let params_str = &after_name[1..close];
        let params: Vec<String> = if params_str.trim().is_empty() {
            Vec::new()
        } else {
            params_str.split(',').map(|s| s.trim().to_string()).collect()
        };
        let body = after_name[close + 1..].trim().to_string();
        Ok((name, MacroDef::Function { params, body }))
    } else {
        // Object-like macro
        let value = remaining.trim().to_string();
        Ok((name, MacroDef::Object(value)))
    }
}

/// Expand macros in a line of text.
pub fn expand_macros(line: &str, defines: &HashMap<String, MacroDef>) -> String {
    let mut result = line.to_string();
    let mut iterations = 0;
    const MAX_ITERATIONS: usize = 100;

    loop {
        let mut changed = false;
        iterations += 1;
        if iterations > MAX_ITERATIONS {
            break; // prevent infinite expansion
        }

        for (name, def) in defines {
            match def {
                MacroDef::Object(value) => {
                    if let Some(new) = replace_word(&result, name, value) {
                        result = new;
                        changed = true;
                    }
                }
                MacroDef::Function { params, body } => {
                    if let Some(new) = expand_function_macro(&result, name, params, body) {
                        result = new;
                        changed = true;
                    }
                }
            }
        }

        if !changed {
            break;
        }
    }

    result
}

/// Replace whole-word occurrences of `name` with `replacement`.
/// Handles multi-byte UTF-8 safely (macro names are always ASCII).
fn replace_word(input: &str, name: &str, replacement: &str) -> Option<String> {
    let mut result = String::new();
    let mut found = false;
    let mut i = 0;
    let bytes = input.as_bytes();
    let name_bytes = name.as_bytes();

    while i < bytes.len() {
        // Non-ASCII byte: skip the entire multi-byte character
        if bytes[i] >= 0x80 {
            let start = i;
            i += 1;
            while i < bytes.len() && (bytes[i] & 0xC0) == 0x80 {
                i += 1;
            }
            result.push_str(&input[start..i]);
            continue;
        }

        if i + name_bytes.len() <= bytes.len()
            && bytes[i..i + name_bytes.len()] == *name_bytes
        {
            // Check word boundaries
            let before_ok = i == 0
                || !(bytes[i - 1]).is_ascii_alphanumeric() && bytes[i - 1] != b'_';
            let after_idx = i + name_bytes.len();
            let after_ok = after_idx == bytes.len()
                || !bytes[after_idx].is_ascii_alphanumeric() && bytes[after_idx] != b'_';
            if before_ok && after_ok {
                result.push_str(replacement);
                i += name_bytes.len();
                found = true;
                continue;
            }
        }
        result.push(bytes[i] as char);
        i += 1;
    }

    if found {
        Some(result)
    } else {
        None
    }
}

/// Try to expand a function-like macro invocation in the input.
fn expand_function_macro(
    input: &str,
    name: &str,
    params: &[String],
    body: &str,
) -> Option<String> {
    // Find `name(` as a whole word
    let search = format!("{}(", name);
    let pos = input.find(&search)?;

    // Check word boundary before
    if pos > 0 {
        let before = input.as_bytes()[pos - 1];
        if (before as char).is_ascii_alphanumeric() || before == b'_' {
            return None;
        }
    }

    // Parse arguments (handle nested parens)
    let args_start = pos + name.len() + 1;
    let mut depth = 1;
    let mut args = Vec::new();
    let mut current_arg = String::new();
    let _args_byte_start = args_start;
    let chars: Vec<char> = input.chars().collect();

    // Recount position in chars
    let mut char_pos = 0;
    let mut byte_pos = 0;
    while byte_pos < args_start {
        byte_pos += chars[char_pos].len_utf8();
        char_pos += 1;
    }

    while char_pos < chars.len() {
        let ch = chars[char_pos];
        match ch {
            '(' => {
                depth += 1;
                current_arg.push(ch);
            }
            ')' => {
                depth -= 1;
                if depth == 0 {
                    args.push(current_arg.trim().to_string());
                    char_pos += 1;
                    break;
                }
                current_arg.push(ch);
            }
            ',' if depth == 1 => {
                args.push(current_arg.trim().to_string());
                current_arg = String::new();
            }
            _ => {
                current_arg.push(ch);
            }
        }
        char_pos += 1;
    }

    if depth != 0 {
        return None; // unclosed
    }

    if args.len() != params.len() {
        return None; // wrong arity
    }

    // Substitute parameters in body
    let mut expanded = body.to_string();
    for (param, arg) in params.iter().zip(args.iter()) {
        if let Some(new) = replace_word(&expanded, param, arg) {
            expanded = new;
        }
    }

    // Reconstruct: before + expanded + after
    let end_byte: usize = chars[..char_pos].iter().map(|c| c.len_utf8()).sum();
    let mut result = input[..pos].to_string();
    result.push_str(&expanded);
    result.push_str(&input[end_byte..]);

    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_define_flag() {
        let (name, def) = parse_define("FOO").unwrap();
        assert_eq!(name, "FOO");
        matches!(def, MacroDef::Object(v) if v.is_empty());
    }

    #[test]
    fn test_parse_define_value() {
        let (name, def) = parse_define("FOO 42").unwrap();
        assert_eq!(name, "FOO");
        match def {
            MacroDef::Object(v) => assert_eq!(v, "42"),
            _ => panic!("expected Object"),
        }
    }

    #[test]
    fn test_parse_define_function() {
        let (name, def) = parse_define("MAX(a, b) ((a) > (b) ? (a) : (b))").unwrap();
        assert_eq!(name, "MAX");
        match def {
            MacroDef::Function { params, body } => {
                assert_eq!(params, vec!["a", "b"]);
                assert_eq!(body, "((a) > (b) ? (a) : (b))");
            }
            _ => panic!("expected Function"),
        }
    }

    #[test]
    fn test_expand_object_macro() {
        let mut defs = HashMap::new();
        defs.insert("PI".to_string(), MacroDef::Object("3.14159".to_string()));
        let result = expand_macros("let x = PI;", &defs);
        assert_eq!(result, "let x = 3.14159;");
    }

    #[test]
    fn test_expand_function_macro() {
        let mut defs = HashMap::new();
        defs.insert(
            "MAX".to_string(),
            MacroDef::Function {
                params: vec!["a".to_string(), "b".to_string()],
                body: "((a) > (b) ? (a) : (b))".to_string(),
            },
        );
        let result = expand_macros("let x = MAX(1, 2);", &defs);
        assert_eq!(result, "let x = ((1) > (2) ? (1) : (2));");
    }

    #[test]
    fn test_word_boundary() {
        let mut defs = HashMap::new();
        defs.insert("A".to_string(), MacroDef::Object("1".to_string()));
        // Should not replace A inside ALPHA
        let result = expand_macros("let ALPHA = A;", &defs);
        assert_eq!(result, "let ALPHA = 1;");
    }

    #[test]
    fn test_unicode_in_comments() {
        // Box-drawing characters (3-byte UTF-8) must not cause panics
        let mut defs = HashMap::new();
        defs.insert("FOO".to_string(), MacroDef::Object("bar".to_string()));
        let result = expand_macros("// ── Section ─────────────────", &defs);
        assert_eq!(result, "// ── Section ─────────────────");
    }

    #[test]
    fn test_macro_replacement_near_unicode() {
        // Macro appears right after multi-byte characters
        let mut defs = HashMap::new();
        defs.insert("X".to_string(), MacroDef::Object("42".to_string()));
        let result = expand_macros("// ── X", &defs);
        assert_eq!(result, "// ── 42");
    }

    #[test]
    fn test_macro_replacement_between_unicode() {
        let mut defs = HashMap::new();
        defs.insert("VAL".to_string(), MacroDef::Object("99".to_string()));
        let result = expand_macros("«VAL»", &defs);
        assert_eq!(result, "«99»");
    }

    #[test]
    fn test_unicode_no_false_word_boundary() {
        // Emoji and CJK characters adjacent to identifiers
        let mut defs = HashMap::new();
        defs.insert("A".to_string(), MacroDef::Object("1".to_string()));
        let result = expand_macros("let 日本 = A;", &defs);
        assert_eq!(result, "let 日本 = 1;");
    }
}
