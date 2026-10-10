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

const SQ: &str = "[ < num > dup. ⋅. ] sq def.";

#[test]
fn a_union_matches_any_of_its_types() {
    assert_eq!(show(&[SQ, "3 sq. 2.5 sq. 3 i64. sq."]), "<3> 9 6.25 9");
    assert_eq!(show(&[SQ, "4.0 f64.", "sq."]), "<1> 16.0");
    assert_eq!(session(&[SQ, "\"a\" sq."]).err(), Some(Kind::NoWord));
    let f = "[ < i64 string or. > drop. 1 ] f def.";
    assert_eq!(show(&[f, "\"x\" f. 4 i64. f."]), "<2> 1 1");
    assert_eq!(session(&[f, "2.5 f64. f."]).err(), Some(Kind::NoWord));
    // Inside a constructor.
    let n = "[ < num ary. > length. ] n def.";
    assert_eq!(show(&[n, "( 1 2 ) n. ( 1.5 ) n."]), "<2> 2 1");
    assert_eq!(session(&[n, "( \"a\" ) n."]).err(), Some(Kind::NoWord));
}

#[test]
fn one_type_beats_a_union_beats_atom_beats_a_variable() {
    let k = [
        "[ < num > drop. 1 ] k def.",
        "[ < i64 > drop. 2 ] k def.",
        "[ < atom > drop. 3 ] k def.",
        "[ < a > drop. 4 ] k def.",
    ];
    // A literal integer is a `num` itself, since `int#` is one of its types.
    assert_eq!(
        show(&[
            &k.join(" "),
            "5 i64. k. 5.0 f64. k. 7 k. \"s\" k. ( 1 ) k. [ ] k."
        ]),
        "<6> 2 1 1 3 4 4"
    );
}

#[test]
fn equal_unions_are_one_type() {
    // In any order, so the second clause replaces the first.
    assert_eq!(
        show(&[
            "[ < f64 i64 or. > drop. 1 ] g def. [ < i64 f64 or. > drop. 2 ] g def.",
            "3 i64. g."
        ]),
        "<1> 2"
    );
    assert_eq!(
        show(&["[ < i64 i64 or. f64 or. > drop. 1 ] h def. 3 i64. h. 3.0 f64. h."]),
        "<2> 1 1"
    );
}

#[test]
fn a_class_checks_and_keeps_the_type() {
    // Applied, or as an output, a class checks the value, which keeps its
    // own type.
    assert_eq!(show(&["3 < num > 4 +. 7 num."]), "<2> 7 7");
    let inc = "[ < num -- num > 1 +. ] inc def.";
    assert_eq!(show(&[inc, "3 inc. 2.5 inc."]), "<2> 4 3.5");
    assert_eq!(session(&["\"x\" num."]).err(), Some(Kind::Mismatch));
    assert_eq!(
        session(&["[ < num -- num > drop. \"no\" ] bad def. 3 bad."]).err(),
        Some(Kind::Mismatch)
    );
    // Messages name the class.
    let mut s = Session::new();
    let e = s.eval("\"x\" < num ary. >").unwrap_err();
    assert!(
        e.to_string().contains("expected num ary, found string"),
        "{e}"
    );
}

#[test]
fn def_names_a_type() {
    let config = "string i64 map. config def. [ < config > keys. ] ks def.";
    assert_eq!(show(&[config, "{ \"a\" 1 } ks."]), "<1> ( \"a\" )");
    assert_eq!(
        show(&["i64 f64 or. whole def. [ < whole > drop. 1 ] w def. 2.0 f64. w."]),
        "<1> 1"
    );
    // A name that is a type already; and `or` on what is not a type is
    // logical, so 2, not a bool, is no truth.
    assert_eq!(
        session(&["i64 f64 or. i64 def."]).err(),
        Some(Kind::Mismatch)
    );
    assert_eq!(show(&["0 1 or."]), "<1> true");
    assert_eq!(session(&["1 2 or."]).err(), Some(Kind::Literal));
}
