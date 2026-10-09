//! Slice 2 (docs/MACHINE.md): guards, the choice between clauses at run
//! time, and arithmetic lifted over arrays by `map`.
use march8::{Kind, Op, Session, machine};

/// A session that has run each piece of source in turn, so values from an
/// earlier piece are known only at run time in a later one.
fn session(pieces: &[&str]) -> Result<Session, Kind> {
    let mut s = Session::new();
    for p in pieces {
        s.eval(p).map_err(|e| e.kind)?;
    }
    Ok(s)
}

fn cells(pieces: &[&str]) -> Result<Vec<u64>, Kind> {
    Ok(session(pieces)?.machine.stack)
}

fn show(pieces: &[&str]) -> String {
    match session(pieces) {
        Ok(s) => s.show(),
        Err(k) => panic!("{pieces:?}: {k:?}"),
    }
}

fn code(defs: &str, src: &str) -> Vec<Op> {
    let mut s = Session::new();
    s.eval(defs).unwrap();
    s.compile(src).unwrap().0
}

const SIGN: &str = "[ 0 gt?. ] positive? def.
    [ < i64 positive? > drop. 1 ] sign def.
    [ < i64 > drop. 0 ] sign def.";

#[test]
fn guards_choose_at_run_time() {
    // The values are known only at run time, so the guard is tested there.
    assert_eq!(cells(&[SIGN, "5 -3", "sign. swap. sign."]).unwrap(), [0, 1]);
    // No clause whose guard holds is no word, at run time.
    let only = "[ 0 gt?. ] positive? def. [ < i64 positive? > drop. 7 ] only def.";
    assert_eq!(cells(&[only, "5", "only."]).unwrap(), [7]);
    assert_eq!(
        cells(&[only, "-1", "only."]),
        Err(Kind::Run(machine::Error::User(1)))
    );
    // A guard over two inputs: the smaller of two, with no `if`.
    let min = "[ < i64 i64 lt? > drop. ] mn def. [ < i64 i64 > swap. drop. ] mn def.";
    assert_eq!(
        cells(&[min, "3 5 9 2", "mn. rot. rot. mn."]).unwrap(),
        [2, 3]
    );
    // One input known, one at run time.
    assert_eq!(cells(&[min, "3", "5 mn."]).unwrap(), [3]);
    assert_eq!(cells(&[min, "9", "5 mn."]).unwrap(), [5]);
}

#[test]
fn guards_on_known_values_are_decided_now() {
    // `5 sign.` tests its guard at compile time: the code is the literal 1.
    assert_eq!(code(SIGN, "5 sign. -3 sign."), code("", "1 0"));
    // A guard that fails now drops its clause; with none left, no word.
    let only = "[ 0 gt?. ] positive? def. [ < i64 positive? > drop. 7 ] only def.";
    assert_eq!(cells(&[only, "-1 only."]), Err(Kind::NoWord));
}

#[test]
fn clauses_chosen_at_run_time_join_their_types() {
    // Different types make a union, with an informative warning
    // (doc/design/TYPES.md 2.9, 3.6).
    let two = "[ 0 gt?. ] positive? def.
        [ < i64 positive? > drop. \"yes\" ] b def.
        [ < i64 > drop. 1 i64. ] b def.";
    let mut s = session(&[two, "3 -3"]).unwrap();
    s.eval("b. swap. b.").unwrap();
    assert_eq!(
        s.show_typed(),
        "<2> 1 < i64 string or > \"yes\" < i64 string or >"
    );
    let w = s.take_warnings();
    assert_eq!(w.len(), 2, "{w:?}");
    assert!(
        w[0].to_string()
            .contains("informative: this leaves a value of `i64 string or`")
    );
    // A literal that cannot become the others' type takes its default.
    let lit = "[ 0 gt?. ] positive? def.
        [ < i64 positive? > drop. \"yes\" ] b def.
        [ < i64 > drop. 1 ] b def.";
    assert_eq!(show(&[lit, "3", "b."]), "<1> \"yes\"");
    // Literals take the type the alternatives share, as in an array literal.
    let lit = "[ 0 gt?. ] positive? def.
        [ < i64 positive? > drop. 1.5 ] b def.
        [ < i64 > drop. 1 ] b def.";
    assert_eq!(show(&[lit, "3 -3", "b. swap. b."]), "<2> 1.0 1.5");
}

#[test]
fn guards_are_words() {
    // A guard's arity comes from its definition: `lt?` looks at two inputs,
    // `positive?` at one.
    let s = "[ 0 gt?. ] positive? def.
        [ < i64 positive? i64 > drop. drop. 1 ] f def.
        [ < i64 i64 > drop. drop. 0 ] f def.";
    // `f` takes -5 and 1, and its guard looks at -5; then 5 and -1.
    assert_eq!(cells(&[s, "5 -1 -5 1", "f. rot. rot. f."]).unwrap(), [0, 1]);
    // A word that does not leave one flag cannot be a guard.
    assert_eq!(
        cells(&["[ dup. ] two def. [ < i64 two > ] f def."]),
        Err(Kind::Mismatch)
    );
    // A guard is applied to the types it sees: `gt?` has no clause for strings.
    assert_eq!(
        cells(&["[ 0 gt?. ] positive? def. [ < string positive? > ] f def."]),
        Err(Kind::NoWord)
    );
    // Clauses with the same types and different guards are two clauses.
    let two = "[ 0 gt?. ] positive? def. [ 0 lt?. ] negative? def.
        [ < i64 positive? > drop. 1 ] g def.
        [ < i64 negative? > drop. -1 ] g def.
        [ < i64 > drop. 0 ] g def.";
    assert_eq!(
        cells(&[two, "5 -5 0", "g. rot. g. rot. g."]).unwrap(),
        [0, 1, -1i64 as u64]
    );
}

#[test]
fn map_applies_a_quotation_to_each_element() {
    assert_eq!(show(&["( 1 2 3 ) [ 10 *. ] map."]), "<1> ( 10 20 30 )");
    assert_eq!(
        show(&["( 1 2 3 ) [ i64>text. ] map."]),
        "<1> ( \"1\" \"2\" \"3\" )"
    );
    // The quotation may read the values below its element.
    assert_eq!(
        show(&["100 ( 1 2 3 ) [ over. +. ] map."]),
        "<2> 100 ( 101 102 103 )"
    );
    // A vec maps to a vec as long; an array of any length to one too.
    assert_eq!(cells(&["( 1 2 3 ) [ 1 +. ] map. length."]).unwrap(), [3]);
    assert_eq!(show(&["( ( 1 2 ) ( 3 ) ) [ length. ] map."]), "<1> ( 2 1 )");
    // It must leave one value and the values below as they were.
    assert_eq!(cells(&["( 1 2 ) [ dup. ] map."]), Err(Kind::Mismatch));
    assert_eq!(cells(&["1 ( 2 3 ) [ +. ] map."]), Err(Kind::Mismatch));
    // `map` given types builds a map type.
    assert_eq!(show(&["{ \"a\" 1 } string i64 map.."]), "<1> { \"a\" 1 }");
}

#[test]
fn arithmetic_lifts_over_arrays() {
    assert_eq!(
        show(&["[ < i64 ary > 1 +. ] l def. ( 1 2 3 ) l."]),
        "<1> ( 2 3 4 )"
    );
    assert_eq!(
        show(&["[ < f64 ary > 2 *. ] l def. ( 1.5 2.5 ) l."]),
        "<1> ( 3.0 5.0 )"
    );
    assert_eq!(show(&["( 1 2 ) 10 *."]), "<1> ( 10 20 )");
    assert_eq!(
        show(&["10 ( 1 2 3 ) -.", "( 1 2 3 ) 1 -."]),
        "<2> ( 9 8 7 ) ( 0 1 2 )"
    );
    // Money is not multiplied, lifted or not.
    assert_eq!(cells(&["[ < money ary > 2 *. ] l def."]), Err(Kind::NoWord));
    // No promotion: a decimal does not become an i64.
    assert_eq!(cells(&["( 1 2 ) 1.5 +."]), Err(Kind::NoWord));
    // The prototype's original bug: with the map's type known, `1 +` on what
    // `at` gives is an array plus a number, so it lifts.
    let h = "[ < string i64 ary map > \"k\" at. 1 +. ] h def.";
    assert_eq!(show(&[h, "{ \"k\" ( 1 2 ) } h."]), "<1> ( 2 3 )");
    // At run time too.
    assert_eq!(show(&["( 4 5 )", "1 +."]), "<1> ( 5 6 )");
}

#[test]
fn guards_must_not_write() {
    // A guard may only look: one that writes is refused where its clause is
    // defined, or, for a clause whose inputs are not all typed, where it is
    // applied, even when its values are known and it would be decided now.
    let noisy = "[ dup. i64>text. write. 0 gt?. ] noisy? def.";
    assert_eq!(
        cells(&[noisy, "[ < i64 noisy? > drop. 1 ] f def."]),
        Err(Kind::Effect)
    );
    let generic = "[ < a noisy? > drop. 1 ] f def. [ < a > drop. 0 ] f def.";
    assert_eq!(cells(&[noisy, generic, "-5 f."]), Err(Kind::Effect));
    // Writes are found through the words a guard applies, instances
    // included: `cd` writes as it counts down.
    let cd = "[ 0 eq?. ] zero? def.
        [ < i64 zero? > ] cd def.
        [ < i64 > dup. print. 1 -. cd. ] cd def.
        [ cd. 0 eq?. ] done? def.";
    assert_eq!(cells(&[cd, "[ < i64 done? > ] g def."]), Err(Kind::Effect));
    // Writing in a clause's body is fine.
    let mut s = session(&[cd, "3 cd."]).unwrap();
    assert_eq!(String::from_utf8(s.take_output()).unwrap(), "321");
}

#[test]
fn value_patterns_choose_by_equality() {
    // A value left in a signature is a value pattern: the input must equal
    // it. Tested in the order defined, like a guard, by `eq?`.
    let cmd = "[ < \"quit\" > drop. 0 ] cmd def. [ < string > length. ] cmd def.";
    assert_eq!(
        cells(&[cmd, "\"quit\" cmd. \"hello\" cmd."]).unwrap(),
        [0, 5]
    );
    assert_eq!(
        cells(&[cmd, "\"quit\" \"hello\"", "cmd. swap. cmd."]).unwrap(),
        [5, 0]
    );
    // On a known value it is decided now.
    assert_eq!(code(cmd, "\"quit\" cmd."), code("", "0"));
    // An untyped number matches a value of any type it becomes, compared in
    // that type; `.` gives it a type of its own.
    let h = "[ < 0.5 > drop. 1 ] h def. [ < f64 > drop. 0 ] h def.";
    assert_eq!(cells(&[h, "0.5 0.25", "h. swap. h."]).unwrap(), [0, 1]);
    let z = "[ < 0 i64. > drop. \"zero\" ] z def. [ < i64 > drop. \"other\" ] z def.";
    assert_eq!(show(&[z, "0 3", "z. swap. z."]), "<2> \"other\" \"zero\"");
    assert_eq!(cells(&[z, "0.0 z."]), Err(Kind::NoWord));
    assert_eq!(cells(&["[ < 1.5 i64. > ] b def."]), Err(Kind::Literal));
    // Inside its clause the input is known to equal the value, so code
    // there folds on it: `1 +.` on the 0 is the constant 1, its cell
    // dropped. Passed through, it costs nothing.
    let g = "[ < 0 > 1 +. ] g def. [ < i64 > 2 *. ] g def.";
    assert_eq!(cells(&[g, "0 5", "g. swap. g."]).unwrap(), [10, 1]);
    let c = code(&format!("{g} 5"), "g.");
    assert!(
        c.windows(2).any(|w| w
            == [
                march8::Op::Prim(march8::Primitive::Drop),
                march8::Op::Lit(1)
            ]),
        "{c:?}"
    );
    let bye = "[ < \"quit\" > \"bye: \" swap. concat. ] cmd def. [ < string > ] cmd def.";
    assert_eq!(
        show(&[bye, "\"quit\" \"hi\"", "cmd. swap. cmd."]),
        "<2> \"hi\" \"bye: quit\""
    );
    // Patterns belong in signatures, and a value is not an output.
    assert_eq!(cells(&["1 < 1 >"]), Err(Kind::Syntax));
    assert_eq!(cells(&["[ < i64 -- 0 > ] b def."]), Err(Kind::Mismatch));
}
