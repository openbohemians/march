//! Failing (doc/design/SYMBOLIC-FORTH.md 29): `fail` takes a kind, or a
//! message and a kind, and never returns. While compiling it is an error
//! where the failing code was applied; where the code may not run, it fails
//! only if it runs, a trap, which the session reports.
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

/// The trap a program's run ended in, its kind and its message.
fn trap(s: &mut Session, src: &str) -> (String, String) {
    match s.eval(src).unwrap_err().kind {
        Kind::Run(march8::machine::Error::Trap(kind, why)) => (kind, why),
        k => panic!("{src}: {k:?}"),
    }
}

#[test]
fn at_run_time_it_traps() {
    // Code that may not run fails only if it runs, a trap: the clause a
    // choice made as the program runs, here on a value known only then.
    let ff = "ff: < 0 > [ ] ; ff: < i64 > [ ( \"not zero\" no-good' ) fail ] ;";
    let mut s = Session::new();
    s.eval(ff).unwrap();
    s.eval("0").unwrap();
    s.eval("ff").unwrap();
    assert_eq!(s.show(), "<1> 0");
    s.eval("drop 3").unwrap();
    assert_eq!(trap(&mut s, "ff"), ("no-good".into(), "not zero".into()));
    // The stack is as it was.
    assert_eq!(s.show(), "<1> 3");
    let e = s.eval("ff").unwrap_err();
    assert_eq!(e.to_string(), "not zero");
    // A kind alone has no message, and is reported by its kind.
    let mut s = Session::new();
    s.eval("gg: < 0 > [ ] ; gg: < i64 > [ nope' fail ] ; 3")
        .unwrap();
    assert_eq!(trap(&mut s, "gg"), ("nope".into(), "".into()));
    assert_eq!(s.eval("gg").unwrap_err().to_string(), "fails: `nope`");
    // Inside a loop's body, as the choice is made for each element.
    let mut s = Session::new();
    s.eval(ff).unwrap();
    s.eval("( 0 0 ) ( 0 5 0 )").unwrap();
    assert_eq!(trap(&mut s, "[ ff ] map").0, "no-good");
    s.eval("drop [ ff ] map").unwrap();
    assert_eq!(s.show(), "<1> ( 0 0 )");
}

#[test]
fn its_kind_and_message_may_be_known_only_then() {
    // A message made as the program runs.
    let mut s = Session::new();
    s.eval("ff: < i64 [ 0 lt? ] ? > [ ( \"\\_ is negative\" bad' ) fail ] ; ff: < i64 > [ ] ;")
        .unwrap();
    s.eval("-5").unwrap();
    assert_eq!(trap(&mut s, "ff"), ("bad".into(), "-5 is negative".into()));
    // And a kind.
    let mut s = Session::new();
    s.eval("gg: < string [ length 0 gt? ] ? > [ sym! fail ] ; gg: < string > [ ] ;")
        .unwrap();
    s.eval("\"oops\"").unwrap();
    assert_eq!(trap(&mut s, "gg"), ("oops".into(), "".into()));
}

#[test]
fn what_is_sure_to_fail_fails_while_compiling() {
    // Code that is sure to fail is an error while compiling, and where its
    // kind is known only as the program runs, it says so.
    let mut s = Session::new();
    s.eval("ff: < 0 > [ a' fail ] ; ff: < i64 > [ b' fail ] ; 3")
        .unwrap();
    let e = s.eval("ff").unwrap_err();
    assert_eq!(e.kind, Kind::Unknown);
    assert!(e.to_string().contains("whichever clause is chosen"), "{e}");
    let mut s = Session::new();
    s.eval("\"x\"").unwrap();
    let e = s.eval("sym! fail").unwrap_err();
    assert_eq!(e.kind, Kind::Unknown, "{e}");
    // A loop's quotation that always fails has no effect to compile by.
    let e = error("( 1 2 ) [ drop nope' fail ] each");
    assert_eq!(e.kind, Kind::Mismatch);
    assert!(e.to_string().contains("always fails"), "{e}");
}
