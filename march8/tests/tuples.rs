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
        typed(&["{ \"a\" 1 } \"a\" get ( _ \"x\" )"]),
        "<1> ( 1 \"x\" ) < ( i64 nil or string ) >"
    );
}

#[test]
fn positions_keep_their_types() {
    // `at` with a known index, from either end, now or at run time.
    assert_eq!(
        typed(&["( 1 \"a\" 2.5 ) dup 2 at swap -1 at"]),
        "<2> \"a\" < string > 2.5 < f64 >"
    );
    assert_eq!(typed(&["( 1 \"a\" )", "1 at"]), "<1> 1 < i64 >");
    assert_eq!(session(&["( 1 \"a\" ) 3 at"]).err(), Some(Kind::Mismatch));
    assert_eq!(
        show(&["( 1 \"a\" ) dup length swap dup first swap last"]),
        "<3> 2 1 \"a\""
    );
    assert_eq!(
        typed(&["( 1 \"a\" ) spread"]),
        "<2> 1 < i64 > \"a\" < string >"
    );
    assert_eq!(
        typed(&["( 1 \"a\" ) reverse"]),
        "<1> ( \"a\" 1 ) < ( string i64 ) >"
    );
}

#[test]
fn map_and_each_are_unrolled() {
    // Each position compiled on its own type.
    assert_eq!(
        typed(&["( 1 \"a\" ) [ show ] map"]),
        "<1> ( \"1\" \"\\\"a\\\"\" ) < 2 string vec >"
    );
    assert_eq!(
        typed(&["( 1 \"a\" ) [ ] map"]),
        "<1> ( 1 \"a\" ) < ( i64 string ) >"
    );
    assert_eq!(show(&["0 ( 1 \"ab\" ) [ show length + ] each"]), "<1> 5");
    // What `each` threads may change type from one position to the next.
    assert_eq!(show(&["( 1 \"ab\" ) [ ] each"]), "<2> 1 \"ab\"");
    assert_eq!(show(&["( 1 \"ab\" ) [ ] each-right"]), "<2> \"ab\" 1");
}

#[test]
fn shown_by_position_without_a_warning() {
    let mut s = Session::new();
    s.eval("( 1 \"a\" ) show ( 2 \"b\" ) >string").unwrap();
    assert_eq!(s.show(), "<2> \"( 1 \\\"a\\\" )\" \"( 2 \\\"b\\\" )\"");
    assert!(s.take_warnings().is_empty());
}

#[test]
fn an_array_is_wanted_so_positions_are_forgotten() {
    // `concat` asks for arrays: each tuple becomes an array of the union of
    // its types, its elements tagged, which is noted.
    let mut s = Session::new();
    s.eval("( 1 \"a\" ) ( 2 \"b\" ) concat").unwrap();
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
        typed(&["( 1 \"a\" ) 2", "at"]),
        "<1> \"a\" < i64 string or >"
    );
    assert_eq!(
        show(&["( 1 \"a\" ) < i64 string or ary > ! [ show ] map"]),
        "<1> ( \"1\" \"\\\"a\\\"\" )"
    );
    // Only where the union fits: `sort` has no clause for it.
    assert_eq!(session(&["( 1 \"a\" ) sort"]).err(), Some(Kind::NoWord));
}

#[test]
fn tuple_types_in_brackets() {
    let p = "pp: < ( i64 string ) >. drop 1 ; pp: < ( a b ) >. drop 2 ;";
    // A vec is a tuple whose types are one, so `( a b )` takes `( 1 2 )`.
    assert_eq!(
        show(&[p, "( 1 \"a\" ) pp ( 1 2 ) pp ( \"x\" 2.0 ) pp"]),
        "<3> 1 2 2"
    );
    assert_eq!(
        show(&["mk: < >. ( 1 \"a\" ) < ( i64 string ) >. ; mk 2 at"]),
        "<1> \"a\""
    );
    assert_eq!(show(&["( 1 \"a\" ) < ( i64 string ) > ! length"]), "<1> 2");
    assert_eq!(
        session(&["( 1 2 ) < ( i64 string ) > !"]).err(),
        Some(Kind::Mismatch)
    );
    assert_eq!(session(&["1 < ( ) > !"]).err(), Some(Kind::Mismatch));
}

#[test]
fn a_symbol_beside_a_string_is_a_tuple() {
    // A symbol is text where a string is taken (doc/design/SYMBOLIC-FORTH.md
    // 28), and nothing takes it in the literal: so a string and a symbol,
    // which a clause takes as it asks.
    assert_eq!(
        typed(&["( \"can't read it\" no-file' )"]),
        "<1> ( \"can't read it\" no-file' ) < ( string sym ) >"
    );
    let ff = "ff: < string ary >. ; ff: < ( string sym ) >. 2 at ;";
    assert_eq!(show(&[ff, "( \"a\" b' ) ff"]), "<1> b'");
    let gg = "gg: < string ary >. 2 at < string >. ;";
    assert_eq!(typed(&[gg, "( \"a\" b' ) gg"]), "<1> \"b\" < string >");
    // Known only as the program runs, too.
    assert_eq!(
        typed(&[gg, "( \"a\" b' ) < ( string sym ) > !", "gg"]),
        "<1> \"b\" < string >"
    );
}

#[test]
fn known_arrays_fold_while_compiling() {
    // An array literal of known elements is known: built only where a value
    // at run time is needed, so it folds as a number does.
    let mut s = Session::new();
    let same = |s: &mut Session, a: &str, b: &str| {
        assert_eq!(s.compile(a).unwrap().0, s.compile(b).unwrap().0, "{a}");
    };
    same(&mut s, "( 1 2 3 ) 2 at", "2");
    same(&mut s, "( 1 2 3 ) -1 at", "3");
    same(&mut s, "( ( 1 2 ) ( 3 4 ) ) 2 at 1 at", "3");
    same(&mut s, "( \"a\" b' ) 1 at", "\"a\"");
    same(&mut s, "( 1 \"a\" ) length", "2");
    // Built where needed, each copy its own.
    assert_eq!(show(&["( 1 2 ) dup 2 at"]), "<2> ( 1 2 ) 2");
    assert_eq!(show(&["( 1 2 ) [ 1 + ] map"]), "<1> ( 2 3 )");
    // Its message shows it.
    let e = Session::new().eval("( 1 2 ) < f64 ary > !").unwrap_err();
    assert!(e.to_string().contains("found ( 1 2 ) (2 i64 vec)"), "{e}");
}
