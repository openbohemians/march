//! Strings with holes (march7/docs/STRINGS.md): `\[ code ]` writes in the
//! code's value, `\_` an input, and on known values the string is a
//! constant.
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

#[test]
fn holes_write_values_in() {
    assert_eq!(show(&["\"1 + 2 = \\[ 1 2 +. ]\""]), "<1> \"1 + 2 = 3\"");
    // On known values the string is folded: the same code as the constant.
    let mut s = Session::new();
    let a = s.compile("\"1 + 2 = \\[ 1 2 +. ]\"").unwrap().0;
    let b = s.compile("\"1 + 2 = 3\"").unwrap().0;
    assert_eq!(a, b);
    // Values are written as the display writes them; a string as its text.
    assert_eq!(
        show(&["\"\\[ ( 1 2 ) ] \\[ ( \"a\" ) ] \\[ 19.99 money. ] \\[ 2.5 ] \\[ \"b\" ]\""]),
        "<1> \"( 1 2 ) ( \\\"a\\\" ) 19.99 2.5 b\""
    );
    // Strings nest, and text resumes right after the `]`.
    assert_eq!(show(&["\"a \\[ \"b\\[ 1 ]c\" ] d\""]), "<1> \"a b1c d\"");
    assert_eq!(
        show(&["[ 3 ] n def. \"\\[ n. ]apples\""]),
        "<1> \"3apples\""
    );
    // `\_` before a subscript's name and `;` is the subscript.
    assert_eq!(show(&["\"H\\_2;O\""]), "<1> \"H₂O\"");
}

#[test]
fn inputs_are_taken_in_reading_order() {
    assert_eq!(show(&["1 2 \"\\_ and \\_\""]), "<1> \"1 and 2\"");
    assert_eq!(show(&["5", "\"n = \\_!\""]), "<1> \"n = 5!\"");
    assert_eq!(show(&["4", "\"\\[ _. dup. *. ]\""]), "<1> \"16\"");
    // In a word, a string with inputs makes the word take them.
    assert_eq!(
        show(&["[ \"<\\[ _. 1 +. ]>\" ] f def. 5 f. 1.5 f."]),
        "<2> \"<6>\" \"<2.5>\""
    );
}

#[test]
fn print_writes_any_value() {
    let mut s = session(&["( 1 2 ) print. \" \" print. ( ( 1 ) ( 2 3 ) ) print."]).unwrap();
    assert_eq!(
        String::from_utf8(s.take_output()).unwrap(),
        "( 1 2 ) ( ( 1 ) ( 2 3 ) )"
    );
}

#[test]
fn bad_holes() {
    assert_eq!(session(&["\"\\[ ]\""]).err(), Some(Kind::Syntax));
    assert_eq!(session(&["\"\\[ 1 2 ]\""]).err(), Some(Kind::Mismatch));
    assert_eq!(session(&["\"abc \\[ 1 "]).err(), Some(Kind::Unfinished));
    assert_eq!(session(&["\"\\_\""]).err(), Some(Kind::Mismatch));
    // A hole reaches only what it makes.
    assert_eq!(session(&["\"a\\[ drop. 1 ]\""]).err(), Some(Kind::Mismatch));
}
