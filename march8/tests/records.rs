//! Records (doc/design/TYPES.md 2.11): a record is a map of types, as a
//! tuple is a sequence of them, and a record value is written as a map is.
//! A map literal of symbols is undecided: a record where a record is
//! wanted, a map where a map is, and with nothing to take it, a map when
//! its values are alike and a record when they are not.
use march8::{Kind, Session};

fn typed(src: &str) -> String {
    let mut s = Session::new();
    if let Err(e) = s.eval(src) {
        panic!("{src}: {e}");
    }
    s.show_typed()
}

fn fails(src: &str) -> march8::Error {
    Session::new().eval(src).expect_err(src)
}

const POINT: &str = "point: { x: < f64 > ; y: < f64 > ; } ;
norm: < point >. [ p | p.x dup * p.y dup * + sqrt ] . ;";

#[test]
fn a_map_literal_settles_by_its_values() {
    assert_eq!(
        typed("{ ab: 1 cd: 2 }"),
        "<1> { ab' 1 cd' 2 } < sym i64 map >"
    );
    // Values not alike make a record, shown as a map is, its type telling
    // them apart.
    assert_eq!(
        typed("{ ab: 1 cd: \"x\" }"),
        "<1> { ab' 1 cd' \"x\" } < { ab: i64 cd: string } >"
    );
    // A key written again replaces the one before.
    assert_eq!(typed("{ ab: 1 ab: 2 }"), "<1> { ab' 2 } < sym i64 map >");
}

#[test]
fn a_record_type_is_a_map_of_types() {
    // `;` in braces makes an entry, so a record type's fields are its
    // entries; written in a bracket, its fields are types already.
    let mut s = Session::new();
    s.eval(POINT).unwrap();
    s.eval("{ x: 1 y: 2 } point !").unwrap();
    assert_eq!(
        s.show_typed(),
        "<1> { x' 1.0 y' 2.0 } < { x: f64 y: f64 } >"
    );
    // Known by its content, so a bracket of the same fields is the type.
    s.eval("< { y: f64 x: f64 } > !").unwrap();
    assert_eq!(
        s.show_typed(),
        "<1> { x' 1.0 y' 2.0 } < { x: f64 y: f64 } >"
    );
}

#[test]
fn a_clause_that_wants_a_record_makes_one() {
    // A map literal taken as the record of its keys, each value converted
    // as it would be alone, here integers to floats.
    assert_eq!(
        typed(&format!("{POINT} {{ x: 3 y: 4 }} norm")),
        "<1> 5.0 < f64 >"
    );
    // At run time too, its values held in the order of its keys.
    assert_eq!(
        typed(&format!(
            "{POINT} 4.0 3.0 [ yy xx | {{ y: yy x: xx }} ] . norm"
        )),
        "<1> 5.0 < f64 >"
    );
    // A map is wanted, a map it is, made as the program runs.
    assert_eq!(
        typed("\"a\" \"b\" [ p q | { x: p y: q } ] . keys"),
        "<1> ( y' x' ) < sym ary >"
    );
    // Fields missing, extra or of another type are no record of it.
    for wrong in ["{ x: 3 }", "{ x: 3 y: 4 zz: 1 }", "{ x: \"a\" y: 4 }"] {
        let e = fails(&format!("{POINT} {wrong} norm"));
        assert_eq!(e.kind, Kind::NoWord, "{wrong}: {e}");
    }
}

#[test]
fn a_field_is_read_by_at_or_a_path() {
    // Its key known while compiling, a field has its own type, and a path
    // from a bracket's name is short for `at`, `p.x` for `p x' at`.
    assert_eq!(typed("{ ab: 1 cd: \"x\" } cd' at"), "<1> \"x\" < string >");
    assert_eq!(
        typed("{ ab: { cd: 7 ef: \"z\" } gh: 1 } [ p | p.ab.cd ] ."),
        "<1> 7 < i64 >"
    );
    assert_eq!(
        typed("( { x: 1 y: \"a\" } { x: 2 y: \"b\" } ) [ y' at ] map"),
        "<1> ( \"a\" \"b\" ) < 2 string vec >"
    );
    // A field the record lacks is an error while compiling, as a misspelled
    // label is.
    let e = fails("{ ab: 1 cd: \"x\" } [ p | p.zz ] .");
    assert_eq!(e.kind, Kind::Mismatch);
    assert!(
        e.msg.contains("{ ab: i64 cd: string } has no field `zz`"),
        "{e}"
    );
}

#[test]
fn an_array_holds_what_it_would_alone() {
    // Undecided map literals in an array are maps, and records records.
    assert_eq!(
        typed("( { x: 1 } { x: 2 } )"),
        "<1> ( { x' 1 } { x' 2 } ) < 2 sym i64 map vec >"
    );
    assert_eq!(
        typed("( { x: 1 y: \"a\" } { x: 2 y: \"b\" } )"),
        "<1> ( { x' 1 y' \"a\" } { x' 2 y' \"b\" } ) < 2 { x: i64 y: string } vec >"
    );
}

#[test]
fn an_entry_by_semicolon_is_a_type_for_now() {
    let e = fails("{ ab: 1 ; }");
    assert_eq!(e.kind, Kind::Limit);
    assert!(
        e.msg.contains("`;` in braces makes an entry of a type"),
        "{e}"
    );
}
