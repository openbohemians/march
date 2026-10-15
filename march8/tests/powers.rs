//! Powers and widths, spellings (doc/design/SYMBOLIC-FORTH.md 21, 22, 24):
//! `w^n` is w's power as mathematics takes it for w's kind, and `w_n` is w
//! with each value it works on a group of n.
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
    assert_eq!(show(&["three: [ 3 ] def three^2"]), "<1> 9");
    // A stack word, applied n times: a permutation's power.
    assert_eq!(show(&["1 2 swap^2"]), "<2> 1 2");
    assert_eq!(show(&["1 2 3 rot^3"]), "<3> 1 2 3");
    assert_eq!(show(&["3 dup^2"]), "<3> 3 3 3");
}

#[test]
fn identities_are_clauses() {
    // The stage cannot know an operation's identity: a clause says it.
    assert_eq!(show(&["+^0"]), "<1> 0");
    assert_eq!(show(&["*^0"]), "<1> 1");
    assert_eq!(show(&["1.5 sqrt^0"]), "<1> 1.5");
    let (kind, msg) = fails("max^0");
    assert_eq!(kind, Kind::NoWord);
    assert!(msg.contains("a clause of `power#`"), "{msg}");
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
}

#[test]
fn an_operation_widened_acts_lane_by_lane() {
    assert_eq!(show(&["1 2 10 20 +_2"]), "<2> 11 22");
    assert_eq!(show(&["1 2", "10 20 +_2"]), "<2> 11 22");
    assert_eq!(show(&["1 2 3 4 [ + ]_2 ."]), "<2> 4 6");
    assert_eq!(show(&["5 6 [ 1 + ]_2 ."]), "<2> 6 7");
    // The width first, then the power: three pairs summed.
    assert_eq!(show(&["1 2 3 4 5 6 +_2^3"]), "<2> 9 12");
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
fn no_word_is_named_by_a_spelling() {
    let (kind, msg) = fails("dup_2: [ over over ] def");
    assert_eq!(kind, Kind::Syntax);
    assert!(msg.contains("`dup_2` is a spelling, `dup` 2 wide"), "{msg}");
    let (_, msg) = fails("sq^2: [ 1 ] def");
    assert!(msg.contains("to the power 2"), "{msg}");
}
