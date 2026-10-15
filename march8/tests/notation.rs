//! The notation (Thomas, 2026-10-12; docs/MACHINE.md): a name applies, as in
//! FORTH, in code and in a bracket alike; a capitalized name is a symbol,
//! one lowercase letter a variable; `name:` labels what follows, and `name'`
//! is a symbol, a word passed. A type is built by words, so a type's name
//! gives the type; a type is data, and `!` converts by it (Thomas,
//! 2026-10-13). This is an intermediate form over Symbolic Forth, where
//! everything is a number, a symbol, `.` or `!`. A definition names first,
//! then what it names. A bracket is a value, a type or a signature, except
//! first in a definition's quotation, where it is the clause's context.
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

fn typed(pieces: &[&str]) -> String {
    match session(pieces) {
        Ok(s) => s.show_typed(),
        Err(k) => panic!("{pieces:?}: {k:?}"),
    }
}

fn error(src: &str) -> String {
    Session::new().eval(src).unwrap_err().to_string()
}

#[test]
fn names_apply_and_symbols_are_spelled() {
    assert_eq!(show(&["3 dup +"]), "<1> 6");
    // A symbol, capitalized or marked, chooses a clause; a word passed is
    // its symbol.
    assert_eq!(
        show(&["\"Hello World\" Snake case ( 1 2 3 ) +' reduce"]),
        "<2> \"hello_world\" 6"
    );
    let step = "step: < i64 Up > [ drop 1 + ] ; step: < i64 Down > [ drop 1 -+ ] ;";
    assert_eq!(show(&[step, "5 Up step Down step Down step"]), "<1> 4");
}

#[test]
fn a_type_is_built_by_words() {
    // A type's name gives the type: no bracket is needed for one in code.
    assert_eq!(
        typed(&["\"42\"", "i64! string i64 map empty"]),
        "<2> 42 < i64 nil or > { } < string i64 map >"
    );
    // `!` converts by a type, built or named; it is `convert` applied.
    assert_eq!(
        typed(&["2 i64! 2 f64! ( 1 2 ) i64 ary !"]),
        "<3> 2 < i64 > 2.0 < f64 > ( 1 2 ) < i64 ary >"
    );
    assert_eq!(show(&["2 f64 convert"]), "<1> 2.0");
    // A type is data, and is not called.
    let e = error("2 i64.");
    assert!(
        e.contains("the type `i64` is not called: `!` converts by it"),
        "{e}"
    );
    assert!(error("1 2 < i64 i64 > .").contains("a signature is not called"));
    // A bracket reads as code does, so the same text means the same there.
    let n = "size: < string i64 map -- i64 > [ length ] ;";
    assert_eq!(show(&[n, "string i64 map empty size"]), "<1> 0");
    // A type is defined as any value is, bracketed or not.
    let real = "real: i64 f64 or ; half: < real > [ 2 div ] ;";
    assert_eq!(show(&[real, "7 half 3.0 half"]), "<2> 3 1.5");
}

#[test]
fn a_question_mark_tests() {
    // `?` applies the value on top as a test, which leaves one bool
    // (doc/design/SYMBOLIC-FORTH.md 20): `positive?` is `positive ?`. Applied
    // in a bracket it is a guard on the inputs before it.
    let sign = "sign: < i64 positive? > [ drop 1 ] ; sign: < i64 > [ drop 0 ] ;";
    assert_eq!(show(&[sign, "5 sign -5 sign"]), "<2> 1 0");
    // A quotation tests too, and guards.
    assert_eq!(show(&["5 [ 0 gt ]?"]), "<1> true");
    let big = "big: < i64 [ 100 gt ]? > [ drop 1 ] ; big: < i64 > [ drop 0 ] ;";
    assert_eq!(show(&[big, "500 big 5 big"]), "<2> 1 0");
    // A test leaves one bool; `.` applies the word unchecked.
    let e = error("two: [ dup ] ; 3 two?");
    assert!(
        e.contains("`two?` tests, so it leaves one bool, and it leaves 2 values"),
        "{e}"
    );
    let e = error("succ: [ 1 + ] ; 3 succ?");
    assert!(e.contains("and it leaves 4 (int#)"), "{e}");
    assert_eq!(show(&["two: [ dup ] ; 3 two"]), "<2> 3 3");
    // No word's name ends in `?`, which tests.
    let e = error("big?: < i64 -- i64 > [ 100 + ] ;");
    assert!(e.contains("`big?` is `big` tested"), "{e}");
}

#[test]
fn a_definition_names_first() {
    // Code in a quotation; `name'` is the same symbol as `name:`, and one
    // letter is a variable, which names no word.
    assert_eq!(
        show(&["sq: [ dup * ] ; cube' [ dup sq * ] ; 3 sq 2 cube"]),
        "<2> 9 8"
    );
    // A type in a bracket: the name is a class, read in other brackets.
    let real = "real: < i64 f64 or > ; half: < real > [ 2 div ] ;";
    assert_eq!(show(&[real, "7 half 3.0 half"]), "<2> 3 1.5");
    assert!(error("x: [ 1 ] ;").contains("`x` is a variable"));
    // Anything else is no definition: a value computed in place could take
    // the name below it.
    let e = error("xx: 5 ;");
    assert!(
        e.contains("`;` takes a name, its signature if it has one, then a quotation or a type"),
        "{e}"
    );
}

#[test]
fn a_bracket_is_a_value() {
    // A bracket of one type is that type, here applied to a string.
    assert_eq!(typed(&["\"42\"", "< i64 > !"]), "<1> 42 < i64 nil or >");
    // Applied, a type converts and a signature gives the values below their
    // types; a class checks.
    assert_eq!(typed(&["1 2.5 < i64 f64 > !"]), "<2> 1 < i64 > 2.5 < f64 >");
    assert_eq!(typed(&["2 < f64 > !"]), "<1> 2.0 < f64 >");
    assert_eq!(session(&["\"x\" < num > !"]).err(), Some(Kind::Mismatch));
    // First in a quotation that is not a definition, it is a value too,
    // here the type each element is converted to, as `f64.` converts.
    assert_eq!(
        show(&["( \"1.5\" \"x\" ) [ < f64 > ! ] map ( \"2\" ) [ f64! ] map"]),
        "<2> ( 1.5 nil ) ( 2.0 )"
    );
    // A clause's context is taken when it is defined, so a bracket first in
    // the rest of its body is a value again.
    let e = "ee: < i64 > [ drop < string i64 map > empty length ] ;";
    assert_eq!(show(&[e, "5 ee"]), "<1> 0");
}

#[test]
fn messages_name_symbols() {
    let door = "door: < Closed Open > [ drop drop Opened ] ;";
    let e = error(&format!("{door} Locked Open door"));
    assert!(e.contains("no word `door` for Locked Open"), "{e}");
    // A capital where a word was meant makes a symbol, and says so.
    let e = error("3 Dup +");
    assert!(e.contains("(`Dup` is a symbol; the word is `dup`)"), "{e}");
}
