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
            "( 1 2 3 ) ( 10 20 30 ) +. ( 1 2 ) ( 3 4 ) ⋅. ( 6.0 8.0 ) ( 2.0 4.0 ) /*. ( 5 6 ) ( 1 1 ) -+."
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
    assert_eq!(show(&["( 6 7 ) ( 4 5 )", "-+."]), "<1> ( 2 2 )");
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

#[test]
fn tensors_of_any_rank() {
    // A tensor and one of lower rank: element by element along the leading
    // axis, so the lower broadcasts across the trailing axes. Shapes stay
    // known.
    let mut s = session(&["( ( 1 2 ) ( 3 4 ) ) 10 ⋅."]).unwrap();
    assert_eq!(
        s.show_typed(),
        "<1> ( ( 10 20 ) ( 30 40 ) ) < 2 2 i64 vec vec >"
    );
    s = session(&["( ( 1 2 ) ( 3 4 ) ) ( 10 20 ) +."]).unwrap();
    assert_eq!(
        s.show_typed(),
        "<1> ( ( 11 22 ) ( 13 24 ) ) < 2 2 i64 vec vec >"
    );
    assert_eq!(
        show(&["( ( ( 1 2 ) ( 3 4 ) ) ) 1 +."]),
        "<1> ( ( ( 2 3 ) ( 4 5 ) ) )"
    );
    assert_eq!(
        show(&["10 ( ( 1 2 ) ( 3 4 ) ) -+. ( 2.0 4.0 ) 2.0 /*."]),
        "<2> ( ( 9 8 ) ( 7 6 ) ) ( 1.0 2.0 )"
    );
    // An atom, anything not a container, at any rank, on either side.
    s = session(&["( ( ( ( 1 2 ) ) ) ) 10 ⋅."]).unwrap();
    assert_eq!(
        s.show_typed(),
        "<1> ( ( ( ( 10 20 ) ) ) ) < 1 1 1 2 i64 vec vec vec vec >"
    );
    assert_eq!(
        show(&["( ( ( ( ( 8 ) ) ) ) ) 2 div. 3 ( ( ( 1 2 ) ) ) ×."]),
        "<2> ( ( ( ( ( 4 ) ) ) ) ) ( ( ( 3 6 ) ) )"
    );
    // A quotation, a type or a name is not an atom.
    for bad in ["( 1 2 ) [ 3 ] +.", "i64 ( 1 2 ) +.", "( 1 2 ) \\x ⋅."] {
        assert_eq!(session(&[bad]).err(), Some(Kind::NoWord), "{bad}");
    }
}

#[test]
fn the_tensor_product_adds_ranks() {
    let s = session(&["( ( 1 2 ) ( 3 4 ) ) ( ( 5 6 ) ( 7 8 ) ) ×."]).unwrap();
    assert!(s.show_typed().ends_with("< 2 2 2 2 i64 vec vec vec vec >"));
    assert_eq!(
        show(&["( 1 2 ) ( 3 4 5 ) ×."]),
        "<1> ( ( 3 4 5 ) ( 6 8 10 ) )"
    );
}

#[test]
fn dot_contracts_last_with_first() {
    // Vector · vector, matrix · vector, matrix · matrix: the matrix product.
    assert_eq!(
        show(&[
            "( 1 2 3 ) ( 4 5 6 ) dot. ( ( 1 2 ) ( 3 4 ) ) ( 5 6 ) dot. ( ( 1 2 ) ( 3 4 ) ) ( ( 5 6 ) ( 7 8 ) ) dot."
        ]),
        "<3> 32 ( 17 39 ) ( ( 19 22 ) ( 43 50 ) )"
    );
    // The contracted lengths must agree: a 2×3 by a 2×2 is refused before
    // anything runs.
    assert_eq!(
        session(&["( ( 1 2 3 ) ( 4 5 6 ) ) ( ( 1 2 ) ( 3 4 ) ) dot."]).err(),
        Some(Kind::Mismatch)
    );
    // At run time too.
    assert_eq!(
        show(&["( ( 1 2 ) ( 3 4 ) )", "( ( 5 6 ) ( 7 8 ) ) dot."]),
        "<1> ( ( 19 22 ) ( 43 50 ) )"
    );
}

#[test]
fn transpose_swaps_indices() {
    let s = session(&["( ( 1 2 3 ) ( 4 5 6 ) ) transpose."]).unwrap();
    assert_eq!(
        s.show_typed(),
        "<1> ( ( 1 4 ) ( 2 5 ) ( 3 6 ) ) < 3 2 i64 vec vec >"
    );
    // A matrix times its transpose.
    assert_eq!(
        show(&["( ( 1 2 ) ( 3 4 ) ) dup. transpose. dot."]),
        "<1> ( ( 5 11 ) ( 11 25 ) )"
    );
}
