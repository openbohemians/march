//! Symbolic Forth, the lowest form (doc/design/TYPES.md 2.2; Thomas,
//! 2026-10-13): numbers, symbols, `.` and `!`. Every name is a symbol until
//! `.` applies it, a type is given by applying its word, `i64 .`, and `!`
//! converts. The form march8 reads is an intermediate one over it, and this
//! is its litmus test: all of it lowers to Symbolic Forth and reads back the
//! same, and the lowered programs run.
use march8::read::{Piece, Tok, Token, lower, read, read_symbolic};
use march8::symbols::Symbols;
use march8::{CORE, Pos, Session};

/// The tokens of source, wherever they were written.
fn toks(src: &str, symbolic: bool) -> Vec<Tok> {
    let s = Symbols::standard();
    let t = if symbolic {
        read_symbolic(src, &s)
    } else {
        read(src, &s)
    };
    t.unwrap().into_iter().map(placeless).collect()
}

fn placeless(t: Token) -> Tok {
    match t.tok {
        Tok::Template(pieces) => Tok::Template(
            pieces
                .iter()
                .map(|p| match p {
                    Piece::Hole(code) => Piece::Hole(
                        code.iter()
                            .map(|t| Token {
                                tok: placeless(t.clone()),
                                pos: Pos { line: 0, col: 0 },
                                word: None,
                            })
                            .collect(),
                    ),
                    text => text.clone(),
                })
                .collect(),
        ),
        tok => tok,
    }
}

/// The source lowered to Symbolic Forth, checked to read back to the same
/// tokens.
fn lowered(src: &str) -> String {
    let low = lower(&read(src, &Symbols::standard()).unwrap());
    assert_eq!(toks(src, false), toks(&low, true), "{low}");
    low
}

const EXAMPLES: [&str; 4] = [
    include_str!("../examples/wordfreq.march"),
    include_str!("../examples/door.march"),
    include_str!("../examples/syllogism.march"),
    include_str!("../examples/summary.march"),
];

#[test]
fn every_application_is_a_dot() {
    assert_eq!(
        lowered("fib: < n > [ dup 1 -+ fib swap 2 -+ fib + ] ;"),
        "fib < n > [ dup . 1 -+ . fib . swap . 2 -+ . fib . + . ] ;"
    );
    // A symbol, a label or a word passed is a symbol, and a type is given
    // by applying its word, then `!` converts.
    assert_eq!(
        lowered("Closed' Open' door ( 1 2 ) +' reduce 2 i64! x y < i64 f64 > !"),
        "Closed Open door . ( 1 2 ) + reduce . 2 i64 . ! x y < i64 . f64 . > !"
    );
    // Strings keep their escapes, and a hole its code, lowered too.
    assert_eq!(
        lowered(r#""say \"hi\" \\ \_ and \[ 1 2 + ]""#),
        r#""say \"hi\" \\ \[ _ . ] and \[ 1 2 + . ]""#
    );
}

#[test]
fn the_core_vocabulary_lowers() {
    lowered(CORE);
}

#[test]
fn the_examples_lower_and_run_the_same() {
    for src in EXAMPLES {
        lowered(src);
    }
    // Run lowered, a program does what it did: core is read as it is, and
    // the program as Symbolic Forth. Not yet one with a pattern word, the
    // syllogism's: `lower` writes a word a pattern reads as the core reads
    // it (doc/design/SYMBOLIC-FORTH.md 25, open).
    for src in &EXAMPLES[..2] {
        let mut a = Session::new();
        a.eval(src).unwrap();
        let mut b = Session::new();
        b.symbolic = true;
        b.eval(&lowered(src)).unwrap();
        assert_eq!(a.show(), b.show());
        assert_eq!(a.take_output(), b.take_output());
    }
}

#[test]
fn symbolic_forth_runs() {
    let mut s = Session::new();
    s.symbolic = true;
    s.eval("sq [ dup . * . ] ; 3 sq . 2.5 sq . 2 f64 ! \"42\" i64 !")
        .unwrap();
    assert_eq!(s.show(), "<4> 9 6.25 2.0 42");
    // `!` is `convert .`, a convenience: the word that structures a value
    // by a type, applied.
    let mut s = Session::new();
    s.symbolic = true;
    s.eval("7 i64 convert . 7 f64 !").unwrap();
    assert_eq!(s.show_typed(), "<2> 7 < i64 > 7.0 < f64 >");
    // There, nothing applies unless a dot says so: a name is a symbol, a
    // value, at run time too.
    let mut s = Session::new();
    s.symbolic = true;
    s.eval("3 dup .").unwrap();
    assert_eq!(s.show(), "<2> 3 3");
    let mut s = Session::new();
    s.symbolic = true;
    s.eval("3 dup Snake").unwrap();
    assert_eq!(s.show_typed(), "<3> 3 < i64 > dup' < sym > Snake' < sym >");
}
