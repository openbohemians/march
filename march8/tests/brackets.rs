//! A bracket is code (Thomas, 2026-10-09): its words mean what they mean
//! anywhere, and what it leaves is the signature. Read as all code is
//! (Thomas, 2026-10-12): a type's name gives the type, a constructor builds
//! one, one letter is a type variable, a word ending in `?` a guard, as it
//! leaves a bool, and a capitalized name a symbol.
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
    // A type's name gives the type, and a constructor builds one from types
    // below it; applied, a type converts the value below.
    let mut s = Session::new();
    s.eval("2 i64. 2.5 f64. ( 1 2 ) < i64 ary > .").unwrap();
    assert_eq!(
        s.show_typed(),
        "<3> 2 < i64 > 2.5 < f64 > ( 1 2 ) < i64 ary >"
    );
    assert_eq!(
        show(&["nn: [ < a ary -- i64 > length ] def ( 1 2 3 ) nn"]),
        "<1> 3"
    );
    assert_eq!(
        show(&["look: [ < k v map k -- v > at ] def { \"a\" 1 } \"a\" look"]),
        "<1> 1"
    );
    // A bracket in a body still types the values on top.
    assert_eq!(show(&["2 < f64 > . 3 +"]), "<1> 5.0");
    // A tuple type is an array of types.
    assert_eq!(show(&["( 1 \"a\" ) < ( i64 string ) > . length"]), "<1> 2");
}

#[test]
fn one_letter_is_a_variable_and_a_question_a_guard() {
    // One letter is a variable, a type variable in a bracket, and names no
    // word.
    assert_eq!(
        show(&["first-of: [ < a a -- a > drop ] def 5 6 first-of"]),
        "<1> 5"
    );
    assert_eq!(session(&["a: [ 1 ] def"]).err(), Some(Kind::Syntax));
    // A word ending in `?` leaves one bool, so applied in a bracket it is a
    // guard; and any quotation leaving a bool is one.
    let even = "even: [ < i64 [ 2 mod zero? ] > drop 1 ] def even: [ < i64 > drop 0 ] def";
    assert_eq!(show(&[even, "4 even 3 even", "7", "even"]), "<3> 1 0 0");
    let sign = "sign: [ < i64 positive? > drop 1 ] def sign: [ < i64 > drop 0 ] def";
    assert_eq!(show(&[sign, "5 sign -5 sign"]), "<2> 1 0");
    // Its code runs while compiling: it can leave no value of run time.
    assert_eq!(
        session(&["7", "ff: [ < _ > ] def"]).err(),
        Some(Kind::Syntax)
    );
}

#[test]
fn brackets_inside_brackets() {
    // A guard may use a type, a bracket in a bracket: a string that
    // converts to a number. The bracket in the guard is one value, applied
    // to the input, so the guard looks at one input.
    let num = "number?: [ < nil > drop false ] def number?: [ < f64 > drop true ] def
        hh: [ < string [ < f64 > . number? ] > drop 1 ] def
        hh: [ < string > drop 0 ] def";
    assert_eq!(
        show(&[num, "\"2.5\" hh \"x\" hh", "\"7\"", "hh"]),
        "<3> 1 0 1"
    );
    // A guard's message shows its brackets as written.
    let e = Session::new()
        .eval("ff: [ < [ < f64 > . number? ] > ] def")
        .unwrap_err();
    assert!(e.to_string().contains("`[ < f64 > . number? ]`"), "{e}");
    // A one-slot bracket in a bracket is its type.
    assert_eq!(
        show(&["ff: [ < < i64 > -- < i64 > > 1 + ] def 1 ff"]),
        "<1> 2"
    );
    // Any other would be a quotation's type, `map`'s `< a -- b >`: not yet.
    for src in [
        "gg: [ < a ary < a -- b > -- b ary > map ] def",
        "gg: [ < i64 -- < i64 i64 > > ] def",
    ] {
        let e = Session::new().eval(src).unwrap_err();
        assert_eq!(e.kind, Kind::Limit, "{src}");
        assert!(e.to_string().contains("would type a quotation"), "{e}");
    }
}
