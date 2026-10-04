//! Value types in the checker (docs/CHECKER.md, slice 2): i64 and f64,
//! through literals, primitives, shuffles, calls, branches and loops.
#[path = "../tools/assembler.rs"]
mod assembler;
use march7::{Driver, Error, Image};

fn system() -> Image {
    let g0 = assembler::assemble(include_str!("../seed/system.asm")).unwrap();
    let mut d = Driver::boot(&g0).unwrap();
    d.fuel = 60_000_000;
    d.evaluate(include_str!("../seed/system.march")).unwrap();
    let boot = d.machine.stack.pop().unwrap();
    d.system_image(boot).unwrap()
}

fn run(image: &Image, source: &str) -> Result<Vec<u64>, Error> {
    let mut d = Driver::boot(image).unwrap();
    d.fuel = 100_000_000;
    d.evaluate(source)?;
    Ok(d.machine.stack.clone())
}

const U: u64 = 0;
const I: u64 = 1;
const F: u64 = 2;

/// Output types, top first, packed a byte each as `stack-types` gives them.
fn types(ts: &[u64]) -> u64 {
    ts.iter().enumerate().map(|(i, t)| t << (8 * i)).sum()
}

/// `stack-types` of the last word `source` defines, as (types, outputs).
fn of(image: &Image, source: &str, word: &str) -> (u64, u64) {
    let r = run(image, &format!("{source} ' {word} stack-types")).unwrap();
    assert_eq!(r[r.len() - 1], 1, "{source}: effect unknown");
    (r[r.len() - 3], r[r.len() - 2])
}

#[test]
fn literals_and_primitives_have_types() {
    let g = system();
    assert_eq!(of(&g, ": a 1.5 2.5 f+ ;", "a"), (types(&[F]), 1));
    assert_eq!(of(&g, ": b 1 2 u+ ;", "b"), (types(&[I]), 1));
    assert_eq!(of(&g, ": c 1 2.0 ;", "c"), (types(&[F, I]), 2));
    assert_eq!(of(&g, ": d 7 i>f 2.0 flt? ;", "d"), (types(&[I]), 1));
    assert_eq!(of(&g, ": e 7.5 f>i ;", "e"), (types(&[I]), 1));
    // A word's inputs are unknown, and so is what passes them through.
    assert_eq!(of(&g, ": p dup ;", "p"), (types(&[U, U]), 2));
}

#[test]
fn shuffles_move_types_with_their_values() {
    let g = system();
    assert_eq!(of(&g, ": s 1 2.0 swap ;", "s"), (types(&[I, F]), 2));
    assert_eq!(of(&g, ": o 1 2.0 over ;", "o"), (types(&[I, F, I]), 3));
    assert_eq!(of(&g, ": r 1 2.0 3 rot ;", "r"), (types(&[I, I, F]), 3));
    assert_eq!(of(&g, ": u 2.0 dup ;", "u"), (types(&[F, F]), 2));
    assert_eq!(of(&g, ": n 1 2.0 drop ;", "n"), (types(&[I]), 1));
}

#[test]
fn calls_give_their_outputs_the_callees_types() {
    let g = system();
    let src = ": half 0.5 f* ; : two 2.0 ; : g two half ;";
    assert_eq!(of(&g, src, "g"), (types(&[F]), 1));
    // Through a tail call too.
    assert_eq!(
        of(&g, ": two 2.0 ; : t 1 drop two ;", "t"),
        (types(&[F]), 1)
    );
}

#[test]
fn branches_and_loops_merge_types() {
    let g = system();
    assert_eq!(of(&g, ": h if 1.0 else 2.0 then ;", "h"), (types(&[F]), 1));
    assert_eq!(of(&g, ": h [ 1.0 ] [ 2.0 ] if ;", "h"), (types(&[F]), 1));
    // Arms that disagree leave the slot unknown.
    assert_eq!(of(&g, ": h if 1 else 2.0 then ;", "h"), (types(&[U]), 1));
    // A float accumulator keeps its type through a loop.
    assert_eq!(
        of(&g, ": l 0.0 10 [ 1.5 f+ ] times ;", "l"),
        (types(&[F]), 1)
    );
    // A loop that changes a slot's type widens it to unknown.
    assert_eq!(
        of(&g, ": l 0 10 [ drop 1.0 ] times ;", "l"),
        (types(&[U]), 1)
    );
    assert_eq!(
        of(&g, ": w 0.0 [ dup 10.0 flt? ] [ 1.0 f+ ] while ;", "w"),
        (types(&[F]), 1)
    );
}

fn floats(xs: &[f64]) -> Vec<u64> {
    xs.iter().map(|x| x.to_bits()).collect()
}

#[test]
fn arithmetic_families_resolve_by_type() {
    let g = system();
    assert_eq!(run(&g, ": a 1.5 2.5 + ; a").unwrap(), floats(&[4.0]));
    assert_eq!(run(&g, ": a 1 2 + ; a").unwrap(), [3]);
    assert_eq!(
        run(&g, ": a 10.0 4.0 / 1.5 * 0.5 - ; a").unwrap(),
        floats(&[3.25])
    );
    assert_eq!(
        run(
            &g,
            ": c 1.5 2.5 lt? 2.5 1.5 gt? 1.5 1.5 lte? 1.0 2.0 gte? ; c"
        )
        .unwrap(),
        [1, 1, 1, 0]
    );
    // A float comparison with NaN is false, lte? included.
    assert_eq!(run(&g, ": n 0.0 0.0 / 1.0 lte? ; n").unwrap(), [0]);
    // Types known through calls, branches and loops.
    assert_eq!(
        run(&g, ": two 2.0 ; : d two two + ; d").unwrap(),
        floats(&[4.0])
    );
    assert_eq!(
        run(&g, ": s 0.0 10 [ 1.5 + ] times ; s").unwrap(),
        floats(&[15.0])
    );
    // The output type follows the clause.
    assert_eq!(of(&g, ": a 1.5 2.5 + ;", "a"), (types(&[F]), 1));
    assert_eq!(of(&g, ": a 1 2 + ;", "a"), (types(&[I]), 1));
}

#[test]
fn integer_literals_take_their_type_from_context() {
    let g = system();
    assert_eq!(run(&g, ": c 2.5 1 + ; c").unwrap(), floats(&[3.5]));
    assert_eq!(
        run(&g, ": s 0.0 10 [ 1 + ] times ; s").unwrap(),
        floats(&[10.0])
    );
    assert_eq!(run(&g, ": h 3.0 2 * ; h").unwrap(), floats(&[6.0]));
}

#[test]
fn unknown_types_keep_the_i64_version_and_mismatches_are_errors() {
    let g = system();
    // Inputs of unknown type: the i64 version, as before families.
    assert_eq!(run(&g, ": f + ; 2 3 f").unwrap(), [5]);
    // Known types that match no clause trap, and nothing is defined.
    assert_eq!(run(&g, ": m 1 2.5 + ;"), Err(Error::User(23)));
    let mut d = Driver::boot(&g).unwrap();
    assert!(d.evaluate(": m 1 2.5 + ;").is_err());
    assert!(d.evaluate("m").is_err());
    d.evaluate(": ok 1.0 2.5 + ; ok").unwrap();
    assert_eq!(d.machine.stack, floats(&[3.5]));
}
