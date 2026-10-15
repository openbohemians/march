//! Symbols as values at run time (doc/design/SYMBOLIC-FORTH.md 6.4): a
//! symbol is its name, an interned string, so symbols compare by identity,
//! key maps, and convert to and from strings, and a family can apply itself
//! to them.
use march8::{Kind, Session};

fn show(src: &str) -> String {
    let mut s = Session::new();
    if let Err(e) = s.eval(src) {
        panic!("{src}: {e}");
    }
    s.show_typed()
}

fn fails(src: &str) -> march8::Error {
    let mut s = Session::new();
    s.eval(src).expect_err(src)
}

const DOOR: &str = "
door: < Closed Open > [ drop drop Opened ] ;
door: < Opened Close > [ drop drop Closed ] ;
";

#[test]
fn a_symbol_is_left_on_the_stack() {
    assert_eq!(show("Locked"), "<1> Locked < sym >");
    // Shown as it reads back: a lowercase name with a prime, and text that
    // is no name as a string made a symbol.
    assert_eq!(
        show("x' +' fib: \"two words\" sym!"),
        "<4> x' < sym > +' < sym > fib' < sym > \"two words\" sym! < sym >"
    );
    assert_eq!(show("( Open Close )"), "<1> ( Open Close ) < 2 sym vec >");
}

#[test]
fn a_symbol_converts_to_and_from_a_string() {
    assert_eq!(
        show("\"Open\" sym! Open string!"),
        "<2> Open < sym > \"Open\" < string >"
    );
    // At run time too: `words` is not folded.
    assert_eq!(
        show("\"Open Close\" words [ sym! ] map"),
        "<1> ( Open Close ) < sym ary >"
    );
    // `show` writes a symbol as it reads back, so a string's holes do.
    assert_eq!(
        show("\"\\[ Locked ] \\[ x' ] \\[ \"a b\" sym! ]\""),
        r#"<1> "Locked x' \"a b\" sym!" < string >"#
    );
}

#[test]
fn symbols_compare_at_run_time() {
    assert_eq!(
        show("( Open Close ) [ Open eq? ] map"),
        "<1> ( true false ) < 2 bool vec >"
    );
    assert_eq!(
        show("sym i64 map empty Open 1 put Close 2 put dup Open get"),
        "<2> { Close 2 Open 1 } < sym i64 map > 1 < i64 nil or >"
    );
}

#[test]
fn a_family_chooses_by_symbols_at_run_time() {
    let src = format!("{DOOR} \"Open Close\" words [ sym! ] map Closed [ door ] fold");
    assert_eq!(show(&src), "<1> Closed < sym >");
    // A transition the table lacks stops the program, and says which.
    let e = fails(&format!("{DOOR} ( Open Open ) Closed [ door ] fold"));
    assert!(matches!(e.kind, Kind::Run(_)), "{e}");
    assert!(
        e.msg
            .contains("no word `door` for sym sym: no clause's guard holds"),
        "{e}"
    );
}

#[test]
fn a_family_applies_itself_to_symbols() {
    let src = "
is: < Socrates Human > [ drop drop true ] ;
is: < x Mortal > [ drop Human is ] ;
is: < x y > [ drop drop false ] ;
Socrates Mortal is Zeus Mortal is";
    assert_eq!(show(src), "<2> true < bool > false < bool >");
}

#[test]
fn a_symbol_applies_while_compiling() {
    // A known symbol names a word, which `.` applies, a string made one too.
    assert_eq!(show("3 \"dup\" sym! ."), "<2> 3 < i64 > 3 < i64 >");
    // Which word a symbol known only at run time names is not known.
    let e = fails("3 \"dup\" words first sym! .");
    assert!(
        e.msg
            .contains("a symbol known only at run time cannot be applied"),
        "{e}"
    );
}

#[test]
fn a_clause_on_symbols_written_again_replaces_itself() {
    let src = "
door: < Closed Open > [ drop drop Opened ] ;
  door: < Closed Open > [ drop drop Ajar ] ;
Closed Open door";
    assert_eq!(show(src), "<1> Ajar < sym >");
}
