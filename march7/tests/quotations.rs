//! Quotation literals and the control flow that consumes them
//! (docs/QUOTATIONS.md), on a rebuilt system.
#[path = "../tools/assembler.rs"]
mod assembler;
use march7::{Driver, Error, Image};

fn system() -> Image {
    let g0 = assembler::assemble(include_str!("../seed/system.asm")).unwrap();
    let mut d = Driver::boot(&g0).unwrap();
    d.fuel = 120_000_000;
    d.evaluate(include_str!("../seed/system.march")).unwrap();
    let boot = d.machine.stack.pop().unwrap();
    d.system_image(boot).unwrap()
}

fn run(image: &Image, source: &str) -> Result<Vec<i64>, Error> {
    let mut d = Driver::boot(image).unwrap();
    d.evaluate(source)?;
    Ok(d.machine.stack.iter().map(|&n| n as i64).collect())
}

/// The effect `stack-effect` reports for `word` after `source`.
fn effect(image: &Image, source: &str, word: &str) -> Vec<i64> {
    run(image, &format!("{source} ' {word} stack-effect")).unwrap()
}

#[test]
fn if_inlines_one_or_two_quotations_and_keeps_its_forth_form() {
    let g = system();
    let ab = ": ab dup 0 lt? [ negate ] [ ] if ;";
    assert_eq!(run(&g, &format!("{ab} -5 ab 3 ab")), Ok(vec![5, 3]));
    assert_eq!(effect(&g, ab, "ab"), [1, 1, 1]);
    let clamp = ": clamp dup 10 gt? [ drop 10 ] if ;";
    assert_eq!(
        run(&g, &format!("{clamp} 42 clamp 7 clamp")),
        Ok(vec![10, 7])
    );
    assert_eq!(
        run(
            &g,
            ": sign dup 0= if drop 0 else drop 1 then ; 0 sign 9 sign"
        ),
        Ok(vec![0, 1])
    );
}

#[test]
fn times_and_while_loop_over_inlined_bodies() {
    let g = system();
    let sum = ": sum 0 swap [ i0 u+ ] times ;";
    assert_eq!(run(&g, &format!("{sum} 5 sum 0 sum")), Ok(vec![10, 0]));
    assert_eq!(effect(&g, sum, "sum"), [1, 1, 1]);
    // Nested loops: the outer index is `i1`.
    assert_eq!(
        run(
            &g,
            ": grid 0 3 [ 4 [ i1 10 u* i0 u+ u+ ] times ] times ; grid"
        ),
        Ok(vec![138])
    );
    let countdown = ": countdown [ dup ] [ 1 u- ] while ;";
    assert_eq!(run(&g, &format!("{countdown} 5 countdown")), Ok(vec![0]));
    assert_eq!(effect(&g, countdown, "countdown"), [1, 1, 1]);
}

#[test]
fn other_uses_make_first_class_quotations_where_they_appear() {
    let g = system();
    let apply = ": apply-sq [ dup u* ] call ;";
    assert_eq!(run(&g, &format!("{apply} 7 apply-sq")), Ok(vec![49]));
    // The checker follows the quotation's effect through `call`.
    assert_eq!(effect(&g, apply, "apply-sq"), [1, 1, 1]);
    assert_eq!(run(&g, "[ 1 2 u+ ] call"), Ok(vec![3]));
    // A quotation is anchored where it appeared, between 1 and 3.
    assert_eq!(run(&g, ": pos 1 [ 2 ] 3 ; pos drop call"), Ok(vec![1, 2]));
    // Quotations nest, and an inner one is materialized inside its arm.
    assert_eq!(
        run(
            &g,
            ": nested dup 0 lt? [ [ negate ] call ] [ ] if ; -4 nested"
        ),
        Ok(vec![4])
    );
}

#[test]
fn the_checker_sees_inlined_branches() {
    let g = system();
    assert_eq!(effect(&g, ": bad [ 1 ] [ ] if ;", "bad"), [3, 0, 0]);
    assert_eq!(
        run(&g, "checked : bad [ 1 ] [ ] if ;"),
        Err(Error::User(103))
    );
    assert_eq!(
        run(&g, "checked : ok dup 0 lt? [ negate ] [ ] if ; -2 ok"),
        Ok(vec![2])
    );
}

#[test]
fn quotation_errors_are_reported_and_recovered_from() {
    let g = system();
    assert_eq!(run(&g, ": e [ exit ] ;"), Err(Error::User(15)));
    assert_eq!(run(&g, ": r [ recur ] ;"), Err(Error::User(15)));
    assert_eq!(run(&g, "]"), Err(Error::User(17)));
    assert_eq!(run(&g, ": u [ 1 ;"), Err(Error::User(20)));
    assert_eq!(run(&g, ": w [ 1 ] [ 2 ] [ 3 ] if ;"), Err(Error::User(19)));
    // After an error inside a quotation, the session compiles normally.
    let mut d = Driver::boot(&g).unwrap();
    assert!(d.evaluate(": f [ [ nosuchword ] ] ;").is_err());
    d.evaluate(": ok [ 5 ] call ; ok").unwrap();
    assert_eq!(d.machine.stack, [5]);
}

#[test]
fn compile_time_brackets_are_now_double() {
    let g = system();
    assert_eq!(
        run(&g, ": ok 1 2 [[ 3 4 u* literal ]] ; ok"),
        Ok(vec![1, 2, 12])
    );
}

#[test]
fn recur_and_exit_work_in_inlined_arms() {
    let g = system();
    // Recursion from inside an if arm: factorial.
    assert_eq!(
        run(&g, ": f dup 0 eq? [ drop 1 ] [ dup 1 - recur * ] if ; 5 f").unwrap(),
        [120]
    );
    // An early exit from inside an arm.
    assert_eq!(
        run(&g, ": g dup 0 lt? [ drop 0 exit ] if 10 + ; -5 g 5 g").unwrap(),
        [0, 15]
    );
    // An arm inlined into an arm that is itself inlined.
    assert_eq!(
        run(
            &g,
            ": m dup 0 gt? [ dup 1 gt? [ 1 - recur ] [ ] if ] if ; 5 m"
        )
        .unwrap(),
        [1]
    );
    // Inside a loop body: exit leaves the word, and its loop with it.
    assert_eq!(
        run(&g, ": t 10 [ i0 3 eq? [ i0 exit ] if ] times 99 ; t").unwrap(),
        [3]
    );
    // The checker solves the recursion from its base case, as before.
    assert_eq!(
        run(
            &g,
            ": f dup 0 eq? [ drop 1 ] [ dup 1 - recur * ] if ; ' f stack-effect"
        )
        .unwrap(),
        [1, 1, 1]
    );
    // Sealed as its own word, recur and exit would change meaning: trap.
    assert_eq!(run(&g, ": k [ recur ] call ;"), Err(Error::User(15)));
    assert_eq!(
        run(&g, ": k [ [ exit ] [ ] if ] call ;"),
        Err(Error::User(15))
    );
    assert_eq!(run(&g, "[ recur ]"), Err(Error::User(15)));
    // After a trap, the next definition starts clean.
    assert_eq!(run(&g, ": k [ recur ] call ;"), Err(Error::User(15)));
    let mut d = march7::Driver::boot(&g).unwrap();
    assert!(d.evaluate(": k [ recur ] call ;").is_err());
    d.evaluate(": ok [ 1 ] call ; ok").unwrap();
    assert_eq!(d.machine.stack, [1]);
}

#[test]
fn consumers_work_at_top_level() {
    let g = system();
    assert_eq!(run(&g, "1 [ 10 ] [ 20 ] if 0 [ 10 ] [ 20 ] if"), Ok(vec![10, 20]));
    assert_eq!(run(&g, "5 [ 7 ] if 0 [ 8 ] if"), Ok(vec![7]));
    assert_eq!(run(&g, "0 [ dup 5 lt? ] [ 1 + ] while"), Ok(vec![5]));
    assert_eq!(run(&g, "3 [ i0 dup * ] times"), Ok(vec![0, 1, 4]));
    assert_eq!(
        run(&g, "2 [ 3 [ i1 10 * i0 + ] times ] times"),
        Ok(vec![0, 1, 2, 10, 11, 12])
    );
    // The temporary word is resolved: float literals and clauses work.
    assert_eq!(
        run(&g, "1 [ 2.5 1 + ] [ 0.0 ] if"),
        Ok(vec![3.5f64.to_bits() as i64])
    );
    // And checked: an array is not a condition.
    assert_eq!(run(&g, "( 1 2 ) [ 3 ] if"), Err(Error::User(26)));
    // FORTH's form needs a definition.
    assert_eq!(run(&g, "1 if 2 then"), Err(Error::User(19)));
    // A quotation that no consumer takes becomes a value where it stands,
    // before the next token or at the end of the input.
    assert_eq!(run(&g, "[ 1 2 ] call 3"), Ok(vec![1, 2, 3]));
    assert_eq!(run(&g, "[ 4 ] dup call swap call"), Ok(vec![4, 4]));
    let mut d = Driver::boot(&g).unwrap();
    d.evaluate("[ 6 ]").unwrap();
    d.evaluate("call").unwrap();
    assert_eq!(d.machine.stack, [6]);
}
