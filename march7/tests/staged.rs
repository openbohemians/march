//! Staged types, a prototype (doc/design/TYPES.md): `typed` lowers a body to
//! the explicit form, where `.` applies, and a type stage settles literals by
//! context and leaves plain code; `explicit` takes the explicit form itself.
#[path = "../tools/assembler.rs"]
mod assembler;
use march7::{Driver, Error, Image};

fn system() -> Image {
    let g0 = assembler::assemble(include_str!("../seed/system.asm")).unwrap();
    let mut d = Driver::boot(&g0).unwrap();
    d.fuel = 30_000_000;
    d.evaluate(include_str!("../seed/system.march")).unwrap();
    let boot = d.machine.stack.pop().unwrap();
    d.system_image(boot).unwrap()
}

fn run(image: &Image, source: &str) -> Result<Vec<u64>, Error> {
    let mut d = Driver::boot(image).unwrap();
    d.evaluate(source)?;
    Ok(d.machine.stack.clone())
}

fn show(image: &Image, source: &str) -> String {
    let mut d = Driver::boot(image).unwrap();
    d.evaluate(source).unwrap();
    d.show()
}

/// The code identities of the words whose tokens `source` leaves.
fn cids(image: &Image, source: &str) -> Vec<march7::Cid> {
    let mut d = Driver::boot(image).unwrap();
    d.evaluate(source).unwrap();
    let xts = d.machine.stack.clone();
    xts.iter().map(|&xt| d.machine.cid(xt).unwrap()).collect()
}

#[test]
fn literals_fold_and_default() {
    let g = system();
    assert_eq!(run(&g, "typed two 1 1 + ; two").unwrap(), [2]);
    // Folded at compile time: the word is one literal, the same code as `2`.
    let c = cids(&g, "typed two 1 1 + ; : two2 2 ; ' two ' two2");
    assert_eq!(c[0], c[1]);
    // An integer and a decimal fold exactly; a decimal left over is a float.
    assert_eq!(show(&g, "typed d 1.5 2.25 + ; d"), "<1> 3.75");
    assert_eq!(show(&g, "typed z 1 2 + 3.5 + ; z"), "<1> 6.5");
    assert_eq!(show(&g, "typed n -2.5 1 + ; n"), "<1> -1.5");
    let c = cids(&g, "typed z 1 2 + 3.5 + ; : z2 6.5 ; ' z ' z2");
    assert_eq!(c[0], c[1]);
}

#[test]
fn stage_one_lowers_to_the_explicit_form() {
    let g = system();
    // `typed` adds the `.` that `explicit` writes out: the same code.
    let c = cids(&g, "typed a 1 1 + ; explicit b 1 1 + . ; ' a ' b");
    assert_eq!(c[0], c[1]);
    let c = cids(
        &g,
        "typed a < f64 > 1 + ; explicit b < f64 > 1 + . ; ' a ' b",
    );
    assert_eq!(c[0], c[1]);
    // Applying a type annotates: these literals are i64 before `+` sees them.
    assert_eq!(run(&g, "explicit e 1 i64 . 1 i64 . + . ; e").unwrap(), [2]);
}

#[test]
fn literals_take_their_type_from_context() {
    let g = system();
    // An integer literal beside a float input is a float.
    assert_eq!(show(&g, "typed inc < f64 > 1 + ; 2.5 inc"), "<1> 3.5");
    assert_eq!(show(&g, "typed y < f64 > 1 + 2 + ; 0.5 y"), "<1> 3.5");
    assert_eq!(run(&g, "typed w < i64 > 1 + ; 41 w").unwrap(), [42]);
    // Money is exact cents: a decimal literal becomes cents without passing
    // through a float, and an integer literal is whole units.
    assert_eq!(
        run(&g, "typed fee < money > 1.10 + ; 1999 fee").unwrap(),
        [2109]
    );
    assert_eq!(run(&g, "typed p 19.99 money ; p").unwrap(), [1999]);
    assert_eq!(run(&g, "typed p 19.99 money 1 + ; p").unwrap(), [2099]);
    // Checked integer addition still traps on overflow, at run time.
    assert_eq!(
        run(&g, "typed w < i64 > 1 + ; 9223372036854775807 w"),
        Err(Error::User(10))
    );
}

#[test]
fn impossible_types_are_compile_errors() {
    let g = system();
    // Money holds cents: a third decimal digit cannot be exact.
    assert_eq!(run(&g, "typed b < money > 1.105 + ;"), Err(Error::User(41)));
    // A decimal literal is never an integer.
    assert_eq!(run(&g, "typed b < i64 > 1.5 + ;"), Err(Error::User(41)));
    // No implicit promotion between types.
    assert_eq!(run(&g, "typed b < i64 f64 > + ;"), Err(Error::User(23)));
    assert_eq!(run(&g, "typed b < i64 money > + ;"), Err(Error::User(23)));
    // A type left unapplied at the end, and a word the prototype lacks.
    assert_eq!(run(&g, "explicit b i64 ;"), Err(Error::User(23)));
    assert_eq!(run(&g, "typed b 1 foo ;"), Err(Error::User(40)));
}
