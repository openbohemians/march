//! Powers and subscripts (doc/design/SYMBOLIC-FORTH.md 21, 22, 24): `w^n`
//! is w's power as mathematics takes it for w's kind, by a rule of the
//! core's, which rewrites the name where it is applied (25). A subscript is
//! an index, which a word's family gives a meaning, and where none does
//! the core's rule makes it a width of a word that moves values or is
//! handed code, `dup_2`, each value moved a group of n.
use march8::{Kind, Op, Primitive, Session};

/// The stack as the display writes it, after each piece in turn, so a value
/// from an earlier piece is one at run time.
fn show(pieces: &[&str]) -> String {
    let mut s = Session::new();
    for p in pieces {
        if let Err(e) = s.eval(p) {
            panic!("{p}: {e}");
        }
    }
    s.show()
}

fn fails(src: &str) -> (Kind, String) {
    let e = Session::new().eval(src).expect_err(src);
    (e.kind, e.msg.clone())
}

/// The code `src` compiles to, after `defs`.
fn code(defs: &str, src: &str) -> Vec<Op> {
    let mut s = Session::new();
    s.eval(defs).unwrap();
    s.compile(src).unwrap().0
}

#[test]
fn a_power_by_the_kind_of_word() {
    // An operation, by its operands: x³ is three x's and two ⋅'s.
    assert_eq!(show(&["1 2 3 4 +^4"]), "<1> 10");
    assert_eq!(show(&["1 2 3 4 +⁴"]), "<1> 10");
    assert_eq!(show(&["5 +^1"]), "<1> 5");
    // A function, applied n times; a value, its copies multiplied.
    assert_eq!(show(&["16.0 sqrt^2"]), "<1> 2.0");
    assert_eq!(show(&["three: [ 3 ] ; three^2"]), "<1> 9");
    // A stack word, applied n times: a permutation's power.
    assert_eq!(show(&["1 2 swap^2"]), "<2> 1 2");
    assert_eq!(show(&["1 2 3 rot^3"]), "<3> 1 2 3");
    assert_eq!(show(&["3 dup^2"]), "<3> 3 3 3");
}

#[test]
fn an_identity_takes_its_type_where_it_is_used() {
    // x⁰ is the identity of x's operation, `+' unit# !`, whose type comes
    // where it is used, as a literal's does, and is i64 where nothing
    // says. core.march states each primitive operation's, and the least and
    // greatest of each order; a word that applies one operation, or flips
    // it, has its identity, and a choice of the greater has the least.
    assert_eq!(show(&["+^0"]), "<1> 0");
    assert_eq!(show(&["*^0"]), "<1> 1");
    assert_eq!(show(&["⋅^0"]), "<1> 1");
    assert_eq!(show(&["1.5 +^0 +"]), "<1> 1.5");
    assert_eq!(show(&["+^0 5 +"]), "<1> 5");
    assert_eq!(show(&["add: [ + ] ; 7 add^0 add"]), "<1> 7");
    assert_eq!(show(&["flip: [ swap + ] ; 7 flip^0 flip"]), "<1> 7");
    assert_eq!(show(&["( 1 2 ) +^0 +"]), "<1> ( 1 2 )");
    assert_eq!(show(&["1.5 sqrt^0"]), "<1> 1.5");
    assert_eq!(show(&["true and^0 and false or^0 or"]), "<2> true false");
    assert_eq!(show(&["-5 i64! max^0 max"]), "<1> -5");
    assert_eq!(show(&["max^0"]), "<1> -9223372036854775808");
    assert_eq!(show(&["2.5 f64! min^0 min"]), "<1> 2.5");
    assert_eq!(show(&["\"x\" concat^0 concat"]), "<1> \"x\"");
    // A value's power is its copies multiplied, so its 0th is `⋅^0`.
    assert_eq!(show(&["three: [ 3 ] ; three^0"]), "<1> 1");
    assert_eq!(show(&["three: [ 3 ] ; 2.5 three^0 *"]), "<1> 2.5");
    // Where the facts give none, there is none; a clause of the core's
    // `unit` or `power#` can, the core's, by its path.
    let (kind, msg) = fails("7 -+^0 -+");
    assert_eq!(kind, Kind::NoWord);
    assert!(msg.contains("for which `unit` gives no identity"), "{msg}");
    assert!(
        msg.contains("`core.unit: < int#-' > [ drop … ] ;`"),
        "{msg}"
    );
    let (kind, msg) = fails("5 max^0 max");
    assert_eq!(kind, Kind::NoWord);
    assert!(msg.contains("`least` gives int# none"), "{msg}");
    assert_eq!(
        show(&["core.unit: < int#-' > [ drop 0 ] ; 7 -+^0 -+"]),
        "<1> 7"
    );
    assert_eq!(
        show(&["core.power#: < -+' 0 > [ drop drop [ 0 ] ] ; -+^0"]),
        "<1> 0"
    );
    // The root's `power#` is the program's own, and the rule applies the
    // core's.
    let (kind, _) = fails("power#: < -+' 0 > [ drop drop [ 0 ] ] ; -+^0");
    assert_eq!(kind, Kind::NoWord);
}

#[test]
fn a_quotations_power_is_a_quotation() {
    // `[ q ]^n` is `[ q^n ]`, still a value, which `.` applies.
    assert_eq!(show(&["( 1 [ dup + ]^3 . )"]), "<1> ( 8 )");
    assert_eq!(show(&["( 1 2 3 ) [ dup + ]^3 map"]), "<1> ( 8 16 24 )");
    assert_eq!(show(&["1 2 3 [ + ]^3 ."]), "<1> 6");
}

#[test]
fn a_width_moves_groups() {
    assert_eq!(show(&["1 2 dup_2"]), "<4> 1 2 1 2");
    assert_eq!(show(&["1 2 3 4 swap_2"]), "<4> 3 4 1 2");
    assert_eq!(show(&["1 2 3 4 drop_2"]), "<2> 1 2");
    assert_eq!(show(&["1 2 3 4 over_2"]), "<6> 1 2 3 4 1 2");
    assert_eq!(show(&["1 2 3 4 5 6", "rot_2"]), "<6> 3 4 5 6 1 2");
    // Code handed to the word is not widened, and runs once.
    assert_eq!(show(&["1 2 3 [ 10 + ] dip_2"]), "<3> 11 2 3");
    assert_eq!(show(&["1 2 [ + ] keep_2"]), "<3> 3 1 2");
    assert_eq!(show(&["3 4", "[ + ] [ * ] bi_2"]), "<2> 7 12");
    // The rule's code is `width#`, which applies a word by its symbol, at a
    // width, as `power#` gives a word's power.
    assert_eq!(show(&["1 2 dup' 2 int# ! width#"]), "<4> 1 2 1 2");
    assert_eq!(show(&["1 2 3 4 +' 4 int# ! power# ."]), "<1> 10");
    let (kind, msg) = fails("1 2 3 4 +' 2 int# ! width#");
    assert_eq!(kind, Kind::NoWord);
    assert!(msg.contains("no word `+_2`"), "{msg}");
}

#[test]
fn a_subscript_is_an_index() {
    // A width is the core's meaning for a word that moves values or is
    // handed code, a program's as well; an operation has none, nor a word
    // that computes, nor a quotation.
    assert_eq!(
        show(&["tuck: [ swap over ] ; 1 2 3 4 tuck_2"]),
        "<6> 3 4 1 2 3 4"
    );
    assert_eq!(show(&["nip: [ swap drop ] ; 1 2 3 4 nip_2"]), "<2> 3 4");
    let (kind, msg) = fails("1 2 3 4 +_2");
    assert_eq!(kind, Kind::NoWord);
    assert!(msg.contains("a subscript is an index"), "{msg}");
    let (kind, _) = fails("dbl: [ dup + ] ; 1 2 dbl_2");
    assert_eq!(kind, Kind::NoWord);
    let (kind, _) = fails("5 log_2");
    assert_eq!(kind, Kind::NoWord);
    let (kind, _) = fails("1 2 [ + ]_2 .");
    assert_eq!(kind, Kind::Syntax);
    // A subscript is part of a name, as x₁ is, and its family is what
    // defines it: a program's `log_2`, or a word of its own.
    assert_eq!(show(&["x_1: [ 5 ] ; x₁"]), "<1> 5");
    assert_eq!(show(&["log_2: [ 1 ] ; log_2"]), "<1> 1");
    // The width first, then the power: `dup_2` cubed.
    assert_eq!(show(&["1 2 dup_2^3"]), "<8> 1 2 1 2 1 2 1 2");
}

#[test]
fn a_power_is_found_where_it_is_applied() {
    // As `dbl` is, in the namespace whose word applies it, and a width of
    // the core in any.
    let ns = "ns.dbl: [ dup + ] ;";
    assert_eq!(
        show(&[&format!("{ns} ns.ff: [ 1 dbl^2 ] ; ns.ff")]),
        "<1> 4"
    );
    assert_eq!(
        show(&[&format!("{ns} ns.ff: [ 1 [ dbl ]^2 . ] ; ns.ff")]),
        "<1> 4"
    );
    assert_eq!(
        show(&[&format!("{ns} ns.ff: [ 1 2 dup_2 ] ; ns.ff")]),
        "<4> 1 2 1 2"
    );
    assert_eq!(show(&[&format!("{ns} 1 ns.dbl^2")]), "<1> 4");
    assert_eq!(
        show(&[&format!("{ns} ns.ff: [ 1 2 dup_2^3 ] ; ns.ff")]),
        "<8> 1 2 1 2 1 2 1 2"
    );
    // A namespace's `dup_2`, or a rule of its own, comes before the core's.
    assert_eq!(
        show(&["ns.dup_2: [ 7 ] ; ns.ff: [ dup_2 ] ; ns.ff"]),
        "<1> 7"
    );
}

#[test]
fn a_width_moves_values_at_run_time_as_the_stack_words_do() {
    use Primitive::*;
    assert_eq!(
        code("1 2", "dup_2"),
        [Op::Prim(Over), Op::Prim(Over), Op::Return]
    );
    assert_eq!(code("1 2", "=_2"), code("1 2", "dup_2"));
    assert_eq!(code("1 2 3 4", "drop_2"), code("1 2 3 4", "drop drop"));
}

#[test]
fn no_word_is_named_by_a_power() {
    let (kind, msg) = fails("sq^2: [ 1 ] ;");
    assert_eq!(kind, Kind::Syntax);
    assert!(
        msg.contains("`sq^2` is a spelling, `sq` to the power 2"),
        "{msg}"
    );
}
