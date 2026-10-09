//! The symbolic stack machine on the explicit form (docs/MACHINE.md): the
//! staged-types prototype's cases (march7/docs/STAGED.md), written as the
//! explicit form, where `.` applies and a definition is `[ … ] name def .`.
use march8::{Kind, Op, Session};

/// The cells left on the stack.
fn run(src: &str) -> Result<Vec<u64>, Kind> {
    let mut s = Session::new();
    s.eval(src).map_err(|e| e.kind)?;
    Ok(s.machine.stack.clone())
}

/// The stack as the display writes it.
fn show(src: &str) -> String {
    let mut s = Session::new();
    if let Err(e) = s.eval(src) {
        panic!("{src}: {e}");
    }
    s.show()
}

/// The code `src` compiles to, after `defs` are defined.
fn code(defs: &str, src: &str) -> Vec<Op> {
    let mut s = Session::new();
    s.eval(defs).unwrap();
    s.compile(src).unwrap().0
}

/// The kind of error `src` makes.
fn fails(src: &str) -> Kind {
    let mut s = Session::new();
    s.eval(src).expect_err(src).kind
}

#[test]
fn literals_fold_and_default() {
    assert_eq!(run("[ 1 1 + . ] two def . two .").unwrap(), [2]);
    // Folded at compile time: the same code as `2`.
    assert_eq!(code("[ 1 1 + . ] two def .", "two ."), code("", "2"));
    // An integer and a decimal fold exactly; a decimal left over is a float.
    assert_eq!(show("1.5 2.25 + ."), "<1> 3.75");
    assert_eq!(show("1 2 + . 3.5 + ."), "<1> 6.5");
    assert_eq!(show("-2.5 1 + ."), "<1> -1.5");
    assert_eq!(code("", "1 2 + . 3.5 + ."), code("", "6.5"));
    // Applying a type annotates: these literals are i64 before `+` sees them.
    assert_eq!(run("1 i64 . 1 i64 . + .").unwrap(), [2]);
}

#[test]
fn a_dot_at_the_end_of_a_word_applies_it() {
    // `sq.` is `sq .`: the same code, and no name ends in a dot.
    assert_eq!(
        code("[ dup. *. ] sq def.", "3 sq. 2.5 sq. 4 [ 1 +. ]. 1 i64. +."),
        code(
            "[ dup . * . ] sq def .",
            "3 sq . 2.5 sq . 4 [ 1 + . ] . 1 i64 . + ."
        )
    );
    assert_eq!(
        show("[ dup. *. ] sq def. 3 sq. 2.5 sq. 4 [ 1 +. ]."),
        "<3> 9 6.25 5"
    );
}

#[test]
fn literals_take_their_type_from_context() {
    // An integer literal beside a float is a float.
    assert_eq!(show("[ < f64 > 1 + . ] inc def . 2.5 inc ."), "<1> 3.5");
    assert_eq!(show("[ < f64 > 1 + . 2 + . ] y def . 0.5 y ."), "<1> 3.5");
    assert_eq!(run("[ < i64 > 1 + . ] w def . 41 w .").unwrap(), [42]);
    // Money is exact cents: a decimal literal becomes cents without passing
    // through a float, and an integer literal is whole units.
    assert_eq!(
        run("[ < money > 1.10 + . ] fee def . 19.99 fee .").unwrap(),
        [2109]
    );
    assert_eq!(
        show("[ < money > 1.10 + . ] fee def . 19.99 fee ."),
        "<1> 21.09"
    );
    assert_eq!(run("19.99 money .").unwrap(), [1999]);
    assert_eq!(run("19.99 money . 1 + .").unwrap(), [2099]);
    // Overflow is found at compile time when the values are known, and at
    // run time when they are not.
    assert_eq!(
        fails("[ < i64 > 1 + . ] w def . 9223372036854775807 w ."),
        Kind::Arithmetic
    );
    let mut s = Session::new();
    s.eval("[ < i64 > 1 + . ] w def . 9223372036854775807")
        .unwrap();
    assert_eq!(
        s.eval("w .").unwrap_err().kind,
        Kind::Run(march8::machine::Error::Arithmetic)
    );
    assert_eq!(
        s.machine.stack,
        [i64::MAX as u64],
        "a failed run leaves the stack"
    );
}

#[test]
fn impossible_types_are_compile_errors() {
    // Money holds cents: a third decimal digit cannot be exact. A clause
    // whose signature types its inputs is compiled where it is defined.
    assert_eq!(fails("[ < money > 1.105 + . ] b def ."), Kind::Literal);
    // A decimal literal is never an integer.
    assert_eq!(fails("1.5 i64 ."), Kind::Literal);
    assert_eq!(fails("[ < i64 > 1.5 + . ] b def ."), Kind::NoWord);
    // No implicit promotion between types: no clause is no word.
    assert_eq!(fails("[ < i64 f64 > + . ] b def ."), Kind::NoWord);
    assert_eq!(fails("[ < i64 money > + . ] b def ."), Kind::NoWord);
    // A type left unapplied at the end, and a word with no definition.
    assert_eq!(fails("i64"), Kind::Mismatch);
    assert_eq!(fails("[ i64 ] b def . b ."), Kind::Mismatch);
    assert_eq!(fails("1 foo ."), Kind::NoWord);
    // A definition that fails is not made.
    let mut s = Session::new();
    s.eval("[ < i64 > 1 + . ] b def .").unwrap();
    assert!(s.eval("[ < i64 > 1.5 + . ] b def .").is_err());
    s.eval("1 b .").unwrap();
    assert_eq!(s.machine.stack, [2]);
}

#[test]
fn words_are_evaluated_for_each_use() {
    // `sq` says nothing of its input's type, and each use works its types
    // out there.
    let sq = "[ dup . * . ] sq def . ";
    assert_eq!(
        run(&format!("{sq} [ < i64 > sq . ] s def . 7 s .")).unwrap(),
        [49]
    );
    assert_eq!(
        show(&format!("{sq} [ < f64 > sq . ] s def . 2.5 s .")),
        "<1> 6.25"
    );
    // Applied to a literal, it folds: `nine` is the constant 9.
    assert_eq!(
        code(&format!("{sq} [ 3 sq . ] nine def ."), "nine ."),
        code("", "9")
    );
    // Words apply words.
    assert_eq!(
        show("[ 1 + . ] inc def . [ inc . inc . ] inc2 def . [ < f64 > inc2 . ] x def . 0.5 x ."),
        "<1> 2.5"
    );
    // A signature types what its word is given: here a literal becomes money.
    assert_eq!(
        run("[ < money > 1.10 + . ] fee def . [ 19.99 fee . ] total def . total .").unwrap(),
        [2109]
    );
    // A word that applies itself, with no case that finishes without doing
    // so, has no types for its results (tests/recursion.rs).
    assert_eq!(fails("[ rec . ] rec def . rec ."), Kind::Mismatch);
}

#[test]
fn brackets_annotate_and_stack_words_move_judgments() {
    // A bracket after values types them, the deepest first.
    assert_eq!(
        run("[ 1 2 < money money > + . ] m def . m .").unwrap(),
        [300]
    );
    assert_eq!(fails("1 2 < money i64 > + ."), Kind::NoWord);
    assert_eq!(run("[ < i64 > 10 swap . - . ] s def . 3 s .").unwrap(), [7]);
    // A literal dropped leaves no code.
    assert_eq!(code("[ 1 2 drop . ] k def .", "k ."), code("", "1"));
    // `dup` copies a literal as a literal, so `1.5 dup +` folds exactly.
    assert_eq!(show("1.5 dup . + ."), "<1> 3.0");
    // Products of decimals stay exact: 1.10 × 1.10 is 1.2100, 121 cents.
    assert_eq!(run("1.10 1.10 * . money .").unwrap(), [121]);
    assert_eq!(show("[ < f64 f64 > * . ] a def . 2.0 3.5 a ."), "<1> 7.0");
    assert_eq!(run("10 3 - .").unwrap(), [7]);
    // Money is added, not multiplied.
    assert_eq!(fails("[ < money money > * . ] b def ."), Kind::NoWord);
    // Types are values: `.` builds them and applies them.
    assert_eq!(run("( 7 8 9 ) 3 i64 vec . . length .").unwrap(), [3]);
    assert_eq!(fails("( 7 8 9 ) 4 i64 vec . ."), Kind::Mismatch);
    assert_eq!(show("( 7 8 9 ) i64 ary . ."), "<1> ( 7 8 9 )");
}

#[test]
fn known_values_have_no_code_until_needed() {
    // Values at run time, from an earlier piece of source, and literals
    // placed under them only when an operation needs both.
    let mut s = Session::new();
    s.eval("5").unwrap();
    s.eval("1 swap . - .").unwrap();
    assert_eq!(s.machine.stack, [(-4i64) as u64]);
    s.eval("10 20 rot .").unwrap();
    assert_eq!(s.show(), "<3> 10 20 -4");
    s.eval("+ . + .").unwrap();
    assert_eq!(s.show(), "<1> 26");
    // `1 2 swap` moves nothing at run time: the code is two literals.
    assert_eq!(code("", "1 2 swap ."), code("", "2 1"));
}

#[test]
fn structured_types_follow_values_through_containers() {
    // The map's type says its values are arrays of i64, so `at` gives an
    // array, and `at` again an i64.
    let h = "[ < string i64 ary map > \"k\" at . 0 at . 1 + . ] h def .";
    assert_eq!(run(&format!("{h} {{ \"k\" ( 41 2 ) }} h .")).unwrap(), [42]);
    // Literals take their type from what the container holds.
    let fl = "[ < string f64 ary map > \"k\" at . 0 at . 1 + . ] fl def .";
    assert_eq!(show(&format!("{fl} {{ \"k\" ( 1.5 ) }} fl .")), "<1> 2.5");
    let m = "[ < string money map > \"fee\" at . 1.10 + . ] m def .";
    assert_eq!(
        run(&format!("{m} {{ \"fee\" 19.99 money . }} m .")).unwrap(),
        [2109]
    );
    // Equal types built apart are one type; different ones do not mix.
    assert_eq!(
        run("[ < i64 ary i64 ary > concat . length . ] c def . ( 1 2 ) ( 3 ) c .").unwrap(),
        [3]
    );
    assert_eq!(
        fails("[ < i64 ary f64 ary > concat . ] c def ."),
        Kind::NoWord
    );
    assert_eq!(fails("[ < i64 ary > \"x\" at . ] k def ."), Kind::NoWord);
    // Strings, with spaces, and their own operations.
    assert_eq!(
        show("[ < string > \"!\" concat . ] s def . \"hi\" s ."),
        "<1> \"hi!\""
    );
    assert_eq!(run("\"a b c\" length .").unwrap(), [5]);
    assert_eq!(fails("[ < string > 1 + . ] s def ."), Kind::NoWord);
    // The display writes every value by its type, nested and mixed.
    assert_eq!(
        show("{ \"k\" ( 1 2 ) } ( \"a\" \"b\" ) ( ( 1.5 ) ( 2.5 3.5 ) )"),
        "<3> { \"k\" ( 1 2 ) } ( \"a\" \"b\" ) ( ( 1.5 ) ( 2.5 3.5 ) )"
    );
}

#[test]
fn a_vec_knows_its_length_at_compile_time() {
    // An array literal is a vec: its length is known, so `length` is a
    // constant and the value is dropped.
    assert_eq!(
        run("[ < 3 i64 vec > length . ] v def . ( 7 8 9 ) v .").unwrap(),
        [3]
    );
    assert_eq!(
        code("[ < 3 i64 vec > length . ] v def .", "( 7 8 9 ) v ."),
        code("", "( 7 8 9 ) drop . 3")
    );
    // Two vecs concatenate to a vec as long as both.
    assert_eq!(
        run("[ < 2 i64 vec 3 i64 vec > concat . length . ] c def . ( 1 2 ) ( 3 4 5 ) c .").unwrap(),
        [5]
    );
    // A literal index past the end is refused at compile time.
    assert_eq!(fails("[ < 3 i64 vec > 5 at . ] b def ."), Kind::Mismatch);
    // A value left in a bracket is a value pattern, so it cannot be an
    // output (tests/choice.rs).
    assert_eq!(fails("[ < -- 2 > ] b def ."), Kind::Mismatch);
}

#[test]
fn signatures_promise_outputs() {
    // After `--`, the outputs, which are checked.
    let fee = "[ < money -- money > 1.10 + . ] fee def .";
    assert_eq!(run(&format!("{fee} 19.99 fee .")).unwrap(), [2109]);
    assert_eq!(fails("[ < i64 -- f64 > 1 + . ] b def ."), Kind::Mismatch);
    assert_eq!(
        fails("[ < i64 -- i64 i64 > 1 + . ] b def ."),
        Kind::Mismatch
    );
    // A literal result takes the type promised.
    assert_eq!(show("[ < -- f64 > 2 ] two def . two ."), "<1> 2.0");
}

#[test]
fn clauses_are_chosen_by_types() {
    let d = "[ < i64 > 2 * . ] d def . [ < string > dup . concat . ] d def .";
    assert_eq!(
        show(&format!(
            "{d} [ < i64 > d . ] a def . [ < string > d . ] b def . 21 a . \"ab\" b ."
        )),
        "<2> 42 \"abab\""
    );
    // The most specific clause wins; a type variable matches anything.
    let first = "[ < a ary > 0 at . ] first def . [ < i64 ary > 1 at . ] first def .";
    assert_eq!(
        show(&format!("{first} ( 5 6 ) first . ( 1.5 2.5 ) first .")),
        "<2> 6 1.5"
    );
    // Equally specific clauses tie.
    let w = "[ < i64 a > drop . drop . 1 ] w def . [ < a i64 > drop . drop . 2 ] w def .";
    assert_eq!(
        fails(&format!("{w} [ < i64 i64 > w . ] x def .")),
        Kind::Mismatch
    );
    // A literal prefers a clause of its default type, then one it converts to.
    let fee = "[ < money > 1.10 + . ] fee def . [ < i64 > 1 + . ] fee def .";
    assert_eq!(run(&format!("{fee} 5 fee . 5.5 fee .")).unwrap(), [6, 660]);
    // A family with no clause for the values is no word.
    assert_eq!(
        fails("[ < f64 > 1 + . ] inc def . 1 i64 . inc ."),
        Kind::NoWord
    );
    // A clause with the same inputs as one before replaces it.
    assert_eq!(
        run("[ < i64 > 1 + . ] f def . [ < i64 > 2 + . ] f def . 1 f .").unwrap(),
        [3]
    );
}

#[test]
fn array_literals_share_one_type() {
    assert_eq!(show("( 1 2 3 )"), "<1> ( 1 2 3 )");
    assert_eq!(show("( 1 2.5 )"), "<1> ( 1.0 2.5 )");
    assert_eq!(run("( ( 1 2 ) ( 3 ) ) length .").unwrap(), [2]);
    assert_eq!(show("( ( 1 2 ) ( 3 ) )"), "<1> ( ( 1 2 ) ( 3 ) )");
    assert_eq!(fails("( )"), Kind::Mismatch);
    assert_eq!(fails("( 1 \"a\" )"), Kind::Literal);
    assert_eq!(fails("( 1 i64 . \"a\" )"), Kind::Mismatch);
    // An element may not take values from outside its literal.
    assert_eq!(fails("1 ( 2 + . )"), Kind::Mismatch);
}

#[test]
fn over_and_rot_move_judgments() {
    assert_eq!(run("1 2 3 rot .").unwrap(), [2, 3, 1]);
    assert_eq!(run("1 2 over .").unwrap(), [1, 2, 1]);
    assert_eq!(
        show("[ < f64 i64 > over . ] o def . 1.5 2 o ."),
        "<3> 1.5 2 1.5"
    );
}

#[test]
fn errors_say_where() {
    let mut s = Session::new();
    let e = s.eval("1 2 +\n  \"a\" + .").unwrap_err();
    assert_eq!(e.kind, Kind::NoWord);
    assert_eq!(e.to_string(), "2:7: no word `+` for symbol string");
    let e = s.eval("[ < i64 > \"x\" + . ] f def .").unwrap_err();
    assert_eq!(
        e.to_string(),
        "1:15: no word `+` for i64 string\n  in `f`, defined at 1:3"
    );
}

#[test]
fn printing() {
    let mut s = Session::new();
    s.eval("\"héllo\\n;\" print . 42 print . 2.5 print .")
        .unwrap();
    assert_eq!(String::from_utf8(s.take_output()).unwrap(), "héllo\n422.5");
    assert_eq!(s.show(), "<0>");
}
