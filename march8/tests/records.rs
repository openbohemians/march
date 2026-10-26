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
fn a_field_is_no_word() {
    // A field named as a word is, `last`, is a key of its record type, so
    // the source defines no `last` of its own, and the core's is applied,
    // before the record type and after.
    assert_eq!(
        typed(
            "ff: < i64 ary >. last ; ww: { last: < i64 > ; } ; \
             ( 1 2 ) < i64 ary >. ff ( 3 4 ) < i64 ary >. last { last: 5 } < ww >. last' at"
        ),
        "<3> 2 < i64 > 4 < i64 > 5 < i64 >"
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
fn a_record_forgets_into_a_map_where_one_is_wanted() {
    // Its values alike, a map of their type; else of their union, tagged.
    let r = "{ x: 1 y: 2 } < { x: < i64 > ; y: < i64 > ; } >.";
    assert_eq!(typed(&format!("{r} values length")), "<1> 2 < i64 >");
    assert_eq!(
        typed(&format!("{r} < sym i64 map >. x' at")),
        "<1> 1 < i64 >"
    );
    assert_eq!(
        typed("{ x: 1 y: \"a\" } y' get"),
        "<1> \"a\" < i64 string nil or or >"
    );
    // A key known only as the program runs reads the map of its fields.
    let mut s = Session::new();
    s.eval("pick: < i64 [ 0 gt ] ? >. drop x' ; pick: < i64 >. drop y' ;")
        .unwrap();
    s.eval("{ x: 1 y: \"a\" } 0").unwrap();
    s.eval("pick at").unwrap();
    assert_eq!(s.show(), "<1> \"a\"");
    // A map of a type the values are not is no word.
    let e = fails("{ x: 1 y: \"a\" } < sym i64 map >.");
    assert_eq!(e.kind, Kind::Mismatch);
}

#[test]
fn a_path_from_a_word_reads_its_fields() {
    // A word that takes nothing heads a path, `cfg.port` for `cfg port'
    // at`, found as any name is, in its namespace too (TYPES.md 2.11).
    let cfg = "cfg: { port: 80 db: { name: \"m\" size: 5 } } ;";
    assert_eq!(
        typed(&format!("{cfg} cfg.port cfg.db.size +")),
        "<1> 85 < i64 >"
    );
    assert_eq!(
        typed("app.cfg: { port: 80 host: \"x\" } ; app.run: cfg.port 1 + ; app.run"),
        "<1> 81 < i64 >"
    );
    let e = fails(&format!("{cfg} cfg.zz"));
    assert!(e.msg.contains("has no field `zz`"), "{e}");
    // Words do not chain: a word that takes something heads no path.
    let e = fails("( 3 1 2 ) sort.length");
    assert_eq!(e.kind, Kind::NoWord);
    assert!(e.msg.contains("`sort length`"), "{e}");
}

#[test]
fn a_field_is_set_by_put() {
    // A new record with the field set, of the same type, so a fold may hand
    // it along; the value takes the field's type, a literal converting.
    assert_eq!(
        typed("{ n: 0 name: \"x\" } n' 5 put"),
        "<1> { n' 5 name' \"x\" } < { n: i64 name: string } >"
    );
    assert_eq!(
        typed(&format!("{POINT} {{ x: 1 y: 2 }} point ! x' 3 put norm")),
        "<1> 3.605551275463989 < f64 >"
    );
    // Undecided, a map literal becomes a record, or, given a key it lacks,
    // the map it is.
    assert_eq!(typed("{ n: 0 } n' 5 put"), "<1> { n' 5 } < { n: i64 } >");
    assert_eq!(
        typed("{ n: 0 } m' 5 put"),
        "<1> { m' 5 n' 0 } < sym i64 map >"
    );
    assert_eq!(
        typed(
            "( 4 5 6 ) { n: 0 last: 0 } [ swap = n' at 1 + n' swap put swap last' swap put ] fold"
        ),
        "<1> { last' 6 n' 3 } < { last: i64 n: i64 } >"
    );
    // A field it lacks, or a value of another type, is an error.
    let e = fails("{ n: 0 name: \"x\" } m' 5 put");
    assert_eq!(e.kind, Kind::Mismatch);
    assert!(e.msg.contains("has no field `m`"), "{e}");
    let e = fails("{ n: 0 name: \"x\" } n' \"y\" put");
    assert_eq!(e.kind, Kind::Mismatch);
    assert!(
        e.msg
            .contains("the field `n` of { n: i64 name: string } is i64, and the value is string"),
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
