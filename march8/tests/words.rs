//! Stack words in pairs, signs, `abs`, `min`, `max`, roots and powers
//! (core/core.march; doc/design/WORDCHART.md).
use march8::{Kind, Session};

fn session(pieces: &[&str]) -> Result<Session, Kind> {
    let mut s = Session::new();
    for p in pieces {
        s.eval(p).map_err(|e| e.kind)?;
    }
    Ok(s)
}

fn show(pieces: &[&str]) -> String {
    match session(pieces) {
        Ok(s) => s.show(),
        Err(k) => panic!("{pieces:?}: {k:?}"),
    }
}

#[test]
fn stack_words_one_and_two_at_a_time() {
    // `=` is `dup` and `~` is `swap`; doubled, FORTH's 2DUP and 2SWAP.
    assert_eq!(show(&["1 2 =. ~."]), "<3> 1 2 2");
    assert_eq!(show(&["1 2 ==."]), "<4> 1 2 1 2");
    assert_eq!(show(&["1 2 3 4 ~~."]), "<4> 3 4 1 2");
    // At run time, known or not.
    assert_eq!(show(&["1 2 3 4", "~~."]), "<4> 3 4 1 2");
    assert_eq!(show(&["1 2", "3 4 ~~."]), "<4> 3 4 1 2");
    assert_eq!(show(&["1 2", "==. +. +. +."]), "<1> 6");
}

#[test]
fn signs_choose_clauses() {
    assert_eq!(show(&["-5 abs. 5 abs. -2.5 abs."]), "<3> 5 5 2.5");
    assert_eq!(show(&["-7", "abs."]), "<1> 7");
    assert_eq!(show(&["3 7 min. 3 7 max. 2.5 1.5 min."]), "<3> 3 7 1.5");
    assert_eq!(show(&["9 4", "min. 6 max."]), "<1> 6");
    assert_eq!(
        show(&["0 zero?. 5 positive?. 5 negative?."]),
        "<3> true true false"
    );
}

#[test]
fn roots_and_powers() {
    assert_eq!(show(&["2 sqrt."]), "<1> 1.4142135623730951");
    assert_eq!(
        show(&["2 10 pow. 2.0 0.5 pow."]),
        "<2> 1024 1.4142135623730951"
    );
    // Two literals stay an exact literal.
    assert_eq!(show(&["2 10 pow. 0.5 *."]), "<1> 512.0");
    // Integer powers are checked: two literals' power is exact, and too
    // large only when it must be an i64; at run time, an overflow.
    assert_eq!(session(&["2 64 pow."]).err(), Some(Kind::Literal));
    assert_eq!(session(&["2 i64. 64 pow."]).err(), Some(Kind::Arithmetic));
    assert_eq!(
        session(&["2", "64 pow."]).err(),
        Some(Kind::Run(march8::machine::Error::Arithmetic))
    );
    assert_eq!(session(&["2 -1 pow."]).err(), Some(Kind::Arithmetic));
}

#[test]
fn divmod_is_floored() {
    // The quotient rounded down, the remainder with the divisor's sign:
    // a = q b + r, for every sign.
    assert_eq!(
        show(&["7 3 divmod. -7 3 divmod. 7 -3 divmod. -7 -3 divmod."]),
        "<8> 2 1 -3 2 -3 -2 2 -1"
    );
    // The same at run time, and `div` and `mod` agree with it.
    assert_eq!(show(&["-7 3", "divmod."]), "<2> -3 2");
    assert_eq!(
        show(&["-7 2", "over. over. div. rot. rot. mod."]),
        "<2> -4 1"
    );
    // Two literals divide exactly at compile time: the code is the results.
    let mut s = Session::new();
    assert_eq!(
        s.compile("-7 3 divmod.").unwrap().0,
        s.compile("-3 2").unwrap().0
    );
    // Floats divide, by `div` or by `/*`, the reciprocal and the product.
    assert_eq!(show(&["7.0 2 div. 7 2 div. 1.0 4 /*."]), "<3> 3.5 3 0.25");
    assert_eq!(session(&["7 0 divmod."]).err(), Some(Kind::Arithmetic));
    assert_eq!(
        session(&["7", "0 mod."]).err(),
        Some(Kind::Run(march8::machine::Error::Arithmetic))
    );
}

#[test]
fn narrowing_says_how_it_rounds() {
    // Down, up, half away from zero, toward zero; on decimal literals,
    // exactly, at compile time.
    assert_eq!(
        show(&["2.7 floor. -2.7 floor. 2.2 ceil. -2.2 ceil. 2.5 round. -2.5 round. -2.7 trunc."]),
        "<7> 2 -3 3 -2 3 -3 -2"
    );
    // A literal's result stays a literal.
    assert_eq!(show(&["2.7 floor. 0.5 +."]), "<1> 2.5");
    // Floats at run time become integers; integers are unchanged.
    assert_eq!(show(&["2.7 -2.5", "floor. swap. round."]), "<2> -3 3");
    assert_eq!(show(&["7", "floor. ceil. round. trunc."]), "<1> 7");
    // A float with no i64 is an error, at compile time or at run time.
    assert_eq!(
        session(&["10.0 300.0 pow. floor."]).err(),
        Some(Kind::Arithmetic)
    );
    assert_eq!(
        session(&["10.0 300.0 pow.", "floor."]).err(),
        Some(Kind::Run(march8::machine::Error::Arithmetic))
    );
}

#[test]
fn multiplying_is_the_dot_operator() {
    // `⋅`, as in algebra; `*` is its ASCII spelling; it lifts as `*` does.
    assert_eq!(show(&["2 3 ⋅. 2.5 2 ⋅. ( 1 2 ) 3 ⋅."]), "<3> 6 5.0 ( 3 6 )");
    // A middle dot or a bullet alone reads as `⋅`; inside a name it stays.
    assert_eq!(show(&["2 3 ·. 2 3 ∙."]), "<2> 6 6");
    assert_eq!(show(&["[ 1 ] col·lecció def. col·lecció."]), "<1> 1");
    assert_eq!(show(&["2 3 \\cdot."]), "<1> 6");
}

#[test]
fn within_runs_a_word_on_the_end_of_an_array() {
    // The array's last elements are the quotation's stack: swap, add,
    // append, drop.
    assert_eq!(
        show(&[
            "( 1 2 3 ) [ ~. ] within. ( 1 2 3 ) [ +. ] within. ( 1 2 3 ) [ 4 ] within. ( 1 2 3 ) [ drop. ] within."
        ]),
        "<4> ( 1 3 2 ) ( 1 5 ) ( 1 2 3 4 ) ( 1 2 )"
    );
    // A vec's new length is known; a word works as a quotation does.
    assert_eq!(show(&["( 1 2 3 ) [ +. ] within. length."]), "<1> 2");
    assert_eq!(show(&["( 5 6 7 )", "~ within."]), "<1> ( 5 7 6 )");
    // Too few elements: at compile time for a vec, at run time otherwise.
    assert_eq!(session(&["( 5 ) ~ within."]).err(), Some(Kind::Mismatch));
    let short = "( ( 5 ) ( 6 7 ) ) 1 at.";
    assert_eq!(
        session(&[short, "~ within."]).err(),
        Some(Kind::Run(march8::machine::Error::User(2)))
    );
    assert_eq!(
        show(&["( ( 5 6 ) ( 7 ) ) 1 at.", "~ within."]),
        "<1> ( 6 5 )"
    );
}

#[test]
fn spread_puts_elements_on_the_stack() {
    assert_eq!(show(&["( 1 2 3 ) spread. +. +."]), "<1> 6");
    assert_eq!(show(&["( ( 1 2 3 ) spread. ~. )"]), "<1> ( 1 3 2 )");
    // Any array, inside a literal, spreads as a run.
    let any = "( ( 4 5 ) ( 6 ) ) 1 at.";
    assert_eq!(show(&[any, "( _. spread. 7 )"]), "<1> ( 4 5 7 )");
    // Outside a literal it has nowhere to go.
    assert_eq!(session(&[any, "spread."]).err(), Some(Kind::Mismatch));
}

#[test]
fn negative_indices_count_from_the_end() {
    // -1 is the last, for arrays and strings, in `at` and `slice`.
    assert_eq!(
        show(&["( 1 2 3 ) -1 at. \"héllo\" -1 at. ( 5 6 7 8 ) 2 -2 slice. \"héllo\" -2 -1 slice."]),
        "<4> 3 111 ( 6 7 ) \"lo\""
    );
    // Known only at run time: the length is added when it is negative.
    let any = "( ( 5 6 7 8 ) ) 1 at.";
    assert_eq!(show(&[any, "dup. -1 at. swap. -2 at."]), "<2> 8 7");
    assert_eq!(show(&["( 5 6 7 ) -1", "at."]), "<1> 7");
    assert_eq!(show(&[any, "-3 -2 slice."]), "<1> ( 6 7 )");
    assert_eq!(show(&["\"abc\"", "-1 at."]), "<1> 99");
    // On a vec, checked at compile time from both ends.
    assert_eq!(session(&["( 1 2 3 ) -4 at."]).err(), Some(Kind::Mismatch));
    assert_eq!(session(&["( 1 2 3 ) 4 at."]).err(), Some(Kind::Mismatch));
    assert_eq!(show(&["( 1 2 3 ) 3 at."]), "<1> 3");
    // Elements count from 1: there is no element 0.
    assert_eq!(session(&["( 1 2 3 ) 0 at."]).err(), Some(Kind::Mismatch));
}

#[test]
fn insert_and_remove_by_gap_and_element() {
    // Gap k is after element k: 0 prepends, -1 appends.
    assert_eq!(
        show(&[
            "( 5 6 7 ) 9 0 insert. ( 5 6 7 ) 9 -1 insert. ( 5 6 7 ) 9 1 insert. ( 5 6 7 ) 9 -2 insert."
        ]),
        "<4> ( 9 5 6 7 ) ( 5 6 7 9 ) ( 5 9 6 7 ) ( 5 6 9 7 )"
    );
    assert_eq!(
        show(&["( 5 6 7 ) 1 remove. ( 5 6 7 ) -1 remove. ( 5 6 7 ) 2 remove. length."]),
        "<3> ( 6 7 ) ( 5 6 ) 2"
    );
    // At run time, from either end; a vec's new length is known.
    let any = "( ( 5 6 7 8 ) ) 1 at.";
    assert_eq!(show(&[any, "9 -1 insert. 2 remove."]), "<1> ( 5 7 8 9 )");
    assert_eq!(show(&["( 5 6 7 ) 9 0 insert. length."]), "<1> 4");
    // `first`, `last`, `rest`, `most`, and inclusive slices.
    assert_eq!(
        show(&["( 5 6 7 ) dup. first. over. last. rot. dup. rest. swap. most."]),
        "<4> 5 7 ( 6 7 ) ( 5 6 )"
    );
    assert_eq!(
        session(&["( 5 6 7 ) 9 5 insert."]).err(),
        Some(Kind::Mismatch)
    );
}

#[test]
fn an_inverse_is_a_word() {
    // `-` negates and `/` is the reciprocal; `-+` and `/*` subtract and
    // divide (Thomas, 2026-10-09). Now and at run time, over arrays too.
    assert_eq!(
        show(&["5 -. 2.5 -. 7 3 -+. 7.0 0.5 -+. ( 1 2 ) -."]),
        "<5> -5 -2.5 4 6.5 ( -1 -2 )"
    );
    assert_eq!(
        show(&["2 /. 7 2 /*. ( 2.0 4.0 ) /. 4 ÷."]),
        "<4> 0.5 3.5 ( 0.5 0.25 ) 0.25"
    );
    assert_eq!(show(&["7 3", "-+. -. 8.0 2.0", "/*. /."]), "<2> -4 0.25");
    // `^` is power; `⋅` is `*`'s glyph.
    assert_eq!(show(&["2 10 ^. 3 4 ⋅."]), "<2> 1024 12");
    // An integer has no reciprocal among the integers: its division is `div`.
    assert_eq!(session(&["7 i64.", "/."]).err(), Some(Kind::NoWord));
    assert_eq!(
        session(&["-9223372036854775807 i64. 1 -+.", "-."]).err(),
        Some(Kind::Run(march8::machine::Error::Arithmetic))
    );
}

#[test]
fn order_keep_and_positions() {
    // The positions that order an array, stable; an array at positions; the
    // elements a mask keeps.
    assert_eq!(
        show(&["( 30 10 20 10 ) order. ( \"pear\" \"fig\" \"apple\" ) order. ( 2.5 -1.0 ) order."]),
        "<3> ( 2 4 3 1 ) ( 3 2 1 ) ( 2 1 )"
    );
    assert_eq!(
        show(&["( 10 20 30 ) ( 3 1 ) at. ( 1 2 3 4 ) ( true. false. true. false. ) keep."]),
        "<2> ( 30 10 ) ( 1 3 )"
    );
    // A sort by any key: the keys ordered, and the array read in their order;
    // `<#` and `#>` are `order` and `dorder`.
    assert_eq!(
        show(&["( \"pear\" \"fig\" \"apple\" ) [ length. ] sort-by. ( 3 -5 2 ) abs sort-by."]),
        "<2> ( \"fig\" \"pear\" \"apple\" ) ( 2 3 -5 )"
    );
    assert_eq!(
        session(&["( 1 2 3 )", "( true. false. ) keep."]).err(),
        Some(Kind::Run(march8::machine::Error::User(4)))
    );
}

#[test]
fn sums_products_and_truth_glyphs() {
    // `∑` and `∏` are the mathematical operators (`\sum`, `\prod`), as words
    // `sum` and `prod`; an empty sum is 0, and a mask's sum counts.
    assert_eq!(
        show(&["( 1 2 3 4 ) sum. ( 1 2 3 4 ) ∏. ( 1.5 2.5 ) \\sum. ( true. false. true. ) sum."]),
        "<4> 10 24 4.0 2"
    );
    assert_eq!(
        show(&["i64 ary. empty. sum. i64 ary. empty. prod."]),
        "<2> 0 1"
    );
    assert_eq!(show(&["⊤. ⊥. \\top. not."]), "<3> true false false");
}

#[test]
fn dorder_and_take() {
    // Stable: equal elements keep their order.
    assert_eq!(
        show(&["( 30 10 20 10 ) dorder. ( \"pear\" \"fig\" \"apple\" ) dorder."]),
        "<2> ( 1 3 2 4 ) ( 1 2 3 )"
    );
    assert_eq!(
        show(&["( 30 10 20 10 ) dup. <#. swap. #>."]),
        "<2> ( 2 4 3 1 ) ( 1 3 2 4 )"
    );
    // The first n, the last n when negative, all if fewer; strings too.
    assert_eq!(
        show(&[
            "( 1 2 3 4 5 ) 2 take. ( 1 2 3 4 5 ) -2 take. ( 1 2 3 ) 9 take. ( 1 2 3 ) -9 take. ( 1 2 3 ) 0 take."
        ]),
        "<5> ( 1 2 ) ( 4 5 ) ( 1 2 3 ) ( 1 2 3 ) ( )"
    );
    assert_eq!(
        show(&["\"hello\" 3 take. \"hello\"", "-2 take."]),
        "<2> \"hel\" \"lo\""
    );
}

#[test]
fn skip_and_thru() {
    // All but the first n, or the last n when negative; strings too.
    assert_eq!(
        show(&["( 1 2 3 4 5 ) 2 skip. ( 1 2 3 4 5 ) -2 skip. ( 1 2 3 ) 9 skip. ( 1 2 3 ) 0 skip."]),
        "<4> ( 3 4 5 ) ( 1 2 3 ) ( ) ( 1 2 3 )"
    );
    assert_eq!(
        show(&["\"hello\" 3 skip. \"hello\"", "-2 skip."]),
        "<2> \"lo\" \"hel\""
    );
    // From a to b, both included, down when a is the larger; `‥` its glyph.
    assert_eq!(
        show(&["1 5 thru. 5 1 thru. 3 3 thru. -2 2 ‥. 0 3 \\thru."]),
        "<5> ( 1 2 3 4 5 ) ( 5 4 3 2 1 ) ( 3 ) ( -2 -1 0 1 2 ) ( 0 1 2 3 )"
    );
    assert_eq!(show(&["4", "1 thru."]), "<1> ( 4 3 2 1 )");
}

#[test]
fn text_words() {
    assert_eq!(
        show(&["\"a,b,,c\" \",\" split. \"abc\" \"\" split. \"line 1\\n;line 2\\n;\" lines."]),
        "<3> ( \"a\" \"b\" \"\" \"c\" ) ( \"a\" \"b\" \"c\" ) ( \"line 1\" \"line 2\" )"
    );
    assert_eq!(
        show(&["\"  the  cat sat \" words. \"Hello\" lower. ( \"a\" \"b\" ) upper."]),
        "<3> ( \"the\" \"cat\" \"sat\" ) \"hello\" ( \"A\" \"B\" )"
    );
    // At run time too.
    assert_eq!(
        show(&["\"x y\"", "words. \"ÉTÉ\"", "lower."]),
        "<2> ( \"x\" \"y\" ) \"été\""
    );
}

#[test]
fn parse_leaves_a_value_or_nil() {
    // A known string is parsed now: the value, or nil.
    let mut s = Session::new();
    s.eval("\"42\" i64 parse. \" 2.5 \" f64 parse. \"x\" i64 parse.")
        .unwrap();
    assert_eq!(s.show_typed(), "<3> 42 < i64 > 2.5 < f64 > nil < nil >");
    // At run time, `i64 nil or`, for clauses to take apart.
    let n = "[ < i64 > ] n def. [ < nil > drop. 0 ] n def.";
    assert_eq!(
        show(&[n, "\"42\" \"nope\"", "i64 parse. n. swap. i64 parse. n."]),
        "<2> 0 42"
    );
    assert_eq!(session(&["\"1\" string parse."]).err(), Some(Kind::NoWord));
}

#[test]
fn at_most_at_least_not_equal() {
    assert_eq!(
        show(&["3 5 le?. 5 5 ≤. 5 3 ge?. 3 5 neq?. 3 3 ≠."]),
        "<5> true true true true false"
    );
    // NaN is at most nothing and at least nothing, and equal to nothing.
    assert_eq!(
        show(&["0.0 0.0 /*. dup. le?. 0.0 0.0 /*. dup. ge?. 0.0 0.0 /*. dup. neq?."]),
        "<3> false false true"
    );
}
