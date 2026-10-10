//! Loops (docs/MACHINE.md): `each` and `each-right`, the folds and
//! reductions written on them in March, `repeat` and `range`.
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

fn printed(src: &str) -> String {
    let mut s = session(&[src]).unwrap();
    String::from_utf8(s.take_output()).unwrap()
}

#[test]
fn each_keeps_an_accumulator() {
    assert_eq!(show(&["0 ( 1 2 3 ) [ +. ] each."]), "<1> 6");
    // A literal accumulator takes the type the loop gives it.
    assert_eq!(show(&["0 ( 1.5 2.5 ) [ +. ] each."]), "<1> 4.0");
    assert_eq!(
        show(&["\"\" ( \"a\" \"b\" ) [ concat. ] each."]),
        "<1> \"ab\""
    );
    // On values known only at run time, and nested.
    assert_eq!(show(&["( 4 5 6 )", "0 swap. [ +. ] each."]), "<1> 15");
    assert_eq!(
        show(&["0 ( ( 1 2 ) ( 3 ) ) [ [ +. ] each. ] each."]),
        "<1> 6"
    );
    // From the first, or from the last.
    assert_eq!(printed("( 1 2 3 ) [ print. ] each."), "123");
    assert_eq!(printed("( 1 2 3 ) [ print. ] each-right."), "321");
}

#[test]
fn each_has_an_invariant() {
    // The quotation takes its element and leaves the values below it, as
    // many as there were, of the same types.
    assert_eq!(session(&["( 1 2 ) [ ] each."]).err(), Some(Kind::Mismatch));
    assert_eq!(
        session(&["1 i64.to. ( \"a\" ) [ drop. drop. \"x\" ] each."]).err(),
        Some(Kind::Mismatch)
    );
    assert_eq!(session(&["( 1 2 ) 5 each."]).err(), Some(Kind::NoWord));
}

#[test]
fn folds_and_reductions_go_either_way() {
    // Left: ((0 - 1) - 2) - 3. Right: 1 - (2 - (3 - 0)), as APL's `-/`.
    assert_eq!(show(&["( 1 2 3 ) 0 [ -+. ] fold."]), "<1> -6");
    assert_eq!(show(&["( 1 2 3 ) 0 [ -+. ] fold-right."]), "<1> 2");
    assert_eq!(show(&["( 1 2 3 ) [ -+. ] reduce."]), "<1> -4");
    assert_eq!(show(&["( 1 2 3 ) [ -+. ] reduce-right."]), "<1> 2");
    // A consumer takes a word as well as a quotation: APL's `+/`.
    assert_eq!(show(&["( 1 2 3 4 ) + reduce."]), "<1> 10");
    assert_eq!(
        show(&["( 1 2 3 ) i64>text map."]),
        "<1> ( \"1\" \"2\" \"3\" )"
    );
}

#[test]
fn ranges_and_repetition() {
    assert_eq!(show(&["5 range."]), "<1> ( 1 2 3 4 5 )");
    // A range of a known count is a vec, so its length is a constant.
    assert_eq!(show(&["5 range. length."]), "<1> 5");
    assert_eq!(show(&["4", "range."]), "<1> ( 1 2 3 4 )");
    assert_eq!(printed("3 [ \"hi \" print. ] repeat."), "hi hi hi ");
    assert_eq!(show(&["0 4 range. [ +. ] each."]), "<1> 10");
}

#[test]
fn arrays_in_parts() {
    assert_eq!(show(&["( 1 2 3 ) reverse."]), "<1> ( 3 2 1 )");
    assert_eq!(show(&["( 1 2 3 ) reverse. length."]), "<1> 3");
    assert_eq!(
        show(&["( 1 2 3 ) dup. first. over. last. rot. rest."]),
        "<3> 1 3 ( 2 3 )"
    );
    assert_eq!(show(&["( 1 2 3 ) most."]), "<1> ( 1 2 )");
    assert_eq!(show(&["( 1 2 3 ) count. \"abc\" count."]), "<2> 3 3");
    assert_eq!(show(&["\"héllo\" 1 3 slice."]), "<1> \"hél\"");
    // `sort`: numbers by value, strings by text; `keys` and `values` in one
    // order.
    assert_eq!(
        show(&["( 3 1 2 ) sort. ( \"pear\" \"apple\" ) sort. ( 2.5 -1.0 ) sort."]),
        "<3> ( 1 2 3 ) ( \"apple\" \"pear\" ) ( -1.0 2.5 )"
    );
    assert_eq!(
        show(&["{ \"b\" 2 \"a\" 1 } dup. keys. sort. swap. values. sort."]),
        "<2> ( \"a\" \"b\" ) ( 1 2 )"
    );
    // `compose` joins two quotations into one, at compile time.
    assert_eq!(show(&["3 [ 1 +. ] [ 2 *. ] compose. ."]), "<1> 8");
}
