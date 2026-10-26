//! Properties (doc/design/TYPES.md 3.17): `prop` defines one over types,
//! with a test or none, and `;` names it. Applied to a type it gives the
//! type with the property, `i64 ary sorted`; a check claims it, `!` tests
//! it, and it is kept or forgotten as a type of its own is.
use march8::{Kind, Session};

fn typed(src: &str) -> String {
    let mut s = Session::new();
    if let Err(e) = s.eval(src) {
        panic!("{src}: {e}");
    }
    s.show_typed()
}

fn fails(src: &str) -> march8::Error {
    Session::new().eval(src).expect_err(src)
}

/// A session reading `input` as its standard input, so that what it reads
/// is known only as the program runs.
fn read(input: &str, src: &str) -> Result<String, march8::Error> {
    let mut s = Session::new();
    s.eval(&src.replace("io.stdin.read", &format!("{input:?}")))?;
    Ok(s.show_typed())
}

#[test]
fn sort_leaves_it() {
    // A word's outputs claim it: `sort` vouches for its result.
    assert_eq!(typed("( 3 1 2 ) sort"), "<1> ( 1 2 3 ) < i64 ary sorted >");
    assert_eq!(
        typed("( \"b\" \"a\" ) sort"),
        "<1> ( \"a\" \"b\" ) < string ary sorted >"
    );
    assert_eq!(typed("( 3 1 2 ) ordered"), "<1> false < bool >");
}

#[test]
fn bang_tests_it() {
    // With no type on top, `sorted` is its class, `a ary sorted`, and `!`
    // gives it by its test, `ordered`, while compiling where the value is
    // known.
    assert_eq!(
        typed("( 1 2 3 ) sorted !"),
        "<1> ( 1 2 3 ) < 3 i64 vec sorted >"
    );
    assert_eq!(
        typed("( 1 2 3 ) i64 ary sorted !"),
        "<1> ( 1 2 3 ) < i64 ary sorted >"
    );
    let e = fails("( 3 1 2 ) sorted !");
    assert!(e.msg.contains("expected 3 i64 vec sorted"), "{e}");
    // A value that has it is not tested again.
    assert_eq!(
        typed("( 3 1 2 ) sort sorted ! sorted !"),
        "<1> ( 1 2 3 ) < i64 ary sorted >"
    );
    // Only of the types it is over.
    let e = fails("\"abc\" sorted !");
    assert_eq!(e.kind, Kind::Mismatch, "{e}");
    let e = fails("i64 sorted");
    assert!(
        e.msg
            .contains("`sorted` is a property of a ary, and i64 is not one"),
        "{e}"
    );
}

#[test]
fn tested_as_the_program_runs() {
    let lengths = "io.stdin.read words [ length ] map";
    assert_eq!(
        read("a bb ccc", &format!("{lengths} sorted !")).unwrap(),
        "<1> ( 1 2 3 ) < i64 ary sorted >"
    );
    let e = read("ccc a bb", &format!("{lengths} sorted !")).unwrap_err();
    assert!(e.msg.contains("expected i64 ary sorted"), "{e}");
}

#[test]
fn a_word_that_takes_it() {
    // A clause for sorted arrays takes only one known to be sorted, and is
    // chosen over one for any array.
    let ff = "ff: < i64 ary sorted >. 1 at ;";
    assert_eq!(typed(&format!("{ff} ( 3 1 2 ) sort ff")), "<1> 1 < i64 >");
    let e = fails(&format!("{ff} ( 3 1 2 ) ff"));
    assert_eq!(e.kind, Kind::NoWord, "{e}");
    let both = "gg: < i64 ary >. drop 1 ; gg: < i64 ary sorted >. drop 2 ;";
    assert_eq!(
        typed(&format!("{both} ( 3 1 2 ) sort gg ( 3 1 2 ) gg")),
        "<2> 2 < i64 > 1 < i64 >"
    );
}

#[test]
fn kept_and_forgotten() {
    // Forgotten where a word that does not name it is applied, kept by
    // moving a value and by a word of any type.
    assert_eq!(typed("( 3 1 2 ) sort reverse"), "<1> ( 3 2 1 ) < i64 ary >");
    assert_eq!(
        typed("( 3 1 2 ) sort 2 -1 insert"),
        "<1> ( 1 2 3 2 ) < i64 ary >"
    );
    assert_eq!(
        typed("( 3 1 2 ) sort ( 5 ) concat"),
        "<1> ( 1 2 3 5 ) < i64 ary >"
    );
    assert_eq!(
        typed("( 3 1 2 ) sort dup swap drop"),
        "<1> ( 1 2 3 ) < i64 ary sorted >"
    );
    assert_eq!(
        typed("ff: < a >. dup drop ; ( 3 1 2 ) sort ff"),
        "<1> ( 1 2 3 ) < i64 ary sorted >"
    );
    // A variable in an output stands for the property only while the
    // value has it: one a word inside forgot is not claimed.
    assert_eq!(
        typed("ff: < a >. dup concat < a >. ; ( 3 1 2 ) sort ff"),
        "<1> ( 1 2 3 1 2 3 ) < i64 ary >"
    );
    // A variable taking a value with it and one without takes neither.
    assert_eq!(
        typed("ff: < a a >. drop ; ( 3 1 2 ) sort ( 5 4 ) < i64 ary > ! ff"),
        "<1> ( 1 2 3 ) < i64 ary >"
    );
}

#[test]
fn defined_by_prop() {
    // With a test, and stacked, one type in either order; by its name alone,
    // a fact `!` trusts.
    let nonempty = "nonempty: < a ary > [ length 0 gt ] prop ;";
    assert_eq!(
        typed(&format!("{nonempty} ( 3 1 2 ) sort nonempty !")),
        "<1> ( 1 2 3 ) < i64 ary sorted nonempty >"
    );
    assert_eq!(
        typed(&format!(
            "{nonempty} ff: < i64 ary nonempty sorted >. 1 at ; \
             ( 3 1 2 ) sort nonempty ! ff"
        )),
        "<1> 1 < i64 >"
    );
    let e = fails(&format!("{nonempty} < i64 ary > empty nonempty !"));
    assert!(e.msg.contains("nonempty"), "{e}");
    assert_eq!(
        typed("checked: string prop ; \"x\" checked !"),
        "<1> \"x\" < string checked >"
    );
    // A check claims one, untested, and a vec keeps its length, as where
    // an array is wanted.
    assert_eq!(
        typed("( 3 1 2 ) < i64 ary sorted >."),
        "<1> ( 3 1 2 ) < 3 i64 vec sorted >"
    );
}

#[test]
fn add_keeps_it_sorted() {
    // `add` puts an element after those at most it, found by halves.
    assert_eq!(
        typed("( 5 3 1 4 2 ) sort 0 add 9 add 3 add 3 add 6 add"),
        "<1> ( 0 1 2 3 3 3 4 5 6 9 ) < i64 ary sorted >"
    );
    assert_eq!(
        typed("< i64 ary > empty sorted ! 4 add 1 add 9 add 5 add"),
        "<1> ( 1 4 5 9 ) < i64 ary sorted >"
    );
    assert_eq!(
        typed("( 2.5 1.0 ) sort 2.0 add"),
        "<1> ( 1.0 2.0 2.5 ) < f64 ary sorted >"
    );
    let lengths = "io.stdin.read words [ length ] map";
    assert_eq!(
        read("ccc a bb", &format!("{lengths} sort 0 add 9 add 2 add")).unwrap(),
        "<1> ( 0 1 2 2 3 9 ) < i64 ary sorted >"
    );
    assert_eq!(
        read("", &format!("{lengths} sorted ! 5 add")).unwrap(),
        "<1> ( 5 ) < i64 ary sorted >"
    );
    // Only to an array known to be sorted.
    let e = fails("( 3 1 2 ) 2 add");
    assert_eq!(e.kind, Kind::NoWord, "{e}");
}
