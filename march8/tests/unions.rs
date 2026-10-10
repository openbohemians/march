//! Values of a union type (doc/design/TYPES.md 3.6): `nil`, unions made where
//! clauses chosen at run time leave different types, and words applied to
//! each of a union's types by its tag.
use march8::{Kind, Level, Op, Primitive, Session};

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

const M: &str = "{ \"a\" 1 \"b\" 2 }";

/// `b` leaves a string for a positive number, and 1 otherwise.
const B: &str = "[ 0 gt?. ] positive? def.
    [ < i64 positive? > drop. \"yes\" ] b def.
    [ < i64 > drop. 1 i64. ] b def.";

#[test]
fn nil_is_its_own_value() {
    assert_eq!(typed(&["nil."]), "<1> nil < nil >");
    assert_eq!(show(&["nil. show."]), "<1> \"nil\"");
    assert_eq!(show(&["[ < atom > drop. 1 ] k def. nil. k."]), "<1> 1");
}

#[test]
fn get_leaves_a_value_or_nil() {
    let s = format!("{M} \"a\" get. {M} \"z\" get.");
    assert_eq!(typed(&[&s]), "<2> 1 < i64 nil or. > nil < i64 nil or. >");
    // Its outputs name the union, so it is not warned of.
    let mut s = Session::new();
    s.eval(&format!("{M} \"a\" get.")).unwrap();
    assert!(s.take_warnings().is_empty());
}

#[test]
fn a_word_is_applied_to_each_type_by_the_tag() {
    let port = "[ < i64 > 10 *. ] port def. [ < nil > drop. 80 ] port def.";
    assert_eq!(
        show(&[port, &format!("{M} \"a\" get. port. {M} \"z\" get. port.")]),
        "<2> 10 80"
    );
    assert_eq!(
        show(&[&format!("{M} \"a\" get. show. {M} \"z\" get. show.")]),
        "<2> \"1\" \"nil\""
    );
    // A type with a better clause of its own is applied apart: `>string` of
    // a string is its text, where `show` quotes it.
    assert_eq!(
        show(&[B, "3 -3", "b. >string. swap. b. >string."]),
        "<2> \"1\" \"yes\""
    );
    assert_eq!(show(&[B, "3", "b. show."]), "<1> \"\\\"yes\\\"\"");
    // Two unions: each of the first's types, then each of the second's.
    let add = format!("{M} \"a\" get. {M} \"b\" get.");
    let both = "[ < i64 i64 > +. ] add def. [ < i64 nil > drop. ] add def.
        [ < nil a > swap. drop. ] add def.";
    assert_eq!(show(&[both, &format!("{add} add.")]), "<1> 3");
    assert_eq!(
        show(&[both, &format!("{M} \"z\" get. {M} \"b\" get. add.")]),
        "<1> 2"
    );
}

#[test]
fn a_type_with_no_clause_is_no_word() {
    // Every type a union may have at run time needs a clause: `+` has none
    // for nil.
    let mut s = Session::new();
    let e = s.eval(&format!("{M} \"a\" get. 1 +.")).unwrap_err();
    assert_eq!(e.kind, Kind::NoWord);
    assert!(
        e.to_string()
            .contains("where the value of `i64 nil or` is nil"),
        "{e}"
    );
    assert_eq!(
        session(&[&format!("{M} \"a\" get. num.")]).err(),
        Some(Kind::Mismatch)
    );
}

#[test]
fn some_words_take_a_union_whole() {
    // Stack words, a type variable, `atom`, and a union that holds it.
    let mut s = Session::new();
    let ops = s
        .compile(&format!("{M} \"a\" get. dup. swap. drop."))
        .unwrap()
        .0;
    assert!(!ops.contains(&Op::Prim(Primitive::UnionTag)));
    let k = "[ < atom > drop. 1 ] k def. [ < i64 nil or. > drop. 2 ] w def.";
    assert_eq!(
        show(&[
            k,
            &format!("{M} \"a\" get. k. {M} \"z\" get. w. 3 i64. w. nil. w.")
        ]),
        "<4> 1 2 2 2"
    );
}

#[test]
fn arrays_hold_unions() {
    let xs = format!("( {M} \"a\" get. {M} \"z\" get. )");
    assert_eq!(typed(&[&xs]), "<1> ( 1 nil ) < 2 i64 nil or. vec. >");
    assert_eq!(
        show(&[&format!("{xs} [ show. ] map.")]),
        "<1> ( \"1\" \"nil\" )"
    );
}

#[test]
fn a_union_made_at_run_time_is_noted() {
    // An informative warning, where the union is made; none where the
    // choice is made now, or the outputs name the union.
    let mut s = Session::new();
    s.eval(B).unwrap();
    s.eval("3").unwrap();
    s.eval("b.").unwrap();
    let w = s.take_warnings();
    assert_eq!(w.len(), 1, "{w:?}");
    assert_eq!(w[0].level, Level::Informative);
    s.eval("drop. 3 b. -3 b.").unwrap();
    assert!(s.take_warnings().is_empty());
    assert_eq!(s.show(), "<2> \"yes\" 1");
    let named = "[ 0 gt?. ] positive? def.
        [ < i64 positive? -- i64 string or. > drop. \"yes\" ] g def.
        [ < i64 -- i64 string or. > drop. 1 i64. ] g def.";
    let mut s = Session::new();
    s.eval(named).unwrap();
    s.eval("3").unwrap();
    s.eval("g.").unwrap();
    assert!(s.take_warnings().is_empty());
    assert_eq!(s.show_typed(), "<1> \"yes\" < i64 string or. >");
}
