//! The reader (docs/MACHINE.md): words separated by white space, as in
//! FORTH, with numbers, strings, `.` and four brackets. A name applies, in
//! code and in a bracket alike, and a symbol or a variable is spelled as one.

use crate::error::{Error, Kind, Pos, Result};
use crate::symbols::Symbols;
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
    /// An integer literal, `10`, which is `10 int# !` in Symbolic Forth.
    Int(i128),
    /// A decimal literal: its digits, and how many follow the point. `1.5`
    /// is `1.5 dec# !` in Symbolic Forth.
    Dec(i128, u32),
    /// A ratio literal, `1:2`: its terms as written, the sign on the first.
    /// `1:2` is `1:2 ratio# !` in Symbolic Forth.
    Ratio(Rc<[i128]>),
    Name(Rc<str>),
    /// `.`, which applies the value on top; the reader puts one after each
    /// name that applies, and `!` and `;` are the words `!` and `;`
    /// applied.
    Apply,
    /// `?`, which applies the value on top as a test, leaving one bool,
    /// or in a signature holds it as a guard (doc/design/SYMBOLIC-FORTH.md
    /// 20): `positive?` is `positive ?`.
    Test,
    /// `[`, the one bracket High Stack's are read as. Only the stage writes
    /// another: it keeps a signature as its code between `<` and `>`.
    Open(u8),
    /// `]`, or the stage's `>`.
    Close(u8),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    pub tok: Tok,
    pub pos: Pos,
}

/// The kind `!` makes of a quotation written in a High Stack bracket.
fn kind_of(open: u8) -> Option<&'static str> {
    match open {
        b'(' => Some("seq#"),
        b'{' => Some("map#"),
        b'<' => Some("sig#"),
        _ => None,
    }
}

fn closer(open: u8) -> u8 {
    match open {
        b'[' => b']',
        b'(' => b')',
        b'{' => b'}',
        b'|' => b'|',
        _ => b'>',
    }
}

/// How a bracket opened is written: `|` stands for `[|`.
fn opening(open: u8) -> &'static str {
    match open {
        b'[' => "[",
        b'(' => "(",
        b'{' => "{",
        b'|' => "[|",
        _ => "<",
    }
}

/// Reads source into tokens, checking that brackets nest. `--` starts a
/// comment that runs to the end of the line.
pub fn read(src: &str, symbols: &Symbols) -> Result<Vec<Token>> {
    read_form(src, symbols, false)
}

/// Reads Symbolic Forth, the lowest form (doc/design/TYPES.md 2.2; Thomas,
/// 2026-10-13): every name is a symbol, applied only by `.`, and `!`
/// converts, `2 i64 !`. Labels, primes and capitals mark nothing there, and
/// digits nothing either: `10` is a symbol, and `10 int# !` the number
/// (doc/design/SYMBOLIC-FORTH.md 6.7).
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
    // Each `[| … |]` open: how words were read outside it.
    let mut lowest: Vec<bool> = Vec::new();
    // A `<` a comment began in.
    let mut dashed = None;
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
            out.extend(r.string(symbols, hole.is_some() && open.is_empty())?);
            continue;
        }
        let word = r.word();
        if word.starts_with('\'') {
            // A quoted symbol runs to the next `'` not doubled, spaces and
            // all, a space first among them (doc/design/SYMBOLIC-FORTH.md
            // 6.12): text until a type structures it, a string where one is
            // taken (28).
            r.at -= word.len();
            r.col = pos.col;
            let s = r.quoted(pos)?;
            out.push(Token {
                tok: Tok::Name(s.into()),
                pos,
            });
            continue;
        }
        // `[| … |]` is the lowest form's quotation written inline
        // (doc/design/SYMBOLIC-FORTH.md 3): its words are read as Symbolic
        // Forth, so nothing applies without a dot, and it is `[ … ]`. Only
        // `|]` closes it.
        if word == "[|" {
            open.push((b'|', pos));
            out.push(Token {
                tok: Tok::Open(b'['),
                pos,
            });
            lowest.push(r.symbolic);
            r.symbolic = true;
            continue;
        }
        if word == "|]" {
            match open.pop() {
                Some((b'|', _)) => {}
                Some((o, p)) => {
                    return Err(Error::new(
                        Kind::Syntax,
                        Some(pos),
                        format!("`|]` closes the `{}` at {p}", opening(o)),
                    ));
                }
                None => return Err(Error::new(Kind::Syntax, Some(pos), "`|]` closes nothing")),
            }
            r.symbolic = lowest.pop().expect("open");
            out.push(Token {
                tok: Tok::Close(b']'),
                pos,
            });
            continue;
        }
        // A signature's two brackets touch, inputs then outputs, `< a a gt? ><
        // a >` (doc/design/SYMBOLIC-FORTH.md 6.8): `><` is `>` then `<`.
        if word == "><" && !r.symbolic {
            let next = Pos {
                col: pos.col + 1,
                ..pos
            };
            for (b, at) in [(">", pos), ("<", next)] {
                piece(
                    b,
                    false,
                    false,
                    &[],
                    "",
                    at,
                    at,
                    true,
                    symbols,
                    &mut open,
                    &mut out,
                )?;
            }
            continue;
        }
        if word.starts_with("--") {
            if let Some(&(b'<', p)) = open.last() {
                dashed = Some(p);
            }
            r.skip_line();
            continue;
        }
        // Each dot at the end of a word applies once more, `[ … ].`, and
        // each `!` structures by what is on top (Thomas, 2026-10-13): `2 i64!`,
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
        // A word that is not a number is a name, `12kg` among them, for a
        // rule to read where it is applied (doc/design/SYMBOLIC-FORTH.md 25).
        let named = is_name(stem)
            || !stem.is_empty()
                && !stem.starts_with(['[', '(', '{', '<', ']', ')', '}', '>'])
                && matches!(number(stem, pos), Ok(None));
        // A letter is a variable, `a var# .` (doc/design/SYMBOLIC-FORTH.md 7):
        // by spelling alone, the bracket it is written in filling it.
        let variable = named && !r.symbolic && is_variable(stem);
        let (stem, applies) = if named && !r.symbolic {
            let (stem, applies) = spelled(stem);
            (stem, applies && !marks.starts_with('?'))
        } else {
            // `;` defines, a meta-op in Symbolic Forth as `!` is
            // (doc/design/SYMBOLIC-FORTH.md 18).
            (stem, stem == ";")
        };
        // A quotation's power is a spelling (doc/design/SYMBOLIC-FORTH.md
        // 21): `[ dup + ]^3` reads as `[ dup + ] 3 power# .`, still a
        // quotation. A name's power and subscript stay in its name, `dup_2^3`,
        // for lookup to read where the name is applied, as it reads a path
        // (22, 25).
        let spelling;
        let mut scripts: Vec<(i128, &str)> = Vec::new();
        let (stem, applies) = if !r.symbolic && stem.len() > 1 && stem.starts_with(']') {
            spelling = crate::symbols::unscripted(&symbolize(stem, symbols));
            match crate::symbols::scripts(&spelling) {
                ("]", "", sup) if !sup.is_empty() => {
                    let n = sup.parse().map_err(|_| {
                        Error::new(
                            Kind::Limit,
                            Some(pos),
                            format!("the power {sup} is too large"),
                        )
                    })?;
                    scripts.push((n, "power#"));
                    ("]", false)
                }
                ("]", sub, _) if !sub.is_empty() => {
                    return Err(Error::new(
                        Kind::Syntax,
                        Some(pos),
                        format!(
                            "`{spelling}`: a quotation has no subscript; a subscript is an \
                             index of a word, which its family gives a meaning, as `dup_2`"
                        ),
                    ));
                }
                _ => (stem, applies),
            }
        } else {
            (stem, applies)
        };
        piece(
            stem,
            applies,
            variable,
            &scripts,
            marks,
            pos,
            at,
            !r.symbolic,
            symbols,
            &mut open,
            &mut out,
        )?;
        fold(&mut out)?;
    }
    if let Some((o, p)) = open.pop() {
        // Until 2026-10-20 `--` divided a bracket's inputs from its outputs.
        if dashed == Some(p) {
            return Err(Error::new(
                Kind::Syntax,
                Some(p),
                "`<` is never closed, a `--` in it beginning a comment: a signature's outputs \
                 are a bracket of their own, `< i64 >< i64 >`",
            ));
        }
        return Err(Error::new(
            Kind::Unfinished,
            Some(p),
            format!("`{}` is never closed", opening(o)),
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
/// lowercase letter is a variable, `a`, which `var#` gives; `name:` labels
/// what follows, a name being defined or a key; and `name'` is a symbol, a
/// word passed rather than applied, `+'`. A capital marks nothing (doc/design/SYMBOLIC-FORTH.md
/// 27): `Snake` applies, and the symbol is `Snake'`.
fn spelled(w: &str) -> (&str, bool) {
    let marked = |m: char| w.len() > m.len_utf8() && w.ends_with(m);
    if marked(':') || marked('\'') {
        (&w[..w.len() - 1], false)
    } else {
        (w, !is_variable(w))
    }
}

/// Whether a name is a variable: one lowercase letter, `a` to `z`, which
/// High Stack reads as `a var# .`, a type variable in a signature. No word
/// has its name.
pub fn is_variable(w: &str) -> bool {
    w.len() == 1 && w.as_bytes()[0].is_ascii_lowercase()
}

/// Folds a literal just read, `10 int# !`, `1.5 dec# !` or `1:2 ratio# !`,
/// into the token the intermediate form reads `10` as
/// (doc/design/SYMBOLIC-FORTH.md 6.7). It is `!` done early: the stage would
/// make the same literal while compiling, and made here it is one wherever a
/// literal is looked for, as a count or a value to match. A symbol not of its
/// kind, `1.5 int# !`, is left for `!`, which says why.
pub(crate) fn fold(out: &mut Vec<Token>) -> Result<()> {
    let [.., sym, kind, bang, apply] = &out[..] else {
        return Ok(());
    };
    let (Tok::Name(s), Tok::Name(k), Tok::Name(b), Tok::Apply) =
        (&sym.tok, &kind.tok, &bang.tok, &apply.tok)
    else {
        return Ok(());
    };
    if &**b != "!" || !matches!(&**k, "int#" | "dec#" | "ratio#") {
        return Ok(());
    }
    let lit = match (number(s, sym.pos)?, &**k) {
        (Some(t @ Tok::Int(_)), "int#")
        | (Some(t @ Tok::Dec(..)), "dec#")
        | (Some(t @ Tok::Ratio(_)), "ratio#") => t,
        _ => return Ok(()),
    };
    let pos = sym.pos;
    out.truncate(out.len() - 4);
    out.push(Token { tok: lit, pos });
    Ok(())
}

/// One word: its token, a number only if `high`, `var#` applied if it is a
/// variable, a quotation's power spelled out, its own application if it is a
/// name that applies, and for each mark after it, the first at `at`, an
/// application, `.`, or `!` applied. High Stack's brackets are Low Stack's
/// one, a quotation, made what each is by `!` where it closes
/// (doc/design/SYMBOLIC-FORTH.md 7): `( … )` is `[ … ] seq# !`, `{ … }` is
/// `[ … ] map# !` and `< … >` is `[ … ] sig# !`.
#[allow(clippy::too_many_arguments)]
fn piece(
    stem: &str,
    applies: bool,
    variable: bool,
    scripts: &[(i128, &str)],
    marks: &str,
    pos: Pos,
    at: Pos,
    high: bool,
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
        let mut made = None;
        let tok = match stem {
            "" => None,
            "(" | "{" | "<" if !high => {
                return Err(Error::new(
                    Kind::Syntax,
                    Some(pos),
                    format!(
                        "`{stem}` is High Stack's: Low Stack's one bracket is `[ … ]`, \
                         made data by `!`, `[ … ] seq# !`, `map# !` or `sig# !`"
                    ),
                ));
            }
            "[" | "(" | "{" | "<" => {
                open.push((stem.as_bytes()[0], pos));
                Some(Tok::Open(b'['))
            }
            "]" | ")" | "}" | ">" => {
                let b = stem.as_bytes()[0];
                match open.pop() {
                    Some((o, p)) if closer(o) == b => {
                        made = kind_of(o).map(|k| (k, p));
                        Some(Tok::Close(b']'))
                    }
                    Some((o, p)) => {
                        return Err(Error::new(
                            Kind::Syntax,
                            Some(pos),
                            format!("`{stem}` closes the `{}` at {p}", opening(o)),
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
            w => Some(
                if high { number(w, pos)? } else { None }.unwrap_or_else(|| {
                    Tok::Name(crate::symbols::unscripted(&symbolize(w, symbols)).into())
                }),
            ),
        };
        if let Some(tok) = tok {
            out.push(Token { tok, pos });
        }
        // The kind is where the bracket opened, which says it.
        if let Some((kind, opened)) = made {
            out.push(Token {
                tok: Tok::Name(kind.into()),
                pos: opened,
            });
            for tok in [Tok::Name("!".into()), Tok::Apply] {
                out.push(Token { tok, pos });
            }
        }
        if variable {
            out.push(Token {
                tok: Tok::Name("var#".into()),
                pos,
            });
            out.push(Token {
                tok: Tok::Apply,
                pos,
            });
        }
        for &(n, word) in scripts {
            out.push(Token {
                tok: Tok::Int(n),
                pos,
            });
            out.push(Token {
                tok: Tok::Name(word.into()),
                pos,
            });
            out.push(Token {
                tok: Tok::Apply,
                pos,
            });
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
                    tok: Tok::Name("!".into()),
                    pos,
                });
            }
            out.push(Token {
                tok: if m == '?' { Tok::Test } else { Tok::Apply },
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
    /// (march7/docs/STRINGS.md), as Low Stack writes it
    /// (doc/design/SYMBOLIC-FORTH.md 6.15, section 7): a quotation of its
    /// text, each run of it a symbol, and its holes' code, made text by
    /// `str# !`. `\_` is `_ .`. In a hole, its `]` may follow at once.
    fn string(&mut self, symbols: &Symbols, in_hole: bool) -> Result<Vec<Token>> {
        let start = self.pos();
        if self.symbolic {
            return Err(Error::new(
                Kind::Syntax,
                Some(start),
                "`\"…\"` is High Stack's: Low Stack's text is a quoted symbol, `'…'`, and a \
                 string of it and code `[ … ] str# !`",
            ));
        }
        self.bump();
        let mut s = String::new();
        // Where the text being read began.
        let mut from = self.pos();
        let mut out = vec![Token {
            tok: Tok::Open(b'['),
            pos: start,
        }];
        let text = |s: &mut String, from: Pos, out: &mut Vec<Token>| {
            if !s.is_empty() {
                out.push(Token {
                    tok: Tok::Name(std::mem::take(s).into()),
                    pos: from,
                });
            }
        };
        let close = loop {
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
                Some('"') => break here,
                Some('\\') => match self.bump() {
                    Some('\\') => s.push('\\'),
                    Some('"') => s.push('"'),
                    Some('[') => {
                        let code = tokens(self, symbols, Some(here))?;
                        text(&mut s, from, &mut out);
                        out.extend(code);
                        from = self.pos();
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
                            text(&mut s, from, &mut out);
                            for tok in [Tok::Name("_".into()), Tok::Apply] {
                                out.push(Token { tok, pos: here });
                            }
                            from = self.pos();
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
        };
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
        text(&mut s, from, &mut out);
        out.push(Token {
            tok: Tok::Close(b']'),
            pos: close,
        });
        // The kind is where the string opened, as a bracket's is.
        out.push(Token {
            tok: Tok::Name("str#".into()),
            pos: start,
        });
        for tok in [Tok::Name("!".into()), Tok::Apply] {
            out.push(Token { tok, pos: close });
        }
        Ok(out)
    }
}

/// Code in Symbolic Forth, the lowest form (doc/design/TYPES.md 2.2): every
/// name a symbol, each application a `.`, and a number a symbol given its
/// kind, `10 int# !`. It
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
                if (&**n == "!" || &**n == ";")
                    && toks.get(i + 1).is_some_and(|t| t.tok == Tok::Apply) =>
            {
                out.push_str(n);
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
    match tok {
        Tok::Int(n) => format!("{n} int# !"),
        Tok::Dec(d, s) => format!("{} dec# !", crate::prims::show_dec(*d, *s)),
        Tok::Ratio(t) => format!("{} ratio# !", crate::prims::show_ratio(t)),
        Tok::Name(n) => {
            let mut s = String::new();
            symbol(n, &mut s);
            s
        }
        Tok::Apply => ".".into(),
        Tok::Test => "?".into(),
        Tok::Open(b) | Tok::Close(b) => (*b as char).to_string(),
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
        // and stay in the name, for lookup (22, 25); a quotation's power is
        // a spelling (21), and a quotation has no subscript.
        let n = |s: &str| Tok::Name(s.into());
        let a = Tok::Apply;
        let want = [
            n("dup_2"),
            a.clone(),
            n("dup_2"),
            a.clone(),
            n("+^4"),
            a.clone(),
            n("dup_2^3"),
            a.clone(),
            n("log_2"),
            Tok::Open(b'['),
            n("q"),
            n("var#"),
            a.clone(),
            Tok::Close(b']'),
            Tok::Int(3),
            n("power#"),
            a.clone(),
            n("x_1"),
            n("Snake_2"),
            a,
        ];
        assert_eq!(toks("dup₂ dup\\_2 +⁴ dup₂³ log₂' [ q ]³ x₁: Snake₂"), want);
        let e = read("[ q ]₂", &Symbols::standard()).unwrap_err();
        assert_eq!(e.kind, Kind::Syntax);
    }

    #[test]
    fn a_word_not_a_number_is_a_name() {
        // `12kg` is a name, which applies, for a rule to read where it is
        // applied (doc/design/SYMBOLIC-FORTH.md 25); with a mark it is
        // passed, as any name is.
        let n = |s: &str| Tok::Name(s.into());
        assert_eq!(
            toks("12kg 12kg' 2.5"),
            [n("12kg"), Tok::Apply, n("12kg"), Tok::Dec(25, 1)]
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
            toks("1 -2 1.10 + [ xs ] < i64 >< i64 > -- a comment\n\\times"),
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
                Tok::Open(b'['),
                n("i64"),
                Tok::Apply,
                Tok::Close(b']'),
                n("sig#"),
                n("!"),
                Tok::Apply,
                Tok::Open(b'['),
                n("i64"),
                Tok::Apply,
                Tok::Close(b']'),
                n("sig#"),
                n("!"),
                Tok::Apply,
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
                Tok::Name("!".into()),
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
        // are symbols; one lowercase letter is a variable, `a var# .`; and
        // `name?` is tested, not applied.
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
                Tok::Open(b'['),
                n("string"),
                Tok::Apply,
                n("Camel"),
                n("a"),
                n("var#"),
                Tok::Apply,
                n("ary"),
                Tok::Apply,
                n("odd"),
                Tok::Test,
                Tok::Close(b']'),
                n("sig#"),
                n("!"),
                Tok::Apply,
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
            [
                n("io.file.read"),
                Tok::Apply,
                Tok::Dec(25, 1),
                n("2.i64"),
                Tok::Apply
            ]
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
                n("!"),
                Tok::Apply,
                Tok::Open(b'['),
                n("i64"),
                Tok::Apply,
                Tok::Close(b']'),
                n("sig#"),
                n("!"),
                Tok::Apply,
                n("!"),
                Tok::Apply,
            ]
        );
    }

    #[test]
    fn numbers_are_symbols_in_symbolic_forth() {
        // Digits mark nothing there (doc/design/SYMBOLIC-FORTH.md 6.7): a
        // number is a symbol given its kind, which reads as the literal the
        // intermediate form reads, in a quotation too.
        let s = Symbols::standard();
        let read = |src| -> Vec<Tok> {
            read_symbolic(src, &s)
                .unwrap()
                .into_iter()
                .map(|t| t.tok)
                .collect()
        };
        let n = |s: &str| Tok::Name(s.into());
        assert_eq!(
            read("10 -2 1.5 1:2"),
            [n("10"), n("-2"), n("1.5"), n("1:2")]
        );
        assert_eq!(
            read("10 int# ! -2 int#! 1.10 dec# ! 1:2 ratio# !"),
            toks("10 -2 1.10 1:2")
        );
        assert_eq!(
            read("[ 1 int# ! ] str# !"),
            [
                Tok::Open(b'['),
                Tok::Int(1),
                Tok::Close(b']'),
                n("str#"),
                n("!"),
                Tok::Apply
            ]
        );
        // A string is High Stack's.
        let e = read_symbolic(r#""\[ 1 int# ! ]""#, &s).unwrap_err();
        assert_eq!(e.kind, Kind::Syntax);
        // A symbol not of its kind is left for `!`, which says why; one that
        // is too large is an error here.
        assert_eq!(
            read("1.5 int# !"),
            [n("1.5"), n("int#"), n("!"), Tok::Apply]
        );
        assert_eq!(read("x int# !"), [n("x"), n("int#"), n("!"), Tok::Apply]);
        let e = read_symbolic("99999999999999999999999999999999999999999 int# !", &s);
        assert_eq!(e.unwrap_err().kind, Kind::Literal);
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
        let n = |s: &str| Tok::Name(s.into());
        assert_eq!(toks("'it''s'"), [n("it's")]);
        // A string is a quotation of its text and its holes' code, made text
        // by `str# !` (6.15, section 7), each run of text a symbol.
        let made = [Tok::Close(b']'), n("str#"), n("!"), Tok::Apply];
        let string = |inner: &[Tok]| -> Vec<Tok> {
            std::iter::once(Tok::Open(b'['))
                .chain(inner.iter().cloned())
                .chain(made.iter().cloned())
                .collect()
        };
        assert_eq!(
            toks(r#""a b\n;\"\\" 'C:\new' "\#x2603;\times;""#),
            [
                string(&[n("a b\n\"\\")]),
                vec![n("C:\\new")],
                string(&[n("☃×")]),
            ]
            .concat()
        );
        assert_eq!(
            toks(r#""\_ and \[ 1 2 + ]!" """#),
            [
                string(&[
                    n("_"),
                    Tok::Apply,
                    n(" and "),
                    Tok::Int(1),
                    Tok::Int(2),
                    n("+"),
                    Tok::Apply,
                    n("!"),
                ]),
                string(&[]),
            ]
            .concat()
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
