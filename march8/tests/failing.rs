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
    assert!(
        e.to_string().contains("and 1 (int#) is none of them"),
        "{e}"
    );
    assert_eq!(error("( 1 no-file' ) fail").kind, Kind::Mismatch);
}

#[test]
fn where_the_failing_code_was_applied() {
    // A word that fails is defined as any other; it fails where applied.
    let ff = "ff: < 0 >. ; ff: < i64 >. ( \"not zero\" no-good' ) fail ;";
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
    s.eval("gg: < i64 >. drop nope' fail < string >. ; 1")
        .unwrap();
    assert_eq!(s.show(), "<1> 1");
    // Its argument is checked where it is defined.
    assert_eq!(
        Session::new()
            .eval("hh: < i64 >. 1 fail ;")
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
    let ff = "ff: < 0 >. ; ff: < i64 >. ( \"not zero\" no-good' ) fail ;";
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
    s.eval("gg: < 0 >. ; gg: < i64 >. nope' fail ; 3").unwrap();
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
    s.eval("ff: < i64 [ 0 lt? ] ? >. ( \"\\_ is negative\" bad' ) fail ; ff: < i64 >. ;")
        .unwrap();
    s.eval("-5").unwrap();
    assert_eq!(trap(&mut s, "ff"), ("bad".into(), "-5 is negative".into()));
    // And a kind.
    let mut s = Session::new();
    s.eval("gg: < string [ length 0 gt? ] ? >. sym! fail ; gg: < string >. ;")
        .unwrap();
    s.eval("\"oops\"").unwrap();
    assert_eq!(trap(&mut s, "gg"), ("oops".into(), "".into()));
}

#[test]
fn what_is_sure_to_fail_fails_while_compiling() {
    // Code that is sure to fail is an error while compiling, and where its
    // kind is known only as the program runs, it says so.
    let mut s = Session::new();
    s.eval("ff: < 0 >. a' fail ; ff: < i64 >. b' fail ; 3")
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

#[test]
fn what_is_not_there_traps() {
    // An element or a key not there, known only as the program runs, is a
    // trap of `mismatch'`, as it is while compiling.
    let mut s = Session::new();
    assert_eq!(
        trap(&mut s, "\"a b\" words [ ws | ws 5 at ] ."),
        (
            "mismatch".into(),
            "the array of 2 has no such element".into()
        )
    );
    assert_eq!(
        trap(&mut s, "sym i64 map empty ab' 1 put ef' at"),
        ("mismatch".into(), "the map has no such key".into())
    );
    let ff = "ff: < string >. words 5 at ; ff: < string k trap >. drop drop \"none\" ;";
    s.eval(ff).unwrap();
    s.eval("\"a b\" ff").unwrap();
    assert_eq!(s.show(), "<1> \"none\"");
}

#[test]
fn a_handler_is_a_clause_for_the_inputs_and_a_trap() {
    // When anything inside the word fails, its inputs come back, the trap
    // goes on top, and the word is applied again, its handler chosen
    // (doc/design/SYMBOLIC-FORTH.md 29). A file not read, at run time:
    let size = "size: < string >. io.file.read length < i64 >. ; \
                size: < string k trap >. drop drop 0 < i64 >. ;";
    let mut s = Session::new();
    s.eval(size).unwrap();
    s.eval("\"no/such/file\" size").unwrap();
    assert_eq!(s.show(), "<1> 0");
    // Arithmetic known while compiling fails where it is handled as the
    // program runs, so it is caught there too.
    let sd = "sd: < i64 i64 >. div ; sd: < i64 i64 k trap >. drop drop drop 0 ;";
    let mut s = Session::new();
    s.eval(sd).unwrap();
    s.eval("7 2 sd 5 0 sd").unwrap();
    assert_eq!(s.show(), "<2> 3 0");
    // A program's own failure, in a word it applies, and in a loop.
    let ww = "ck: < 0 >. ; ck: < i64 >. no-good' fail ; \
              ww: < i64 >. ck 1 + ; ww: < i64 k trap >. drop drop 99 ;";
    let mut s = Session::new();
    s.eval(ww).unwrap();
    s.eval("( 0 3 0 )").unwrap();
    s.eval("[ ww ] map").unwrap();
    assert_eq!(s.show(), "<1> ( 1 99 1 )");
    // Nothing fails, nothing is caught; its effect is the word's own.
    let mut s = Session::new();
    s.eval(sd).unwrap();
    s.eval("sd' effect#").unwrap();
    assert_eq!(s.show(), "<2> 2 1");
}

#[test]
fn a_kind_in_the_type_chooses_the_handler() {
    // `no-file' trap` before `k trap`, as `i64 ary` before `a ary`; a trap no
    // handler takes goes on outward.
    let sd = "sd: < i64 i64 >. div ; sd: < i64 i64 arithmetic' trap >. drop drop drop -1 ;";
    let mut s = Session::new();
    s.eval(sd).unwrap();
    s.eval("5 0 sd").unwrap();
    assert_eq!(s.show(), "<1> -1");
    s.eval("sd: < i64 i64 k trap >. drop drop drop -2 ;")
        .unwrap();
    s.eval("drop 5 0 sd").unwrap();
    assert_eq!(s.show(), "<1> -1");
    let mut s = Session::new();
    s.eval("sd: < i64 i64 >. div ; sd: < i64 i64 io' trap >. drop drop drop -1 ;")
        .unwrap();
    let e = s.eval("5 0 sd").unwrap_err();
    assert_eq!(e.to_string(), "arithmetic overflow, or division by zero");
    // `kind` and `why`, its message or, with none, its kind's name.
    let mut s = Session::new();
    s.eval("gg: < i64 >. drop nope' fail < string >. ; gg: < i64 k trap >. why swap drop < string >. ;")
        .unwrap();
    s.eval("3").unwrap();
    s.eval("gg").unwrap();
    assert_eq!(s.show(), "<1> \"nope\"");
    let mut s = Session::new();
    s.eval(
        "size: < string >. io.file.read sym! < sym >. ; \
         size: < string k trap >. kind swap drop < sym >. ; \"no/such/file\" size",
    )
    .unwrap();
    assert_eq!(s.show(), "<1> io'");
}

#[test]
fn the_innermost_handler_catches() {
    // A handler that fails passes the trap on, as it is.
    let mut s = Session::new();
    s.eval(
        "in: < i64 >. ( \"inner\" deep' ) fail ; in: < i64 k trap >. fail ; \
         out: < i64 >. in drop \"no\" < string >. ; out: < i64 k trap >. why swap drop < string >. ; \
         1 out",
    )
    .unwrap();
    assert_eq!(s.show(), "<1> \"inner\"");
    // A recursion is handled at each application.
    let mut s = Session::new();
    s.eval(
        "dn: < 0 >. bottom' fail < i64 >. ; dn: < i64 >. 1 -+ dn 1 + < i64 >. ; \
         dn: < i64 k trap >. drop 100 + < i64 >. ; 3",
    )
    .unwrap();
    s.eval("dn").unwrap();
    assert_eq!(s.show(), "<1> 103");
}

#[test]
fn a_trap_lives_only_while_it_is_handled() {
    // A handler leaves what its word leaves, and never the trap.
    let e = error("sd: < i64 i64 >. div ; sd: < i64 i64 k trap >. swap drop swap drop ; 5 0 sd");
    assert_eq!(e.kind, Kind::Mismatch);
    assert!(e.to_string().contains("leaves its trap"), "{e}");
    let e =
        error("sd: < i64 i64 >. div ; sd: < i64 i64 k trap >. ( a ) swap drop swap drop ; 5 0 sd");
    assert!(e.to_string().contains("leaves its trap"), "{e}");
    // It takes the inputs and the trap.
    let e = error("sd: < i64 i64 >. div ; sd: < i64 k trap >. drop drop 0 ; 5 0 sd");
    assert!(
        e.to_string()
            .contains("takes its inputs and a trap, 3 values"),
        "{e}"
    );
    // `trap` takes its kind, or a letter for any.
    let e = error("ff: < i64 trap >. ;");
    assert!(e.to_string().contains("`trap` takes a kind"), "{e}");
    // A word that takes a trap is applied as any is, in a handler.
    let mut s = Session::new();
    s.eval(
        "lg: < k trap >. drop \"logged\" ; sd: < i64 i64 >. div drop \"ok\" < string >. ; \
         sd: < i64 i64 k trap >. lg swap drop swap drop < string >. ; 5 0 sd 4 2 sd",
    )
    .unwrap();
    assert_eq!(s.show(), "<2> \"logged\" \"ok\"");
}
