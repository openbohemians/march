//! Tail calls: a call or `recur` just before a return becomes a tail call
//! (opcode 9) or a tail recur (opcode 11), so recursion in tail position runs
//! in constant space.
#[path = "../tools/assembler.rs"]
mod assembler;
use march7::{Driver, Error, Image, Op};

fn system() -> Image {
    let g0 = assembler::assemble(include_str!("../seed/system.asm")).unwrap();
    let mut d = Driver::boot(&g0).unwrap();
    d.fuel = 120_000_000;
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

/// The canonical code of the word whose token `source` leaves on top.
fn code(image: &Image, source: &str) -> Vec<Op> {
    let mut d = Driver::boot(image).unwrap();
    d.evaluate(source).unwrap();
    let xt = d.machine.stack.pop().unwrap();
    let cid = d.machine.cid(xt).unwrap();
    let snapshot = d.snapshot().unwrap();
    let march7::Blob::Code(bytes) = &snapshot.blobs[&cid] else {
        panic!("code")
    };
    march7::code::decode(bytes).unwrap()
}

#[test]
fn recursion_in_tail_position_runs_in_constant_space() {
    let g = system();
    // Far deeper than the 16,384 return frames a call may use.
    let count = ": count dup 0 gt? [ 1 - recur ] if ; ";
    assert_eq!(run(&g, &format!("{count} 100000 count")).unwrap(), [0]);
    // A recur just before `;`, and one just before an `exit`.
    assert_eq!(
        run(&g, ": c dup 0 eq? [ exit ] if 1 - recur ; 100000 c").unwrap(),
        [0]
    );
    assert_eq!(
        run(&g, ": e dup 0 gt? [ 1 - recur exit ] if 7 + ; 100000 e").unwrap(),
        [7]
    );
    // A recur that is not in tail position still uses a frame per level.
    let fact = ": f dup 0 eq? [ drop 1 ] [ dup 1 - recur * ] if ; ";
    assert_eq!(run(&g, &format!("{fact} 5 f")).unwrap(), [120]);
    assert_eq!(run(&g, &format!("{fact} 100000 f")), Err(Error::Stack));
}

#[test]
fn the_compiler_marks_only_calls_in_tail_position() {
    let g = system();
    let ops = code(&g, ": count dup 0 gt? [ 1 - recur ] if ; ' count");
    assert!(ops.contains(&Op::TailRecur) && !ops.contains(&Op::Recur));
    let ops = code(&g, ": f dup 0 eq? [ drop 1 ] [ dup 1 - recur * ] if ; ' f");
    assert!(ops.contains(&Op::Recur) && !ops.contains(&Op::TailRecur));
    // A call to another word at the end becomes a tail call; the return
    // stays after it, for any branch that targets it.
    let ops = code(&g, ": a 1 + ; : b 2 * a ; ' b");
    assert!(matches!(ops[ops.len() - 2], Op::Tail(_)));
    assert_eq!(ops[ops.len() - 1], Op::Return);
    assert_eq!(run(&g, ": a 1 + ; : b 2 * a ; 5 b").unwrap(), [11]);
    // A call followed by anything else is an ordinary call. (`u*` is an
    // inline primitive, so `b` ends in no call at all.)
    let ops = code(&g, ": a 1 + ; : b a 2 u* ; ' b");
    assert!(ops.iter().any(|o| matches!(o, Op::Call(_))));
    assert!(!ops.iter().any(|o| matches!(o, Op::Tail(_))));
}

#[test]
fn the_checker_understands_tail_calls() {
    let g = system();
    assert_eq!(
        run(
            &g,
            ": count dup 0 gt? [ 1 - recur ] if ; ' count stack-effect"
        )
        .unwrap(),
        [1, 1, 1]
    );
    assert_eq!(
        run(&g, ": a 1 + ; : b 2 * a ; ' b stack-effect").unwrap(),
        [1, 1, 1]
    );
    // Checked mode accepts both.
    assert_eq!(
        run(&g, "checked : count dup 0 gt? [ 1 - recur ] if ; 3 count").unwrap(),
        [0]
    );
}
