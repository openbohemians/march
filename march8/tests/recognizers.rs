//! Recognizers (doc/design/SYMBOLIC-FORTH.md 25): words whose names are
//! patterns, `/log_([0-9]+)/`, each matching a whole word. What a pattern
//! captures goes on the stack, each a symbol, after the word's inputs, and
//! its clauses are a family. A pattern reads a word in its own namespace and
//! where that namespace is imported, before the core does.
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

const OPS: &str = "ops./times([0-9]+)/: < i64 sym -- i64 > [ int# ! * ] ;";

#[test]
fn a_pattern_word_reads_a_word_where_imported() {
    assert_eq!(
        show(&format!("{OPS} import#: <| ops.* |> ; 5 times3")),
        "<1> 15"
    );
    // A word the core reads as a symbol, as it does a number's kin.
    let units = "units./([0-9]+)kg/: < sym > [ int# ! 1000 * ] ;";
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
    // In a quotation, which is code like any, its effect without the
    // captures.
    assert_eq!(
        show(&format!(
            "{OPS} import#: <| ops.* |> ; ( 1 2 3 ) [ times3 ] map 5 [ times2 ]^3 ."
        )),
        "<2> ( 3 6 9 ) 40"
    );
}

#[test]
fn a_pattern_word_reads_its_own_namespace() {
    assert_eq!(
        show(&format!("{OPS} ops.ff: [ 5 times3 ] ; ops.ff")),
        "<1> 15"
    );
    let root = "/times([0-9]+)/: < i64 sym -- i64 > [ i64 ! * ] ;";
    assert_eq!(show(&format!("{root} 5 times3")), "<1> 15");
}

#[test]
fn captures_go_on_the_stack_in_order() {
    let src = "/([0-9]+)x([0-9]+)/: < sym sym > [ swap int# ! 10 * swap int# ! + ] ; 7x2";
    assert_eq!(show(src), "<1> 72");
}

#[test]
fn clauses_of_one_pattern_are_a_family() {
    let src = "/times([0-9]+)/: < i64 sym -- i64 > [ int# ! * ] ; \
               /times([0-9]+)/: < f64 sym -- f64 > [ f64 ! * ] ; 5 times3 2.5 times2";
    assert_eq!(show(src), "<2> 15 5.0");
}

#[test]
fn an_imported_pattern_comes_before_the_core() {
    // `add_2` would be `add` two wide.
    let ops = "ops./add_([0-9]+)/: < i64 sym -- i64 > [ int# ! + ] ;";
    assert_eq!(
        show(&format!("{ops} import#: <| ops.* |> ; 5 add_2 add₃")),
        "<1> 10"
    );
    let (kind, _) = error("5 add_2");
    assert_eq!(kind, Kind::NoWord);
}

#[test]
fn a_body_names_words_of_its_namespace() {
    let ops = "ops.sq: [ dup * ] ; ops./sq([0-9]+)/: < sym > [ int# ! sq ] ;";
    assert_eq!(show(&format!("{ops} import#: <| ops.* |> ; sq7")), "<1> 49");
}

#[test]
fn a_recognized_word_takes_marks_as_a_name_does() {
    let src = format!("{OPS} import#: <| ops.* |> ; 5 times3' .");
    assert_eq!(show(&src), "<1> 15");
}

#[test]
fn a_pattern_holds_for_all_of_its_namespace() {
    // A word applied later reads by it wherever it is written.
    assert_eq!(
        show(&format!(
            "import#: <| ops.* |> ; main: [ 5 times3 ] ; {OPS} main"
        )),
        "<1> 15"
    );
    // Before its word is defined, a word it matches is not read another way.
    let (kind, msg) = error(&format!("import#: <| ops.* |> ; 5 times3 {OPS}"));
    assert_eq!(kind, Kind::NoWord);
    assert!(msg.contains("whose word is not defined yet"), "{msg}");
}

#[test]
fn an_export_holds_for_patterns() {
    let ops = format!("{OPS} ops.ee: [ 1 ] ;");
    let (kind, _) = error(&format!(
        "{ops} ops.export#: <| ee |> ; import#: <| ops.* |> ; 5 times3"
    ));
    assert_eq!(kind, Kind::NoWord);
    // Exported by its pattern, it is imported.
    assert_eq!(
        show(&format!(
            "{ops} ops.export#: <| ee /times([0-9]+)/ |> ; import#: <| ops.* |> ; 5 times3"
        )),
        "<1> 15"
    );
}

#[test]
fn a_word_is_read_one_way() {
    let (kind, msg) = error(&format!(
        "{OPS} ops./t(.*)/: < sym > [ drop ] ; import#: <| ops.* |> ; 5 times3"
    ));
    assert_eq!(kind, Kind::Syntax);
    assert!(
        msg.contains("`times3` matches both `ops`'s `/times([0-9]+)/`, and `ops`'s `/t(.*)/`"),
        "{msg}"
    );
    let (_, msg) = error(&format!(
        "{OPS} more./(.+)3/: < sym > [ drop ] ; import#: <| ops.* more.* |> ; 5 times3"
    ));
    assert!(
        msg.contains("`times3` matches both `ops`'s `/times([0-9]+)/`, and `more`'s `/(.+)3/`"),
        "{msg}"
    );
    // A name defined or imported, as two imports are.
    let (kind, msg) = error(&format!(
        "{OPS} import#: <| ops.* |> ; times3: [ 0 ] ; 5 times3"
    ));
    assert_eq!(kind, Kind::Syntax);
    assert!(
        msg.contains("`times3` is defined here, and matches `ops`'s `/times([0-9]+)/`"),
        "{msg}"
    );
}

#[test]
fn a_pattern_is_checked_when_defined() {
    let (kind, msg) = error("/times([0-9]+)?/: < i64 sym > [ drop ] ;");
    assert_eq!(kind, Kind::Syntax);
    assert!(msg.contains("has a group that may not match"), "{msg}");
    let (_, msg) = error("/(a)|(b)/: < sym sym > [ drop drop ] ;");
    assert!(msg.contains("has a group that may not match"), "{msg}");
    let (_, msg) = error("/times([0-9]+/: < sym > [ drop ] ;");
    assert!(msg.contains("is no pattern: unclosed group"), "{msg}");
    let (_, msg) = error("/times([0-9]+)/: < > [ 1 ] ;");
    assert!(
        msg.contains("captures 1, and its signature takes 0"),
        "{msg}"
    );
}

#[test]
fn a_pattern_keeps_its_dots_and_slashes() {
    let dots = "ops./(.+)\\.kg/: < sym > [ drop 1 ] ; import#: <| ops.* |> ; a.kg";
    assert_eq!(show(dots), "<1> 1");
    let slash = "ops./km\\/(.+)/: < sym > [ int# ! ] ; import#: <| ops.* |> ; km/5";
    assert_eq!(show(slash), "<1> 5");
}
