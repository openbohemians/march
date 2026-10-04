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
    // Elements whose known types differ: 252. An empty array: 254.
    assert_eq!(ty(": a ( 1 2.5 ) ;"), (252, 1));
    assert_eq!(ty(": a ( ) ;"), (254, 1));
    // Arrays of arrays: each rank adds 3.
    assert_eq!(ty(": a ( ( 1 2 ) ( 3 ) ) ;"), (6, 1));
    assert_eq!(ty(": a ( ( 1.5 ) ( 2.5 ) ) ;"), (7, 1));
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

#[test]
fn arrays_are_persistent_vectors() {
    let g = system();
    // Fifty thousand elements, built and lifted. A literal gathers from the
    // data stack, so it holds at most the stack's 65,536 cells.
    assert_eq!(
        run(&g, ": big ( 50000 [ i0 ] times ) ; big 1 + 49999 at").unwrap(),
        [50_000]
    );
    assert_eq!(
        run(&g, ": big ( 70000 [ i0 ] times ) ; big"),
        Err(Error::Stack)
    );
    // Lifting leaves its input as it was.
    assert_eq!(
        run(&g, ": a ( 1 2 3 ) ; a dup 10 + drop 0 at").unwrap(),
        [1]
    );
}

#[test]
fn literals_whose_count_varies_still_check() {
    let g = system();
    let effect = |src: &str| run(&g, &format!("{src} ' w stack-types")).unwrap();
    // A loop that pushes per iteration, and a branch that drops in one arm:
    // the literal still leaves one array, whose type covers every element
    // either path leaves.
    assert_eq!(effect(": w ( 5 [ i0 ] times ) ;"), [3, 1, 1]);
    assert_eq!(effect(": w ( 1 2 3 0 [ drop ] if ) ;"), [3, 1, 1]);
    assert_eq!(effect(": w ( 1 2 3.5 0 [ drop ] if ) ;"), [252, 1, 1]);
    assert_eq!(effect(": w ( 3 [ i0 i>f ] times ) ;"), [4, 1, 1]);
    assert_eq!(
        run(&g, ": w ( 1 2 3 0 [ drop ] if ) ; w length").unwrap(),
        [3]
    );
    assert_eq!(
        run(&g, ": w ( 1 2 3 1 [ drop ] if ) ; w length").unwrap(),
        [2]
    );
    // Checked mode accepts such words, and lifting works on their arrays.
    assert_eq!(
        run(&g, "checked : w ( 4 [ i0 ] times ) 10 * ; w 3 at").unwrap(),
        [30]
    );
    // Outside a literal, a loop that changes the depth is still an error.
    assert_eq!(
        run(&g, "checked : w 4 [ i0 ] times ;"),
        Err(Error::User(104))
    );
}

#[test]
fn lifting_goes_all_the_way_down_nested_arrays() {
    let g = system();
    let nested = |src: &str, i: usize, j: usize| {
        run(&g, &format!(": a {src} ; a {i} at {j} at")).unwrap()[0]
    };
    assert_eq!(nested("( ( 1 2 ) ( 3 ) ) 10 +", 0, 1), 12);
    assert_eq!(nested("( ( 1 2 ) ( 3 ) ) 10 +", 1, 0), 13);
    assert_eq!(nested("( ( 1 2 ) ( 3 4 ) ) ( 10 20 ) +", 1, 1), 24);
    // The integer literal becomes a float for arrays of arrays of floats.
    assert_eq!(nested("( ( 1.5 ) ( 2.5 ) ) 1 +", 1, 0), 3.5f64.to_bits());
    let r = run(&g, ": a ( ( 1.5 ) ( 2.5 ) ) 1 + ; ' a stack-types").unwrap();
    assert_eq!(&r[r.len() - 3..], [7, 1, 1]);
    // At top level too.
    assert_eq!(run(&g, "( ( 1 2 ) ( 3 ) ) 10 + 0 at 1 at").unwrap(), [12]);
}

#[test]
fn known_type_errors_stop_the_definition() {
    let g = system();
    // Elements whose known types differ do not lift: the i64 version would be
    // wrong for some of them.
    assert_eq!(run(&g, ": w ( 1 2.5 ) 1 + ;"), Err(Error::User(23)));
    assert_eq!(run(&g, "( 1 2.5 ) 1 +"), Err(Error::User(23)));
    // An array is not a condition (trap 26), directly, through a generic
    // word's instance, or in a loop's test.
    assert_eq!(run(&g, ": w ( 1 2 ) [ 3 ] if ;"), Err(Error::User(26)));
    assert_eq!(
        run(&g, ": mag dup 0 lt? [ 0 swap - ] if ; : w ( 0 5 - 7 ) mag ;"),
        Err(Error::User(26))
    );
    assert_eq!(run(&g, ": w [ ( 1 ) ] [ ] while ;"), Err(Error::User(26)));
    // Without a literal or a family call a definition is only analysed in
    // checked mode.
    assert_eq!(
        run(&g, ": mk ( 1 2 ) ; checked : w mk [ 3 ] if ;"),
        Err(Error::User(26))
    );
    // The definition was not installed.
    assert_eq!(run(&g, ": w ( 1 2 ) [ 3 ] if ; w"), Err(Error::User(26)));
}

#[test]
fn each_fold_and_map_consume_arrays() {
    let g = system();
    assert_eq!(run(&g, ": s ( 1 2 3 4 ) 0 [ + ] fold ; s").unwrap(), [10]);
    assert_eq!(run(&g, ": s 0 ( 1 2 3 ) [ + ] each ; s").unwrap(), [6]);
    assert_eq!(
        run(&g, ": s ( 1.5 2.5 ) 0.0 [ + ] fold ; s").unwrap(),
        floats(&[4.0])
    );
    assert_eq!(elements(&g, "( 1 2 3 ) [ 1 + ] map", 3), [2, 3, 4]);
    // Element types reach the body: the literal becomes a float.
    assert_eq!(
        elements(&g, "( 1.5 2.5 ) [ 1 + ] map", 2),
        floats(&[2.5, 3.5])
    );
    // What lifting cannot do: a branch for each element, and choosing the
    // level in nested arrays.
    assert_eq!(
        elements(&g, "( 0 5 - 7 ) [ dup 0 lt? [ 0 swap - ] if ] map", 2),
        [5, 7]
    );
    assert_eq!(elements(&g, "( ( 1 2 ) ( 3 ) ) [ length ] map", 2), [2, 1]);
    // The body sees the values beneath the array, and the index as `i0`.
    assert_eq!(
        run(&g, ": m 10 ( 1 2 3 ) [ over + ] map ; m 2 at").unwrap(),
        [10, 13]
    );
    assert_eq!(elements(&g, "( 10 20 30 ) [ i0 + ] map", 3), [10, 21, 32]);
    assert_eq!(
        run(&g, ": m 2 [ ( 10 20 ) [ i1 + ] map ] times ; m 1 at").unwrap()[1],
        21
    );
    // `each` inside a literal is a comprehension: it may keep, drop or repeat.
    assert_eq!(
        elements(&g, "( ( 1 5 2 7 ) [ dup 3 lt? [ drop ] if ] each )", 2),
        [5, 7]
    );
    assert_eq!(run(&g, ": e ( ( 1 2 3 ) [ dup ] each ) ; e length").unwrap(), [6]);
    // Results are typed, so families resolve on them.
    let ty = |src: &str| {
        let r = run(&g, &format!(": w {src} ; ' w stack-types")).unwrap();
        r[r.len() - 3]
    };
    assert_eq!(ty("( 1 2 ) [ i>f ] map"), 4);
    assert_eq!(ty("( ( 1 2 ) ( 3 ) ) [ length ] map"), 3);
    assert_eq!(ty("( 1.5 2.5 ) 0.0 [ + ] fold"), 2);
    // A map builds its result directly, not on the data stack.
    assert_eq!(
        run(&g, ": m ( 50000 [ i0 ] times ) [ 2 * ] map ; m 49999 at").unwrap(),
        [99_998]
    );
    // Checked mode accepts them; a map body must leave one value per element.
    assert_eq!(
        run(&g, "checked : w ( 1 2 3 ) [ 1 + ] map ; w 2 at").unwrap(),
        [4]
    );
    assert_eq!(
        run(&g, "checked : w ( 1 2 3 ) [ dup ] map ;"),
        Err(Error::User(104))
    );
    // Like `times`, they are compiled: at top level there is nothing to inline.
    assert_eq!(run(&g, "( 1 2 ) [ 1 + ] map"), Err(Error::User(19)));
}

#[test]
fn literals_inside_loop_bodies_keep_the_index() {
    let g = system();
    // Marks have their own stack, so `i0` inside a literal is the loop's index.
    let r = run(&g, ": r 3 [ ( i0 i0 1 + ) ] times ; r 1 at").unwrap();
    assert_eq!(r[r.len() - 1], 3);
}

#[test]
fn words_that_build_arrays_from_inputs_get_instances() {
    let g = system();
    // `pair`'s elements are its inputs, of unknown type, so it is generic: a
    // call with arrays gets an instance whose result is an array of arrays.
    let pair = ": pair >r >r ( r> r> ) ;";
    assert_eq!(
        run(&g, &format!("{pair} : w ( 1 2 ) ( 3 4 ) pair 1 + ; w 0 at 1 at")).unwrap(),
        [3]
    );
    let r = run(&g, &format!("{pair} : w 1.5 2.5 pair ; ' w stack-types")).unwrap();
    assert_eq!(&r[r.len() - 3..], [4, 1, 1]);
}
