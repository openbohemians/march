//! Arrays and the lifting rule (docs/ARRAYS.md).
#[path = "../tools/assembler.rs"]
mod assembler;
use march7::{Driver, Error, Image};

fn system() -> Image {
    let g0 = assembler::assemble(include_str!("../seed/system.asm")).unwrap();
    let mut d = Driver::boot(&g0).unwrap();
    d.fuel = 80_000_000;
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

fn floats(xs: &[f64]) -> Vec<u64> {
    xs.iter().map(|x| x.to_bits()).collect()
}

/// The elements of the array `source` leaves, through `at`.
fn elements(image: &Image, source: &str, n: usize) -> Vec<u64> {
    let reads: Vec<String> = (0..n).map(|i| format!("a {i} at")).collect();
    run(image, &format!(": a {source} ; {}", reads.join(" "))).unwrap()
}

#[test]
fn literals_gather_what_their_body_leaves() {
    let g = system();
    assert_eq!(run(&g, ": a ( 1 2 3 ) ; a length").unwrap(), [3]);
    assert_eq!(elements(&g, "( 10 20 30 )", 3), [10, 20, 30]);
    assert_eq!(run(&g, "( 1 2 3 ) length").unwrap(), [3]);
    assert_eq!(run(&g, "( 4 5 6 ) 2 at").unwrap(), [6]);
    assert_eq!(run(&g, "( ) length").unwrap(), [0]);
    // Computed elements, and arrays of arrays.
    assert_eq!(elements(&g, "( 1 2 + 3 4 * )", 2), [3, 12]);
    assert_eq!(run(&g, "( ( 1 2 ) ( 3 ) ) length").unwrap(), [2]);
    assert_eq!(run(&g, "( ( 1 2 ) ( 3 ) ) 0 at length").unwrap(), [2]);
    // The body may loop; the marker is taken at run time.
    assert_eq!(run(&g, ": r ( 5 [ i0 ] times ) ; r length").unwrap(), [5]);
    // Reading past the end is an error.
    assert_eq!(run(&g, "( 1 2 ) 2 at"), Err(Error::Memory));
}

#[test]
fn arrays_have_types() {
    let g = system();
    let ty = |src: &str| {
        let r = run(&g, &format!("{src} ' a stack-types")).unwrap();
        (r[r.len() - 3], r[r.len() - 2])
    };
    assert_eq!(ty(": a ( 1 2 3 ) ;"), (3, 1));
    assert_eq!(ty(": a ( 1.5 2.5 ) ;"), (4, 1));
    assert_eq!(ty(": a ( 1 2.5 ) ;"), (5, 1));
    // `at` gives the element type, so families resolve on elements.
    assert_eq!(
        run(&g, ": f ( 1.5 2.5 ) 1 at 1.0 + ; f").unwrap(),
        floats(&[3.5])
    );
    assert_eq!(run(&g, "( 1.5 2.5 ) 0 at 1.0 +").unwrap(), floats(&[2.5]));
}

#[test]
fn families_lift_over_arrays() {
    let g = system();
    assert_eq!(elements(&g, "( 1 2 3 ) 1 +", 3), [2, 3, 4]);
    assert_eq!(elements(&g, "10 ( 1 2 3 ) -", 3), [9, 8, 7]);
    assert_eq!(elements(&g, "( 1 2 ) ( 10 20 ) +", 2), [11, 22]);
    assert_eq!(elements(&g, "( 1 5 ) 3 lt?", 2), [1, 0]);
    assert_eq!(elements(&g, "( 1.5 2.5 ) 2.0 *", 2), floats(&[3.0, 5.0]));
    // An integer literal next to a float array becomes a float.
    assert_eq!(elements(&g, "( 1.5 2.5 ) 1 +", 2), floats(&[2.5, 3.5]));
    // At top level too.
    assert_eq!(run(&g, "( 1 2 3 ) 1 + 2 at").unwrap(), [4]);
    assert_eq!(run(&g, "( 1.5 2.5 ) 1 + 1 at").unwrap(), floats(&[3.5]));
    // Lifting chains: the result is an array of the element result type.
    assert_eq!(elements(&g, "( 1 2 3 ) 1 + 2 *", 3), [4, 6, 8]);
    let r = run(&g, ": a ( 1.5 2.5 ) 2.0 * ; ' a stack-types").unwrap();
    assert_eq!(&r[r.len() - 3..], [4, 1, 1]);
    // A generic word gets an instance for an array, and lifts inside it.
    assert_eq!(
        run(&g, ": double dup + ; : a ( 1.5 2.5 ) double ; a 1 at").unwrap(),
        floats(&[5.0])
    );
    // Arrays of different lengths do not combine.
    assert_eq!(run(&g, "( 1 2 ) ( 1 2 3 ) +"), Err(Error::User(25)));
}

#[test]
fn lifted_words_are_ordinary_checked_words() {
    let g = system();
    assert_eq!(run(&g, "checked : l ( 1 2 3 ) 1 + ; l 1 at").unwrap(), [3]);
}
