//! Recognizers (doc/design/SYMBOLIC-FORTH.md 25): rewrite rules, whose
//! patterns, `/log_([0-9]+)/`, each match a whole name. Where a name a rule
//! matches is applied, what the pattern captures goes on the stack, each a
//! symbol, and the rule's code follows, its names meaning what they mean
//! where the rule was defined. A rule holds in its own namespace and where
//! that namespace is imported, before the core's words; the core's own
//! rules, a power and its stack words' subscripts, come last.
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

const OPS: &str = "ops./times([0-9]+)/: [ int# ! * ] ;";

#[test]
fn a_rule_holds_where_imported() {
    assert_eq!(
        show(&format!("{OPS} import#: [| ops.* |] ; 5 times3")),
        "<1> 15"
    );
    // A name the core would not know, as a number's kin.
    let units = "units./([0-9]+)kg/: [ int# ! 1000 * ] ;";
    assert_eq!(
        show(&format!("{units} import#: [| units.* |] ; 12kg 2kg +")),
        "<1> 14000"
    );
    // In a namespace's words, by its own import.
    assert_eq!(
        show(&format!(
            "{OPS} app.import#: [| ops.* |] ; app.main: [ 5 times3 ] ; app.main"
        )),
        "<1> 15"
    );
    // In a quotation, which is code like any, its effect the rewrite's.
    assert_eq!(
        show(&format!(
            "{OPS} import#: [| ops.* |] ; ( 1 2 3 ) [ times3 ] map 5 [ times2 ]^3 ."
        )),
        "<2> ( 3 6 9 ) 40"
    );
}

#[test]
fn a_rule_holds_in_its_own_namespace() {
    assert_eq!(
        show(&format!("{OPS} ops.ff: [ 5 times3 ] ; ops.ff")),
        "<1> 15"
    );
    let root = "/times([0-9]+)/: [ i64 ! * ] ;";
    assert_eq!(show(&format!("{root} 5 times3")), "<1> 15");
}

#[test]
fn captures_go_on_the_stack_in_order() {
    let src = "/([0-9]+)x([0-9]+)/: [ swap int# ! 10 * swap int# ! + ] ; 7x2";
    assert_eq!(show(src), "<1> 72");
}

#[test]
fn a_rule_has_one_meaning_and_no_signature() {
    let (kind, msg) = error(&format!("{OPS} ops./times([0-9]+)/: [ f64 ! * ] ;"));
    assert_eq!(kind, Kind::Syntax);
    assert!(
        msg.contains("has its code already: a rule has one meaning"),
        "{msg}"
    );
    let (kind, msg) = error("/times([0-9]+)/: < i64 sym >< i64 > [ int# ! * ] ;");
    assert_eq!(kind, Kind::Syntax);
    assert!(msg.contains("is a rule"), "{msg}");
    assert!(msg.contains("has no signature"), "{msg}");
    // More meanings are a family's, which the rule's code applies.
    let src = "times: < i64 sym >< i64 > [ int# ! * ] ; \
               times: < f64 sym >< f64 > [ f64 ! * ] ; \
               /times([0-9]+)/: [ times ] ; 5 times3 2.5 times2";
    assert_eq!(show(src), "<2> 15 5.0");
}

#[test]
fn an_imported_rule_comes_before_the_core() {
    // A subscript is an index, which the rule gives a meaning; the core
    // gives `add_2` none.
    let ops = "ops./add_([0-9]+)/: [ int# ! + ] ;";
    assert_eq!(
        show(&format!("{ops} import#: [| ops.* |] ; 5 add_2 add₃")),
        "<1> 10"
    );
    let (kind, _) = error("5 add_2");
    assert_eq!(kind, Kind::NoWord);
    // Before the core's own rules, as it comes before any core word.
    let ops = "ops./dup_([0-9]+)/: [ int# ! ] ;";
    assert_eq!(
        show(&format!("{ops} import#: [| ops.* |] ; dup_7")),
        "<1> 7"
    );
}

#[test]
fn a_rule_names_words_where_it_was_defined() {
    let ops = "ops.sq: [ dup * ] ; ops./sq([0-9]+)/: [ int# ! sq ] ;";
    assert_eq!(show(&format!("{ops} import#: [| ops.* |] ; sq7")), "<1> 49");
    // A capture is a symbol, global text; `name#` reads it as a name where
    // the name was applied.
    let ops = "ops./twice-(.+)/: [ name# 2 power# . ] ;";
    assert_eq!(
        show(&format!(
            "{ops} app.import#: [| ops.* |] ; app.dbl: [ dup + ] ; \
             app.main: [ 3 twice-dbl ] ; app.main"
        )),
        "<1> 12"
    );
}

#[test]
fn a_matched_name_takes_marks_as_any_name_does() {
    let src = format!("{OPS} import#: [| ops.* |] ; 5 times3' .");
    assert_eq!(show(&src), "<1> 15");
}

#[test]
fn a_rule_holds_for_all_of_its_namespace() {
    // A word applied later is rewritten by it wherever it is written.
    assert_eq!(
        show(&format!(
            "import#: [| ops.* |] ; main: [ 5 times3 ] ; {OPS} main"
        )),
        "<1> 15"
    );
    // Before its code is defined, a name it matches has no other meaning.
    let (kind, msg) = error(&format!("import#: [| ops.* |] ; 5 times3 {OPS}"));
    assert_eq!(kind, Kind::NoWord);
    assert!(msg.contains("whose code is not defined yet"), "{msg}");
}

#[test]
fn an_export_holds_for_rules() {
    let ops = format!("{OPS} ops.ee: [ 1 ] ;");
    let (kind, _) = error(&format!(
        "{ops} ops.export#: [| ee |] ; import#: [| ops.* |] ; 5 times3"
    ));
    assert_eq!(kind, Kind::NoWord);
    // Exported by its pattern, it is imported.
    assert_eq!(
        show(&format!(
            "{ops} ops.export#: [| ee /times([0-9]+)/ |] ; import#: [| ops.* |] ; 5 times3"
        )),
        "<1> 15"
    );
}

#[test]
fn a_name_has_one_meaning() {
    let (kind, msg) = error(&format!(
        "{OPS} ops./t(.*)/: [ drop ] ; import#: [| ops.* |] ; 5 times3"
    ));
    assert_eq!(kind, Kind::Syntax);
    assert!(
        msg.contains("`times3` matches both `ops`'s `/times([0-9]+)/`, and `ops`'s `/t(.*)/`"),
        "{msg}"
    );
    let (_, msg) = error(&format!(
        "{OPS} more./(.+)3/: [ drop ] ; import#: [| ops.* more.* |] ; 5 times3"
    ));
    assert!(
        msg.contains("`times3` matches both `ops`'s `/times([0-9]+)/`, and `more`'s `/(.+)3/`"),
        "{msg}"
    );
    // A name defined or imported, as two imports are.
    let (kind, msg) = error(&format!(
        "{OPS} import#: [| ops.* |] ; times3: [ 0 ] ; 5 times3"
    ));
    assert_eq!(kind, Kind::Syntax);
    assert!(
        msg.contains("`times3` is defined here, and matches `ops`'s `/times([0-9]+)/`"),
        "{msg}"
    );
}

#[test]
fn a_pattern_is_checked_when_defined() {
    let (kind, msg) = error("/times([0-9]+)?/: [ drop ] ;");
    assert_eq!(kind, Kind::Syntax);
    assert!(msg.contains("has a group that may not match"), "{msg}");
    let (_, msg) = error("/(a)|(b)/: [ drop drop ] ;");
    assert!(msg.contains("has a group that may not match"), "{msg}");
    let (_, msg) = error("/times([0-9]+/: [ drop ] ;");
    assert!(msg.contains("is no pattern: unclosed group"), "{msg}");
    let (kind, msg) = error("/times([0-9]+)/: 1 ;");
    assert_eq!(kind, Kind::Mismatch);
    assert!(msg.contains("takes its code, a quotation"), "{msg}");
}

#[test]
fn a_pattern_keeps_its_dots_and_slashes() {
    let dots = "ops./(.+)\\.kg/: [ drop 1 ] ; import#: [| ops.* |] ; a.kg";
    assert_eq!(show(dots), "<1> 1");
    let slash = "ops./km\\/(.+)/: [ int# ! ] ; import#: [| ops.* |] ; km/5";
    assert_eq!(show(slash), "<1> 5");
}
