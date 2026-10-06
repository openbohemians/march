//! The formatter's symbol rewrite (docs/SURFACE.md): `\times` in March
//! source is the word `×`. March's reader accepts either spelling itself; this
//! module only rewrites text, so a formatted file reads the same as its
//! source. The table is the one the reader uses, `symbol-table` in
//! seed/system.march, so the two cannot disagree.

use std::collections::HashMap;

/// LaTeX-style names and their symbols.
pub struct Symbols {
    table: HashMap<String, String>,
}

impl Symbols {
    /// The table in seed/system.march.
    pub fn standard() -> Symbols {
        let source = include_str!("../seed/system.march");
        let start = source
            .find(": symbol-table s\"\n")
            .expect("system.march defines symbol-table")
            + ": symbol-table s\"\n".len();
        let length = source[start..].find('"').expect("the table ends");
        Symbols::parse(&source[start..start + length]).expect("the table is well formed")
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

    /// Rewrites each `\name` in code and comments as its symbol. Strings are
    /// left alone, except the code in their holes, and so is a name that is
    /// not in the table.
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
                // A `--` word starts a comment, which runs to the end of the line.
                if word == "--" {
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
                // A raw string runs to the next `'`; `'` alone quotes a word.
                if b == b'\'' && word.len() > 1 {
                    let end = source[i + 1..]
                        .find('\'')
                        .map_or(bytes.len(), |n| i + n + 2);
                    out.push_str(&source[i..end]);
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
