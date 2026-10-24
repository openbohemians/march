//! A bracket is code (Thomas, 2026-10-09): its words mean what they mean
//! anywhere, and what it leaves is the signature. Read as all code is
//! (Thomas, 2026-10-12): a type's name gives the type, a constructor builds
//! one, one letter is a type variable, a word tested, `positive?`, a guard,
//! and a capitalized name a symbol.
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
fn a_symbol_chooses_a_clause() {
    // A family's clauses can be its styles: `< string Snake >`, a string and
    // the symbol `Snake`, which the word `Snake` leaves (SYMBOLIC-FORTH.md
    // 27).
    let style = "Snake: Snake' ; Kebab: Kebab' ; \
                 style: < string Snake >. drop case.snake < string >. ; \
                 style: < string Kebab >. drop case.kebab < string >. ;";
    assert_eq!(
        show(&[
            style,
            "\"Hello World\" Snake style \"helloWorld\" Kebab style"
        ]),
        "<2> \"hello_world\" \"hello-world\""
    );
    // Chosen while compiling: a known string's style is a constant.
    let mut s = Session::new();
    s.eval(style).unwrap();
    assert_eq!(
        s.compile("\"Hello World\" Snake style").unwrap().0,
        s.compile("\"hello_world\"").unwrap().0
    );
    // A style with no clause is no word, said where `style` is applied.
    let e = s.eval("\"x\" Title' style").unwrap_err();
    assert_eq!(e.kind, Kind::NoWord);
    assert!(e.to_string().starts_with("1:12: no word `style`"), "{e}");
    // A type defined later by the symbol's name changes nothing: the clause
    // has the symbol, and so does the call.
    assert_eq!(
        show(&[style, "snake: < i64 > ;", "\"hello world\" Snake style"]),
        "<1> \"hello_world\""
    );
    // Any family can choose by symbols, of one arity.
    let w = "Up: Up' ; Down: Down' ; step: < i64 Up >. drop 1 + ; step: < i64 Down >. drop 1 -+ ;";
    assert_eq!(show(&[w, "5 Up step 5 Down step"]), "<2> 6 4");
}

#[test]
fn a_type_applied_converts() {
    // A type's name gives the type, and a constructor builds one from types
    // below it; applied, a type converts the value below.
    let mut s = Session::new();
    s.eval("2 i64! 2.5 f64! ( 1 2 ) < i64 ary > !").unwrap();
    assert_eq!(
        s.show_typed(),
        "<3> 2 < i64 > 2.5 < f64 > ( 1 2 ) < i64 ary >"
    );
    assert_eq!(
        show(&["nn: < a ary >. length < i64 >. ; ( 1 2 3 ) nn"]),
        "<1> 3"
    );
    assert_eq!(
        show(&["look: < k v map k >. at < v >. ; { \"a\" 1 } \"a\" look"]),
        "<1> 1"
    );
    // A bracket in a body still types the values on top.
    assert_eq!(show(&["2 < f64 > ! 3 +"]), "<1> 5.0");
    // A tuple type is an array of types.
    assert_eq!(show(&["( 1 \"a\" ) < ( i64 string ) > ! length"]), "<1> 2");
}

#[test]
fn one_letter_is_a_variable_and_a_question_a_guard() {
    // One letter is a variable, a type variable in a bracket, and names no
    // word.
    assert_eq!(
        show(&["first-of: < a a >. drop < a >. ; 5 6 first-of"]),
        "<1> 5"
    );
    assert_eq!(session(&["a: 1 ;"]).err(), Some(Kind::Syntax));
    // A word tested, `positive?`, is a guard in a bracket, and so is a
    // quotation tested, leaving a bool; each role has its mark
    // (doc/design/SYMBOLIC-FORTH.md 2), so one left bare is refused.
    let even = "even: < i64 [ 2 mod zero ] ? >. drop 1 ; even: < i64 >. drop 0 ;";
    assert_eq!(show(&[even, "4 even 3 even", "7", "even"]), "<3> 1 0 0");
    let e = Session::new()
        .eval("even: < i64 [ 2 mod zero ] >. drop 1 ;")
        .unwrap_err();
    assert_eq!(e.kind, Kind::Syntax);
    assert!(
        e.to_string()
            .contains("`[ 2 mod zero ]` is left bare, and a guard is a test: `[ 2 mod zero ] ?`"),
        "{e}"
    );
    let sign = "sign: < i64 positive? >. drop 1 ; sign: < i64 >. drop 0 ;";
    assert_eq!(show(&[sign, "5 sign -5 sign"]), "<2> 1 0");
    // Its code runs while compiling: it can leave no value of run time.
    assert_eq!(session(&["7", "ff: < _ >. ;"]).err(), Some(Kind::Syntax));
}

#[test]
fn brackets_inside_brackets() {
    // A guard may use a type, a bracket in a bracket: a string that
    // converts to a number. The bracket in the guard is one value, applied
    // to the input, so the guard looks at one input.
    let num = "number: < nil >. drop false ; number: < f64 >. drop true ;
        hh: < string [ < f64 > ! number ] ? >. drop 1 ;
        hh: < string >. drop 0 ;";
    assert_eq!(
        show(&[num, "\"2.5\" hh \"x\" hh", "\"7\"", "hh"]),
        "<3> 1 0 1"
    );
    // A guard's message shows its brackets as written.
    let e = Session::new()
        .eval("ff: < [ < f64 > ! number ] ? >. ;")
        .unwrap_err();
    assert!(e.to_string().contains("`[ < f64 > ! number ] ?`"), "{e}");
    // A one-slot bracket in a bracket is its type.
    assert_eq!(show(&["ff: < < i64 > >. 1 + < < i64 > >. ; 1 ff"]), "<1> 2");
    // Any other would be a quotation's type, as `map`'s code is: not yet.
    for src in [
        "gg: < a ary < a b > >. map < b ary >. ;",
        "gg: < i64 >. < < i64 i64 > >. ;",
    ] {
        let e = Session::new().eval(src).unwrap_err();
        assert_eq!(e.kind, Kind::Limit, "{src}");
        assert!(e.to_string().contains("would type a quotation"), "{e}");
    }
}

#[test]
fn a_signature_is_a_check() {
    // A type bracket applied checks the values on top where it is applied
    // (doc/design/SYMBOLIC-FORTH.md 6.32): a body's first check is the
    // clause's signature, its inputs, and its last what it leaves, worked
    // out where it is left off; `--` is a comment, in a bracket too.
    assert_eq!(show(&["sq: < i64 >. dup * < i64 >. ; 3 sq"]), "<1> 9");
    assert_eq!(show(&["sq: < i64 >. dup * ; 3 sq"]), "<1> 9");
    assert_eq!(
        show(&["sq: < i64 -- its side\n >. dup * < i64 >. ; 3 sq"]),
        "<1> 9"
    );
    assert_eq!(show(&["one: < >. 1 < i64 >. ; one"]), "<1> 1");
    // An identity clause is a signature and no more.
    assert_eq!(show(&["same: < i64 >. < i64 >. ; 3 same"]), "<1> 3");
    // Anywhere else a check is one, settling a literal as a signature does.
    assert_eq!(show(&["3 < i64 >. 1 +"]), "<1> 4");
    assert_eq!(show(&["three: 3 < f64 >. ; three"]), "<1> 3.0");
    let e = Session::new().eval("3.5 < i64 >.").unwrap_err();
    assert_eq!(e.kind, Kind::Mismatch);
    assert!(e.to_string().contains("expected i64, found dec#"), "{e}");
    // A bracket not applied is a value, a type, which `;` names.
    assert_eq!(show(&["whole: < i64 int# or > ; 2 whole !"]), "<1> 2");
    // Until 2026-10-24 a signature's two brackets touched, `< i64 >< i64 >`,
    // and came before its body.
    for old in ["sq: < i64 >< i64 > [ dup * ] ;", "[ >< ]"] {
        let e = Session::new().eval(old).unwrap_err();
        assert_eq!(e.kind, Kind::Syntax);
        assert!(e.to_string().contains("a signature is a check"), "{e}");
    }
    let e = Session::new().eval("sq' < i64 > [ dup * ] ;").unwrap_err();
    assert!(e.to_string().contains("first in its body"), "{e}");
    // The divider it was is a comment now, which says so.
    let e = Session::new()
        .eval("sq: < i64 -- i64 > dup * ;")
        .unwrap_err();
    assert_eq!(e.kind, Kind::Syntax);
    assert!(
        e.to_string()
            .contains("a signature's outputs are a check of their own"),
        "{e}"
    );
}
