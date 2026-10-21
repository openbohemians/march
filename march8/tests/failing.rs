//! Failing (doc/design/SYMBOLIC-FORTH.md 29): `fail` takes a kind, or a
//! message and a kind, and never returns. While compiling it is an error
//! where the failing code was applied; at run time it would be a trap, which
//! is not built yet.
use march8::{Kind, Session};

fn error(src: &str) -> march8::Error {
    Session::new().eval(src).unwrap_err()
}

#[test]
fn a_kind_or_a_message_and_a_kind() {
    let e = error("no-file' fail");
    assert_eq!(e.kind, Kind::Fail("no-file".into()));
    assert_eq!(e.to_string(), "1:10: fails: `no-file`");
    let e = error("( \"can't read it\" no-file' ) fail");
    assert_eq!(e.kind, Kind::Fail("no-file".into()));
    assert_eq!(e.to_string(), "1:30: can't read it");
    // The machine's own kinds by their names.
    assert_eq!(error("( \"none\" no-word' ) fail").kind, Kind::NoWord);
    assert_eq!(error("limit' fail").kind, Kind::Limit);
    // Anything else is no kind.
    let e = error("1 fail");
    assert_eq!(e.kind, Kind::Mismatch);
    assert!(e.to_string().contains("and 1 (int#) is neither"), "{e}");
    assert_eq!(error("( 1 no-file' ) fail").kind, Kind::Mismatch);
}

#[test]
fn where_the_failing_code_was_applied() {
    // A word that fails is defined as any other; it fails where applied.
    let ff = "ff: < 0 > [ ] ; ff: < i64 > [ ( \"not zero\" no-good' ) fail ] ;";
    let mut s = Session::new();
    s.eval(ff).unwrap();
    s.eval("0 ff").unwrap();
    assert_eq!(s.show(), "<1> 0");
    let e = s.eval("3 ff").unwrap_err();
    assert_eq!(e.kind, Kind::Fail("no-good".into()));
    assert!(
        e.to_string()
            .ends_with("not zero\n  in `ff`, applied at 1:3"),
        "{e}"
    );
    // A clause that fails leaves whatever it must.
    let mut s = Session::new();
    s.eval("gg: < i64 >< string > [ drop nope' fail ] ; 1")
        .unwrap();
    assert_eq!(s.show(), "<1> 1");
    // Its argument is checked where it is defined.
    assert_eq!(
        Session::new()
            .eval("hh: < i64 > [ 1 fail ] ;")
            .unwrap_err()
            .kind,
        Kind::Mismatch
    );
}

#[test]
fn at_run_time_not_yet() {
    // Code that may not run fails only if it runs: a trap, not built.
    for src in [
        "ff: < 0 > [ ] ; ff: < i64 > [ nope' fail ] ; io.stdin.read length ff",
        "( 1 2 ) [ drop nope' fail ] each",
        "io.stdin.read sym! fail",
    ] {
        let e = error(src);
        assert_eq!(e.kind, Kind::Limit, "{src}");
        assert!(e.to_string().contains("a trap, is not built yet"), "{e}");
    }
}
