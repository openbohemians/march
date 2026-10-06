//! Strings (docs/STRINGS.md): `"…"` literals with escapes and holes, raw
//! `'…'` literals, UTF-8 text in a content-defined sequence measured in
//! characters and lines; `length`, `at`, `concat`, `slice` and `same?` on
//! strings and arrays; `print`, conversions, ordering and search.
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
    // The display writes a string as a literal that reads back (see
    // `escapes`), so a newline shows as `\n;`.
    assert_eq!(show(&g, "\"two\nlines\""), "<1> \"two\\n;lines\"");
    assert_eq!(show(&g, "\"\" length"), "<1> 0");
    // In a definition the text is data, made into a string when it runs.
    assert_eq!(
        show(&g, ": hi \"héllo\" ; hi hi"),
        "<2> \"héllo\" \"héllo\""
    );
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
        run(
            &g,
            "\"abc\" \"abc\" same? \"abc\" \"abd\" same? \"ab\" \"c\" concat \"abc\" same?"
        )
        .unwrap(),
        [1, 0, 1]
    );
    // Arrays too.
    assert_eq!(
        show(&g, "( 1 2 3 ) ( 4 5 ) concat 1 4 slice"),
        "<1> ( 2 3 4 )"
    );
    assert_eq!(
        run(&g, "( 1 2 ) ( 1 2 ) same? ( 1 2 ) ( 2 1 ) same?").unwrap(),
        [1, 0]
    );
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

#[test]
fn escapes() {
    let g = system();
    // `\\` and `\"` take no `;`; the display writes them back the same way.
    assert_eq!(show(&g, r#""a\\b\"c""#), r#"<1> "a\\b\"c""#);
    assert_eq!(run(&g, r#""a\\b\"c" length"#).unwrap(), [5]);
    // Control characters by name, and any code point by number.
    assert_eq!(run(&g, r#""x\n;y\t;z\r;" length"#).unwrap(), [6]);
    assert_eq!(show(&g, r#""x\n;y\t;z\r;""#), r#"<1> "x\n;y\t;z\r;""#);
    assert_eq!(show(&g, r#""\#9731;\#x2603;\#x41;""#), "<1> \"☃☃A\"");
    assert_eq!(show(&g, r#""\#7;""#), r#"<1> "\#7;""#);
    // Names from the symbol table, as `\name` in code.
    assert_eq!(show(&g, r#""a \times; b \leq; c""#), "<1> \"a × b ≤ c\"");
    // `\_x;` is a subscript when `_x` names one, as `\_1` does in code; any
    // other `\_` is an input (see `holes`).
    assert_eq!(show(&g, r#""H\_2;O x\_n;""#), "<1> \"H₂O xₙ\"");
    assert_eq!(show(&g, r#"5 "\_ 1;""#), "<1> \"5 1;\"");
    assert_eq!(show(&g, r#"5 "\_y;""#), "<1> \"5y;\"");
    // An unknown name, a missing `;`, a code point out of range or a
    // surrogate is an error (trap 27); a backslash at the end leaves the
    // string unclosed (trap 8).
    for bad in [
        r#""\bogus;""#,
        r#""\times""#,
        r#""\#x110000;""#,
        r#""\#xD800;""#,
        r#""\#;""#,
        r#""\#12a;""#,
    ] {
        assert_eq!(run(&g, bad), Err(Error::User(27)), "{bad}");
    }
    assert_eq!(run(&g, r#""abc\"#), Err(Error::User(8)));
    // The same in a definition.
    assert_eq!(show(&g, r#": w "\#x263A;\n;" ; w"#), r#"<1> "☺\n;""#);
}

#[test]
fn holes() {
    let g = system();
    // Code in `\[ … ]` runs and its value is written in; text resumes right
    // after the `]`.
    assert_eq!(show(&g, r#""\[ 1 2 + ] apples""#), "<1> \"3 apples\"");
    assert_eq!(show(&g, r#""x\[1 ]y""#), "<1> \"x1y\"");
    // `\_` and `_` take inputs from the stack, in reading order.
    assert_eq!(show(&g, r#"3 "n = \_""#), "<1> \"n = 3\"");
    assert_eq!(show(&g, r#"1 2 "\_ and \_""#), "<1> \"1 and 2\"");
    assert_eq!(show(&g, r#"4 "sq: \[ _ dup * ]""#), "<1> \"sq: 16\"");
    // Each value is written by its type; a string as its text.
    assert_eq!(
        show(&g, r#"( 1 2 ) "a: \_" 2.5 "b: \_" "x" "c: \_""#),
        r#"<3> "a: ( 1 2 )" "b: 2.5" "c: x""#
    );
    // In a definition, a word that takes inputs, resolved by their types.
    assert_eq!(
        show(&g, r#": f "<\[ _ 1 + ]>" ; 5 f 1.5 f"#),
        r#"<2> "<6>" "<2.5>""#
    );
    // Literals nest.
    assert_eq!(
        show(&g, r#""in \[ "nested \[ 2 3 * ]" ] out""#),
        "<1> \"in nested 6 out\""
    );
    // An empty hole is an error (27); `_` outside a hole, or inside a
    // quotation within one, is too (29); taking more inputs than the stack
    // holds is a stack error.
    assert_eq!(run(&g, r#""\[ ]""#), Err(Error::User(27)));
    assert_eq!(run(&g, "_"), Err(Error::User(29)));
    assert_eq!(
        run(&g, r#": k "\[ ( 1 2 ) [ _ + ] map ]" ;"#),
        Err(Error::User(29))
    );
    assert_eq!(run(&g, r#"4 "\[ _ _ * ]""#), Err(Error::Stack));
}

#[test]
fn raw_literals() {
    let g = system();
    // No escapes: a backslash is itself, and a double quote needs none.
    assert_eq!(show(&g, r"'raw \n; text'"), r#"<1> "raw \\n; text""#);
    assert_eq!(show(&g, r#"'say "hi"'"#), r#"<1> "say \"hi\"""#);
    assert_eq!(show(&g, r#": w 'a\b' ; w length"#), "<1> 3");
    // `'` alone still quotes a word.
    assert_eq!(run(&g, ": w 7 ; ' w call").unwrap(), [7]);
}

#[test]
fn print_and_conversions() {
    let g = system();
    let output = |src: &str| {
        let mut d = Driver::boot(&g).unwrap();
        d.evaluate(src).unwrap();
        String::from_utf8(d.machine.take_output()).unwrap()
    };
    // A string prints as its text; anything else as March writes it.
    assert_eq!(
        output(r#""hi " print 42 print ( "a" "b" ) print"#),
        "hi 42( \"a\" \"b\" )"
    );
    assert_eq!(
        output(r#": greet "hello, \_" print ; "you" greet"#),
        "hello, you"
    );
    assert_eq!(show(&g, "42 >string 2.5 >string"), r#"<2> "42" "2.5""#);
    assert_eq!(run(&g, r#""42" >integer 1 +"#).unwrap(), [43]);
    assert_eq!(run(&g, r#""-7" >integer"#).unwrap(), [-7i64 as u64]);
    assert_eq!(show(&g, r#""2.5" >float 1.0 +"#), "<1> 3.5");
    // Text that is not a number, or not a finite one, traps (28).
    assert_eq!(run(&g, r#""abc" >integer"#), Err(Error::User(28)));
    assert_eq!(run(&g, r#""1e400" >float"#), Err(Error::User(28)));
}

#[test]
fn ordering_and_search() {
    let g = system();
    assert_eq!(
        run(
            &g,
            r#""apple" "banana" compare "b" "a" compare "a" "a" compare"#
        )
        .unwrap(),
        [u64::MAX, 1, 0]
    );
    // The comparison family has string clauses.
    assert_eq!(
        run(
            &g,
            r#""apple" "banana" lt? "apple" "banana" gt? "a" "a" lte? "b" "a" gte?"#
        )
        .unwrap(),
        [1, 0, 1, 1]
    );
    // `search` gives a character index, or -1.
    assert_eq!(
        run(
            &g,
            r#""hello world" "world" search "héllo" "llo" search "abc" "z" search"#
        )
        .unwrap(),
        [6, 2, u64::MAX]
    );
}
