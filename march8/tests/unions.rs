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
const B: &str = "positive: 0 gt? ;
    bb: < i64 positive? >. drop \"yes\" ;
    bb: < i64 >. drop 1 i64! ;";

#[test]
fn nil_is_its_own_value() {
    assert_eq!(typed(&["nil"]), "<1> nil < nil >");
    assert_eq!(show(&["nil show"]), "<1> \"nil\"");
    assert_eq!(show(&["kk: < atom >. drop 1 ; nil kk"]), "<1> 1");
}

#[test]
fn get_leaves_a_value_or_nil() {
    let s = format!("{M} \"a\" get {M} \"z\" get");
    assert_eq!(typed(&[&s]), "<2> 1 < i64 nil or > nil < i64 nil or >");
    // Its outputs name the union, so it is not warned of.
    let mut s = Session::new();
    s.eval(&format!("{M} \"a\" get")).unwrap();
    assert!(s.take_warnings().is_empty());
}

#[test]
fn a_word_is_applied_to_each_type_by_the_tag() {
    let port = "port: < i64 >. 10 * ; port: < nil >. drop 80 ;";
    assert_eq!(
        show(&[port, &format!("{M} \"a\" get port {M} \"z\" get port")]),
        "<2> 10 80"
    );
    assert_eq!(
        show(&[&format!("{M} \"a\" get show {M} \"z\" get show")]),
        "<2> \"1\" \"nil\""
    );
    // A type with a better clause of its own is applied apart: `>string` of
    // a string is its text, where `show` quotes it.
    assert_eq!(
        show(&[B, "3 -3", "bb >string swap bb >string"]),
        "<2> \"1\" \"yes\""
    );
    assert_eq!(show(&[B, "3", "bb show"]), "<1> \"\\\"yes\\\"\"");
    // Two unions: each of the first's types, then each of the second's.
    let add = format!("{M} \"a\" get {M} \"b\" get");
    let both = "add: < i64 i64 >. + ; add: < i64 nil >. drop ;
        add: < nil a >. swap drop ;";
    assert_eq!(show(&[both, &format!("{add} add")]), "<1> 3");
    assert_eq!(
        show(&[both, &format!("{M} \"z\" get {M} \"b\" get add")]),
        "<1> 2"
    );
}

#[test]
fn a_type_with_no_clause_is_no_word() {
    // Every type a union may have at run time needs a clause: `+` has none
    // for nil.
    let mut s = Session::new();
    let e = s.eval(&format!("{M} \"a\" get 1 +")).unwrap_err();
    assert_eq!(e.kind, Kind::NoWord);
    assert!(
        e.to_string()
            .contains("where the value of `i64 nil or` is nil"),
        "{e}"
    );
    assert_eq!(
        session(&[&format!("{M} \"a\" get num!")]).err(),
        Some(Kind::Mismatch)
    );
}

#[test]
fn some_words_take_a_union_whole() {
    // Stack words, a type variable, `atom`, and a union that holds it.
    let mut s = Session::new();
    let ops = s
        .compile(&format!("{M} \"a\" get dup swap drop"))
        .unwrap()
        .0;
    assert!(!ops.contains(&Op::Prim(Primitive::UnionTag)));
    let k = "kk: < atom >. drop 1 ; ww: < i64 nil or >. drop 2 ;";
    assert_eq!(
        show(&[
            k,
            &format!("{M} \"a\" get kk {M} \"z\" get ww 3 i64! ww nil ww")
        ]),
        "<4> 1 2 2 2"
    );
}

#[test]
fn arrays_hold_unions() {
    let xs = format!("( {M} \"a\" get {M} \"z\" get )");
    assert_eq!(typed(&[&xs]), "<1> ( 1 nil ) < 2 i64 nil or vec >");
    assert_eq!(
        show(&[&format!("{xs} [ show ] map")]),
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
    s.eval("bb").unwrap();
    let w = s.take_warnings();
    assert_eq!(w.len(), 1, "{w:?}");
    assert_eq!(w[0].level, Level::Informative);
    s.eval("drop 3 bb -3 bb").unwrap();
    assert!(s.take_warnings().is_empty());
    assert_eq!(s.show(), "<2> \"yes\" 1");
    let named = "positive: 0 gt? ;
        gg: < i64 positive? >. drop \"yes\" < i64 string or >. ;
        gg: < i64 >. drop 1 i64! < i64 string or >. ;";
    let mut s = Session::new();
    s.eval(named).unwrap();
    s.eval("3").unwrap();
    s.eval("gg").unwrap();
    assert!(s.take_warnings().is_empty());
    assert_eq!(s.show_typed(), "<1> \"yes\" < i64 string or >");
}

/// A union of values, `code' data' or` (TYPES.md 3.6): the type of just
/// those symbols, a subset of `sym`.
const KIND: &str = "read: code' data' text' or^3 ; name: own' top' or ; kind: read name or ;
  like: < kind >. code' eq ;
  tk: { kind: < kind > ; n: < i64 > ; } ;
  tok: < kind i64 >. [ kd nn | { kind: kd n: nn } < tk >. ] . ;
  pick: < i64 [ 0 gt ] ? >. drop code' ;
  pick: < i64 >. drop top' ;
  stray: < i64 [ 0 gt ] ? >. drop code' ;
  stray: < i64 >. drop frob' ;";

#[test]
fn a_union_of_values_takes_just_them() {
    // Known while compiling, a symbol is one of them or not.
    assert_eq!(show(&[KIND, "code' like data' like"]), "<2> true false");
    assert_eq!(session(&[KIND, "frob' like"]).err(), Some(Kind::NoWord));
    // Known only as the program runs, it is tested then, and inside the
    // clause it is one of them, so it fills a field of their type.
    assert_eq!(show(&[KIND, "1", "pick like"]), "<1> true");
    assert_eq!(show(&[KIND, "0", "pick 5 tok kind' at"]), "<1> top'");
    // One that is another symbol as the program runs fails then, as a
    // value pattern does.
    assert!(matches!(
        session(&[KIND, "0", "stray like"]).err(),
        Some(Kind::Run(march8::machine::Error::Trap(k, _))) if k == "no-word"
    ));
    // A known symbol fills a field of their type, and `put` sets one.
    assert_eq!(
        show(&[KIND, "{ kind: own' n: 1 } < tk >. kind' text' put kind' at"]),
        "<1> text'"
    );
    assert_eq!(
        session(&[KIND, "{ kind: frob' n: 1 } < tk >."]).err(),
        Some(Kind::Mismatch)
    );
    // A symbol known only as the program runs is no `kind` without a test.
    let bare = "bare: < sym i64 >. [ kd nn | { kind: kd n: nn } < tk >. ] . ;";
    assert_eq!(session(&[KIND, bare]).err(), Some(Kind::Mismatch));
    // `or` joins symbols, or types, not one with the other.
    assert_eq!(session(&["code' i64 or"]).err(), Some(Kind::Limit));
}

#[test]
fn a_union_of_values_is_named_and_forgotten() {
    // Named by `;`, and a symbol where a symbol is wanted.
    assert_eq!(typed(&[KIND, "code' 1 tok kind' at"]), "<1> code' < kind >");
    assert_eq!(show(&[KIND, "own' 1 tok kind' at top' eq"]), "<1> false");
    // Unnamed, it is written as made, and a check is one where applied.
    assert_eq!(show(&["code' < code' data' or >."]), "<1> code'");
    let e = Session::new()
        .eval("frob' < code' data' or >.")
        .unwrap_err();
    assert_eq!(e.msg, "expected code' data' or, found frob'");
}
