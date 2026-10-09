//! Arrays as vectors: arithmetic element by element on two arrays, `zip`,
//! the outer product `×` and `table`, and `dot` (core/core.march).
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

#[test]
fn two_arrays_element_by_element() {
    assert_eq!(
        show(&[
            "( 1 2 3 ) ( 10 20 30 ) +. ( 1 2 ) ( 3 4 ) ⋅. ( 6.0 8.0 ) ( 2.0 4.0 ) ÷. ( 5 6 ) ( 1 1 ) -."
        ]),
        "<4> ( 11 22 33 ) ( 3 8 ) ( 3.0 2.0 ) ( 4 5 )"
    );
    // Nested arrays add level by level; vecs keep their length.
    assert_eq!(
        show(&["( ( 1 2 ) ( 3 4 ) ) ( ( 5 6 ) ( 7 8 ) ) +."]),
        "<1> ( ( 6 8 ) ( 10 12 ) )"
    );
    assert_eq!(show(&["( 1 2 3 ) ( 4 5 6 ) +. length."]), "<1> 3");
    // At run time too.
    assert_eq!(show(&["( 6 7 ) ( 4 5 )", "-."]), "<1> ( 2 2 )");
    // No promotion: integers and floats do not mix.
    assert_eq!(
        session(&["( 1 2 ) ( 1.5 2.5 ) +."]).err(),
        Some(Kind::NoWord)
    );
}

#[test]
fn lengths_must_agree() {
    // Known lengths are checked at compile time; others at run time.
    assert_eq!(
        session(&["( 1 2 ) ( 3 4 5 ) +."]).err(),
        Some(Kind::Mismatch)
    );
    let any = "( ( 1 2 ) ( 3 ) ) 1 at.";
    assert_eq!(
        session(&[any, "( 1 2 3 ) +."]).err(),
        Some(Kind::Run(march8::machine::Error::User(3)))
    );
    assert_eq!(show(&[any, "( 10 20 ) +."]), "<1> ( 11 22 )");
}

#[test]
fn outer_and_dot_products() {
    // `×`: the table of products, a row for each element of the first.
    assert_eq!(
        show(&["( 1 2 ) ( 10 20 30 ) ×."]),
        "<1> ( ( 10 20 30 ) ( 20 40 60 ) )"
    );
    assert_eq!(show(&["( 1 2 3 ) ( 4 5 6 ) ×. length."]), "<1> 3");
    // `table` for any word; `zip` pairs in step.
    assert_eq!(
        show(&["( 1 2 ) ( \"a\" \"b\" ) [ ~. i64>text. concat. ] table."]),
        "<1> ( ( \"a1\" \"b1\" ) ( \"a2\" \"b2\" ) )"
    );
    assert_eq!(show(&["( 1 2 ) ( 3 4 ) [ ⋅. 1 +. ] zip."]), "<1> ( 4 9 )");
    // `dot`: the sum of the products.
    assert_eq!(show(&["( 1 2 3 ) ( 4 5 6 ) dot."]), "<1> 32");
    assert_eq!(show(&["( 1.5 2.0 ) ( 2.0 4.0 ) dot."]), "<1> 11.0");
}
