//! Symbols (march7/docs/SURFACE.md): `\times` in March source is the word
//! `×`, and `\times;` in a string is the character. The reader applies the
//! table; the formatter rewrites text with it, so a formatted file reads the
//! same as its source. The table is core/symbols.txt.

use std::collections::HashMap;

/// LaTeX-style names and their symbols.
pub struct Symbols {
    table: HashMap<String, String>,
}

impl Symbols {
    /// The table in core/symbols.txt.
    pub fn standard() -> Symbols {
        Symbols::parse(include_str!("../core/symbols.txt")).expect("the table is well formed")
    }

    /// Parses one entry per line: a name, one space, a symbol.
    pub fn parse(text: &str) -> Result<Symbols, String> {
        let mut table = HashMap::new();
        for line in text.lines() {
            let (name, symbol) = line
                .split_once(' ')
                .ok_or_else(|| format!("no symbol: {line:?}"))?;
            if name.is_empty() || symbol.is_empty() || symbol.contains(' ') {
                return Err(format!("malformed entry: {line:?}"));
            }
            if table.insert(name.to_string(), symbol.to_string()).is_some() {
                return Err(format!("duplicate name: {name}"));
            }
        }
        Ok(Symbols { table })
    }

    /// The symbol for a name, if there is one.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.table.get(name).map(String::as_str)
    }

    /// Every entry, as (name, symbol).
    pub fn entries(&self) -> impl Iterator<Item = (&str, &str)> {
        self.table.iter().map(|(n, s)| (n.as_str(), s.as_str()))
    }

    /// Rewrites each `\name` in code and comments as its symbol, and a
    /// name's trailing `_` and digits as subscript digits and `^` and digits
    /// as superscript digits. Strings are left alone, except the code in
    /// their holes, and so is a name that is not in the table.
    pub fn format(&self, source: &str) -> String {
        let mut out = String::with_capacity(source.len());
        self.code(source, 0, false, &mut out);
        out
    }

    /// Formats code from byte `i` to the end of the source, or, in a string's
    /// hole, through the `]` that closes it, and returns where it stopped. As
    /// in the reader, a `]` closes a hole wherever a word would start at the
    /// hole's own depth, outside any quotation opened in it.
    fn code(&self, source: &str, mut i: usize, hole: bool, out: &mut String) -> usize {
        let bytes = source.as_bytes();
        let mut token_start = true;
        let mut depth = 0usize;
        while i < bytes.len() {
            let b = bytes[i];
            if token_start && !b.is_ascii_whitespace() {
                let end = bytes[i..]
                    .iter()
                    .position(u8::is_ascii_whitespace)
                    .map_or(bytes.len(), |n| i + n);
                let word = &source[i..end];
                // A line of three or more `-` opens a block comment, written
                // as it is, escapes aside, through the line that closes it.
                let line_start = source[..i].rfind('\n').map_or(0, |n| n + 1);
                let line_end = source[i..].find('\n').map_or(bytes.len(), |n| i + n);
                if source[line_start..i].trim().is_empty()
                    && crate::read::opens_block(&source[i..line_end])
                {
                    let mut end = line_end;
                    while end < bytes.len() {
                        let next = source[end + 1..]
                            .find('\n')
                            .map_or(bytes.len(), |n| end + 1 + n);
                        let closing = crate::read::closes_block(&source[end + 1..next]);
                        end = next;
                        if closing {
                            break;
                        }
                    }
                    out.push_str(&self.rewrite(&source[i..end]));
                    i = end;
                    continue;
                }
                // A `--` word starts a comment, which runs to the end of the line.
                if word.starts_with("--") {
                    let end = source[i..].find('\n').map_or(bytes.len(), |n| i + n);
                    out.push_str(&self.rewrite(&source[i..end]));
                    i = end;
                    continue;
                }
                if hole {
                    if b == b']' && depth == 0 {
                        out.push(']');
                        return i + 1;
                    }
                    match word {
                        "[" => depth += 1,
                        "]" => depth = depth.saturating_sub(1),
                        _ => {}
                    }
                }
                // A word that starts with `"` is a string literal.
                if b == b'"' {
                    i = self.string(source, i, out);
                    token_start = false;
                    continue;
                }
                // A quoted symbol runs to the next `'` not doubled
                // (doc/design/SYMBOLIC-FORTH.md 6.12); `'` alone quotes a
                // word.
                if b == b'\'' && word.len() > 1 {
                    let mut end = bytes.len();
                    let mut j = i + 1;
                    while let Some(n) = source[j..].find('\'') {
                        if bytes.get(j + n + 1) == Some(&b'\'') {
                            j += n + 2;
                        } else {
                            end = j + n + 1;
                            break;
                        }
                    }
                    out.push_str(&source[i..end]);
                    i = end;
                    token_start = false;
                    continue;
                }
                // A word with no raw text in it, whole, its escapes and then
                // its end rewritten.
                if !word.contains('"') {
                    out.push_str(&scripted(&self.rewrite(word)));
                    i = end;
                    token_start = false;
                    continue;
                }
            }
            // A quote inside a word, as in `s" …"`, starts raw text that runs
            // to the next quote, as in the reader.
            if b == b'"' {
                let end = source[i + 1..].find('"').map_or(bytes.len(), |n| i + n + 2);
                out.push_str(&source[i..end]);
                i = end;
                token_start = false;
                continue;
            }
            if b == b'\\'
                && let Some((length, symbol)) = self.escape(&source[i..])
            {
                out.push_str(symbol);
                i += length;
                token_start = false;
                continue;
            }
            // Copy one character, whole, so multi-byte text stays intact.
            let c = source[i..].chars().next().expect("a character");
            out.push(c);
            token_start = c.is_whitespace();
            i += c.len_utf8();
        }
        i
    }

    /// Copies the string literal at byte `i` (its opening quote) as it is,
    /// formatting the code in its holes, and returns the byte after it. An
    /// escape is a backslash and the character after it, so `\"` does not end
    /// the string.
    fn string(&self, source: &str, mut i: usize, out: &mut String) -> usize {
        let bytes = source.as_bytes();
        out.push('"');
        i += 1;
        while i < bytes.len() {
            match bytes[i] {
                b'"' => {
                    out.push('"');
                    return i + 1;
                }
                b'\\' if bytes.get(i + 1) == Some(&b'[') => {
                    out.push_str("\\[");
                    i = self.code(source, i + 2, true, out);
                }
                _ => {
                    let c = source[i..].chars().next().expect("a character");
                    out.push(c);
                    i += c.len_utf8();
                    if c == '\\'
                        && let Some(c) = source[i..].chars().next()
                    {
                        out.push(c);
                        i += c.len_utf8();
                    }
                }
            }
        }
        i
    }

    /// Rewrites every escape in a comment.
    fn rewrite(&self, text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        let mut i = 0;
        while i < text.len() {
            if text.as_bytes()[i] == b'\\'
                && let Some((length, symbol)) = self.escape(&text[i..])
            {
                out.push_str(symbol);
                i += length;
                continue;
            }
            let c = text[i..].chars().next().expect("a character");
            out.push(c);
            i += c.len_utf8();
        }
        out
    }

    /// The escape at the start of `text` (which begins with a backslash), as
    /// its length in bytes and its symbol. The name is `_` or `^` with the
    /// character after it, or the longest run of ASCII letters, the same rule
    /// as March's reader.
    fn escape(&self, text: &str) -> Option<(usize, &str)> {
        let rest = &text.as_bytes()[1..];
        let length = match rest.first()? {
            b'_' | b'^' => 1 + text[2..].chars().next()?.len_utf8(),
            _ => rest.iter().take_while(|b| b.is_ascii_alphabetic()).count(),
        };
        if length == 0 {
            return None;
        }
        let symbol = self.get(&text[1..1 + length])?;
        Some((1 + length, symbol))
    }
}

const SUBSCRIPTS: [char; 10] = ['₀', '₁', '₂', '₃', '₄', '₅', '₆', '₇', '₈', '₉'];
const SUPERSCRIPTS: [char; 10] = ['⁰', '¹', '²', '³', '⁴', '⁵', '⁶', '⁷', '⁸', '⁹'];

/// A word as `fmt` writes it (doc/design/SYMBOLIC-FORTH.md 23): a name's
/// trailing `_` and digits as subscript digits, and `^` and digits after
/// them as superscript digits, before any marks, so `dup_2` is `dup₂`, `+^4`
/// is `+⁴` and `dup_2^3:` is `dup₂³:`. Only digits, and only at the end, so
/// `file_read` is as it was, and so is a number.
pub fn scripted(word: &str) -> String {
    let digits = word.strip_prefix('-').unwrap_or(word);
    if digits.starts_with(|c: char| c.is_ascii_digit()) {
        return word.to_string();
    }
    let name = word.trim_end_matches(['.', '!', ':', '\'']);
    let marks = &word[name.len()..];
    let (name, sub, sup) = scripts(name);
    if name.is_empty() {
        return word.to_string();
    }
    let digit = |table: &[char; 10], d: u8| table[(d - b'0') as usize];
    let mut out = name.to_string();
    out.extend(sub.bytes().map(|d| digit(&SUBSCRIPTS, d)));
    out.extend(sup.bytes().map(|d| digit(&SUPERSCRIPTS, d)));
    out.push_str(marks);
    out
}

/// A name's width and power (doc/design/SYMBOLIC-FORTH.md 21, 22): what
/// comes before them, and the digits of each, or none. `dup_2^3` is `dup`,
/// "2" and "3", and `dup` is itself and two empty strings.
pub fn scripts(name: &str) -> (&str, &str, &str) {
    let (name, sup) = trailing(name, '^');
    let (name, sub) = trailing(name, '_');
    (name, sub, sup)
}

/// A name and the digits after its last `mark`, if it ends in the mark and
/// digits; else the name and none.
fn trailing(name: &str, mark: char) -> (&str, &str) {
    let stem = name.trim_end_matches(|c: char| c.is_ascii_digit());
    match stem.strip_suffix(mark) {
        Some(s) if stem.len() < name.len() => (s, &name[stem.len()..]),
        _ => (name, ""),
    }
}

/// A name as the reader keeps it: its trailing subscript and superscript
/// digits spelled `_` and `^` and digits, the spelling typed, so `dup₂` is
/// `dup_2` and `+⁴` is `+^4`.
pub fn unscripted(name: &str) -> String {
    let sup_at = name.trim_end_matches(|c| SUPERSCRIPTS.contains(&c)).len();
    let sub_at = name[..sup_at]
        .trim_end_matches(|c| SUBSCRIPTS.contains(&c))
        .len();
    if sub_at == 0 || sub_at == name.len() {
        return name.to_string();
    }
    let ascii = |s: &str, table: &[char; 10]| -> String {
        s.chars()
            .map(|c| (b'0' + table.iter().position(|&t| t == c).expect("a digit") as u8) as char)
            .collect()
    };
    let mut out = name[..sub_at].to_string();
    if sub_at < sup_at {
        out.push('_');
        out.push_str(&ascii(&name[sub_at..sup_at], &SUBSCRIPTS));
    }
    if sup_at < name.len() {
        out.push('^');
        out.push_str(&ascii(&name[sup_at..], &SUPERSCRIPTS));
    }
    out
}
