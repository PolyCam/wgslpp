use std::collections::HashMap;

/// Evaluate a preprocessor expression (for `#if` / `#elif`).
///
/// Grammar (recursive descent):
/// ```text
/// expr       = or_expr
/// or_expr    = and_expr ("||" and_expr)*
/// and_expr   = bitor_expr ("&&" bitor_expr)*
/// bitor_expr = cmp_expr ("|" cmp_expr)*
/// cmp_expr   = bitand_expr (("==" | "!=") bitand_expr)?
/// bitand_expr = unary_expr ("&" unary_expr)*
/// unary_expr = "!" unary_expr | primary
/// primary    = "defined" "(" IDENT ")" | "(" expr ")" | NUMBER | IDENT
/// ```
pub fn evaluate(expr: &str, defines: &HashMap<String, String>) -> Result<bool, String> {
    let tokens = tokenize(expr)?;
    let mut parser = Parser {
        tokens: &tokens,
        pos: 0,
        defines,
    };
    let val = parser.expr()?;
    if parser.pos < parser.tokens.len() {
        return Err(format!(
            "unexpected token '{}' after expression",
            parser.tokens[parser.pos]
        ));
    }
    Ok(val != 0)
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Ident(String),
    Number(i64),
    LParen,
    RParen,
    Not,
    And,
    Or,
    BitAnd,
    BitOr,
    Eq,
    Ne,
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::Ident(s) => write!(f, "{}", s),
            Token::Number(n) => write!(f, "{}", n),
            Token::LParen => write!(f, "("),
            Token::RParen => write!(f, ")"),
            Token::Not => write!(f, "!"),
            Token::And => write!(f, "&&"),
            Token::Or => write!(f, "||"),
            Token::BitAnd => write!(f, "&"),
            Token::BitOr => write!(f, "|"),
            Token::Eq => write!(f, "=="),
            Token::Ne => write!(f, "!="),
        }
    }
}

fn tokenize(input: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            ' ' | '\t' | '\r' | '\n' => i += 1,
            '(' => {
                tokens.push(Token::LParen);
                i += 1;
            }
            ')' => {
                tokens.push(Token::RParen);
                i += 1;
            }
            '!' if i + 1 < chars.len() && chars[i + 1] == '=' => {
                tokens.push(Token::Ne);
                i += 2;
            }
            '!' => {
                tokens.push(Token::Not);
                i += 1;
            }
            '&' if i + 1 < chars.len() && chars[i + 1] == '&' => {
                tokens.push(Token::And);
                i += 2;
            }
            '&' => {
                tokens.push(Token::BitAnd);
                i += 1;
            }
            '|' if i + 1 < chars.len() && chars[i + 1] == '|' => {
                tokens.push(Token::Or);
                i += 2;
            }
            '|' => {
                tokens.push(Token::BitOr);
                i += 1;
            }
            '=' if i + 1 < chars.len() && chars[i + 1] == '=' => {
                tokens.push(Token::Eq);
                i += 2;
            }
            c if c.is_ascii_digit() => {
                let start = i;
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
                let num: i64 = input[start..i]
                    .parse()
                    .map_err(|e| format!("invalid number: {}", e))?;
                tokens.push(Token::Number(num));
            }
            c if c.is_ascii_alphabetic() || c == '_' => {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                tokens.push(Token::Ident(input[start..i].to_string()));
            }
            c => return Err(format!("unexpected character '{}' in expression", c)),
        }
    }
    Ok(tokens)
}

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
    defines: &'a HashMap<String, String>,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<&Token> {
        let tok = self.tokens.get(self.pos);
        if tok.is_some() {
            self.pos += 1;
        }
        tok
    }

    fn expect(&mut self, expected: &Token) -> Result<(), String> {
        match self.advance() {
            Some(tok) if tok == expected => Ok(()),
            Some(tok) => Err(format!("expected '{}', got '{}'", expected, tok)),
            None => Err(format!("expected '{}', got end of expression", expected)),
        }
    }

    fn expr(&mut self) -> Result<i64, String> {
        self.or_expr()
    }

    fn or_expr(&mut self) -> Result<i64, String> {
        let mut val = self.and_expr()?;
        while self.peek() == Some(&Token::Or) {
            self.advance();
            let rhs = self.and_expr()?;
            val = if val != 0 || rhs != 0 { 1 } else { 0 };
        }
        Ok(val)
    }

    fn and_expr(&mut self) -> Result<i64, String> {
        let mut val = self.bitor_expr()?;
        while self.peek() == Some(&Token::And) {
            self.advance();
            let rhs = self.bitor_expr()?;
            val = if val != 0 && rhs != 0 { 1 } else { 0 };
        }
        Ok(val)
    }

    fn bitor_expr(&mut self) -> Result<i64, String> {
        let mut val = self.cmp_expr()?;
        while self.peek() == Some(&Token::BitOr) {
            self.advance();
            let rhs = self.cmp_expr()?;
            val |= rhs;
        }
        Ok(val)
    }

    fn cmp_expr(&mut self) -> Result<i64, String> {
        let val = self.bitand_expr()?;
        match self.peek() {
            Some(&Token::Eq) => {
                self.advance();
                let rhs = self.bitand_expr()?;
                Ok(if val == rhs { 1 } else { 0 })
            }
            Some(&Token::Ne) => {
                self.advance();
                let rhs = self.bitand_expr()?;
                Ok(if val != rhs { 1 } else { 0 })
            }
            _ => Ok(val),
        }
    }

    fn bitand_expr(&mut self) -> Result<i64, String> {
        let mut val = self.unary_expr()?;
        while self.peek() == Some(&Token::BitAnd) {
            self.advance();
            let rhs = self.unary_expr()?;
            val &= rhs;
        }
        Ok(val)
    }

    fn unary_expr(&mut self) -> Result<i64, String> {
        if self.peek() == Some(&Token::Not) {
            self.advance();
            let val = self.unary_expr()?;
            return Ok(if val == 0 { 1 } else { 0 });
        }
        self.primary()
    }

    fn primary(&mut self) -> Result<i64, String> {
        match self.advance().cloned() {
            Some(Token::Number(n)) => Ok(n),
            Some(Token::LParen) => {
                let val = self.expr()?;
                self.expect(&Token::RParen)?;
                Ok(val)
            }
            Some(Token::Ident(name)) if name == "defined" => {
                self.expect(&Token::LParen)?;
                match self.advance().cloned() {
                    Some(Token::Ident(ident)) => {
                        self.expect(&Token::RParen)?;
                        Ok(if self.defines.contains_key(&ident) {
                            1
                        } else {
                            0
                        })
                    }
                    _ => Err("expected identifier after 'defined('".into()),
                }
            }
            Some(Token::Ident(name)) => {
                // Look up as a define; undefined identifiers are 0
                match self.defines.get(&name) {
                    Some(val) if val.is_empty() => Ok(1), // flag define
                    Some(val) => val
                        .parse::<i64>()
                        .map_err(|_| format!("define '{}' has non-integer value '{}'", name, val)),
                    None => Ok(0),
                }
            }
            Some(tok) => Err(format!("unexpected token '{}' in expression", tok)),
            None => Err("unexpected end of expression".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defs(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn test_defined() {
        let d = defs(&[("FOO", "")]);
        assert!(evaluate("defined(FOO)", &d).unwrap());
        assert!(!evaluate("defined(BAR)", &d).unwrap());
    }

    #[test]
    fn test_logic() {
        let d = defs(&[("A", ""), ("B", "")]);
        assert!(evaluate("defined(A) && defined(B)", &d).unwrap());
        assert!(evaluate("defined(A) || defined(C)", &d).unwrap());
        assert!(!evaluate("defined(A) && defined(C)", &d).unwrap());
    }

    #[test]
    fn test_not() {
        let d = defs(&[]);
        assert!(evaluate("!defined(X)", &d).unwrap());
        assert!(!evaluate("!1", &d).unwrap());
    }

    #[test]
    fn test_comparison() {
        let d = defs(&[("X", "3")]);
        assert!(evaluate("X == 3", &d).unwrap());
        assert!(evaluate("X != 2", &d).unwrap());
        assert!(!evaluate("X == 2", &d).unwrap());
    }

    #[test]
    fn test_bitwise() {
        let d = defs(&[("X", "6")]);
        assert!(evaluate("X & 2", &d).unwrap());
        assert!(!evaluate("X & 1", &d).unwrap());
        assert!(evaluate("X | 1", &d).unwrap());
    }

    #[test]
    fn test_complex() {
        let d = defs(&[("A", ""), ("V", "2")]);
        assert!(evaluate("defined(A) && (V == 2 || V == 3)", &d).unwrap());
        assert!(!evaluate("defined(A) && (V == 1 || V == 3)", &d).unwrap());
    }
}
