//! Letters as places (doc/design/SYMBOLIC-FORTH.md 14, section 7 step 5):
//! in a bracket one lowercase letter is the input at its place from the
//! top, `a` the top, and written again it is a copy. A bracket takes as
//! many inputs as its deepest letter names and drops those it names
//! nowhere. In Low Stack a letter is `a var# .`, and High Stack's `_` is
//! written as the letter of its place.
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
fn a_letter_is_an_input_by_its_place() {
    // Letters in the stack's order give the plain word.
    assert_eq!(show(&["7 3 [ b a -+ ] . 7 3 -+"]), "<2> 4 4");
    assert_eq!(show(&["7 3 [ a b -+ ] ."]), "<1> -4");
    assert_eq!(show(&["3 4 [ b a b + ] ."]), "<2> 3 7");
    // `x`, `y` and `z` are the first three, as math names them.
    assert_eq!(show(&["1 2 3 [ x y z ] ."]), "<3> 3 2 1");
    // Known only at run time, the same.
    assert_eq!(show(&["7 3", "[ a b -+ ] ."]), "<1> -4");
    assert_eq!(show(&["3 4", "[ b a b + ] ."]), "<2> 3 7");
}

#[test]
fn a_bracket_takes_to_its_deepest_letter() {
    // Those it names nowhere are dropped.
    assert_eq!(show(&["1 2 3 [ c ] ."]), "<1> 1");
    assert_eq!(show(&["1 2 3", "[ c a ] ."]), "<2> 1 3");
    // Its code may reach below them, as they are taken first.
    assert_eq!(show(&["10 1 [ a + ] ."]), "<1> 11");
    assert_eq!(show(&["10 1 2", "[ 1 + b a ] ."]), "<3> 11 1 2");
    // Given fewer, it says which place the letter is.
    let e = error("[ n 2 * ] .");
    assert!(
        e.contains("`n`, at 1:3, is the 14th input from the top, and 0 are in reach"),
        "{e}"
    );
}

#[test]
fn a_body_takes_its_words_inputs() {
    assert_eq!(show(&["sq: a a * ; 7 sq"]), "<1> 49");
    // A signature's letters are types, and its body's places: two things.
    let add = "add: < a a >. b a + < a >. ;";
    assert_eq!(show(&[add, "1 2 add 1.5 2.5 add"]), "<2> 3 4.0");
    let sub = "sub: < i64 i64 >. a b -+ ;";
    assert_eq!(show(&[sub, "7 3 sub"]), "<1> -4");
    assert_eq!(show(&[sub, "7 3", "sub"]), "<1> -4");
}

#[test]
fn a_letter_belongs_to_its_bracket() {
    // In a loop the inner quotation's input is the element.
    assert_eq!(show(&["( 1 2 3 ) [ a a * ] map"]), "<1> ( 1 4 9 )");
    let e = error("( 1 2 3 ) [ b a * ] map");
    assert!(e.contains("`b`, at 1:13, is the second input"), "{e}");
    // A literal and a string are applied where they stand.
    assert_eq!(show(&["1 2 ( b a b )"]), "<1> ( 1 2 1 )");
    assert_eq!(show(&["5 \"\\[ a ] is \\[ a ]\""]), "<1> \"5 is 5\"");
    assert_eq!(
        show(&["ff: \"\\[ a ] is \\[ a ]\" ; 5 ff"]),
        "<1> \"5 is 5\""
    );
    // A power applies its code each time, and `compose` code that names
    // its inputs, so composing is composition still; `concat` joins their
    // tokens, as it does any sequences'.
    assert_eq!(show(&["5 [ a 1 + ]^3 . 5 [ _ 1 + ]^3 ."]), "<2> 8 8");
    assert_eq!(show(&["1 2 [ b a -+ ] [ a 10 * ] compose ."]), "<1> -10");
    assert_eq!(show(&["5 [ a ] [ a ] concat ."]), "<2> 5 5");
    // Outside any bracket a letter is no input.
    let e = error("5 a");
    assert!(
        e.contains("`a` is an input of the bracket it is written in, and this one is in none"),
        "{e}"
    );
}

#[test]
fn an_underscore_is_the_letter_of_its_place() {
    // The first written the deepest, so `[ _ _ -+ ]` is `[ b a -+ ]`.
    let mut s = Session::new();
    s.eval("7 3").unwrap();
    let a = s.compile("[ _ _ -+ ] .").unwrap().0;
    let b = s.compile("[ b a -+ ] .").unwrap().0;
    assert_eq!(a, b);
    // A bracket names its inputs by `_`s or by letters, not both, and a
    // signature has none.
    assert!(error("1 2 [ _ a ] .").contains("by `_`s or by letters, not both"));
    assert!(error("1 2 < _ > !").contains("a signature's letters are types"));
}

#[test]
fn letters_move_as_the_plain_words_do() {
    // On values at run time a letter compiles to the stack's own moves.
    let mut s = Session::new();
    s.eval("7 3").unwrap();
    for (letters, plain) in [
        ("[ b a b + ] .", "over +"),
        ("[ a b -+ ] .", "swap -+"),
        ("[ a a * ] .", "dup *"),
        ("[ b ] .", "drop"),
    ] {
        let a = s.compile(letters).unwrap().0;
        let b = s.compile(plain).unwrap().0;
        assert_eq!(a, b, "{letters}");
    }
}
