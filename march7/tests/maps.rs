//! Maps (docs/MAPS.md): `{ k v … }` literals, persistent CHAMP maps of
//! cells, with string keys by interning; `at`, `put`, `has?`, `remove`,
//! `length`, `keys`, `values` and `same?`; and their types.
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

fn show(image: &Image, source: &str) -> String {
    let mut d = Driver::boot(image).unwrap();
    d.evaluate(source).unwrap();
    d.show()
}

#[test]
fn literals_and_lookups() {
    let g = system();
    assert_eq!(
        show(&g, "{ \"port\" 8080 \"host\" 7 }"),
        "<1> { \"host\" 7 \"port\" 8080 }"
    );
    assert_eq!(show(&g, "{ }"), "<1> { }");
    assert_eq!(run(&g, "{ \"a\" 1 \"b\" 2 } \"b\" at").unwrap(), [2]);
    assert_eq!(run(&g, "{ 10 100 20 200 } 20 at").unwrap(), [200]);
    // In a definition too.
    assert_eq!(
        run(&g, ": config { \"port\" 8080 } ; config \"port\" at 1 +").unwrap(),
        [8081]
    );
    // A missing key is a memory error; pairs must pair up.
    assert_eq!(run(&g, "{ \"a\" 1 } \"z\" at"), Err(Error::Memory));
    assert_eq!(run(&g, "{ 1 2 3 }"), Err(Error::Stack));
}

#[test]
fn persistent_updates() {
    let g = system();
    assert_eq!(
        show(&g, "{ } \"x\" 10 put \"y\" 20 put"),
        "<1> { \"x\" 10 \"y\" 20 }"
    );
    // The old version is unchanged.
    assert_eq!(
        show(&g, "{ \"a\" 1 } dup \"a\" 2 put"),
        "<2> { \"a\" 1 } { \"a\" 2 }"
    );
    assert_eq!(show(&g, "{ \"a\" 1 \"b\" 2 } \"a\" remove"), "<1> { \"b\" 2 }");
    assert_eq!(
        run(&g, "{ \"a\" 1 } \"a\" has? { \"a\" 1 } \"z\" has?").unwrap(),
        [1, 0]
    );
    assert_eq!(run(&g, "{ \"a\" 1 \"b\" 2 } length").unwrap(), [2]);
    assert_eq!(
        show(&g, "{ \"a\" 1 \"b\" 2 } keys { \"a\" 1 \"b\" 2 } values"),
        "<2> ( \"a\" \"b\" ) ( 1 2 )"
    );
    // `put` replaces an array's element too.
    assert_eq!(show(&g, "( 1 2 3 ) 1 99 put"), "<1> ( 1 99 3 )");
    assert_eq!(run(&g, "( 1 2 3 ) 3 0 put"), Err(Error::Memory));
}

#[test]
fn equal_contents_are_same() {
    let g = system();
    // Insertion order does not matter, and interned strings compare as
    // cells, so `eq?` works on strings.
    assert_eq!(
        run(&g, "{ \"a\" 1 \"b\" 2 } { \"b\" 2 \"a\" 1 } same?").unwrap(),
        [1]
    );
    assert_eq!(
        run(&g, "\"ab\" \"ab\" eq? \"a\" \"b\" concat \"ab\" eq? \"ab\" \"ba\" eq?").unwrap(),
        [1, 1, 0]
    );
    // A key made by concatenation finds the entry a literal made.
    assert_eq!(
        run(&g, "{ \"ab\" 5 } \"a\" \"b\" concat at").unwrap(),
        [5]
    );
}

#[test]
fn maps_have_types() {
    let g = system();
    let ty = |src: &str| {
        let r = run(&g, &format!(": w {src} ; ' w stack-types")).unwrap();
        r[r.len() - 3]
    };
    // 231 + 5 x key kind (1 i64, 2 string) + value kind (1 i64, 2 f64,
    // 3 string); 246 empty; unknown kinds 0.
    assert_eq!(ty("{ }"), 246);
    assert_eq!(ty("{ \"a\" 1 }"), 231 + 10 + 1);
    assert_eq!(ty("{ 1 1.5 }"), 231 + 5 + 2);
    assert_eq!(ty("{ \"a\" \"b\" }"), 231 + 10 + 3);
    assert_eq!(ty("{ \"a\" 1 \"b\" \"c\" }"), 231 + 10);
    assert_eq!(ty("{ } \"a\" 1.5 put"), 231 + 10 + 2);
    // Built in a loop, the map keeps its type.
    assert_eq!(ty("{ } 5 [ i0 dup dup * put ] times"), 231 + 5 + 1);
    // Values have their type: a float value adds as a float.
    assert_eq!(
        run(&g, "{ 1 1.5 2 2.5 } 2 at 1.0 +").unwrap(),
        [3.5f64.to_bits()]
    );
    assert_eq!(
        run(&g, ": w { \"x\" 0.5 } \"x\" at 1 + ; w").unwrap(),
        [1.5f64.to_bits()]
    );
    // Keys of strings are an array of strings (247).
    assert_eq!(ty("{ \"a\" 1 } keys"), 247);
    assert_eq!(ty("{ \"a\" 1 } values"), 3);
}
