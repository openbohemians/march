//! Symbols as values at run time (doc/design/SYMBOLIC-FORTH.md 6.4): a
//! symbol is its name, an interned string, so symbols compare by identity,
//! key maps, and convert to and from strings, and a family can apply itself
//! to them. A capital marks nothing (27), so these programs read their
//! capitalized names as symbols by a pattern word, as a program of symbols
//! does.
use march8::{Kind, Session};

const CAPS: &str = "/([A-Z][A-Za-z0-9]*)/: < sym -- sym > [ ] ;";

fn show(src: &str) -> String {
    let mut s = Session::new();
    if let Err(e) = s.eval(&format!("{CAPS} {src}")) {
        panic!("{src}: {e}");
    }
    s.show_typed()
}

fn fails(src: &str) -> march8::Error {
    let mut s = Session::new();
    s.eval(&format!("{CAPS} {src}")).expect_err(src)
}

const DOOR: &str = "
door: < Closed Open > [ drop drop Opened ] ;
door: < Opened Close > [ drop drop Closed ] ;
";

#[test]
fn a_symbol_is_left_on_the_stack() {
    assert_eq!(show("Locked"), "<1> Locked' < sym >");
    // Shown as it reads back: a name with a prime, and text that is no
    // name quoted, as a string made a symbol is.
    assert_eq!(
        show("x' +' fib: \"two words\" sym!"),
        "<4> x' < sym > +' < sym > fib' < sym > 'two words' < sym >"
    );
    assert_eq!(show("( Open Close )"), "<1> ( Open' Close' ) < 2 sym vec >");
}

#[test]
fn a_symbol_converts_to_and_from_a_string() {
    assert_eq!(
        show("\"Open\" sym! Open string!"),
        "<2> Open' < sym > \"Open\" < string >"
    );
    // At run time too: `words` is not folded.
    assert_eq!(
        show("\"Open Close\" words [ sym! ] map"),
        "<1> ( Open' Close' ) < sym ary >"
    );
    // A symbol written into a string is its text, as a symbol taken as a
    // string is (doc/design/SYMBOLIC-FORTH.md 15, 28).
    assert_eq!(
        show("\"\\[ Locked ] \\[ x' ] \\[ \"a b\" sym! ]\""),
        r#"<1> "Locked x a b" < string >"#
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
        "<2> { Close' 2 Open' 1 } < sym i64 map > 1 < i64 nil or >"
    );
}

#[test]
fn a_family_chooses_by_symbols_at_run_time() {
    let src = format!("{DOOR} \"Open Close\" words [ sym! ] map Closed [ door ] fold");
    assert_eq!(show(&src), "<1> Closed' < sym >");
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
    assert_eq!(show(src), "<1> Ajar' < sym >");
}

#[test]
fn a_capital_marks_nothing() {
    // A capitalized name is a word like any (doc/design/SYMBOLIC-FORTH.md
    // 27): the core's `Snake` leaves its symbol, and a name that is no word
    // says which word it is spelled like.
    let plain = |src: &str| {
        let mut s = Session::new();
        s.eval(src).unwrap_or_else(|e| panic!("{src}: {e}"));
        s.show()
    };
    assert_eq!(
        plain("Snake \"Hello World\" Snake case"),
        "<2> Snake' \"hello_world\""
    );
    let e = Session::new().eval("3 Dup").unwrap_err();
    assert_eq!(e.kind, Kind::NoWord);
    assert!(e.msg.contains("no word `Dup`; the word is `dup`"), "{e}");
    // A module defines the symbols it needs, as words that leave them, and
    // they resolve as words do, by namespace and import.
    assert_eq!(plain("Human: Human' ; Human"), "<1> Human'");
    assert_eq!(
        plain("bio.Human: Human' ; import#: <| bio.* |> ; Human"),
        "<1> Human'"
    );
    // In a signature the word is applied, so it leaves the symbol to match.
    let door = "Shut: Shut' ; Open: Open' ; door: < Shut Open > [ drop drop 1 ] ;";
    assert_eq!(plain(&format!("{door} Shut Open door")), "<1> 1");
    // The old rule is a pattern word, for a program of symbols.
    assert_eq!(
        plain(&format!("{CAPS} Socrates Is Human")),
        "<3> Socrates' Is' Human'"
    );
}

#[test]
fn a_symbol_taken_as_text_is_a_string() {
    // A symbol is text until a type structures it, so a word that takes a
    // string takes it (doc/design/SYMBOLIC-FORTH.md 28), known while
    // compiling or only at run time.
    assert_eq!(
        show("hello' length 'two words' upper"),
        "<2> 5 < i64 > \"TWO WORDS\" < string >"
    );
    assert_eq!(
        show("\"ab c\" words [ sym! ] map [ length ] map"),
        "<1> ( 2 1 ) < i64 ary >"
    );
    // A symbol and a string compare as strings.
    assert_eq!(show("x' \"x\" eq"), "<1> true < bool >");
    // The exact clause comes first.
    let sz = "size: < sym > [ drop 1 ] ; size: < string > [ drop 2 ] ;";
    assert_eq!(
        show(&format!("{sz} x' size \"x\" size")),
        "<2> 1 < i64 > 2 < i64 >"
    );
    // One way: a string is no symbol unless made one.
    let e = fails("sy: < sym > [ ] ; \"x\" sy");
    assert_eq!(e.kind, Kind::NoWord);
    // `'…'` quotes a symbol, a quote in it doubled (6.12), and is shown so
    // when it is no name.
    assert_eq!(
        show("'it''s' 'two words' ( 'a' )"),
        "<3> 'it''s' < sym > 'two words' < sym > ( a' ) < 1 sym vec >"
    );
}
