//! The reader (docs/MACHINE.md): words separated by white space, as in
//! FORTH, with numbers, strings, `.` and four brackets. A name applies, in
//! code and in a bracket alike, and a symbol or a variable is spelled as one.

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
    /// `.`, which applies the value on top; the reader puts one after each
    /// name that applies, and `!` is `!!` applied.
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
    read_form(src, symbols, false)
}

/// Reads Symbolic Forth, the lowest form (doc/design/TYPES.md 2.2; Thomas,
/// 2026-10-13): every name is a symbol, applied only by `.`, and `!`
/// converts, `2 i64 !`. Labels, primes and capitals mark nothing there.
pub fn read_symbolic(src: &str, symbols: &Symbols) -> Result<Vec<Token>> {
    read_form(src, symbols, true)
}

fn read_form(src: &str, symbols: &Symbols, symbolic: bool) -> Result<Vec<Token>> {
    let mut r = Reader {
        src,
        at: 0,
        line: 1,
        col: 1,
        symbolic,
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
        // Each dot at the end of a word applies once more, `[ … ].`, and
        // each `!` converts by what is on top (Thomas, 2026-10-13): `2 i64!`,
        // `x y < i64 f64 > !`. So no name ends in either. A dot inside a word
        // is part of it: `2.5` is a number, and in a name it divides a path
        // in a namespace, `io.file.read` (doc/design/TYPES.md 3.12).
        // `!!` alone is the word `!` applies, which structures a value by a
        // type: `2 i64 !! .` is `2 i64 !`.
        let stem = match word {
            "!!" => word,
            _ => word.trim_end_matches(['.', '!']),
        };
        let marks = &word[stem.len()..];
        let at = Pos {
            col: pos.col + stem.chars().count() as u32,
            ..pos
        };
        let (stem, applies) = if is_name(stem) && !r.symbolic {
            spelled(stem)
        } else {
            (stem, false)
        };
        piece(stem, applies, marks, pos, at, symbols, &mut open, &mut out)?;
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

/// Whether a word is a name: not a number or a bracket.
fn is_name(w: &str) -> bool {
    let digits = w.strip_prefix('-').unwrap_or(w);
    !(w.is_empty()
        || matches!(w, "[" | "(" | "{" | "<" | "]" | ")" | "}" | ">")
        || digits.starts_with(|c: char| c.is_ascii_digit()))
}

/// A name, and whether it applies (Thomas, 2026-10-12): a name applies, as
/// in FORTH, everywhere, and a symbol or a variable is spelled as one. A
/// capitalized name is a symbol, `Snake`; one lowercase letter is a
/// variable, `a`; `name:` labels what follows, a name being defined or a
/// key; and `name'` is a symbol, a word passed rather than applied, `+'`.
fn spelled(w: &str) -> (&str, bool) {
    let marked = |m: char| w.len() > m.len_utf8() && w.ends_with(m);
    if marked(':') || marked('\'') {
        (&w[..w.len() - 1], false)
    } else {
        (w, !(w.starts_with(char::is_uppercase) || is_variable(w)))
    }
}

/// Whether a name is a variable: one lowercase letter, `a` to `z`, a type
/// variable in a bracket. It never applies, and no word has its name.
pub fn is_variable(w: &str) -> bool {
    w.len() == 1 && w.as_bytes()[0].is_ascii_lowercase()
}

/// One word: its token, its own application if it is a name that applies,
/// and for each mark after it, the first at `at`, an application, `.`, or
/// `!!` applied, `!`.
#[allow(clippy::too_many_arguments)]
fn piece(
    stem: &str,
    applies: bool,
    marks: &str,
    pos: Pos,
    at: Pos,
    symbols: &Symbols,
    open: &mut Vec<(u8, Pos)>,
    out: &mut Vec<Token>,
) -> Result<()> {
    {
        // A middle dot or a bullet, alone, is the dot operator, `⋅`: they look
        // alike, and some keyboards make one where `⋅` was meant. Inside a
        // name, as Catalan's `l·l`, it stays.
        let stem = match stem {
            "·" | "∙" => "⋅",
            s => s,
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
        if applies {
            out.push(Token {
                tok: Tok::Apply,
                pos,
            });
        }
        for (k, m) in marks.chars().enumerate() {
            let pos = Pos {
                col: at.col + k as u32,
                ..at
            };
            if m == '!' {
                out.push(Token {
                    tok: Tok::Name("!!".into()),
                    pos,
                });
            }
            out.push(Token {
                tok: Tok::Apply,
                pos,
            });
        }
    }
    Ok(())
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
    /// Reading Symbolic Forth, where no name applies by itself.
    symbolic: bool,
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

/// Code in Symbolic Forth, the lowest form (doc/design/TYPES.md 2.2): every
/// name a symbol, each application a `.`, and `!!` applied `!`. It
/// reads back, by `read_symbolic`, to the same tokens: the litmus test of
/// the intermediate form (Thomas, 2026-10-13). Lines are kept, so the text
/// reads as the source did.
pub fn lower(toks: &[Token]) -> String {
    let mut out = String::new();
    let mut line = toks.first().map_or(1, |t| t.pos.line);
    let mut i = 0;
    while i < toks.len() {
        let t = &toks[i];
        if t.pos.line > line {
            out.push_str(if t.pos.line > line + 1 { "\n\n" } else { "\n" });
            line = t.pos.line;
        } else if !out.is_empty() {
            out.push(' ');
        }
        match &t.tok {
            Tok::Name(n)
                if &**n == "!!" && toks.get(i + 1).is_some_and(|t| t.tok == Tok::Apply) =>
            {
                out.push('!');
                i += 1;
            }
            tok => out.push_str(&lowered(tok)),
        }
        i += 1;
    }
    out
}

/// One token in Symbolic Forth.
fn lowered(tok: &Tok) -> String {
    let text = |t: &str| t.replace('\\', "\\\\").replace('"', "\\\"");
    match tok {
        Tok::Int(n) => n.to_string(),
        Tok::Dec(d, s) => crate::prims::show_dec(*d, *s),
        Tok::Str(s) => format!("\"{}\"", text(s)),
        Tok::Name(n) => n.to_string(),
        Tok::Apply => ".".into(),
        Tok::Open(b) | Tok::Close(b) => (*b as char).to_string(),
        Tok::Dashes => "--".into(),
        Tok::Template(pieces) => {
            let mut s = String::from('"');
            for p in pieces.iter() {
                match p {
                    Piece::Text(t) => s.push_str(&text(t)),
                    Piece::Hole(code) => {
                        s.push_str("\\[ ");
                        s.push_str(&lower(code).replace('\n', " "));
                        s.push_str(" ]");
                    }
                }
            }
            s.push('"');
            s
        }
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
        let n = |s: &str| Tok::Name(s.into());
        assert_eq!(
            toks("1 -2 1.10 + [ xs ] < i64 -- i64 > -- a comment\n\\times"),
            [
                Tok::Int(1),
                Tok::Int(-2),
                Tok::Dec(110, 2),
                n("+"),
                Tok::Apply,
                Tok::Open(b'['),
                n("xs"),
                Tok::Apply,
                Tok::Close(b']'),
                Tok::Open(b'<'),
                n("i64"),
                Tok::Apply,
                Tok::Dashes,
                n("i64"),
                Tok::Apply,
                Tok::Close(b'>'),
                n("×"),
                Tok::Apply,
            ]
        );
    }

    #[test]
    fn names_apply_and_symbols_are_spelled() {
        // A name applies, in a bracket too; a capitalized one is a symbol,
        // as are `name:`, a label, and `name'`; one lowercase letter is a
        // variable.
        let n = |s: &str| Tok::Name(s.into());
        assert_eq!(
            toks("dup Snake fib: +' < string Camel a ary odd? > σ'"),
            [
                n("dup"),
                Tok::Apply,
                n("Snake"),
                n("fib"),
                n("+"),
                Tok::Open(b'<'),
                n("string"),
                Tok::Apply,
                n("Camel"),
                n("a"),
                n("ary"),
                Tok::Apply,
                n("odd?"),
                Tok::Apply,
                Tok::Close(b'>'),
                n("σ"),
            ]
        );
    }

    #[test]
    fn dots_inside_a_word_are_part_of_it() {
        // A number keeps its point, and a name its dots, a path in a
        // namespace.
        let n = |s: &str| Tok::Name(s.into());
        assert_eq!(
            toks("io.file.read 2.5 2.i64"),
            [n("io.file.read"), Tok::Apply, Tok::Dec(25, 1), n("2.i64")]
        );
    }

    #[test]
    fn dots_at_the_end_apply_once_more() {
        let n = |s: &str| Tok::Name(s.into());
        assert_eq!(
            toks("1.5. Snake. vec.. [ ]. .."),
            [
                Tok::Dec(15, 1),
                Tok::Apply,
                n("Snake"),
                Tok::Apply,
                n("vec"),
                Tok::Apply,
                Tok::Apply,
                Tok::Apply,
                Tok::Open(b'['),
                Tok::Close(b']'),
                Tok::Apply,
                Tok::Apply,
                Tok::Apply,
            ]
        );
        // A name's own application is where the name is, and each dot where
        // it is written.
        let t = read("xs  sq..", &Symbols::standard()).unwrap();
        let cols: Vec<u32> = t.iter().map(|t| t.pos.col).collect();
        assert_eq!(cols, [1, 1, 5, 5, 7, 8]);
    }

    #[test]
    fn a_bang_converts() {
        let n = |s: &str| Tok::Name(s.into());
        assert_eq!(
            toks("2 i64! < i64 > !"),
            [
                Tok::Int(2),
                n("i64"),
                Tok::Apply,
                n("!!"),
                Tok::Apply,
                Tok::Open(b'<'),
                n("i64"),
                Tok::Apply,
                Tok::Close(b'>'),
                n("!!"),
                Tok::Apply,
            ]
        );
    }

    #[test]
    fn bangs_alone_name_the_word_a_bang_applies() {
        let n = |s: &str| Tok::Name(s.into());
        assert_eq!(toks("!!"), [n("!!"), Tok::Apply]);
        let t: Vec<Tok> = read_symbolic("2 i64 !! .", &Symbols::standard())
            .unwrap()
            .into_iter()
            .map(|t| t.tok)
            .collect();
        assert_eq!(t, [Tok::Int(2), n("i64"), n("!!"), Tok::Apply]);
    }

    #[test]
    fn symbols_inside_words() {
        let n = |s: &str| Tok::Name(s.into());
        assert_eq!(
            toks("x\\_1 \\times a\\times\\times \\infty \\foo y\\^2"),
            [
                n("x₁"),
                Tok::Apply,
                n("×"),
                Tok::Apply,
                n("a××"),
                Tok::Apply,
                n("∞"),
                Tok::Apply,
                n("\\foo"),
                Tok::Apply,
                n("y²"),
                Tok::Apply,
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
