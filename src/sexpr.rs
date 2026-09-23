//! Minimal S-expression parser used by the inet DSL and future Lisp-y helpers.
//!
//! The parser accepts ASCII-friendly lists composed of bare symbols, quoted
//! string literals, and balanced parentheses. It intentionally keeps the
//! surface small—no dotted pairs or reader macros—so the DSL stays approachable
//! while remaining easy to extend.

use anyhow::{Result, bail};

/// S-expression representation used throughout the March codebase.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SExpr {
    Sym(String),
    Str(String),
    List(Vec<SExpr>),
}

/// Parse one or more S-expressions from `input`.
///
/// When the top-level form is `(seq ...)`, the list items inside are returned
/// directly to mirror the inet reducer expectations.
pub fn parse_sequence(input: &str) -> Result<Vec<SExpr>> {
    let mut tokens = tokenize(input)?;
    let mut forms = Vec::new();
    while !tokens.is_empty() {
        forms.push(parse_one(&mut tokens)?);
    }
    if forms.len() == 1 {
        if let SExpr::List(items) = &forms[0] {
            if let Some(SExpr::Sym(head)) = items.get(0) {
                if head == "seq" {
                    let mut seq = Vec::new();
                    for item in items.iter().skip(1) {
                        seq.push(item.clone());
                    }
                    return Ok(seq);
                }
            }
        }
    }
    Ok(forms)
}

/// Parse a single S-expression from `input`.
pub fn parse(input: &str) -> Result<SExpr> {
    let mut tokens = tokenize(input)?;
    let expr = parse_one(&mut tokens)?;
    if !tokens.is_empty() {
        bail!("unexpected tokens after S-expression");
    }
    Ok(expr)
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    LParen,
    RParen,
    Atom(String),
    Str(String),
}

fn tokenize(input: &str) -> Result<Vec<Token>> {
    let mut out = Vec::new();
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '(' => out.push(Token::LParen),
            ')' => out.push(Token::RParen),
            '"' => {
                let mut buf = String::new();
                while let Some(c) = chars.next() {
                    match c {
                        '\\' => {
                            let Some(esc) = chars.next() else {
                                bail!("incomplete escape sequence in string literal");
                            };
                            match esc {
                                'n' => buf.push('\n'),
                                'r' => buf.push('\r'),
                                't' => buf.push('\t'),
                                '\\' => buf.push('\\'),
                                '"' => buf.push('"'),
                                other => bail!("unsupported escape `\\{other}` in string literal"),
                            }
                        }
                        '"' => {
                            out.push(Token::Str(buf));
                            break;
                        }
                        other => buf.push(other),
                    }
                }
                if !matches!(out.last(), Some(Token::Str(_))) {
                    bail!("unterminated string literal");
                }
            }
            c if c.is_whitespace() => continue,
            other => {
                let mut buf = String::new();
                buf.push(other);
                while let Some(&next) = chars.peek() {
                    if next.is_whitespace() || next == '(' || next == ')' {
                        break;
                    }
                    buf.push(chars.next().unwrap());
                }
                out.push(Token::Atom(buf));
            }
        }
    }
    Ok(out)
}

fn parse_one(tokens: &mut Vec<Token>) -> Result<SExpr> {
    if tokens.is_empty() {
        bail!("unexpected EOF in s-expr");
    }
    let tok = tokens.remove(0);
    match tok {
        Token::LParen => {
            let mut items = Vec::new();
            while !tokens.is_empty() && tokens[0] != Token::RParen {
                items.push(parse_one(tokens)?);
            }
            if tokens.is_empty() {
                bail!("unbalanced parentheses");
            }
            tokens.remove(0); // consume ')'
            Ok(SExpr::List(items))
        }
        Token::RParen => bail!("unexpected ')' in s-expr"),
        Token::Atom(atom) => Ok(SExpr::Sym(atom)),
        Token::Str(text) => Ok(SExpr::Str(text)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_symbol() -> Result<()> {
        let expr = parse("alpha")?;
        assert_eq!(expr, SExpr::Sym("alpha".into()));
        Ok(())
    }

    #[test]
    fn parse_string_literal() -> Result<()> {
        let expr = parse(r#""hello world""#)?;
        assert_eq!(expr, SExpr::Str("hello world".into()));
        Ok(())
    }

    #[test]
    fn parse_nested_list() -> Result<()> {
        let expr = parse("(a (b c) d)")?;
        assert_eq!(
            expr,
            SExpr::List(vec![
                SExpr::Sym("a".into()),
                SExpr::List(vec![SExpr::Sym("b".into()), SExpr::Sym("c".into()),]),
                SExpr::Sym("d".into())
            ])
        );
        Ok(())
    }

    #[test]
    fn parse_escaped_string() -> Result<()> {
        let expr = parse(r#""line\n\"quoted\"""#)?;
        assert_eq!(expr, SExpr::Str("line\n\"quoted\"".into()));
        Ok(())
    }

    #[test]
    fn parse_sequence_unwraps_seq() -> Result<()> {
        let forms = parse_sequence("(seq (connect (A x) (B y)) (delete A B))")?;
        assert_eq!(forms.len(), 2);
        if let SExpr::List(first) = &forms[0] {
            assert_eq!(first.len(), 3);
        } else {
            panic!("expected list");
        }
        Ok(())
    }

    #[test]
    fn parse_sequence_multiple_top_level() -> Result<()> {
        let forms = parse_sequence("(connect (A x) (B y)) (delete A)")?;
        assert_eq!(forms.len(), 2);
        Ok(())
    }

    #[test]
    fn unterminated_string_errors() {
        let err = parse("\"oops").unwrap_err();
        assert!(err.to_string().contains("unterminated string"));
    }
}
