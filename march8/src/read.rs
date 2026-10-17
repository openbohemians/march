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
    /// A ratio literal, `1:2`: its terms as written, the sign on the first.
    Ratio(Rc<[i128]>),
    Str(Rc<str>),
    Name(Rc<str>),
    /// `.`, which applies the value on top; the reader puts one after each
    /// name that applies, and `!` is `convert` applied.
    Apply,
    /// `?`, which applies the value on top as a test, leaving one bool,
    /// or in a signature holds it as a guard (doc/design/SYMBOLIC-FORTH.md
    /// 20): `positive?` is `positive ?`.
    Test,
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
    /// On the first token of a word read as a name, the word as written.
    pub word: Option<Word>,
}

/// A word read as a name, for a recognizer to read in place of the core
/// (doc/design/SYMBOLIC-FORTH.md 25): how many tokens the core read it as
/// before its marks, whose text `text` gives, and whether it applies.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Word {
    pub len: u8,
    pub applies: bool,
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
    // Each `<| … |>` open: how deep the brackets were, and how words were
    // read outside it.
    let mut lowest: Vec<(usize, bool)> = Vec::new();
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
        if r.alone().is_some_and(opens_block) {
            r.skip_block(pos)?;
            continue;
        }
        if hole.is_some() && open.is_empty() && c == ']' {
            r.bump();
            if out.is_empty() {
                return Err(Error::new(Kind::Syntax, hole, "a hole holds no code"));
            }
            return Ok(out);
        }
        if c == '"' {
            let tok = r.string(symbols, hole.is_some() && open.is_empty())?;
            out.push(Token {
                tok,
                pos,
                word: None,
            });
            continue;
        }
        let word = r.word();
        if word.len() > 1 && word.starts_with('\'') {
            // A quoted symbol runs to the next `'` not doubled, spaces and
            // all (doc/design/SYMBOLIC-FORTH.md 6.12): text until a type
            // structures it, a string where one is taken (28).
            r.at -= word.len();
            r.col = pos.col;
            let s = r.quoted(pos)?;
            out.push(Token {
                tok: Tok::Name(s.into()),
                pos,
                word: None,
            });
            continue;
        }
        // `<| … |>` is the lowest form's quotation written inline
        // (doc/design/SYMBOLIC-FORTH.md 3): its words are read as Symbolic
        // Forth, so nothing applies without a dot, and it is `[ … ]`.
        if word == "<|" {
            open.push((b'[', pos));
            out.push(Token {
                tok: Tok::Open(b'['),
                pos,
                word: None,
            });
            lowest.push((open.len(), r.symbolic));
            r.symbolic = true;
            continue;
        }
        if word == "|>" {
            if lowest.last().map(|l| l.0) != Some(open.len()) {
                return Err(Error::new(Kind::Syntax, Some(pos), "`|>` closes no `<|`"));
            }
            open.pop();
            r.symbolic = lowest.pop().expect("open").1;
            out.push(Token {
                tok: Tok::Close(b']'),
                pos,
                word: None,
            });
            continue;
        }
        let in_bracket = open.iter().any(|(b, _)| *b == b'<');
        if word == "--" && in_bracket {
            out.push(Token {
                tok: Tok::Dashes,
                pos,
                word: None,
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
        // A `?` there tests (doc/design/SYMBOLIC-FORTH.md 20): `positive?`
        // is `positive ?`, applied as a test in place of its own application.
        let stem = word.trim_end_matches(['.', '!', '?']);
        let marks = &word[stem.len()..];
        let at = Pos {
            col: pos.col + stem.chars().count() as u32,
            ..pos
        };
        let primed = is_name(stem) && stem.len() > 1 && stem.ends_with('\'');
        let readable = if r.symbolic {
            None
        } else {
            readable(stem, marks)
        };
        let (stem, applies) = if is_name(stem) && !r.symbolic {
            let (stem, applies) = spelled(stem);
            (stem, applies && !marks.starts_with('?'))
        } else {
            // `;` defines, a meta-op in Symbolic Forth as `!` is
            // (doc/design/SYMBOLIC-FORTH.md 18).
            (stem, stem == ";")
        };
        // A width and a power are spellings (doc/design/SYMBOLIC-FORTH.md
        // 21, 22): `dup_2^3` reads as `dup 3 power# . 2 width# .`, applied as
        // `dup` would be, and `[ dup + ]^3` stays a quotation.
        let spelling;
        let mut scripts: Vec<(i128, &str)> = Vec::new();
        let (stem, applies) = if !r.symbolic && (applies || primed || stem.starts_with(']')) {
            spelling = crate::symbols::unscripted(&symbolize(stem, symbols));
            let (base, sub, sup) = crate::symbols::scripts(&spelling);
            // The power first, by the kind of the word itself: a power of a
            // width is the width of the power.
            let counts = [(sup, "power#"), (sub, "width#")]
                .into_iter()
                .filter(|(digits, _)| !digits.is_empty())
                .map(|(digits, word)| digits.parse().ok().map(|n| (n, word)))
                .collect::<Option<Vec<_>>>();
            match counts {
                Some(c)
                    if !c.is_empty() && (base == "]" || is_name(base) && !is_variable(base)) =>
                {
                    scripts = c;
                    (base, applies && base != "]")
                }
                _ => (stem, applies),
            }
        } else {
            (stem, applies)
        };
        let start = out.len();
        piece(
            stem, applies, &scripts, marks, pos, at, symbols, &mut open, &mut out,
        )?;
        if let Some(applies) = readable {
            let marked: usize = marks.chars().map(|m| if m == '!' { 2 } else { 1 }).sum();
            let len = (out.len() - start - marked) as u8;
            out[start].word = Some(Word { len, applies });
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

/// Whether a line opens a block comment: three or more `-`, and nothing
/// else (Thomas, 2026-10-15). The block runs to a line that closes it.
pub fn opens_block(line: &str) -> bool {
    let t = line.trim();
    t.len() >= 3 && t.bytes().all(|b| b == b'-')
}

/// Whether a line closes a block comment: three or more `-` and a period,
/// `---.`.
pub fn closes_block(line: &str) -> bool {
    line.trim().strip_suffix('.').is_some_and(opens_block)
}

/// Whether a word is a name: not a number or a bracket.
fn is_name(w: &str) -> bool {
    let digits = w.strip_prefix('-').unwrap_or(w);
    !(w.is_empty()
        || matches!(w, "[" | "(" | "{" | "<" | "]" | ")" | "}" | ">")
        || digits.starts_with(|c: char| c.is_ascii_digit()))
}

/// A name, and whether it applies (Thomas, 2026-10-12): a name applies, as
/// in FORTH, everywhere, and a symbol or a variable is spelled as one. One
/// lowercase letter is a variable, `a`; `name:` labels what follows, a name
/// being defined or a key; and `name'` is a symbol, a word passed rather
/// than applied, `+'`. A capital marks nothing (doc/design/SYMBOLIC-FORTH.md
/// 27): `Snake` applies, and the symbol is `Snake'`.
fn spelled(w: &str) -> (&str, bool) {
    let marked = |m: char| w.len() > m.len_utf8() && w.ends_with(m);
    if marked(':') || marked('\'') {
        (&w[..w.len() - 1], false)
    } else {
        (w, !is_variable(w))
    }
}

/// Whether a pattern may read a word (doc/design/SYMBOLIC-FORTH.md 25): one
/// the core reads as a name, `12kg` among them, but not `;` or `_`, or a
/// label, `name:`, a name being defined; and if so, whether it applies, as a
/// name does.
fn readable(stem: &str, marks: &str) -> Option<bool> {
    let first = stem.chars().next()?;
    if matches!(stem, ";" | "_")
        || stem.len() > 1 && stem.ends_with(':')
        || "[](){}<>".contains(first)
        || !matches!(number(stem, Pos::default()), Ok(None))
    {
        return None;
    }
    Some(spelled(stem).1 && !marks.starts_with('?'))
}

/// A word's text as a recognizer reads it, from the tokens the core read it
/// as: spelled as typed, without its marks, so `log₂'`, read `log 2 width#
/// .`, is `log_2`.
pub fn text(toks: &[Token]) -> std::borrow::Cow<'_, str> {
    // A name the core keeps as written, `12kg'`, has its mark yet.
    let name = match toks.first().map(|t| &t.tok) {
        Some(Tok::Name(n)) if is_name(n) => n,
        Some(Tok::Name(n)) => spelled(n).0,
        _ => "",
    };
    if toks.len() < 3 {
        return name.into();
    }
    let mut out = name.to_string();
    let (mut sub, mut sup) = (None, None);
    for (k, t) in toks.iter().enumerate().skip(1) {
        match &t.tok {
            Tok::Int(n)
                if toks
                    .get(k + 1)
                    .is_some_and(|t| matches!(&t.tok, Tok::Name(w) if &**w == "width#")) =>
            {
                sub = Some(n)
            }
            Tok::Int(n) => sup = Some(n),
            _ => {}
        }
    }
    if let Some(n) = sub {
        out.push_str(&format!("_{n}"));
    }
    if let Some(n) = sup {
        out.push_str(&format!("^{n}"));
    }
    out.into()
}

/// Whether a name is a variable: one lowercase letter, `a` to `z`, a type
/// variable in a bracket. It never applies, and no word has its name.
pub fn is_variable(w: &str) -> bool {
    w.len() == 1 && w.as_bytes()[0].is_ascii_lowercase()
}

/// One word: its token, its width and power spelled out, its own
/// application if it is a name that applies, and for each mark after it,
/// the first at `at`, an application, `.`, or `convert` applied, `!`.
#[allow(clippy::too_many_arguments)]
fn piece(
    stem: &str,
    applies: bool,
    scripts: &[(i128, &str)],
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
            w => Some(number(w, pos)?.unwrap_or_else(|| {
                Tok::Name(crate::symbols::unscripted(&symbolize(w, symbols)).into())
            })),
        };
        if let Some(tok) = tok {
            out.push(Token {
                tok,
                pos,
                word: None,
            });
        }
        for &(n, word) in scripts {
            out.push(Token {
                tok: Tok::Int(n),
                pos,
                word: None,
            });
            out.push(Token {
                tok: Tok::Name(word.into()),
                pos,
                word: None,
            });
            out.push(Token {
                tok: Tok::Apply,
                pos,
                word: None,
            });
        }
        if applies {
            out.push(Token {
                tok: Tok::Apply,
                pos,
                word: None,
            });
        }
        for (k, m) in marks.chars().enumerate() {
            let pos = Pos {
                col: at.col + k as u32,
                ..at
            };
            if m == '!' {
                out.push(Token {
                    tok: Tok::Name("convert".into()),
                    pos,
                    word: None,
                });
            }
            out.push(Token {
                tok: if m == '?' { Tok::Test } else { Tok::Apply },
                pos,
                word: None,
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
pub(crate) fn number(w: &str, pos: Pos) -> Result<Option<Tok>> {
    let digits = w.strip_prefix('-').unwrap_or(w);
    if digits.starts_with(|c: char| c.is_ascii_digit()) && digits.contains(':') {
        return ratio(w, pos).map(Some);
    }
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

/// A ratio, `1:2`, or one of more terms, `12:30:15` (Thomas, 2026-10-13):
/// whole numbers between colons, a sign only in front. The terms are kept as
/// written, so that what takes the literal reads them: `12:30` is not `2:5`.
fn ratio(w: &str, pos: Pos) -> Result<Tok> {
    let err = |why: &str| Error::new(Kind::Literal, Some(pos), format!("{w} {why}"));
    let mut terms = Vec::new();
    for t in w.strip_prefix('-').unwrap_or(w).split(':') {
        if t.is_empty() || !t.bytes().all(|b| b.is_ascii_digit()) {
            return Err(err(
                "is not a ratio: whole numbers between colons, a sign only in front",
            ));
        }
        let n: i64 = t
            .parse()
            .map_err(|_| err("is too large: each term is an i64"))?;
        terms.push(i128::from(n));
    }
    if w.starts_with('-') {
        terms[0] = -terms[0];
    }
    Ok(Tok::Ratio(terms.into()))
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
    /// The rest of the line, if only white space comes before it on it.
    fn alone(&self) -> Option<&'a str> {
        let start = self.src[..self.at].rfind('\n').map_or(0, |i| i + 1);
        if !self.src[start..self.at].trim().is_empty() {
            return None;
        }
        let end = self.src[self.at..]
            .find('\n')
            .map_or(self.src.len(), |i| self.at + i);
        Some(&self.src[self.at..end])
    }
    /// Skips a block comment, from the line that opens it through the line
    /// that closes it.
    fn skip_block(&mut self, pos: Pos) -> Result<()> {
        self.skip_line();
        loop {
            if self.bump().is_none() {
                return Err(Error::new(
                    Kind::Unfinished,
                    Some(pos),
                    "a block comment is never closed: a line of `---.` closes it",
                ));
            }
            let closing = self.alone().is_some_and(closes_block);
            self.skip_line();
            if closing {
                return Ok(());
            }
        }
    }
    fn word(&mut self) -> &'a str {
        let start = self.at;
        while self.peek().is_some_and(|c| !c.is_whitespace()) {
            self.bump();
        }
        &self.src[start..self.at]
    }
    /// A quoted symbol, from its opening `'`: its text as it is, newlines
    /// among it, a quote in it doubled, `'it''s'`.
    fn quoted(&mut self, pos: Pos) -> Result<String> {
        self.bump();
        let mut s = String::new();
        loop {
            match self.bump() {
                Some('\'') if self.peek() == Some('\'') => {
                    self.bump();
                    s.push('\'');
                }
                Some('\'') => break,
                Some(c) => s.push(c),
                None => {
                    return Err(Error::new(
                        Kind::Unfinished,
                        Some(pos),
                        "a quoted symbol is never closed",
                    ));
                }
            }
        }
        if self.peek().is_some_and(|c| !c.is_whitespace()) {
            return Err(Error::new(
                Kind::Syntax,
                Some(pos),
                "text runs on after a quoted symbol's closing `'`",
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
                                    word: None,
                                },
                                Token {
                                    tok: Tok::Apply,
                                    pos: here,
                                    word: None,
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
/// name a symbol, each application a `.`, and `convert` applied `!`. It
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
                if (&**n == "convert" || &**n == ";")
                    && toks.get(i + 1).is_some_and(|t| t.tok == Tok::Apply) =>
            {
                out.push(if &**n == ";" { ';' } else { '!' });
                i += 1;
            }
            tok => out.push_str(&lowered(tok)),
        }
        i += 1;
    }
    out
}

/// A symbol in Symbolic Forth: its name, or quoted if it reads otherwise,
/// `'two words'`.
fn symbol(n: &str, out: &mut String) {
    let none = Symbols::parse("").expect("an empty table");
    match read_symbolic(n, &none).as_deref() {
        Ok([t]) if t.tok == Tok::Name(n.into()) && !n.contains('\'') => out.push_str(n),
        _ => quote(n, out),
    }
}

/// Text as a quoted symbol, a quote in it doubled: `'it''s'`.
pub fn quote(n: &str, out: &mut String) {
    out.push('\'');
    out.push_str(&n.replace('\'', "''"));
    out.push('\'');
}

/// One token in Symbolic Forth.
fn lowered(tok: &Tok) -> String {
    let text = |t: &str| t.replace('\\', "\\\\").replace('"', "\\\"");
    match tok {
        Tok::Int(n) => n.to_string(),
        Tok::Dec(d, s) => crate::prims::show_dec(*d, *s),
        Tok::Ratio(t) => crate::prims::show_ratio(t),
        Tok::Str(s) => format!("\"{}\"", text(s)),
        Tok::Name(n) => {
            let mut s = String::new();
            symbol(n, &mut s);
            s
        }
        Tok::Apply => ".".into(),
        Tok::Test => "?".into(),
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
    fn scripts_read_as_typed() {
        // A name's trailing subscript and superscript digits are its `_` and
        // `^` and digits (doc/design/SYMBOLIC-FORTH.md 23), escaped or not,
        // and a width and a power are spellings, lowered (21, 22), on a word
        // applied or passed, and on a quotation; a label keeps its name, and
        // a capital marks nothing, so `Snake₂` is `Snake` two wide (27).
        let n = |s: &str| Tok::Name(s.into());
        let spelled = |w: &str, k: i128, by: &str| [n(w), Tok::Int(k), n(by), Tok::Apply];
        let mut want = Vec::new();
        for _ in 0..2 {
            want.extend(spelled("dup", 2, "width#"));
            want.push(Tok::Apply);
        }
        want.extend(spelled("+", 4, "power#"));
        want.push(Tok::Apply);
        want.extend(spelled("dup", 3, "power#"));
        want.extend([Tok::Int(2), n("width#"), Tok::Apply, Tok::Apply]);
        want.extend(spelled("log", 2, "width#"));
        want.extend([Tok::Open(b'['), n("q"), Tok::Close(b']')]);
        want.extend([Tok::Int(3), n("power#"), Tok::Apply]);
        want.push(n("x_1"));
        want.extend(spelled("Snake", 2, "width#"));
        want.push(Tok::Apply);
        assert_eq!(toks("dup₂ dup\\_2 +⁴ dup₂³ log₂' [ q ]³ x₁: Snake₂"), want);
    }

    #[test]
    fn a_name_keeps_its_word_for_recognizers() {
        // Its text as typed, without marks, the tokens the core read it as
        // before them, and whether it applies (doc/design/SYMBOLIC-FORTH.md
        // 25). A number, a bracket and the meta-ops have none.
        let toks = read("log₂' dup_2³. 12kg' Snake 3 [ _ ] ;", &Symbols::standard()).unwrap();
        let words: Vec<_> = (0..toks.len())
            .filter_map(|i| {
                let w = toks[i].word?;
                Some((
                    text(&toks[i..i + w.len as usize]).into_owned(),
                    w.len,
                    w.applies,
                ))
            })
            .collect();
        let w = |t: &str, len, applies| (t.to_string(), len, applies);
        assert_eq!(
            words,
            [
                w("log_2", 4, false),
                w("dup_2^3", 8, true),
                w("12kg", 1, false),
                w("Snake", 2, true),
            ]
        );
    }

    #[test]
    fn block_comments() {
        // A line of three or more `-` opens a block comment, and one ending
        // in a period closes it; a line of them with more on it, or with no
        // period, is no edge of a block.
        let src = "1\n  -----\n  2 \"x\n----\n--- 3\n  -----.\n4\n--- 5\n6";
        assert_eq!(toks(src), [Tok::Int(1), Tok::Int(4), Tok::Int(6)]);
        let e = read("1\n---\n2", &Symbols::standard()).unwrap_err();
        assert_eq!(e.kind, Kind::Unfinished);
        assert_eq!(e.pos, Some(Pos { line: 2, col: 1 }));
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
    fn a_ratio_keeps_its_terms() {
        let r = |t: &[i128]| Tok::Ratio(t.into());
        assert_eq!(
            toks("1:2 -1:3 12:30 2026:10:13 1:2!"),
            [
                r(&[1, 2]),
                r(&[-1, 3]),
                r(&[12, 30]),
                r(&[2026, 10, 13]),
                r(&[1, 2]),
                Tok::Name("convert".into()),
                Tok::Apply,
            ]
        );
        for w in ["1:", "1:-2", "1:2.5", "1::2", "1:x"] {
            let e = read(w, &Symbols::standard()).unwrap_err();
            assert_eq!(e.kind, Kind::Literal, "{w}");
            assert!(e.msg.contains("is not a ratio"), "{w}: {e}");
        }
    }

    #[test]
    fn names_apply_and_symbols_are_spelled() {
        // A name applies, in a bracket too, a capitalized one as any
        // (doc/design/SYMBOLIC-FORTH.md 27); `name:`, a label, and `name'`
        // are symbols; one lowercase letter is a variable; and `name?` is
        // tested, not applied.
        let n = |s: &str| Tok::Name(s.into());
        assert_eq!(
            toks("dup Snake Snake' fib: +' < string Camel' a ary odd? > σ'"),
            [
                n("dup"),
                Tok::Apply,
                n("Snake"),
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
                n("odd"),
                Tok::Test,
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
            toks("1.5. Snake'. vec.. [ ]. .."),
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
                n("convert"),
                Tok::Apply,
                Tok::Open(b'<'),
                n("i64"),
                Tok::Apply,
                Tok::Close(b'>'),
                n("convert"),
                Tok::Apply,
            ]
        );
    }

    #[test]
    fn symbols_inside_words() {
        // A trailing subscript or superscript digit is then spelled as typed.
        let n = |s: &str| Tok::Name(s.into());
        assert_eq!(
            toks("x\\_1 \\times a\\times\\times \\infty \\foo y\\^2"),
            [
                n("x_1"),
                Tok::Apply,
                n("×"),
                Tok::Apply,
                n("a××"),
                Tok::Apply,
                n("∞"),
                Tok::Apply,
                n("\\foo"),
                Tok::Apply,
                n("y^2"),
                Tok::Apply,
            ]
        );
    }

    #[test]
    fn strings_and_escapes() {
        // A string's escapes are decoded; a quoted symbol has none, and
        // doubles a quote in it (doc/design/SYMBOLIC-FORTH.md 6.12).
        assert_eq!(toks("'it''s'"), [Tok::Name("it's".into())]);
        assert_eq!(
            toks(r#""a b\n;\"\\" 'C:\new' "\#x2603;\times;""#),
            [
                Tok::Str("a b\n\"\\".into()),
                Tok::Name("C:\\new".into()),
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
