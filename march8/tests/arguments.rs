//! A bracket's arguments (doc/design/SYMBOLIC-FORTH.md 14): a bracket with
//! k `_`s takes k values from below when it is applied, and each `_` puts
//! the next where it stands, the first written the deepest. A quotation is
//! an anonymous word, applied by `.` or by what takes it; a literal and a
//! string are applied where they stand. Nothing is captured.
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
fn a_quotation_takes_its_arguments_when_applied() {
    assert_eq!(show(&["1 2 3 [ _ _ + _ * ] ."]), "<1> 9");
    // Held, it takes nothing until applied.
    assert_eq!(show(&["[ 2 _ -+ ] 10 swap ."]), "<1> -8");
    // Its code may reach below its arguments, which were taken first.
    assert_eq!(show(&["10 1 [ _ + ] ."]), "<1> 11");
    // Known only at run time, the same.
    assert_eq!(show(&["10 1", "[ 100 _ -+ _ * ] ."]), "<1> 90");
}

#[test]
fn a_word_body_takes_the_words_inputs() {
    assert_eq!(show(&["ff: [ _ 1 + ] ; 5 ff"]), "<1> 6");
    assert_eq!(
        show(&["gg: < i64 i64 > [ 1 _ -+ _ * ] ; 7 3 gg"]),
        "<1> -18"
    );
    assert_eq!(
        show(&["gg: < i64 i64 > [ 1 _ -+ _ * ] ;", "7 3", "gg"]),
        "<1> -18"
    );
}

#[test]
fn a_nested_bracket_has_its_own_arguments() {
    // In a loop the inner quotation's argument is the element.
    assert_eq!(show(&["( 1 2 3 ) [ _ dup * ] map"]), "<1> ( 1 4 9 )");
    // Two `_`s are two arguments, which a loop does not give.
    assert!(session(&["( 1 2 3 ) [ _ _ * ] map"]).is_err());
    // The outer literal's `_` and the inner quotation's differ.
    assert_eq!(
        show(&["( 1 2 3 ) 10 ( _ [ _ 1 + ] map _ )"]),
        "<1> ( ( 2 3 4 ) 10 )"
    );
}

#[test]
fn a_string_is_text_of_a_quotation() {
    assert_eq!(show(&["1 2 [ _ \" and \" _ ] string !"]), "<1> \"1 and 2\"");
    assert_eq!(show(&["[ 1 2 + \"x\" 2.5 ] string !"]), "<1> \"3x2.5\"");
    assert_eq!(
        show(&["1 2", "[ _ \" and \" _ ] string !"]),
        "<1> \"1 and 2\""
    );
    assert_eq!(show(&["[ ] string !"]), "<1> \"\"");
    // Its code reaches only its arguments and what it makes.
    assert!(session(&["1 [ 2 + ] string !"]).is_err());
}

#[test]
fn a_string_joins_its_pieces_in_one_buffer() {
    let mut s = Session::new();
    s.eval("1 2").unwrap();
    let (ops, _) = s.compile("\"a \\_ b \\_ c\"").unwrap();
    let joins = ops
        .iter()
        .filter(|op| format!("{op:?}").contains("TextJoin"))
        .count();
    let concats = ops
        .iter()
        .filter(|op| format!("{op:?}").contains("Concat"))
        .count();
    assert_eq!((joins, concats), (1, 0), "{ops:?}");
    s.eval("\"a \\_ b \\_ c\"").unwrap();
    assert_eq!(s.show(), "<1> \"a 1 b 2 c\"");
    // Known pieces join while compiling.
    assert_eq!(show(&["\"a\\[ 1 2 + ]b\""]), "<1> \"a3b\"");
}

#[test]
fn an_argument_belongs_to_a_bracket() {
    assert_eq!(session(&["1 _"]).err(), Some(Kind::Syntax));
}
