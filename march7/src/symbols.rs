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
    /// left alone, and so is a name that is not in the table.
    pub fn format(&self, source: &str) -> String {
        let bytes = source.as_bytes();
        let mut out = String::with_capacity(source.len());
        let mut i = 0;
        let mut token_start = true;
        while i < bytes.len() {
            let b = bytes[i];
            // A `--` word starts a comment, which runs to the end of the line.
            if token_start
                && bytes[i..].starts_with(b"--")
                && bytes.get(i + 2).is_none_or(|c| c.is_ascii_whitespace())
            {
                let end = source[i..].find('\n').map_or(bytes.len(), |n| i + n);
                out.push_str(&self.rewrite(&source[i..end]));
                i = end;
                continue;
            }
            // A string runs to the next quote, as `s" …"` does in the reader;
            // strings have no escapes yet.
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
        out
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
