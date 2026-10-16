//! Recognizers (doc/design/SYMBOLIC-FORTH.md 25): a namespace's clauses of
//! `recognize#`, each a pattern that matches a whole word and a template,
//! the code the word reads as, its holes filled by what the pattern
//! captures. They read a word where their namespace is imported, before the
//! core does.
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

const OPS: &str = "ops.recognize#: < 'times([0-9]+)' > [ _ int# ! * ] ;";

#[test]
fn a_recognizer_reads_a_word_where_imported() {
    assert_eq!(
        show(&format!("{OPS} import#: <| ops.* |> ; 5 times3")),
        "<1> 15"
    );
    // A word the core reads as a symbol, as it does a number's kin.
    let units = "units.recognize#: < '([0-9]+)kg' > [ _ int# ! 1000 * ] ;";
    assert_eq!(
        show(&format!("{units} import#: <| units.* |> ; 12kg 2kg +")),
        "<1> 14000"
    );
    // In a namespace's words, by its own import.
    assert_eq!(
        show(&format!(
            "{OPS} app.import#: <| ops.* |> ; app.main: [ 5 times3 ] ; app.main"
        )),
        "<1> 15"
    );
    // In a quotation, which is code like any.
    assert_eq!(
        show(&format!(
            "{OPS} import#: <| ops.* |> ; ( 1 2 3 ) [ times3 ] map 5 [ times2 ]^3 ."
        )),
        "<2> ( 3 6 9 ) 40"
    );
}

#[test]
fn an_imported_recognizer_comes_before_the_core() {
    // `add_2` would be `add` two wide.
    let ops = "ops.recognize#: < 'add_([0-9]+)' > [ _ int# ! + ] ;";
    assert_eq!(
        show(&format!("{ops} import#: <| ops.* |> ; 5 add_2 add₃")),
        "<1> 10"
    );
    let (kind, _) = error("5 add_2");
    assert_eq!(kind, Kind::NoWord);
}

#[test]
fn a_template_names_words_of_its_namespace() {
    let ops = "ops.sq: [ dup * ] ; ops.recognize#: < 'sq([0-9]+)' > [ _ int# ! sq ] ;";
    assert_eq!(show(&format!("{ops} import#: <| ops.* |> ; sq7")), "<1> 49");
}

#[test]
fn a_recognized_word_takes_marks_as_a_name_does() {
    let src = format!("{OPS} import#: <| ops.* |> ; 5 times3' .");
    assert_eq!(show(&src), "<1> 15");
}

#[test]
fn recognizers_are_fixed_before_the_text() {
    // Wherever written, they are defined first, as an import is.
    assert_eq!(
        show(&format!("import#: <| ops.* |> ; 5 times3 {OPS}")),
        "<1> 15"
    );
    // Never in their own namespace.
    let (kind, msg) = error(&format!("{OPS} ops.ff: [ 5 times3 ] ; ops.ff"));
    assert_eq!(kind, Kind::NoWord);
    assert!(msg.contains("no word `times3`"), "{msg}");
}

#[test]
fn an_export_holds_for_recognizers() {
    let ops = format!("{OPS} ops.ee: [ 1 ] ; ops.export#: <| ee |> ;");
    let (kind, _) = error(&format!("{ops} import#: <| ops.* |> ; 5 times3"));
    assert_eq!(kind, Kind::NoWord);
    // Named, they are imported.
    assert_eq!(
        show(&format!("{ops} import#: <| ops.recognize# |> ; 5 times3")),
        "<1> 15"
    );
}

#[test]
fn a_word_two_recognizers_match_is_an_error() {
    let (kind, msg) = error(&format!(
        "{OPS} ops.recognize#: < 't(.*)' > [ _ drop ] ; import#: <| ops.* |> ; 5 times3"
    ));
    assert_eq!(kind, Kind::Syntax);
    assert!(
        msg.contains("`times3` matches both `ops`'s `times([0-9]+)`, and `ops`'s `t(.*)`"),
        "{msg}"
    );
    let (_, msg) = error(&format!(
        "{OPS} more.recognize#: < '(.+)3' > [ _ drop ] ; import#: <| ops.* more.* |> ; 5 times3"
    ));
    assert!(
        msg.contains("`times3` matches both `ops`'s `times([0-9]+)`, and `more`'s `(.+)3`"),
        "{msg}"
    );
}

#[test]
fn a_recognizer_is_checked_when_defined() {
    let (kind, msg) = error("ops.recognize#: < 'times([0-9]+)' > [ _ _ ] ;");
    assert_eq!(kind, Kind::Syntax);
    assert!(
        msg.contains("captures 1, and its template has 2 holes"),
        "{msg}"
    );
    let (_, msg) = error("ops.recognize#: < 'times([0-9]+' > [ _ ] ;");
    assert!(msg.contains("is no pattern: unclosed group"), "{msg}");
    let (_, msg) = error("ops.recognize#: [ _ ] ;");
    assert!(msg.contains("takes a pattern, then a template"), "{msg}");
}
