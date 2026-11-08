//! Lexer for the yatare JS subset. Hand-rolled (repo has zero dependencies).

use super::{JsError, err};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tok {
    Int(i32),
    Str(String),
    Ident(String),
    Kw(&'static str),
    P(&'static str),
    Eof,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub tok: Tok,
    pub line: usize,
}

impl Tok {
    pub fn describe(&self) -> String {
        match self {
            Tok::Int(v) => format!("`{v}`"),
            Tok::Str(s) => format!("`\"{s}\"`"),
            Tok::Ident(s) => format!("identifier `{s}`"),
            Tok::Kw(k) => format!("keyword `{k}`"),
            Tok::P(p) => format!("`{p}`"),
            Tok::Eof => "end of file".to_string(),
        }
    }
}

const KEYWORDS: &[&str] = &[
    "var", "let", "const", "function", "if", "else", "while", "return", "true", "false",
];

/// Two-char punctuation must be matched before one-char ( `+` vs `+=` ).
const PUNCT2: &[&str] = &["+=", "-=", "==", "!=", "<=", ">=", "&&", "||"];
const PUNCT1: &[&str] = &[
    "+", "-", "*", "^", "<", ">", "!", "=", "(", ")", "{", "}", "[", "]", ";", ",", ".", "~",
];

pub fn lex(src: &str) -> Result<Vec<Token>, JsError> {
    let b: Vec<char> = src.chars().collect();
    let mut i = 0usize;
    let mut line = 1usize;
    let mut out = Vec::new();

    while i < b.len() {
        let c = b[i];
        if c == '\n' {
            line += 1;
            i += 1;
            continue;
        }
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        // comments
        if c == '/' && b.get(i + 1) == Some(&'/') {
            while i < b.len() && b[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c == '/' && b.get(i + 1) == Some(&'*') {
            i += 2;
            let mut closed = false;
            while i < b.len() {
                if b[i] == '\n' {
                    line += 1;
                }
                if b[i] == '*' && b.get(i + 1) == Some(&'/') {
                    i += 2;
                    closed = true;
                    break;
                }
                i += 1;
            }
            if !closed {
                return err(line, "unterminated block comment");
            }
            continue;
        }
        if c == '/' {
            return err(
                line,
                "unsupported `/`: the VM has no division instruction in v0",
            );
        }
        // numbers
        if c.is_ascii_digit() {
            if c == '0' && matches!(b.get(i + 1), Some('x') | Some('X')) {
                i += 2;
                let digits_start = i;
                while i < b.len() && b[i].is_ascii_hexdigit() {
                    i += 1;
                }
                if i == digits_start {
                    return err(line, "hex literal needs at least one digit");
                }
                let text: String = b[digits_start..i].iter().collect();
                let v = u32::from_str_radix(&text, 16).map_err(|_| JsError {
                    line,
                    msg: format!("hex literal `0x{text}` out of range"),
                })? as i32;
                out.push(Token {
                    tok: Tok::Int(v),
                    line,
                });
                continue;
            }
            let start = i;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            if b.get(i) == Some(&'.') {
                return err(
                    line,
                    "floating point literals are unsupported (the VM is i32)",
                );
            }
            let text: String = b[start..i].iter().collect();
            let v = text.parse::<i32>().map_err(|_| JsError {
                line,
                msg: format!("integer literal `{text}` out of i32 range"),
            })?;
            out.push(Token {
                tok: Tok::Int(v),
                line,
            });
            continue;
        }
        // strings (no escapes in v0)
        if c == '"' || c == '\'' {
            i += 1;
            let mut s = String::new();
            let mut closed = false;
            while i < b.len() {
                let ch = b[i];
                if ch == '\n' {
                    break;
                }
                if ch == '\\' {
                    return err(line, "string escapes are unsupported");
                }
                if ch == c {
                    closed = true;
                    i += 1;
                    break;
                }
                s.push(ch);
                i += 1;
            }
            if !closed {
                return err(line, "unterminated string literal");
            }
            out.push(Token {
                tok: Tok::Str(s),
                line,
            });
            continue;
        }
        // identifiers / keywords
        if c.is_ascii_alphabetic() || c == '_' || c == '$' {
            let start = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == '_' || b[i] == '$') {
                i += 1;
            }
            let text: String = b[start..i].iter().collect();
            let tok = match KEYWORDS.iter().find(|&&k| k == text) {
                Some(k) => Tok::Kw(k),
                None => Tok::Ident(text),
            };
            out.push(Token { tok, line });
            continue;
        }
        // punctuation, longest match first
        let two: String = b[i..(i + 2).min(b.len())].iter().collect();
        if let Some(p) = PUNCT2.iter().find(|p| **p == two) {
            out.push(Token {
                tok: Tok::P(p),
                line,
            });
            i += 2;
            continue;
        }
        let one = c.to_string();
        if let Some(p) = PUNCT1.iter().find(|p| **p == one) {
            out.push(Token {
                tok: Tok::P(p),
                line,
            });
            i += 1;
            continue;
        }
        return err(line, format!("unexpected character `{c}`"));
    }
    out.push(Token {
        tok: Tok::Eof,
        line,
    });
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(src: &str) -> Vec<Tok> {
        lex(src).unwrap().into_iter().map(|t| t.tok).collect()
    }

    #[test]
    fn basic_tokens() {
        assert_eq!(
            toks("var x = 0x10 + 12;"),
            vec![
                Tok::Kw("var"),
                Tok::Ident("x".into()),
                Tok::P("="),
                Tok::Int(16),
                Tok::P("+"),
                Tok::Int(12),
                Tok::P(";"),
                Tok::Eof,
            ]
        );
    }

    #[test]
    fn two_char_punct_wins() {
        assert_eq!(
            toks("a += 1 && 2 || b != c"),
            vec![
                Tok::Ident("a".into()),
                Tok::P("+="),
                Tok::Int(1),
                Tok::P("&&"),
                Tok::Int(2),
                Tok::P("||"),
                Tok::Ident("b".into()),
                Tok::P("!="),
                Tok::Ident("c".into()),
                Tok::Eof,
            ]
        );
    }

    #[test]
    fn dollar_identifiers() {
        assert_eq!(
            toks(r##"$("#banner").css("left", 1)"##),
            vec![
                Tok::Ident("$".into()),
                Tok::P("("),
                Tok::Str("#banner".into()),
                Tok::P(")"),
                Tok::P("."),
                Tok::Ident("css".into()),
                Tok::P("("),
                Tok::Str("left".into()),
                Tok::P(","),
                Tok::Int(1),
                Tok::P(")"),
                Tok::Eof,
            ]
        );
    }

    #[test]
    fn comments_are_skipped_and_lines_track() {
        let ts = lex("// hi\n/* multi\nline */\nx").unwrap();
        assert_eq!(ts[0].tok, Tok::Ident("x".into()));
        assert_eq!(ts[0].line, 4); // block comment spans lines 2-3
    }

    #[test]
    fn lexer_errors() {
        for (src, want) in [
            ("1.5", "floating point"),
            ("a / b", "division"),
            ("\"abc", "unterminated string"),
            ("99999999999", "out of i32 range"),
            ("@", "unexpected character"),
            ("/* x", "unterminated block"),
        ] {
            let e = lex(src).expect_err(src);
            assert!(
                e.msg.contains(want),
                "src={src:?}: got {:?}, want {want:?}",
                e.msg
            );
        }
    }
}
