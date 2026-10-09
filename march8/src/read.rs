//! The reader for the explicit form (docs/MACHINE.md): words separated by
//! white space, as in FORTH, with numbers, strings, `.` and four brackets.

use crate::error::{Error, Kind, Pos, Result};
use crate::symbols::Symbols;
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
    /// An integer literal.
    Int(i128),
    /// A decimal literal: its digits, and how many follow the point.
    Dec(i128, u32),
    Str(Rc<str>),
    Name(Rc<str>),
    /// `.`, which applies the value on top.
    Apply,
    /// `[`, `(`, `{` or `<`.
    Open(u8),
    /// `]`, `)`, `}` or `>`.
    Close(u8),
    /// `--` inside `< >`, between inputs and outputs.
    Dashes,
    /// A string with holes: text and code, in order.
    Template(Rc<[Piece]>),
}

/// A part of a string with holes.
#[derive(Clone, Debug, PartialEq)]
pub enum Piece {
    Text(Rc<str>),
    /// `\[ code ]`, whose value is written in; `\_` is `\[ _. ]`.
    Hole(Rc<[Token]>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    pub tok: Tok,
    pub pos: Pos,
}

fn closer(open: u8) -> u8 {
    match open {
        b'[' => b']',
        b'(' => b')',
        b'{' => b'}',
        _ => b'>',
    }
}

/// Reads source into tokens, checking that brackets nest. `--` outside
/// `< >` starts a comment that runs to the end of the line.
pub fn read(src: &str, symbols: &Symbols) -> Result<Vec<Token>> {
    let mut r = Reader {
        src,
        at: 0,
        line: 1,
        col: 1,
    };
    tokens(&mut r, symbols, None)
}

/// Reads tokens to the end, or, for a hole opened at `hole`, to the `]` that
/// closes it: one where a word would start, with the hole's own brackets
/// closed (march7/docs/STRINGS.md).
fn tokens(r: &mut Reader, symbols: &Symbols, hole: Option<Pos>) -> Result<Vec<Token>> {
    let mut out = Vec::new();
    let mut open: Vec<(u8, Pos)> = Vec::new();
    loop {
        r.skip_space();
        let Some(c) = r.peek() else {
            if let Some(p) = hole {
                return Err(Error::new(
                    Kind::Unfinished,
                    Some(p),
                    "a hole is never closed",
                ));
            }
            break;
        };
        let pos = r.pos();
        if hole.is_some() && open.is_empty() && c == ']' {
            r.bump();
            if out.is_empty() {
                return Err(Error::new(Kind::Syntax, hole, "a hole holds no code"));
            }
            return Ok(out);
        }
        if c == '"' {
            let tok = r.string(symbols, hole.is_some() && open.is_empty())?;
            out.push(Token { tok, pos });
            continue;
        }
        let word = r.word();
        if word.len() > 1 && word.starts_with('\'') {
            // A raw string runs to the next `'`, spaces and all.
            let s = r.raw(word, pos)?;
            out.push(Token {
                tok: Tok::Str(s.into()),
                pos,
            });
            continue;
        }
        let in_bracket = open.iter().any(|(b, _)| *b == b'<');
        if word == "--" && in_bracket {
            out.push(Token {
                tok: Tok::Dashes,
                pos,
            });
            continue;
        }
        if word.starts_with("--") {
            r.skip_line();
            continue;
        }
        // Each dot at the end of a word applies it: `sq.` is `sq .`, and
        // `vec..` is `vec . .`. So no name ends in a dot.
        let stem = word.trim_end_matches('.');
        let applies = (word.len() - stem.len()) as u32;
        let dots = Pos {
            line: pos.line,
            col: pos.col + stem.chars().count() as u32,
        };
        let tok = match stem {
            "" => None,
            "[" | "(" | "{" | "<" => {
                let b = stem.as_bytes()[0];
                open.push((b, pos));
                Some(Tok::Open(b))
            }
            "]" | ")" | "}" | ">" => {
                let b = stem.as_bytes()[0];
                match open.pop() {
                    Some((o, _)) if closer(o) == b => Some(Tok::Close(b)),
                    Some((o, p)) => {
                        return Err(Error::new(
                            Kind::Syntax,
                            Some(pos),
                            format!("`{stem}` closes the `{}` at {p}", o as char),
                        ));
                    }
                    None => {
                        return Err(Error::new(
                            Kind::Syntax,
                            Some(pos),
                            format!("`{stem}` closes nothing"),
                        ));
                    }
                }
            }
            w => Some(number(w, pos)?.unwrap_or_else(|| Tok::Name(symbolize(w, symbols).into()))),
        };
        if let Some(tok) = tok {
            out.push(Token { tok, pos });
        }
        for k in 0..applies {
            out.push(Token {
                tok: Tok::Apply,
                pos: Pos {
                    col: dots.col + k,
                    ..dots
                },
            });
        }
    }
    if let Some((o, p)) = open.pop() {
        return Err(Error::new(
            Kind::Unfinished,
            Some(p),
            format!("`{}` is never closed", o as char),
        ));
    }
    Ok(out)
}

/// A word with its symbol escapes rewritten (march7/docs/SURFACE.md): after
/// `\`, a name is `_` or `^` with the character after it, or the longest run
/// of ASCII letters, so `\infty` is not `\in` and `fty`. `x\_1` is `x₁`, and
/// `\times` is `×`. An escape whose name is not in the table stays as it is.
fn symbolize(w: &str, symbols: &Symbols) -> String {
    let mut out = String::new();
    let mut rest = w;
    while let Some(i) = rest.find('\\') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        let len = match after.chars().next() {
            Some(c @ ('_' | '^')) => {
                c.len_utf8() + after[1..].chars().next().map_or(0, char::len_utf8)
            }
            _ => after.bytes().take_while(u8::is_ascii_alphabetic).count(),
        };
        match symbols.get(&after[..len]) {
            Some(sym) if len > 0 => out.push_str(sym),
            _ => out.push_str(&rest[i..i + 1 + len]),
        }
        rest = &after[len..];
    }
    out.push_str(rest);
    out
}

/// A number, if the word is one: digits with an optional `-`, and for a
/// decimal a point with digits on both sides.
fn number(w: &str, pos: Pos) -> Result<Option<Tok>> {
    let digits = w.strip_prefix('-').unwrap_or(w);
    let (whole, frac) = match digits.split_once('.') {
        Some((a, b)) => (a, Some(b)),
        None => (digits, None),
    };
    let is_digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    if !is_digits(whole) || frac.is_some_and(|f| !is_digits(f)) {
        return Ok(None);
    }
    let too_big = || Error::new(Kind::Literal, Some(pos), format!("{w} is too large"));
    let all: String = whole.chars().chain(frac.unwrap_or("").chars()).collect();
    let mut n: i128 = all.parse().map_err(|_| too_big())?;
    if w.starts_with('-') {
        n = -n;
    }
    Ok(Some(match frac {
        None => Tok::Int(n),
        Some(f) => Tok::Dec(n, u32::try_from(f.len()).map_err(|_| too_big())?),
    }))
}

struct Reader<'a> {
    src: &'a str,
    at: usize,
    line: u32,
    col: u32,
}

impl<'a> Reader<'a> {
    fn pos(&self) -> Pos {
        Pos {
            line: self.line,
            col: self.col,
        }
    }
    fn peek(&self) -> Option<char> {
        self.src[self.at..].chars().next()
    }
    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.at += c.len_utf8();
        if c == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(c)
    }
    fn skip_space(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.bump();
        }
    }
    fn skip_line(&mut self) {
        while self.peek().is_some_and(|c| c != '\n') {
            self.bump();
        }
    }
    fn word(&mut self) -> &'a str {
        let start = self.at;
        while self.peek().is_some_and(|c| !c.is_whitespace()) {
            self.bump();
        }
        &self.src[start..self.at]
    }
    fn raw(&mut self, word: &str, pos: Pos) -> Result<String> {
        if let Some(end) = word[1..].find('\'') {
            if end + 2 != word.len() {
                return Err(Error::new(
                    Kind::Syntax,
                    Some(pos),
                    "text runs on after a raw string's closing `'`",
                ));
            }
            return Ok(word[1..=end].to_string());
        }
        let mut s = word[1..].to_string();
        loop {
            match self.bump() {
                Some('\'') => break,
                Some(c) => s.push(c),
                None => {
                    return Err(Error::new(
                        Kind::Unfinished,
                        Some(pos),
                        "a raw string is never closed",
                    ));
                }
            }
        }
        if self.peek().is_some_and(|c| !c.is_whitespace()) {
            return Err(Error::new(
                Kind::Syntax,
                Some(pos),
                "text runs on after a raw string's closing `'`",
            ));
        }
        Ok(s)
    }
    /// A string literal, from its opening `"`, with its escapes and holes
    /// (march7/docs/STRINGS.md). In a hole, its `]` may follow at once.
    fn string(&mut self, symbols: &Symbols, in_hole: bool) -> Result<Tok> {
        let start = self.pos();
        self.bump();
        let mut s = String::new();
        let mut pieces: Vec<Piece> = Vec::new();
        loop {
            let here = self.pos();
            let bad = |msg: &str| Error::new(Kind::Syntax, Some(here), msg.to_string());
            match self.bump() {
                None => {
                    return Err(Error::new(
                        Kind::Unfinished,
                        Some(start),
                        "a string is never closed",
                    ));
                }
                Some('"') => break,
                Some('\\') => match self.bump() {
                    Some('\\') => s.push('\\'),
                    Some('"') => s.push('"'),
                    Some('[') => {
                        let code = tokens(self, symbols, Some(here))?;
                        pieces.push(Piece::Text(std::mem::take(&mut s).into()));
                        pieces.push(Piece::Hole(code.into()));
                    }
                    // `\_` is an input, `\[ _. ]`, unless a subscript's name
                    // and `;` follow: `\_1;` is `₁`.
                    Some('_') => {
                        let mut next = self.src[self.at..].chars();
                        let sub = match (next.next(), next.next()) {
                            (Some(c), Some(';')) => symbols.get(&format!("_{c}")),
                            _ => None,
                        };
                        if let Some(sub) = sub {
                            s.push_str(sub);
                            self.bump();
                            self.bump();
                        } else {
                            let pull = [
                                Token {
                                    tok: Tok::Name("_".into()),
                                    pos: here,
                                },
                                Token {
                                    tok: Tok::Apply,
                                    pos: here,
                                },
                            ];
                            pieces.push(Piece::Text(std::mem::take(&mut s).into()));
                            pieces.push(Piece::Hole(pull.into()));
                        }
                    }
                    Some(c) => {
                        let mut name = String::from(c);
                        loop {
                            match self.bump() {
                                Some(';') => break,
                                Some(c) if !c.is_whitespace() && c != '"' => name.push(c),
                                _ => return Err(bad("an escape needs a `;` after its name")),
                            }
                        }
                        s.push_str(
                            &escape(&name, symbols)
                                .ok_or_else(|| bad(&format!("`\\{name};` is not an escape")))?,
                        );
                    }
                    None => {
                        return Err(Error::new(
                            Kind::Unfinished,
                            Some(start),
                            "a string is never closed",
                        ));
                    }
                },
                Some(c) => s.push(c),
            }
        }
        if self
            .peek()
            .is_some_and(|c| !(c.is_whitespace() || in_hole && c == ']'))
        {
            return Err(Error::new(
                Kind::Syntax,
                Some(self.pos()),
                "text runs on after a string's closing `\"`",
            ));
        }
        if pieces.is_empty() {
            return Ok(Tok::Str(s.into()));
        }
        pieces.push(Piece::Text(s.into()));
        pieces.retain(|p| !matches!(p, Piece::Text(t) if t.is_empty()));
        Ok(Tok::Template(pieces.into()))
    }
}

/// The text of a named or numeric escape: `n`, `t`, `r`, `#9731`, `#x2603`,
/// or a name in the symbol table.
fn escape(name: &str, symbols: &Symbols) -> Option<String> {
    match name {
        "n" => return Some("\n".into()),
        "t" => return Some("\t".into()),
        "r" => return Some("\r".into()),
        _ => {}
    }
    if let Some(n) = name.strip_prefix('#') {
        let code = match n.strip_prefix('x') {
            Some(h) => u32::from_str_radix(h, 16).ok()?,
            None => n.parse().ok()?,
        };
        return char::from_u32(code).map(String::from);
    }
    symbols.get(name).map(String::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(s: &str) -> Vec<Tok> {
        read(s, &Symbols::standard())
            .unwrap()
            .into_iter()
            .map(|t| t.tok)
            .collect()
    }

    #[test]
    fn words_numbers_and_brackets() {
        assert_eq!(
            toks("1 -2 1.10 + . [ x ] < i64 -- i64 > -- a comment\n\\times"),
            [
                Tok::Int(1),
                Tok::Int(-2),
                Tok::Dec(110, 2),
                Tok::Name("+".into()),
                Tok::Apply,
                Tok::Open(b'['),
                Tok::Name("x".into()),
                Tok::Close(b']'),
                Tok::Open(b'<'),
                Tok::Name("i64".into()),
                Tok::Dashes,
                Tok::Name("i64".into()),
                Tok::Close(b'>'),
                Tok::Name("×".into()),
            ]
        );
    }

    #[test]
    fn trailing_dots_apply() {
        let n = |s: &str| Tok::Name(s.into());
        assert_eq!(
            toks("sq. 1.5. 1. vec.. [ ]. \\times. .."),
            [
                n("sq"),
                Tok::Apply,
                Tok::Dec(15, 1),
                Tok::Apply,
                Tok::Int(1),
                Tok::Apply,
                n("vec"),
                Tok::Apply,
                Tok::Apply,
                Tok::Open(b'['),
                Tok::Close(b']'),
                Tok::Apply,
                n("×"),
                Tok::Apply,
                Tok::Apply,
                Tok::Apply,
            ]
        );
        // Each dot is where it is written.
        let t = read("a  sq..", &Symbols::standard()).unwrap();
        assert_eq!(t[2].pos, Pos { line: 1, col: 6 });
        assert_eq!(t[3].pos, Pos { line: 1, col: 7 });
    }

    #[test]
    fn symbols_inside_words() {
        let n = |s: &str| Tok::Name(s.into());
        assert_eq!(
            toks("x\\_1 \\times a\\times\\times \\infty \\foo y\\^2."),
            [
                n("x₁"),
                n("×"),
                n("a××"),
                n("∞"),
                n("\\foo"),
                n("y²"),
                Tok::Apply
            ]
        );
    }

    #[test]
    fn strings_and_escapes() {
        assert_eq!(
            toks(r#""a b\n;\"\\" 'C:\new' "\#x2603;\times;""#),
            [
                Tok::Str("a b\n\"\\".into()),
                Tok::Str("C:\\new".into()),
                Tok::Str("☃×".into()),
            ]
        );
    }

    #[test]
    fn brackets_must_nest() {
        let s = Symbols::standard();
        assert_eq!(read("[ ( ]", &s).unwrap_err().kind, Kind::Syntax);
        assert_eq!(
            read("[ 1", &s).unwrap_err().pos,
            Some(Pos { line: 1, col: 1 })
        );
        assert_eq!(read(")", &s).unwrap_err().kind, Kind::Syntax);
    }
}
