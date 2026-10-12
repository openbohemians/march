//! A bracket is code (Thomas, 2026-10-09): its words mean what they mean
//! anywhere, and what it leaves is the signature. A name left is read by its
//! spelling: one letter is a type variable, `?` ends a guard, a capital
//! starts a symbol, and any other name is a type's (Thomas, 2026-10-12).
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
    // `case`'s clauses are its styles: `< string Snake >`, a string and the
    // symbol `Snake`, spelled the same outside a bracket.
    assert_eq!(
        show(&[
            "\"Hello World\" Snake case \"helloWorld\" Kebab case \"hello_world\" Camel case \
             \"HTTPServer error\" Pascal case \"Hi\" Upper case"
        ]),
        "<5> \"hello_world\" \"hello-world\" \"helloWorld\" \"HttpServerError\" \"HI\""
    );
    // Chosen while compiling: a known string's case is a constant.
    let mut s = Session::new();
    assert_eq!(
        s.compile("\"Hello World\" Snake case").unwrap().0,
        s.compile("\"hello_world\"").unwrap().0
    );
    // A style with no clause is no word, said where `case` is applied.
    let e = s.eval("\"x\" Title case").unwrap_err();
    assert_eq!(e.kind, Kind::NoWord);
    assert!(e.to_string().starts_with("1:11: no word `case`"), "{e}");
    // A type defined later by the symbol's name changes nothing: the clause
    // has the symbol, and so does the call.
    assert_eq!(
        show(&["camel: < i64 > def", "\"hello world\" Camel case"]),
        "<1> \"helloWorld\""
    );
    // Any family can choose by symbols, of one arity.
    let w = "step: [ < i64 Up > drop 1 + ] def step: [ < i64 Down > drop 1 -+ ] def";
    assert_eq!(show(&[w, "5 Up step 5 Down step"]), "<2> 6 4");
}

#[test]
fn a_type_applied_converts() {
    // A type's name is a symbol; applied, it converts the value below, and
    // a constructor builds a type from names below it, which applied
    // converts too.
    let mut s = Session::new();
    s.eval("2 i64 2.5 f64 ( 1 2 ) < i64 ary. > .").unwrap();
    assert_eq!(
        s.show_typed(),
        "<3> 2 < i64 > 2.5 < f64 > ( 1 2 ) < i64 ary. >"
    );
    assert_eq!(
        show(&["n: [ < a ary. -- i64 > length ] def ( 1 2 3 ) n"]),
        "<1> 3"
    );
    assert_eq!(
        show(&["look: [ < k v map. k -- v > at ] def { \"a\" 1 } \"a\" look"]),
        "<1> 1"
    );
    // A bracket in a body still types the values on top.
    assert_eq!(show(&["2 < f64 > . 3 +"]), "<1> 5.0");
    // A tuple type is an array of types.
    assert_eq!(show(&["( 1 \"a\" ) < ( i64 string ) > . length"]), "<1> 2");
}

#[test]
fn one_letter_is_a_variable_and_a_question_a_guard() {
    // A word `a` changes nothing: one letter is a type variable.
    assert_eq!(
        show(&["a: [ 1 ] def first-of: [ < a a -- a > drop ] def 5 6 first-of"]),
        "<1> 5"
    );
    // A guard by its `?`, or any quotation leaving a bool.
    let even = "even: [ < i64 [ 2 mod zero? ] > drop 1 ] def even: [ < i64 > drop 0 ] def";
    assert_eq!(show(&[even, "4 even 3 even", "7", "even"]), "<3> 1 0 0");
    let sign = "sign: [ < i64 positive? > drop 1 ] def sign: [ < i64 > drop 0 ] def";
    assert_eq!(show(&[sign, "5 sign -5 sign"]), "<2> 1 0");
    // Its code runs while compiling: it can leave no value of run time.
    assert_eq!(
        session(&["7", "f: [ < _. > ] def"]).err(),
        Some(Kind::Syntax)
    );
}
