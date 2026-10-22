//! Symbolic Forth, the lowest form (doc/design/TYPES.md 2.2; Thomas,
//! 2026-10-13): symbols, `.` and `!`. Every name is a symbol until `.`
//! applies it, a type is given by applying its word, `i64 .`, `!` converts,
//! and a number is a symbol given its kind, `10 int# !`. The form march8
//! parses is an intermediate one over it, and this is its litmus test: all
//! of it lowers to Symbolic Forth and parses back the same, and the lowered
//! programs run.
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
        "fib [ n var# . ] sig# ! [ dup . 1 int# ! -+ . fib . swap . 2 int# ! -+ . fib . + . ] ;"
    );
    // A bracket is a quotation made data where it closes (7, step 3): a
    // sequence, a map, or a signature, its code run over types.
    assert_eq!(
        lowered("( 1 2 ) { a: 1 } < ( i64 string ) >< i64 >"),
        "[ 1 int# ! 2 int# ! ] seq# ! [ a 1 int# ! ] map# ! \
         [ [ i64 . string . ] seq# ! ] sig# ! [ i64 . ] sig# !"
    );
    // A symbol, a label or a word passed is a symbol, and a type is given
    // by applying its word, then `!` converts. A number is a symbol given
    // its kind (doc/design/SYMBOLIC-FORTH.md 6.7), and a letter a
    // variable, `x var# .` (7).
    assert_eq!(
        lowered("Closed' Open' door ( 1 2 ) +' reduce 2 i64! x y < i64 f64 > !"),
        "Closed Open door . [ 1 int# ! 2 int# ! ] seq# ! + reduce . 2 int# ! i64 . ! \
         x var# . y var# . [ i64 . f64 . ] sig# ! !"
    );
    // A name keeps its subscript and its power, for a rule to rewrite where
    // the name is applied, and a quotation's power is `power#` (6.21, 6.22).
    assert_eq!(
        lowered("-2 1.10 1:2 12:30:15 '10' dup_2 +⁴ [ dup + ]^3"),
        "-2 int# ! 1.10 dec# ! 1:2 ratio# ! 12:30:15 ratio# ! 10 \
         dup_2 . +^4 . [ dup . + . ] 3 int# ! power# ."
    );
    // Strings keep their escapes, and a hole its code, lowered too.
    assert_eq!(
        lowered(r#""say \"hi\" \\ \_ and \[ 1 2 + ]""#),
        r#""say \"hi\" \\ \[ _ . ] and \[ 1 int# ! 2 int# ! + . ]""#
    );
}

#[test]
fn the_lowest_form_written_inline() {
    // `[| … |]` is a quotation read as Symbolic Forth (SYMBOLIC-FORTH.md
    // 3), brackets inside it and all.
    assert_eq!(
        toks("dup [| sq math.pi 2 [ dup . ] |] dup", false),
        toks("dup . [ sq math.pi 2 [ dup . ] ] dup .", true)
    );
    // Only `|]` closes it, and it closes nothing else.
    let error = |src| read(src, &Symbols::standard()).unwrap_err().msg;
    assert_eq!(error("[| sq ]"), "`]` closes the `[|` at 1:1");
    assert_eq!(error("[ sq |]"), "`|]` closes the `[` at 1:1");
    assert_eq!(error("sq |]"), "`|]` closes nothing");
    assert_eq!(error("[| sq"), "`[|` is never closed");
    // Low Stack has one bracket.
    assert!(error("[| ( 1 ) |]").starts_with("`(` is High Stack's"));
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
    // the program as Symbolic Forth. The syllogism's rule rewrites its
    // capitalized names where `.` applies them, as it does where they are
    // written (doc/design/SYMBOLIC-FORTH.md 25). The summary reads its
    // input, and runs in tests/programs.rs.
    for src in &EXAMPLES[..3] {
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
    s.eval("sq [ dup . * . ] ; 3 int# ! sq . 2.5 dec# ! sq . 2 int# ! f64 ! \"42\" i64 !")
        .unwrap();
    assert_eq!(s.show(), "<4> 9 6.25 2.0 42");
    // A subscript and a power are the core's rules, which rewrite a name
    // where `.` applies it, as in High Stack.
    let mut s = Session::new();
    s.symbolic = true;
    s.eval("1 int# ! 2 int# ! dup_2 . +^4 .").unwrap();
    assert_eq!(s.show(), "<1> 6");
    // `!` is a meta-op, as `;` is (doc/design/SYMBOLIC-FORTH.md 6.5): it
    // structures the value below by the type on top, given or named, and no
    // word `convert` remains.
    let mut s = Session::new();
    s.symbolic = true;
    s.eval("7 int# ! i64 . ! 7 int# ! f64 !").unwrap();
    assert_eq!(s.show_typed(), "<2> 7 < i64 > 7.0 < f64 >");
    let e = s.eval("7 int# ! i64 . convert .").unwrap_err();
    assert_eq!(e.kind, march8::Kind::NoWord);
    // Digits mark nothing (6.7): a number unstructured is a symbol, shown
    // quoted, as March reads it, and any symbol of digits is a number by its
    // kind, or by a type.
    let mut s = Session::new();
    s.symbolic = true;
    s.eval("10 10 int# ! '10' i64 !").unwrap();
    assert_eq!(s.show_typed(), "<3> '10' < sym > 10 < i64 > 10 < i64 >");
    // There, nothing applies unless a dot says so: a name is a symbol, a
    // value, at run time too.
    let mut s = Session::new();
    s.symbolic = true;
    s.eval("3 int# ! dup .").unwrap();
    assert_eq!(s.show(), "<2> 3 3");
    let mut s = Session::new();
    s.symbolic = true;
    s.eval("3 int# ! dup Snake").unwrap();
    assert_eq!(s.show_typed(), "<3> 3 < i64 > dup' < sym > Snake' < sym >");
}

#[test]
fn a_letter_is_a_variable_and_its_symbol_a_value() {
    // A signature's letter is `a var# .`, a type its bracket fills by
    // matching, and a bare letter in Low Stack is a symbol, a value to match,
    // as `a'` is in High Stack (SYMBOLIC-FORTH.md 7, step 2).
    assert_eq!(
        lowered("ff: < a' a >< a > [ drop ] ;"),
        "ff [ a a var# . ] sig# ! [ a var# . ] sig# ! [ drop . ] ;"
    );
    let high = "ff: < a' > [ drop 1 ] ; ff: < a > [ drop 2 ] ; 7 ff a' ff";
    let mut s = Session::new();
    s.eval(high).unwrap();
    assert_eq!(s.show(), "<2> 2 1");
    let mut s = Session::new();
    s.symbolic = true;
    s.eval(
        "ff [ a ] sig# ! [ drop . 1 int# ! ] ; ff [ a var# . ] sig# ! [ drop . 2 int# ! ] ; \
         7 int# ! ff . a ff .",
    )
    .unwrap();
    assert_eq!(s.show(), "<2> 2 1");
    // In a body a letter is an input by its place, not built yet.
    let e = Session::new().eval("1 [ a ] .").unwrap_err();
    assert_eq!(e.kind, march8::Kind::Limit);
    assert!(e.msg.contains("the symbol is `a'`"), "{}", e.msg);
}
