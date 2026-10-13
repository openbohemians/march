//! Classes in patterns: unions with `or`, `num`, and `def` naming a type
//! (doc/design/TYPES.md 3.6).
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

const SQ: &str = "sq: [ < num > dup ⋅ ] def";

#[test]
fn a_union_matches_any_of_its_types() {
    assert_eq!(show(&[SQ, "3 sq 2.5 sq 3 i64. sq"]), "<3> 9 6.25 9");
    assert_eq!(show(&[SQ, "4.0 f64.", "sq"]), "<1> 16.0");
    assert_eq!(session(&[SQ, "\"a\" sq"]).err(), Some(Kind::NoWord));
    let f = "ff: [ < i64 string or > drop 1 ] def";
    assert_eq!(show(&[f, "\"x\" ff 4 i64. ff"]), "<2> 1 1");
    assert_eq!(session(&[f, "2.5 f64. ff"]).err(), Some(Kind::NoWord));
    // Inside a constructor.
    let n = "nn: [ < num ary > length ] def";
    assert_eq!(show(&[n, "( 1 2 ) nn ( 1.5 ) nn"]), "<2> 2 1");
    assert_eq!(session(&[n, "( \"a\" ) nn"]).err(), Some(Kind::NoWord));
}

#[test]
fn one_type_beats_a_union_beats_atom_beats_a_variable() {
    let k = [
        "kk: [ < num > drop 1 ] def",
        "kk: [ < i64 > drop 2 ] def",
        "kk: [ < atom > drop 3 ] def",
        "kk: [ < a > drop 4 ] def",
    ];
    // A literal integer is a `num` itself, since `int#` is one of its types.
    assert_eq!(
        show(&[
            &k.join(" "),
            "5 i64. kk 5.0 f64. kk 7 kk \"s\" kk ( 1 ) kk [ ] kk"
        ]),
        "<6> 2 1 1 3 4 4"
    );
}

#[test]
fn equal_unions_are_one_type() {
    // In any order, so the second clause replaces the first.
    assert_eq!(
        show(&[
            "gg: [ < f64 i64 or > drop 1 ] def gg: [ < i64 f64 or > drop 2 ] def",
            "3 i64. gg"
        ]),
        "<1> 2"
    );
    assert_eq!(
        show(&["hh: [ < i64 i64 or f64 or > drop 1 ] def 3 i64. hh 3.0 f64. hh"]),
        "<2> 1 1"
    );
}

#[test]
fn a_class_checks_and_keeps_the_type() {
    // Applied, or as an output, a class checks the value, which keeps its
    // own type.
    assert_eq!(show(&["3 < num > . 4 + 7 num."]), "<2> 7 7");
    let inc = "inc: [ < num -- num > 1 + ] def";
    assert_eq!(show(&[inc, "3 inc 2.5 inc"]), "<2> 4 3.5");
    assert_eq!(session(&["\"x\" num."]).err(), Some(Kind::Mismatch));
    assert_eq!(
        session(&["bad: [ < num -- num > drop \"no\" ] def 3 bad"]).err(),
        Some(Kind::Mismatch)
    );
    // Messages name the class.
    let mut s = Session::new();
    let e = s.eval("\"x\" < num ary > .").unwrap_err();
    assert!(
        e.to_string().contains("expected num ary, found string"),
        "{e}"
    );
}

#[test]
fn def_names_a_type() {
    let config = "config: < string i64 map > def ks: [ < config > keys ] def";
    assert_eq!(show(&[config, "{ \"a\" 1 } ks"]), "<1> ( \"a\" )");
    assert_eq!(
        show(&["whole: < i64 f64 or > def ww: [ < whole > drop 1 ] def 2.0 f64. ww"]),
        "<1> 1"
    );
    // A name that is a type already; and `or` on what is not a type is
    // logical, so 2, not a bool, is no truth.
    assert_eq!(
        session(&["i64: < i64 f64 or > def"]).err(),
        Some(Kind::Mismatch)
    );
    assert_eq!(show(&["0 1 or"]), "<1> true");
    assert_eq!(session(&["1 2 or"]).err(), Some(Kind::Literal));
}
