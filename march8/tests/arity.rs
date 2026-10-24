//! Clauses of different arity (doc/design/TYPES.md 3.13, Thomas,
//! 2026-10-12): a family's clauses need not take as many values as each
//! other. When clauses of different lengths match, the shortest wins, the
//! most specific among those, so a longer clause never changes what a call
//! to a shorter one did. A choice made at run time between clauses of
//! different lengths must leave the stack as deep.
use march8::{Kind, Level, Session};

fn show(src: &str) -> String {
    let mut s = Session::new();
    if let Err(e) = s.eval(src) {
        panic!("{src}: {e}");
    }
    s.show()
}

fn fails(src: &str) -> march8::Error {
    let mut s = Session::new();
    s.eval(src).expect_err(src)
}

const MOVE: &str = "
Up: Up' ;  Down: Down' ;
step: < Up >. drop 1 ;
step: < i64 Down >. drop drop 2 ;
";

#[test]
fn a_family_takes_as_many_values_as_its_clause() {
    // Each clause takes what it says, and the values beneath are left.
    assert_eq!(show(&format!("{MOVE} 5 Up step")), "<2> 5 1");
    assert_eq!(show(&format!("{MOVE} 5 Down step")), "<1> 2");
    // The longer clause needs its values: with nothing beneath, only the
    // shorter can match.
    let e = fails(&format!("{MOVE} Down step"));
    assert_eq!(e.kind, Kind::NoWord);
    assert!(e.msg.contains("no word `step` for Down'"), "{e}");
}

#[test]
fn the_shortest_clause_wins() {
    let src = "
add: < i64 i64 >. + ;
add: < i64 >. 10 * ;
2 3 add";
    assert_eq!(show(src), "<2> 2 30");
    // Among clauses of one length, the most specific, as ever.
    let src = "
name: < string Kind' >. drop drop \"long\" ;
name: < i64 >. drop \"i64\" ;
name: < a >. drop \"any\" ;
\"x\" Kind' name 7 name";
    assert_eq!(show(src), r#"<3> "x" "any" "i64""#);
}

#[test]
fn a_shorter_clause_is_warned_of() {
    // Adding a shorter clause may change what a call did, so it is said.
    let mut s = Session::new();
    s.eval("add: < i64 i64 >. + ;").unwrap();
    assert!(s.take_warnings().is_empty());
    s.eval("add: < i64 >. 10 * ;").unwrap();
    let w = s.take_warnings();
    assert_eq!(w.len(), 1, "{w:?}");
    assert_eq!(w[0].level, Level::Informative);
    assert!(
        w[0].msg.contains(
            "this clause of `add`, `< i64 >`, takes fewer values than its clause \
             `< i64 i64 >`, so it is chosen wherever both match"
        ),
        "{}",
        w[0]
    );
    // A longer clause under a shorter one with no guards is not chosen
    // where that one matches.
    let mut s = Session::new();
    s.eval("add: < i64 >. 10 * ; add: < i64 i64 >. + ;")
        .unwrap();
    let w = s.take_warnings();
    assert_eq!(w.len(), 1, "{w:?}");
    assert!(
        w[0].msg.contains(
            "this clause of `add`, `< i64 i64 >`, takes more values than its clause \
             `< i64 >`, which is chosen wherever both match"
        ),
        "{}",
        w[0]
    );
    // Not where they cannot both match: other symbols, other types.
    let mut s = Session::new();
    s.eval(MOVE).unwrap();
    s.eval("step: < f64 >. drop 3 ;").unwrap();
    assert!(s.take_warnings().is_empty());
    // A string they can, a symbol being one where a string is taken
    // (doc/design/SYMBOLIC-FORTH.md 28).
    s.eval("step: < string >. drop 4 ;").unwrap();
    let w = s.take_warnings();
    assert_eq!(w.len(), 1, "{w:?}");
    assert!(
        w[0].msg.contains("`< string >`, takes fewer values"),
        "{}",
        w[0]
    );
}

#[test]
fn a_choice_at_run_time_must_leave_the_stack_as_deep() {
    // A symbol known only at run time chooses between clauses that take
    // one value and two and leave one each: the stack would not be as deep.
    let e = fails(&format!("{MOVE} 5 \"Up\" words first sym! step"));
    assert_eq!(e.kind, Kind::Mismatch);
    assert!(
        e.msg
            .contains("clauses of `step` taking 1 and 2 values are chosen between at run time"),
        "{e}"
    );
    // Clauses that agree in what they leave can be chosen between: the
    // value beneath the shorter's is left as it was, or replaced, a union.
    let src = "
Left: Left' ;  Right: Right' ;
turn: < Left >. drop 1 ;
turn: < i64 Right >. drop 2 * 0 ;
5 \"Right\" words first sym! turn";
    assert_eq!(show(src), "<2> 10 0");
    let src = src.replace("\"Right\"", "\"Left\"");
    assert_eq!(show(&src), "<2> 5 1");
}

#[test]
fn a_clause_that_needs_known_values_is_checked_where_applied() {
    // `vec` needs its length while compiling, so a clause of it cannot be
    // compiled on an i64 known only at run time when it is defined.
    let vv = "vv: < i64 >. f64 vec drop 1 ;";
    assert_eq!(show(&format!("{vv} 2 vv")), "<1> 1");
    let e = fails(&format!("{vv} \"a b\" words length vv"));
    assert_eq!(e.kind, Kind::Unknown);
    assert!(
        e.msg.contains("`vec` needs a length known at compile time"),
        "{e}"
    );
}
