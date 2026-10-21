//! A word's clauses as data, and its effect (doc/design/SYMBOLIC-FORTH.md
//! 6), while compiling: `w ins clauses#` gives the clauses dispatch would
//! choose for inputs of the types `ins` gives, each a quotation of what `;`
//! takes after the name, its inputs, its outputs if it gives them, and its
//! body; `w effect#` how many values w takes and leaves. So March can read a
//! word, as it reads a quotation.
use march8::{Kind, Session};

fn show(src: &str) -> String {
    let mut s = Session::new();
    if let Err(e) = s.eval(src) {
        panic!("{src}: {e}");
    }
    s.show()
}

fn error(src: &str) -> (Kind, String) {
    let e = Session::new().eval(src).expect_err(src);
    (e.kind, e.msg.clone())
}

#[test]
fn a_clause_is_its_signature_and_its_body() {
    // `max: < a a gt? >< a > [ drop ] ;`: what `;` takes after the name,
    // each bracket as its code, `a a gt ?` and `a`, then the body, `drop .`.
    let first = "max' < i64 i64 > clauses# first";
    assert_eq!(show(&format!("{first} length")), "<1> 3");
    assert_eq!(
        show(&format!(
            "{first} first dup length swap dup first swap dup 3 at swap last"
        )),
        "<4> 4 a' gt' ?'"
    );
    assert_eq!(
        show(&format!("{first} 2 at dup length swap first")),
        "<2> 1 a'"
    );
    assert_eq!(
        show(&format!("{first} last dup length swap first")),
        "<2> 2 drop'"
    );
    // A type given alone is written by its name.
    assert_eq!(
        show(
            "inc: < i64 >< i64 > [ 1 + ] ; inc' i64 clauses# first dup length swap first dup length swap first"
        ),
        "<3> 3 2 i64'"
    );
    // A clause with no signature is its body alone, and a guard's
    // quotation is one element.
    assert_eq!(
        show("add: [ + ] ; add' < i64 i64 > clauses# first dup length swap first first"),
        "<2> 1 +'"
    );
    assert_eq!(
        show(
            "even: < i64 [ 2 mod zero ] ? > [ drop 1 ] ; even' i64 clauses# first first 3 at length"
        ),
        "<1> 7"
    );
}

#[test]
fn they_are_the_clauses_dispatch_would_choose() {
    // Those with guards first, as defined, then the best without.
    assert_eq!(show("max' [ i64 i64 ] clauses# length"), "<1> 2");
    assert_eq!(show("max' [ i64 i64 ] clauses# last first length"), "<1> 2");
    let fib = "fib: < 0 > [ 0 ] ; fib: < 1 > [ 1 ] ; fib: < i64 > [ 2 ] ; fib: < f64 > [ 3 ] ;";
    assert_eq!(show(&format!("{fib} fib' i64 clauses# length")), "<1> 3");
    // A value pattern, `0`, matches a value of any type its literal becomes.
    assert_eq!(
        show(&format!(
            "{fib} fib' f64 clauses# dup length swap last last ."
        )),
        "<2> 3 3"
    );
    assert_eq!(show(&format!("{fib} fib' string clauses# length")), "<1> 0");
    // None for types no clause takes, nor fewer values than they take; and
    // a primitive has none, its code being the machine's.
    assert_eq!(show("max' i64 clauses# length"), "<1> 0");
    assert_eq!(show("gt' < string string > clauses# length"), "<1> 0");
    assert_eq!(show("dup' i64 clauses# length"), "<1> 0");
    // A body taken out runs as written, its names paths where it was.
    assert_eq!(show("3 4 max' [ i64 i64 ] clauses# last last ."), "<1> 4");
    assert_eq!(
        show("m.ff: < i64 > [ gg ] ; m.gg: [ 7 ] ; gg: [ 0 ] ; m.ff' i64 clauses# first last ."),
        "<1> 7"
    );
}

#[test]
fn an_effect_is_two_counts() {
    assert_eq!(show("swap' effect# over' effect#"), "<4> 2 2 2 3");
    assert_eq!(show("[ swap + ] effect# max' effect#"), "<4> 2 1 2 1");
    // A spelling a rule rewrites has the rewrite's.
    assert_eq!(show("dup_2' effect# +^3' effect#"), "<4> 2 4 3 1");
    let (kind, msg) = error("dip' effect#");
    assert_eq!(kind, Kind::Mismatch);
    assert!(
        msg.contains("no stack effect known while compiling"),
        "{msg}"
    );
}

#[test]
fn what_they_take_is_checked() {
    let (kind, msg) = error("foo' i64 clauses#");
    assert_eq!(kind, Kind::NoWord);
    assert_eq!(msg, "no word `foo`");
    let (_, msg) = error("max' [ i64 positive? ] clauses#");
    assert!(msg.contains("its inputs' types alone"), "{msg}");
    let (_, msg) = error("max' 3 clauses#");
    assert!(msg.contains("and 3 (int#) is neither"), "{msg}");
    let (_, msg) = error("3 effect#");
    assert!(msg.contains("a word's symbol or a quotation"), "{msg}");
    assert_eq!(error("foo' effect#").1, "no word `foo`");
    // `--` is a comment everywhere, so a symbol of that name is a symbol.
    assert_eq!(show("[ '--' ] length"), "<1> 1");
}
