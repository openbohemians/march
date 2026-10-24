//! Ratio literals (Thomas, 2026-10-13): `1:2` is one half, exact while it
//! is a literal, an f64 rounded once when it must be a value. Its terms are
//! kept as written, so what takes it can read them: `12:30` is not `2:5`.
use march8::{Kind, Session};

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

#[test]
fn a_ratio_is_a_number() {
    assert_eq!(show("1:2 -1:3 2:4"), "<3> 0.5 -0.3333333333333333 0.5");
    // A literal takes its type from what takes it, as `2.5` does.
    assert_eq!(
        show("half: < f64 >. 2.0 /* < f64 >. ; 1:4 half"),
        "<1> 0.125"
    );
    assert_eq!(show("1:4 money!"), "<1> 0.25");
    let e = fails("1:3 money!");
    assert_eq!(e.kind, Kind::Literal);
    assert!(e.msg.contains("not a whole number of cents"), "{e}");
}

#[test]
fn literals_divide_exactly() {
    // Two literals fold exactly and stay literals, so a ratio's sums are
    // exact where floats' are not.
    assert_eq!(show("1:10 1:10 + 1:10 + 3:10 -+"), "<1> 0.0");
    assert_eq!(
        show("0.1 f64! 0.1 f64! + 0.1 f64! + 0.3 f64! -+"),
        "<1> 5.551115123125783e-17"
    );
    // Dividing two literals makes a ratio: `1 3 /*` is 1:3, and three of it
    // is exactly 1, compiled as the literal.
    let mut s = Session::new();
    assert_eq!(
        s.compile("1 3 /* 3 *").unwrap().0,
        s.compile("1.0").unwrap().0
    );
    assert_eq!(show("2 / 1.5 1:2 +"), "<2> 0.5 2.0");
    assert_eq!(fails("1 0 /*").kind, Kind::Arithmetic);
}

#[test]
fn narrowing_a_ratio_is_exact() {
    assert_eq!(
        show("7:2 round -7:2 round 7:2 floor -7:2 floor 7:2 ceil 7:2 trunc -7:2 trunc"),
        "<7> 4 -4 3 -4 4 3 -3"
    );
    let e = fails("1:2 i64!");
    assert_eq!(e.kind, Kind::Literal);
    assert!(e.msg.contains("say how it rounds"), "{e}");
}

#[test]
fn a_ratio_keeps_its_terms() {
    // A ratio of more terms reads, for what takes it to read, but is no
    // number; nor is one whose second term is zero, as a time may have.
    let e = fails("2026:10:13");
    assert_eq!(e.kind, Kind::Literal);
    assert!(
        e.msg
            .contains("2026:10:13 is not a number: a number is a ratio of two terms"),
        "{e}"
    );
    assert_eq!(fails("12:00 f64!").kind, Kind::Arithmetic);
    // Whole numbers between colons, a sign only in front.
    let e = fails("1:-2");
    assert_eq!(e.kind, Kind::Literal);
    assert!(e.msg.contains("is not a ratio"), "{e}");
}
