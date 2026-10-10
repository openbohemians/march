//! Truth (doc/design/TYPES.md 2.16): `bool` is 0 and 1, Iverson's brackets,
//! and so an i64 too; comparisons leave one, and guards take one.
use march8::{Kind, Session};

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
fn comparisons_leave_a_bool() {
    assert_eq!(
        typed(&["true. false. 3 4 lt?."]),
        "<3> true < bool > false < bool > true < bool >"
    );
    assert_eq!(
        show(&["3 4", "eq?. 2.5 1.5 gt?. \"a\" \"a\" eq?."]),
        "<3> false true true"
    );
    assert_eq!(
        show(&["3 4", "lt?. show. false. >string."]),
        "<2> \"true\" \"false\""
    );
    // Only 0 and 1 are bools.
    assert_eq!(session(&["2 bool."]).err(), Some(Kind::Literal));
}

#[test]
fn not_and_or() {
    // Now and at run time: 1 - x, the minimum, the maximum.
    let table = "true. not. false. not. true. false. and. true. true. and. \
                 false. false. or. true. false. or.";
    assert_eq!(show(&[table]), "<6> false true false true false true");
    assert_eq!(
        show(&["true. false.", "and. true. false.", "or. false.", "not."]),
        "<3> false true true"
    );
}

#[test]
fn a_bool_is_an_i64() {
    // Iverson's brackets: truth counts, and a mask multiplies.
    assert_eq!(typed(&["true. true. +."]), "<1> 2 < i64 >");
    assert_eq!(
        typed(&["( 1 2 3 4 ) [ 2 gt?. ] map.", "+ reduce."]),
        "<1> 2 < i64 >"
    );
    assert_eq!(
        typed(&["( true. false. ) ( 5 6 ) ⋅. 3 true. max. false. true. max."]),
        "<3> ( 5 0 ) < 2 i64 vec. > 3 < i64 > true < bool >"
    );
    // A clause for bool is more specific than one for i64.to.
    let k = "[ < i64 > drop. 1 ] k def. [ < bool > drop. 2 ] k def.";
    assert_eq!(show(&[k, "5 k. true. k."]), "<2> 1 2");
}

#[test]
fn guards_take_a_bool() {
    // A count is not a truth: FORTH's flag that was a number is caught.
    let odd = "[ 2 mod. ] odd? def. [ < i64 odd? > drop. 1 ] f def.";
    assert_eq!(session(&[odd]).err(), Some(Kind::Mismatch));
    let odd = "[ 2 mod. 1 eq?. ] odd? def. [ < i64 odd? > drop. 1 ] f def. \
               [ < i64 > drop. 0 ] f def.";
    assert_eq!(show(&[odd, "3 4", "f. swap. f."]), "<2> 0 1");
}
