//! Types of their own (doc/design/TYPES.md 2.11): `own` makes one of a
//! representation, and `;` names it with the path it defines, so two over
//! the same representation are two. It is a subset of its representation,
//! as `bool` is of i64: accepted wherever that is, matched less well than a
//! clause of its own, and forgotten where the representation's word is
//! applied.
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

const TEMPS: &str = "celsius: f64 own ; fahrenheit: f64 own ;";

#[test]
fn named_where_it_is_defined() {
    // `!` gives the type to a value of its representation, which checks
    // nothing and costs nothing.
    assert_eq!(
        typed(&format!("{TEMPS} 20.0 celsius ! 20.0 fahrenheit !")),
        "<2> 20.0 < celsius > 20.0 < fahrenheit >"
    );
    // Both f64s, and two types.
    let e = fails(&format!(
        "{TEMPS} ff: < celsius >. drop 1 ; 20.0 fahrenheit ! ff"
    ));
    assert_eq!(e.kind, Kind::NoWord, "{e}");
    // Bound once named, it is an alias.
    assert_eq!(
        typed(&format!("{TEMPS} temp: celsius ; 2.0 temp !")),
        "<1> 2.0 < celsius >"
    );
    // A namespace's is its path.
    assert_eq!(
        typed("math.cc: f64 own ; math.pt: { ab: < cc > ; } ; { ab: 1.0 } math.pt !"),
        "<1> { ab' 1.0 } < { ab: math.cc } >"
    );
}

#[test]
fn one_no_semicolon_binds_is_an_error() {
    for src in ["f64 own 1.0 swap !", "meter: < f64 own > ;"] {
        let e = fails(src);
        assert_eq!(e.kind, Kind::Mismatch, "{src}: {e}");
        assert!(
            e.msg
                .contains("f64 own is a type of its own, named where it is defined"),
            "{e}"
        );
    }
}

#[test]
fn a_subset_of_its_representation() {
    // The representation's word forgets it, a generic word keeps it.
    assert_eq!(
        typed(&format!(
            "{TEMPS} 20.0 celsius ! = sqrt swap 1.0 celsius ! max"
        )),
        "<2> 4.47213595499958 < f64 > 20.0 < celsius >"
    );
    // Wanted, it is not made of a value of its representation, known only
    // as the program runs, but by `!`; a literal takes it from context.
    let ff = format!("{TEMPS} ff: < celsius >. drop 1 ;");
    let e = fails(&format!("{ff} 20.0 sqrt ff"));
    assert_eq!(e.kind, Kind::NoWord, "{e}");
    assert_eq!(
        typed(&format!("{ff} 20.0 sqrt celsius ! ff 20 ff")),
        "<2> 1 < i64 > 1 < i64 >"
    );
    let e = fails(&format!("{TEMPS} \"a\" celsius !"));
    assert_eq!(e.kind, Kind::Mismatch, "{e}");
}

#[test]
fn a_clause_of_its_own_keeps_it() {
    // The program's `+`, tried before the core's: `price 3 +` takes it, a
    // literal taking its type from context, and a plain f64 goes on to the
    // core's, its own body's `+` among them.
    let cash = "cash: f64 own ; \
                +: < cash cash >. < f64 f64 > ! + cash ! < cash >. ;";
    assert_eq!(
        typed(&format!("{cash} 2.5 cash ! = 3 + swap 2.0 sqrt +")),
        "<2> 5.5 < cash > 3.914213562373095 < f64 >"
    );
    // The clause that fits best, wherever it is found: literals alone are
    // the core's, a literal's own kind fitting it best (TYPES.md 2.8).
    assert_eq!(typed(&format!("{cash} 2.0 3.0 +")), "<1> 5.0 < f64 >");
    assert_eq!(typed(&format!("{cash} 1 2 +")), "<1> 3 < i64 >");
    assert_eq!(
        typed(&format!("{cash} 2.0 sqrt 3.0 +")),
        "<1> 4.414213562373095 < f64 >"
    );
}

#[test]
fn over_a_record() {
    let place = "point: { x: < f64 > ; y: < f64 > ; } ; place: point own ;";
    assert_eq!(
        typed(&format!("{place} {{ x: 1 y: 2 }} place !")),
        "<1> { x' 1.0 y' 2.0 } < place >"
    );
    assert_eq!(
        typed(&format!("{place} {{ x: 1 y: 2 }} place ! x' at")),
        "<1> 1.0 < f64 >"
    );
}

#[test]
fn a_type_made_of_itself_is_to_come() {
    let e = fails("tree: { ab: < tree > ; cd: < i64 > ; } ;");
    assert_eq!(e.kind, Kind::Limit);
    assert!(e.msg.contains("a type made of itself"), "{e}");
    assert_eq!(e.pos.unwrap().col, 1);
}
