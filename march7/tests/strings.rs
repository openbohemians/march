//! Strings (docs/STRINGS.md): `"…"` literals, UTF-8 text in a content-defined
//! sequence measured in characters and lines; `length`, `at`, `concat`,
//! `slice` and `same?` on strings and arrays.
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
fn literals_make_strings() {
    let g = system();
    assert_eq!(show(&g, "\"hello\""), "<1> \"hello\"");
    // Raw to the closing quote: spaces, other scripts, newlines.
    assert_eq!(show(&g, "\"hello world\""), "<1> \"hello world\"");
    assert_eq!(show(&g, "\"two\nlines\""), "<1> \"two\nlines\"");
    assert_eq!(show(&g, "\"\" length"), "<1> 0");
    // In a definition the text is data, made into a string when it runs.
    assert_eq!(show(&g, ": hi \"héllo\" ; hi hi"), "<2> \"héllo\" \"héllo\"");
    // A missing closing quote is an error (trap 8, as for `s\"`).
    assert_eq!(run(&g, "\"never closed"), Err(Error::User(8)));
}

#[test]
fn length_and_at_count_characters() {
    let g = system();
    assert_eq!(run(&g, "\"hello\" length").unwrap(), [5]);
    // Characters, not bytes: 2-, 3- and 4-byte UTF-8 count once.
    assert_eq!(run(&g, "\"héllo 中文 😀\" length").unwrap(), [10]);
    assert_eq!(run(&g, "\"héllo 中文 😀\" 1 at").unwrap(), ['é' as u64]);
    assert_eq!(run(&g, "\"héllo 中文 😀\" 6 at").unwrap(), ['中' as u64]);
    assert_eq!(run(&g, "\"héllo 中文 😀\" 9 at").unwrap(), ['😀' as u64]);
    assert_eq!(run(&g, "\"abc\" 3 at"), Err(Error::Memory));
    // `each` and `map` walk the characters.
    assert_eq!(
        run(&g, ": w 0 \"héllo\" [ + ] each ; w").unwrap(),
        ["héllo".chars().map(|c| c as u64).sum::<u64>()]
    );
    assert_eq!(show(&g, "\"abc\" [ 1 + ] map"), "<1> ( 98 99 100 )");
}

#[test]
fn concat_slice_and_same() {
    let g = system();
    assert_eq!(show(&g, "\"ab\" \"cd\" concat"), "<1> \"abcd\"");
    assert_eq!(show(&g, "\"hello world\" 6 11 slice"), "<1> \"world\"");
    assert_eq!(show(&g, "\"中文字\" 1 3 slice"), "<1> \"文字\"");
    assert_eq!(run(&g, "\"abc\" 2 4 slice"), Err(Error::Memory));
    assert_eq!(run(&g, "\"abc\" 2 1 slice"), Err(Error::Memory));
    // Equal contents, by identity, however they were made.
    assert_eq!(
        run(&g, "\"abc\" \"abc\" same? \"abc\" \"abd\" same? \"ab\" \"c\" concat \"abc\" same?")
            .unwrap(),
        [1, 0, 1]
    );
    // Arrays too.
    assert_eq!(show(&g, "( 1 2 3 ) ( 4 5 ) concat 1 4 slice"), "<1> ( 2 3 4 )");
    assert_eq!(run(&g, "( 1 2 ) ( 1 2 ) same? ( 1 2 ) ( 2 1 ) same?").unwrap(), [1, 0]);
    // Not a string with an array.
    assert_eq!(run(&g, "\"ab\" ( 1 ) concat"), Err(Error::Memory));
}

#[test]
fn strings_have_a_type() {
    let g = system();
    let ty = |src: &str| {
        let r = run(&g, &format!("{src} ' w stack-types")).unwrap();
        r[r.len() - 3]
    };
    assert_eq!(ty(": w \"abc\" ;"), 253);
    assert_eq!(ty(": w \"ab\" \"c\" concat ;"), 253);
    assert_eq!(ty(": w \"abc\" 0 2 slice ;"), 253);
    // A character read with `at` is an integer, its code point.
    assert_eq!(ty(": w \"abc\" 0 at ;"), 1);
    // Families do not lift over a string's characters.
    assert_eq!(run(&g, ": w \"abc\" 1 + ;"), Err(Error::User(23)));
    assert_eq!(run(&g, "\"abc\" 1 +"), Err(Error::User(23)));
}

#[test]
fn long_strings_stay_cheap_to_edit() {
    let g = system();
    // "héllo 中文 😀 " is 11 characters; doubled 12 times, 45,056. Then a
    // character joined into the middle by slicing and concatenating.
    let src = ": big \"héllo 中文 😀 \" 12 [ dup concat ] times ; \
               big dup 0 20000 slice \"!\" concat swap 20000 45056 slice concat \
               dup length swap 20000 at";
    assert_eq!(run(&g, src).unwrap(), [45_057, '!' as u64]);
}
