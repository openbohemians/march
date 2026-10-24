//! A bracket names its inputs (doc/design/SYMBOLIC-FORTH.md 6.31): names
//! before a lone `|`, in a signature's order, the deepest first, each the
//! input, copied where it is written. The bracket takes all it names. A
//! string or a literal inside it, made where it stands, sees them, each
//! passed in on the stack; a quotation, which may be carried off, does not.
//! The reader writes each name as the letter of its place, so Low Stack has
//! no names.
use march8::read::{Tok, lower, read, read_symbolic};
use march8::symbols::Symbols;
use march8::{Kind, Session};

fn session(pieces: &[&str]) -> Result<Session, Kind> {
    let mut s = Session::new();
    for p in pieces {
        s.eval(p).map_err(|e| e.kind)?;
    }
    Ok(s)
}

fn show(pieces: &[&str]) -> String {
    match session(pieces) {
        Ok(s) => s.show(),
        Err(k) => panic!("{pieces:?}: {k:?}"),
    }
}

fn error(src: &str) -> String {
    Session::new().eval(src).unwrap_err().to_string()
}

#[test]
fn a_name_is_an_input() {
    // In a signature's order: the last named is the top.
    assert_eq!(show(&["7 3 [ x y | x y -+ ] ."]), "<1> 4");
    assert_eq!(show(&["7 3 [ x y | y x -+ ] ."]), "<1> -4");
    // Written again, a copy.
    assert_eq!(show(&["3 [ n | n n * n + ] ."]), "<1> 12");
    // A name may be a letter, in the same order, and may hide a word.
    assert_eq!(show(&["7 3 [ a b | a b -+ ] ."]), "<1> 4");
    assert_eq!(show(&["7 3 [ drop dup | drop dup -+ ] ."]), "<1> 4");
    // A word's body is a bracket.
    let growth = "growth: < f64 f64 >. [ rate term | rate 1.0 + term ^ ] . < f64 >. ;";
    let annuity = "annuity: < f64 f64 f64 >. [ p r g | p r ⋅ g ⋅  g 1.0 -+  div ] . < f64 >. ;";
    let payment = "payment: < f64 f64 f64 >. [ principal rate term | principal rate  rate term growth  annuity ] . < f64 >. ;";
    assert_eq!(
        show(&[growth, annuity, payment, "50000.0 0.005 360.0 payment"]),
        "<1> 299.77526257637845"
    );
    // A literal names its inputs too, applied where it stands.
    assert_eq!(show(&["1 2 ( x y | y x )"]), "<1> ( 2 1 )");
}

#[test]
fn a_bracket_takes_all_it_names() {
    // Those its code does not use are dropped, the deepest among them.
    assert_eq!(show(&["1 2 3 [ x y z | y z + ] ."]), "<1> 5");
    assert_eq!(show(&["1 2 3", "[ x y z | x z + ] ."]), "<1> 4");
    assert_eq!(show(&["1 2", "[ x y | 5 ] ."]), "<1> 5");
    assert_eq!(show(&["1 2 ( x y | y )"]), "<1> ( 2 )");
    // Its code may reach below them, as a bracket of letters' may.
    assert_eq!(show(&["10 1 [ n | n + ] ."]), "<1> 11");
    // Given fewer, it says so, naming the letter, as for `_`.
    let e = error("1 [ x y | x y + ] .");
    assert!(
        e.contains("is the second input from the top, and 1 is in reach"),
        "{e}"
    );
}

#[test]
fn names_move_as_the_plain_words_do() {
    let mut s = Session::new();
    s.eval("1 2 3").unwrap();
    for (names, plain) in [
        ("[ x y | x y -+ ] .", "-+"),
        ("[ x y | y x -+ ] .", "swap -+"),
        ("[ x y | x y x + ] .", "over +"),
        ("[ x y z | y z + ] .", "rot drop +"),
        ("[ x y | 5 ] .", "drop drop 5"),
        (
            "[ p r g | p r * g * g 1 -+ div ] .",
            "[ c b * a * a 1 -+ div ] .",
        ),
    ] {
        let a = s.compile(names).unwrap().0;
        let b = s.compile(plain).unwrap().0;
        assert_eq!(a, b, "{names}");
    }
}

#[test]
fn a_quotation_sees_only_its_own_names() {
    // A quotation may be carried off, so it is given the value on the stack,
    // and nothing is captured.
    assert_eq!(
        show(&["( 1 2 3 ) 10 [ xs k | xs k + ] ."]),
        "<1> ( 11 12 13 )"
    );
    assert_eq!(
        show(&["( 1 2 3 ) 10 [ xs k | xs [ x | x x * ] map k + ] ."]),
        "<1> ( 11 14 19 )"
    );
    // Naming the enclosing bracket's name there is refused, unless it names
    // it itself, and a literal around it does not open it.
    let e = error("( 1 2 ) 10 [ xs k | xs [ k + ] map ] .");
    assert!(
        e.contains(
            "1:26: `k` is a name of the bracket at 1:12, and a quotation sees no names but its \
             own"
        ),
        "{e}"
    );
    assert!(error("1 [ x | [ x ] . ] .").contains("`x` is a name of the bracket at 1:3"));
    assert!(error("1 [ x | ( [ x ] ) ] .").contains("`x` is a name of the bracket at 1:3"));
    // A symbol is not a name: `x'` is text.
    assert_eq!(show(&["5 [ x | x' ] ."]), "<1> x'");
}

#[test]
fn a_string_or_a_literal_sees_the_names_around_it() {
    // It is made where it stands, so each name is passed in on the stack.
    assert_eq!(show(&["1 2 [ x y | ( x y ) ] ."]), "<1> ( 1 2 )");
    assert_eq!(show(&["5 [ n | \"n is \\[ n ]\" ] ."]), "<1> \"n is 5\"");
    assert_eq!(
        show(&["72 3 [ t r | { term: t rate: r } ] ."]),
        "<1> { rate' 3 term' 72 }"
    );
    // Through a literal inside a literal, and beside its own inputs, which
    // lie below those passed in.
    assert_eq!(
        show(&["1 2 [ x y | ( ( x ) y ( y x ) ) ] ."]),
        "<1> ( ( 1 ) 2 ( 2 1 ) )"
    );
    assert_eq!(show(&["3 [ n | ( \"\\[ n ]\" n ) ] ."]), "<1> ( \"3\" 3 )");
    assert_eq!(
        show(&["9 5 [ n | \"\\_ and \\[ n ]\" ] ."]),
        "<1> \"9 and 5\""
    );
    assert_eq!(show(&["9 1 2 [ a b | ( x | x b ) ] ."]), "<1> ( 9 2 )");
    // The same code as passing them by hand.
    let mut s = Session::new();
    s.eval("1 2").unwrap();
    for (names, hand) in [
        ("[ x y | ( x y ) ] .", "[ x y | x y ( _ _ ) ] ."),
        ("[ x y | ( y x y ) ] .", "[ x y | y x ( b a b ) ] ."),
        ("[ n | \"n is \\[ n ]\" ] .", "[ n | n \"n is \\_\" ] ."),
    ] {
        let a = s.compile(names).unwrap().0;
        let b = s.compile(hand).unwrap().0;
        assert_eq!(a, b, "{names}");
    }
}

#[test]
fn only_names_come_before_the_bar() {
    let says = |src: &str, msg: &str| {
        let e = error(src);
        assert!(e.contains(msg), "{src}: {e}");
    };
    says("[ 1 | 2 ] .", "only names come before a bracket's `|`");
    says("[ x 1 + | 2 ] .", "1:5: only names come before");
    says("[ x' | 2 ] .", "only names come before");
    says("[ io.read | 2 ] .", "only names come before");
    says("[ | 2 ] .", "this one names none");
    says("[ x | 1 | 2 ] .", "a bracket has one `|`");
    says("1 | 2", "this one is in no bracket");
    says("[ x x | x ] .", "`x` names two of the bracket's inputs");
    // No other letter and no `_`.
    says(
        "[ x | x a + ] .",
        "uses no other letter, and `a` is none of its names",
    );
    says("[ x | _ ] .", "names its inputs, so it has no `_`");
    // A signature's letters are types, and a string's text comes first.
    says("< x | x > !", "a signature names no inputs");
    says("\"\\[ x | x ]\"", "a string names no inputs");
}

/// The tokens of source, wherever they were written.
fn toks(src: &str, symbolic: bool) -> Vec<Tok> {
    let s = Symbols::standard();
    let t = if symbolic {
        read_symbolic(src, &s)
    } else {
        read(src, &s)
    };
    t.unwrap().into_iter().map(|t| t.tok).collect()
}

#[test]
fn low_stack_has_letters() {
    // A name is the letter of its place, `a var# .`, and the bar goes.
    let lowered = |src: &str| {
        let low = lower(&read(src, &Symbols::standard()).unwrap());
        assert_eq!(toks(src, false), toks(&low, true), "{low}");
        low
    };
    assert_eq!(
        lowered("[ rate term | rate 1 + term ^ ]"),
        "[ b var# . 1 int# ! + . a var# . ^ . ]"
    );
    // The deepest, unused, is written once and dropped after the letters the
    // code begins with.
    assert_eq!(
        lowered("[ x y z | y z + ]"),
        "[ b var# . a var# . c var# . drop . + . ]"
    );
    assert_eq!(lowered("[ x y | 5 ]"), "[ b var# . drop . 5 int# ! ]");
    // A name a literal sees is passed in, and taken there by its letter.
    assert_eq!(
        lowered("[ x y | ( x y ) ]"),
        "[ b var# . a var# . [ b var# . a var# . ] seq# ! ]"
    );
    // In Low Stack a bar is a symbol, as any name is.
    assert_eq!(toks("[ x | x . ]", true)[2], Tok::Name("|".into()));
}
