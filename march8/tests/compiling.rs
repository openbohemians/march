//! Running while compiling, `[ … ] !` (doc/design/SYMBOLIC-FORTH.md 6.32):
//! with a quotation on top, `!` runs its code once, while compiling, and
//! leaves its values known, as FORTH's `[ … ] LITERAL` does. `!` so faces
//! `.`, which runs code where it is applied, each time.
use march8::{Kind, Session};

fn typed(src: &str) -> String {
    let mut s = Session::new();
    if let Err(e) = s.eval(src) {
        panic!("{src}: {e}");
    }
    s.show_typed()
}

fn fails(src: &str) -> march8::Error {
    Session::new().eval(src).expect_err(src)
}

#[test]
fn a_quotation_runs_while_compiling() {
    // What it leaves is known, so what follows folds on it: the code is the
    // constant.
    let mut s = Session::new();
    assert_eq!(
        s.compile("[ 1 2 + ] ! 4 +").unwrap().0,
        s.compile("7").unwrap().0
    );
    // Values its code makes as it runs are read back by their types:
    // numbers, strings, symbols, and arrays and tuples of them.
    assert_eq!(
        typed("[ ( 1 2 ) ( 3 4 ) [ + ] zip ] ! [ \"a b\" words [ sym! ] map ] !"),
        "<2> ( 4 6 ) < 2 i64 vec > ( a' b' ) < sym ary >"
    );
    assert_eq!(
        typed("[ ( 1 \"a\" ) ] ! [ \"b c\" words ] ! length"),
        "<2> ( 1 \"a\" ) < ( i64 string ) > 2 < i64 >"
    );
    // One `[ … ] !` inside another runs first.
    assert_eq!(typed("[ [ 2 3 * ] ! 1 + ] !"), "<1> 7 < i64 >");
}

#[test]
fn it_takes_nothing_from_below() {
    // What is below is known only as the program runs.
    let e = fails("2 [ 1 + ] !");
    assert_eq!(e.kind, Kind::Mismatch);
    assert!(
        e.msg
            .contains("`[ … ] !` takes nothing from below, known only as the program runs"),
        "{e}"
    );
}

#[test]
fn code_made_while_compiling() {
    // A value known as its code is compiled stays as it is: a quotation,
    // applied, is code made while compiling, `[ … ] ! .`, and a type is one
    // to structure by.
    assert_eq!(typed("3 [ [ dup * ] ] ! ."), "<1> 9 < i64 >");
    assert_eq!(typed("5 [ f64 ] ! !"), "<1> 5.0 < f64 >");
}

#[test]
fn its_effects_run_once() {
    // Each quotation runs once, kept by its code's identity, wherever it is
    // applied, and what it writes is written before the program runs.
    let mut s = Session::new();
    s.eval("ff: [ \"now \" print 7 ] ! ; \"then\" print ff ff [ \"now \" print 7 ] !")
        .unwrap();
    assert_eq!(s.show(), "<3> 7 7 7");
    assert_eq!(String::from_utf8(s.take_output()).unwrap(), "now then");
}

#[test]
fn what_it_cannot_do() {
    // A failure as its code runs is an error where it is applied.
    let e = fails("[ \"a b c\" words length 0 div ] !");
    assert_eq!(e.kind, Kind::Arithmetic);
    assert!(e.msg.ends_with("in `[ … ] !`, run while compiling"), "{e}");
    assert_eq!(e.pos.unwrap().col, 32);
    // A map made as it runs is not read back yet.
    let e = fails("[ string i64 map empty ] !");
    assert_eq!(e.kind, Kind::Limit);
    assert!(e.msg.contains("leaves string i64 map"), "{e}");
}
