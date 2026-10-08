//! Slice 3 (docs/MACHINE.md): recursion. A family applied again to the same
//! types inside its own application is compiled as an instance, a word of
//! its own; its calls to itself are typed by a ghost, the results of the
//! alternatives that finish without recursing (doc/design/TYPES.md 2.7).
use march8::{Kind, Op, Session, machine};

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

/// The code of `src`, after `pieces` have run.
fn code(pieces: &[&str], src: &str) -> Vec<Op> {
    let mut s = session(pieces).unwrap();
    s.compile(src).unwrap().0
}

const FACT: &str = "[ 0 eq?. ] zero? def.
    [ < i64 zero? > drop. 1 ] fact def.
    [ < i64 > dup. 1 -. fact. *. ] fact def.";

#[test]
fn families_recurse_through_their_instances() {
    assert_eq!(
        cells(&[FACT, "5 fact. 0 fact. 10 fact."]).unwrap(),
        [120, 1, 3_628_800]
    );
    // From a value known only at run time.
    assert_eq!(cells(&[FACT, "6", "fact."]).unwrap(), [720]);
    // Inside another word.
    assert_eq!(cells(&[FACT, "[ 5 fact. ] f def. f."]).unwrap(), [120]);
    // `fact` is called as its i64 instance, last, so as a tail call; at 0
    // its guard is decided now, and it folds.
    let c = code(&[FACT], "5 fact.");
    assert!(
        matches!(c[..], [Op::Lit(5), Op::Tail(_), Op::Return]),
        "{c:?}"
    );
    assert_eq!(code(&[FACT], "0 fact."), code(&[], "1"));
    let fib = "[ 0 eq?. ] zero? def. [ 1 eq?. ] one? def.
        [ < i64 zero? > ] fib def.
        [ < i64 one? > ] fib def.
        [ < i64 > dup. 1 -. fib. swap. 2 -. fib. +. ] fib def.";
    assert_eq!(cells(&[fib, "15 fib."]).unwrap(), [610]);
}

#[test]
fn instances_are_compiled_once_and_shared() {
    // Two words applying `fact` to an i64 call one instance.
    let words = "[ fact. ] a def. [ 1 +. fact. ] b def.";
    let a = code(&[FACT, words, "7"], "a.");
    let b = code(&[FACT, words, "7"], "1 -. b.");
    assert_eq!(a.last(), Some(&Op::Return));
    assert_eq!(a[a.len() - 2], b[b.len() - 2]);
    assert!(matches!(a[a.len() - 2], Op::Tail(_)));
}

#[test]
fn the_ghost_takes_the_type_of_what_it_meets() {
    // The base's literal 1 becomes the f64 the step multiplies.
    let s = session(&[FACT.replace("i64", "f64").as_str(), "5.0 fact."]).unwrap();
    assert_eq!(s.show(), "<1> 120.0");
}

#[test]
fn a_recursive_clause_may_come_before_the_base() {
    // The step is guarded and tested first, but the base, with no guard,
    // finishes first while compiling, and types the recursion.
    let down = "[ 0 gt?. ] positive? def.
        [ < i64 > ] down def.
        [ < i64 positive? > 1 -. down. ] down def.";
    assert_eq!(cells(&[down, "5 down.", "3", "down."]).unwrap(), [0, 0]);
}

#[test]
fn tail_calls_run_in_constant_space() {
    // A million levels would exhaust the machine's 16384 return frames.
    let sumto = "[ 0 eq?. ] zero? def.
        [ < i64 i64 zero? > drop. ] sumto def.
        [ < i64 i64 > swap. over. +. swap. 1 -. sumto. ] sumto def.";
    let mut s = session(&[sumto]).unwrap();
    s.fuel = 50_000_000;
    s.eval("0 1000000 sumto.").unwrap();
    assert_eq!(s.machine.stack, [500_000_500_000]);
}

#[test]
fn recursion_needs_a_case_that_finishes() {
    // No clause finishes without applying itself, so its results have no
    // types, an error where it is defined.
    assert_eq!(cells(&["[ < i64 > loop. ] loop def."]), Err(Kind::Mismatch));
    // Unless its signature promises them: then it is a loop, which runs
    // until the step budget is spent, in constant space.
    let mut s = session(&["[ < i64 -- i64 > 1 +. loop. ] loop def."]).unwrap();
    s.fuel = 100_000;
    assert_eq!(
        s.eval("1 loop.").unwrap_err().kind,
        Kind::Run(machine::Error::Fuel)
    );
}
