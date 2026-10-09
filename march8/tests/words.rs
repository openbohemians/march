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
