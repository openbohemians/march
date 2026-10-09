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
    assert_eq!(show(&["0 zero?. 5 positive?. 5 negative?."]), "<3> 1 1 0");
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
    // Floats divide; `/` is `div`.
    assert_eq!(show(&["7.0 2 div. 7 2 /. 1.0 4 /."]), "<3> 3.5 3 0.25");
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
    let short = "( ( 5 ) ( 6 7 ) ) 0 at.";
    assert_eq!(
        session(&[short, "~ within."]).err(),
        Some(Kind::Run(march8::machine::Error::User(2)))
    );
    assert_eq!(
        show(&["( ( 5 6 ) ( 7 ) ) 0 at.", "~ within."]),
        "<1> ( 6 5 )"
    );
}

#[test]
fn spread_puts_elements_on_the_stack() {
    assert_eq!(show(&["( 1 2 3 ) spread. +. +."]), "<1> 6");
    assert_eq!(show(&["( ( 1 2 3 ) spread. ~. )"]), "<1> ( 1 3 2 )");
    // Any array, inside a literal, spreads as a run.
    let any = "( ( 4 5 ) ( 6 ) ) 0 at.";
    assert_eq!(show(&[any, "( _. spread. 7 )"]), "<1> ( 4 5 7 )");
    // Outside a literal it has nowhere to go.
    assert_eq!(session(&[any, "spread."]).err(), Some(Kind::Mismatch));
}

#[test]
fn negative_indices_count_from_the_end() {
    // -1 is the last, for arrays and strings, in `at` and `slice`.
    assert_eq!(
        show(&["( 1 2 3 ) -1 at. \"héllo\" -1 at. ( 5 6 7 8 ) 1 -1 slice. \"héllo\" -2 -1 slice."]),
        "<4> 3 111 ( 6 7 ) \"l\""
    );
    // Known only at run time: the length is added when it is negative.
    let any = "( ( 5 6 7 8 ) ) 0 at.";
    assert_eq!(show(&[any, "dup. -1 at. swap. -2 at."]), "<2> 8 7");
    assert_eq!(show(&["( 5 6 7 ) -1", "at."]), "<1> 7");
    assert_eq!(show(&[any, "-3 -1 slice."]), "<1> ( 6 7 )");
    assert_eq!(show(&["\"abc\"", "-1 at."]), "<1> 99");
    // On a vec, checked at compile time from both ends.
    assert_eq!(session(&["( 1 2 3 ) -4 at."]).err(), Some(Kind::Mismatch));
    assert_eq!(session(&["( 1 2 3 ) 3 at."]).err(), Some(Kind::Mismatch));
}
