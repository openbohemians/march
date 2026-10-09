//! Values written by their types, as March reads them back.

use crate::machine::Machine;
use crate::prims::show_dec;
use crate::types::{Base, Term, Type, Types};

/// Elements shown of a collection, before the count of the rest.
const SHOWN: usize = 16;

pub fn value(m: &Machine, types: &Types, v: u64, t: Type, out: &mut String) {
    match types.term(t) {
        Term::Base(Base::I64) => out.push_str(&(v as i64).to_string()),
        Term::Base(Base::F64) => out.push_str(&format!("{:?}", f64::from_bits(v))),
        Term::Base(Base::Money) => out.push_str(&show_dec((v as i64).into(), 2)),
        Term::Base(Base::Nil) => out.push_str("nil"),
        // A union's value, by the type its tag names.
        Term::Or(..) => match m.sequence(v) {
            Ok(seq) if seq.len() == 2 => {
                let mut cells = seq.iter();
                let (&tag, &x) = (cells.next().expect("two"), cells.next().expect("two"));
                value(m, types, x, tag as Type, out)
            }
            _ => out.push_str(&format!("<union {v}>")),
        },
        Term::Base(Base::String) => match m.text(v).ok().and_then(|t| t.to_text()) {
            Some(s) => quoted(&s, out),
            None => out.push_str(&format!("<string {v}>")),
        },
        Term::Ary(e) | Term::Vec(_, e) => {
            let Ok(seq) = m.sequence(v) else {
                out.push_str(&format!("<array {v}>"));
                return;
            };
            out.push('(');
            for &x in seq.iter().take(SHOWN) {
                out.push(' ');
                value(m, types, x, e, out);
            }
            if seq.len() > SHOWN {
                out.push_str(&format!(" … {} more", seq.len() - SHOWN));
            }
            out.push_str(" )");
        }
        Term::Tuple(_) => {
            let es = types.elements(t).expect("a tuple").to_vec();
            let Ok(seq) = m.sequence(v) else {
                out.push_str(&format!("<tuple {v}>"));
                return;
            };
            out.push('(');
            for (&x, e) in seq.iter().zip(es) {
                out.push(' ');
                value(m, types, x, e, out);
            }
            out.push_str(" )");
        }
        Term::Map(k, val) => {
            let Ok(map) = m.map(v) else {
                out.push_str(&format!("<map {v}>"));
                return;
            };
            let mut entries: Vec<(String, u64, u64)> = map
                .iter()
                .map(|(&key, &x)| {
                    let mut s = String::new();
                    value(m, types, key, k, &mut s);
                    (s, key, x)
                })
                .collect();
            // By key, as `sort` orders: numbers by value, strings by text.
            match types.term(k) {
                Term::Base(Base::I64 | Base::Money) => entries.sort_by_key(|e| e.1 as i64),
                Term::Base(Base::F64) => {
                    entries.sort_by(|x, y| f64::from_bits(x.1).total_cmp(&f64::from_bits(y.1)))
                }
                Term::Base(Base::String) => {
                    entries.sort_by_cached_key(|e| m.text(e.1).ok().and_then(|t| t.to_text()))
                }
                _ => entries.sort(),
            }
            out.push('{');
            for (key, _, x) in entries.iter().take(SHOWN) {
                out.push(' ');
                out.push_str(key);
                out.push(' ');
                value(m, types, *x, val, out);
            }
            if entries.len() > SHOWN {
                out.push_str(&format!(" … {} more", entries.len() - SHOWN));
            }
            out.push_str(" }");
        }
        _ => out.push_str(&(v as i64).to_string()),
    }
}

/// A string as a literal: escapes for `\`, `"` and control characters.
pub fn quoted(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '\\' | '"' => {
                out.push('\\');
                out.push(c);
            }
            '\n' => out.push_str("\\n;"),
            '\t' => out.push_str("\\t;"),
            '\r' => out.push_str("\\r;"),
            c if c.is_control() => out.push_str(&format!("\\#{};", u32::from(c))),
            c => out.push(c),
        }
    }
    out.push('"');
}
