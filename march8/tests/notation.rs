//! The notation (Thomas, 2026-10-12; docs/MACHINE.md): a name applies, as in
//! FORTH; a capitalized name is a symbol; `name:` labels what follows, and
//! `name'` is a symbol where there are no capitals. A definition names
//! first, then what it names. A bracket is a value, a type or a signature,
//! except first in a definition's quotation, where it is the clause's
//! context.
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
    let step = "step: [ < i64 Up > drop 1 + ] def step: [ < i64 Down > drop 1 -+ ] def";
    assert_eq!(show(&[step, "5 Up step Down step Down step"]), "<1> 4");
}

#[test]
fn a_definition_names_first() {
    // Code in a quotation; `name'` is the same symbol as `name:`.
    assert_eq!(
        show(&["sq: [ dup * ] def cube' [ dup sq * ] def 3 sq 2 cube"]),
        "<2> 9 8"
    );
    // A type in a bracket: the name is a class, read in other brackets.
    let real = "real: < i64 f64 or. > def half: [ < real > 2 div ] def";
    assert_eq!(show(&[real, "7 half 3.0 half"]), "<2> 3 1.5");
    // Anything else is no definition: a value computed in place could take
    // the name below it.
    let e = error("x: 5 def");
    assert!(
        e.contains("`def` takes a name, then a quotation or a type"),
        "{e}"
    );
}

#[test]
fn a_bracket_is_a_value() {
    // A type, here as `parse`'s argument.
    assert_eq!(
        typed(&["\"42\"", "< i64 > parse"]),
        "<1> 42 < i64 nil or. >"
    );
    // Applied, a type converts and a signature gives the values below their
    // types; a class checks.
    assert_eq!(typed(&["1 2.5 < i64 f64 > ."]), "<2> 1 < i64 > 2.5 < f64 >");
    assert_eq!(typed(&["2 < f64 > ."]), "<1> 2.0 < f64 >");
    assert_eq!(session(&["\"x\" < num > ."]).err(), Some(Kind::Mismatch));
    // First in a quotation that is not a definition, it is a value too:
    // `parse`'s type.
    assert_eq!(
        show(&["( \"1.5\" \"x\" ) [ < f64 > parse ] map"]),
        "<1> ( 1.5 nil )"
    );
    // A clause's context is taken when it is defined, so a bracket first in
    // the rest of its body is a value again.
    let e = "e: [ < i64 > drop < string i64 map. > empty length ] def";
    assert_eq!(show(&[e, "5 e"]), "<1> 0");
}

#[test]
fn messages_name_symbols() {
    let door = "door: [ < Closed Open > drop drop Opened ] def";
    let e = error(&format!("{door} Locked Open door"));
    assert!(e.contains("no word `door` for Locked Open"), "{e}");
    // A capital where a word was meant makes a symbol, and says so.
    let e = error("3 Dup +");
    assert!(e.contains("(`Dup` is a symbol; the word is `dup`)"), "{e}");
}
