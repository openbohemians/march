//! Comprehensions (docs/MACHINE.md): inside an array literal, what code
//! leaves is collected, as many values as it leaves, so `each` with the
//! literal maps, filters, repeats and scans; `_.` takes its arguments.
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

/// Leaves an element under 3, and nothing for any other.
const KEEP: &str = "small: [ 3 lt? ] ;
    keep: < i64 small? > [ ] ;
    keep: < i64 > [ drop ] ;";

#[test]
fn each_inside_a_literal_collects() {
    // One value for each element: a map.
    assert_eq!(show(&["( ( 1 2 3 ) [ 10 * ] each )"]), "<1> ( 10 20 30 )");
    // Two: each element twice.
    assert_eq!(show(&["( ( 1 2 3 ) [ dup ] each )"]), "<1> ( 1 1 2 2 3 3 )");
    // Zero or one, chosen by clauses: a filter.
    assert_eq!(show(&[KEEP, "( ( 1 5 2 7 ) keep' each )"]), "<1> ( 1 2 )");
    // Reading the last value collected: a scan.
    assert_eq!(
        show(&["( 0 ( 1 2 3 ) [ over + ] each )"]),
        "<1> ( 0 1 3 6 )"
    );
    assert_eq!(show(&["( 3 [ 7 ] repeat )"]), "<1> ( 7 7 7 )");
    assert_eq!(show(&["( 4 range [ dup * ] each )"]), "<1> ( 1 4 9 16 )");
    // With a count known only at run time, the result is an array of any
    // length.
    let s = session(&[KEEP, "( ( 1 5 ) keep' each ) length"]).unwrap();
    assert_eq!(s.machine.stack, [1]);
}

#[test]
fn pulls_take_values_from_below() {
    // The first `_.` takes the deepest, so the literal reads as the code it
    // replaces; the literal takes its arguments where it stands.
    assert_eq!(show(&["1 2 ( _ _ div )"]), "<1> ( 0 )");
    assert_eq!(show(&["1.0 2 ( _ _ div )"]), "<1> ( 0.5 )");
    // A `_` in a loop's quotation is that quotation's argument, the
    // element; a value from outside is written in.
    assert_eq!(show(&["( ( 1 2 3 ) [ _ 10 + ] each )"]), "<1> ( 11 12 13 )");
    assert_eq!(
        show(&["( 1 2 3 )", "( _ [ 10 _ + ] each )"]),
        "<1> ( 11 12 13 )"
    );
    // Its code reaches below, as any code does: `+` folds into the 10.
    assert_eq!(show(&["10 ( ( 1 2 3 ) [ _ + ] each )"]), "<1> ( 16 )");
    // An array known only at run time, filtered.
    assert_eq!(
        show(&[KEEP, "( 9 1 4 0 )", "( _ keep' each )"]),
        "<1> ( 1 0 )"
    );
    assert_eq!(session(&["( _ )"]).err(), Some(Kind::Mismatch));
    assert_eq!(session(&["1 _"]).err(), Some(Kind::Syntax));
}

#[test]
fn runs_can_only_be_gathered() {
    // What a filter leaves may be nothing, so it cannot be added to.
    assert_eq!(
        session(&[KEEP, "( ( 1 5 ) keep' each 1 + )"]).err(),
        Some(Kind::Mismatch)
    );
    // Outside a literal, a body must keep the loop's shape, and clauses
    // chosen at run time must leave as many values.
    assert_eq!(
        session(&["( 1 2 ) [ dup ] each"]).err(),
        Some(Kind::Mismatch)
    );
    assert_eq!(session(&[KEEP, "5", "keep"]).err(), Some(Kind::Mismatch));
    // The elements still share one type.
    assert_eq!(
        session(&["( 1.5 ( 1 2 ) [ 2 * ] each )"]).err(),
        Some(Kind::Literal)
    );
    // A map literal does not collect runs.
    assert_eq!(
        session(&[KEEP, "{ ( 1 5 ) keep' each }"]).err(),
        Some(Kind::Mismatch)
    );
}
