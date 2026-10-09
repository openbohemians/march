//! Tuples (doc/design/TYPES.md 2.5): array literals of mixed types, typed by
//! position, which forget their positions where an array is wanted.
use march8::{Kind, Level, Session};

/// A session that has run each piece of source in turn, so values from an
/// earlier piece are known only at run time in a later one.
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

#[test]
fn mixed_literals_are_tuples() {
    assert_eq!(
        typed(&["( 1 \"a\" )"]),
        "<1> ( 1 \"a\" ) < ( i64 string ) >"
    );
    assert_eq!(
        typed(&["( ( 1 \"a\" ) 3 )"]),
        "<1> ( ( 1 \"a\" ) 3 ) < ( ( i64 string ) i64 ) >"
    );
    // Tuples of one type are an array of them; positions of one type, a vec.
    assert_eq!(
        typed(&["( ( 1 \"a\" ) ( 2 \"b\" ) )"]),
        "<1> ( ( 1 \"a\" ) ( 2 \"b\" ) ) < 2 ( i64 string ) vec >"
    );
    assert_eq!(typed(&["( 1 2.5 )"]), "<1> ( 1.0 2.5 ) < 2 f64 vec >");
    assert_eq!(
        typed(&["{ \"a\" 1 } \"a\" get. ( _. \"x\" )"]),
        "<1> ( 1 \"x\" ) < ( i64 nil or string ) >"
    );
}

#[test]
fn positions_keep_their_types() {
    // `at` with a known index, from either end, now or at run time.
    assert_eq!(
        typed(&["( 1 \"a\" 2.5 ) dup. 2 at. swap. -1 at."]),
        "<2> \"a\" < string > 2.5 < f64 >"
    );
    assert_eq!(typed(&["( 1 \"a\" )", "1 at."]), "<1> 1 < i64 >");
    assert_eq!(session(&["( 1 \"a\" ) 3 at."]).err(), Some(Kind::Mismatch));
    assert_eq!(
        show(&["( 1 \"a\" ) dup. length. swap. dup. first. swap. last."]),
        "<3> 2 1 \"a\""
    );
    assert_eq!(
        typed(&["( 1 \"a\" ) spread."]),
        "<2> 1 < i64 > \"a\" < string >"
    );
    assert_eq!(
        typed(&["( 1 \"a\" ) reverse."]),
        "<1> ( \"a\" 1 ) < ( string i64 ) >"
    );
}

#[test]
fn map_and_each_are_unrolled() {
    // Each position compiled on its own type.
    assert_eq!(
        typed(&["( 1 \"a\" ) [ show. ] map."]),
        "<1> ( \"1\" \"\\\"a\\\"\" ) < 2 string vec >"
    );
    assert_eq!(
        typed(&["( 1 \"a\" ) [ ] map."]),
        "<1> ( 1 \"a\" ) < ( i64 string ) >"
    );
    assert_eq!(
        show(&["0 ( 1 \"ab\" ) [ show. length. +. ] each."]),
        "<1> 5"
    );
    // What `each` threads may change type from one position to the next.
    assert_eq!(show(&["( 1 \"ab\" ) [ ] each."]), "<2> 1 \"ab\"");
    assert_eq!(show(&["( 1 \"ab\" ) [ ] each-right."]), "<2> \"ab\" 1");
}

#[test]
fn shown_by_position_without_a_warning() {
    let mut s = Session::new();
    s.eval("( 1 \"a\" ) show. ( 2 \"b\" ) >string.").unwrap();
    assert_eq!(s.show(), "<2> \"( 1 \\\"a\\\" )\" \"( 2 \\\"b\\\" )\"");
    assert!(s.take_warnings().is_empty());
}

#[test]
fn an_array_is_wanted_so_positions_are_forgotten() {
    // `concat` asks for arrays: each tuple becomes an array of the union of
    // its types, its elements tagged, which is noted.
    let mut s = Session::new();
    s.eval("( 1 \"a\" ) ( 2 \"b\" ) concat.").unwrap();
    assert_eq!(
        s.show_typed(),
        "<1> ( 1 \"a\" 2 \"b\" ) < i64 string or ary >"
    );
    let w = s.take_warnings();
    assert_eq!(w.len(), 1, "{w:?}");
    assert_eq!(w[0].level, Level::Informative);
    assert!(
        w[0].to_string()
            .starts_with("1:21: informative: this makes an array")
    );
    // An index known only at run time: an element of the union.
    assert_eq!(
        typed(&["( 1 \"a\" ) 2", "at."]),
        "<1> \"a\" < i64 string or >"
    );
    assert_eq!(
        show(&["( 1 \"a\" ) < i64 string or ary > [ show. ] map."]),
        "<1> ( \"1\" \"\\\"a\\\"\" )"
    );
    // Only where the union fits: `sort` has no clause for it.
    assert_eq!(session(&["( 1 \"a\" ) sort."]).err(), Some(Kind::NoWord));
}

#[test]
fn tuple_types_in_brackets() {
    let p = "[ < ( i64 string ) > drop. 1 ] p def. [ < ( a b ) > drop. 2 ] p def.";
    // A vec is a tuple whose types are one, so `( a b )` takes `( 1 2 )`.
    assert_eq!(
        show(&[p, "( 1 \"a\" ) p. ( 1 2 ) p. ( \"x\" 2.0 ) p."]),
        "<3> 1 2 2"
    );
    assert_eq!(
        show(&["[ < -- ( i64 string ) > ( 1 \"a\" ) ] mk def. mk. 2 at."]),
        "<1> \"a\""
    );
    assert_eq!(show(&["( 1 \"a\" ) < ( i64 string ) > length."]), "<1> 2");
    assert_eq!(
        session(&["( 1 2 ) < ( i64 string ) >"]).err(),
        Some(Kind::Mismatch)
    );
    assert_eq!(session(&["1 < ( ) >"]).err(), Some(Kind::Mismatch));
}
