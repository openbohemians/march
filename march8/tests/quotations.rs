//! A quotation as data (doc/design/SYMBOLIC-FORTH.md 6), while compiling:
//! its elements are what Low Stack holds, each a symbol or a quotation, so
//! `[ swap + ]`, which is `[| swap . + . |]`, has four. Run, its symbols land
//! on the stack and its meta-ops act; taken apart, nothing runs, so a
//! meta-op is held as the symbol of its name, `'.'`, and put back, acts.
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
fn its_elements_are_low_stacks() {
    assert_eq!(show("[ swap + ] length"), "<1> 4");
    assert_eq!(show("[ swap + ] first [ swap + ] 2 at"), "<2> swap' .'");
    // A quotation inside is one element, whole, and runs as any does.
    assert_eq!(show("[ [ dup ] dip swap ] length"), "<1> 5");
    assert_eq!(show("3 [ [ dup ] dip swap ] first . +"), "<1> 6");
    // A number is three, `10 int# !`, and `!`, `?` and `;` are one each.
    assert_eq!(show("[ 1 2 + ] length [ 1 2 + ] 2 at"), "<2> 8 int#'");
    assert_eq!(
        show("[ 2 i64 ! ] last [ positive? ] last [ x ; ] last"),
        "<3> !' ?' ;'"
    );
    // A bracket is three, a quotation made data by `!`, `[ … ] seq# !`.
    assert_eq!(show("[ ( 1 2 ) ] length [ { a: 1 } ] 2 at"), "<2> 3 map#'");
    assert_eq!(
        show("[ ( 1 2 ) ] first length [ ( 1 2 ) ] ."),
        "<2> 6 ( 1 2 )"
    );
    // A string is three too, `[ … ] str# !`, its text a symbol (7, step 4).
    assert_eq!(
        show("[ \"a\" ] length [ \"a\" ] 2 at [ \"a b\" ] first first"),
        "<3> 3 str#' 'a b'"
    );
    assert_eq!(
        show("[ \"x\\_\" ] first length 5 [ \"x\\_\" ] first str# !"),
        "<2> 3 \"x5\""
    );
}

#[test]
fn put_back_its_meta_ops_act() {
    assert_eq!(show("5 [ ] dup' 0 insert '.' -1 insert . *"), "<1> 25");
    assert_eq!(show("3 4 [ -+ ] swap' 0 insert '.' 1 insert ."), "<1> 1");
    // Its numbers read back: taken apart and put together, it is the same.
    assert_eq!(show("[ 1 2 + ] 1 3 slice ."), "<1> 1");
    assert_eq!(
        show("[ 1 2 + ] dup first swap rest swap 0 insert ."),
        "<1> 3"
    );
    assert_eq!(show("[ 2 ] [ 3 + ] concat ."), "<1> 5");
    assert_eq!(show("[ + ] rest rest length"), "<1> 0");
}

#[test]
fn its_names_are_paths() {
    // Its names were expanded where it was written, so taken out, a name is
    // its path, and put together, each piece means what it meant.
    let math = "math.dbl: [ dup + ] ; math.q: [ [ dbl ] ] ; math.s: [ dbl' ] ; dbl: [ 0 ] ;";
    assert_eq!(show(&format!("{math} math.q first")), "<1> math.dbl'");
    assert_eq!(show(&format!("{math} 3 math.q [ 1 + ] concat .")), "<1> 7");
    assert_eq!(
        show(&format!("{math} 3 math.q [ dbl ] concat .")),
        "<2> 6 0"
    );
    // A symbol is global text: `math.s`'s `dbl'` is the root's.
    assert_eq!(
        show(&format!("{math} 3 [ 1 + ] math.s 0 insert '.' 1 insert .")),
        "<2> 3 1"
    );
}

#[test]
fn a_word_walks_one_by_recursion() {
    // Its values known only while compiling, a family applied to one again
    // unfolds, its guards decided by the values.
    let dots = "none: [ length 0 eq? ] ; \
                tally: < '.' > [ drop 1 ] ; tally: < sym > [ drop 0 ] ; \
                tally: < quote > [ drop 0 ] ; \
                dots: < quote none? >< i64 > [ drop 0 ] ; \
                dots: < quote >< i64 > [ dup rest dots swap first tally + ] ;";
    assert_eq!(show(&format!("{dots} [ swap + ] dots")), "<1> 2");
    assert_eq!(show(&format!("{dots} [ [ dup ] dip swap ] dots")), "<1> 2");
    assert_eq!(show(&format!("{dots} [ ] dots")), "<1> 0");
    assert_eq!(
        show(&format!("{dots} ff: [ [ 1 2 + 3 * ] dots ] ; ff")),
        "<1> 2"
    );
    // One that never finishes is stopped.
    let (kind, _) = error("loop: < quote > [ loop ] ; [ 1 ] loop");
    assert_eq!(kind, Kind::Limit);
}

#[test]
fn a_symbol_a_meta_op_would_be_taken_for_is_not_taken_apart() {
    let (kind, msg) = error("[ '.' ] length");
    assert_eq!(kind, Kind::Mismatch);
    assert!(
        msg.contains("holding the symbol `.` is not taken apart"),
        "{msg}"
    );
}

#[test]
fn its_positions_are_checked() {
    let (kind, msg) = error("[ + ] 3 at");
    assert_eq!(kind, Kind::Mismatch);
    assert!(
        msg.contains("there is no element 3 in a quotation of 2"),
        "{msg}"
    );
    let (_, msg) = error("[ + ] 0 1 slice");
    assert!(msg.contains("there is no element 0"), "{msg}");
    let (_, msg) = error("[ + ] [ ] 3 insert");
    assert!(msg.contains("there is no gap 3"), "{msg}");
    let (_, msg) = error("[ + ] 5 0 insert");
    assert!(msg.contains("holds symbols and quotations, and 5"), "{msg}");
    // A position must be known while compiling.
    let mut s = Session::new();
    s.eval("1").unwrap();
    let e = s.eval("[ + ] swap at").unwrap_err();
    assert_eq!(e.kind, Kind::Unknown);
}
