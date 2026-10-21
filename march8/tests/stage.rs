//! The symbolic stack machine (docs/MACHINE.md): the staged-types
//! prototype's cases (march7/docs/STAGED.md), written in March, where a name
//! applies and a definition is `name: [ … ] ;`.
use march8::{Kind, Op, Primitive, Session};

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
    assert_eq!(run("two: [ 1 1 + ] ; two").unwrap(), [2]);
    // Folded at compile time: the same code as `2`.
    assert_eq!(code("two: [ 1 1 + ] ;", "two"), code("", "2"));
    // An integer and a decimal fold exactly; a decimal left over is a float.
    assert_eq!(show("1.5 2.25 +"), "<1> 3.75");
    assert_eq!(show("1 2 + 3.5 +"), "<1> 6.5");
    assert_eq!(show("-2.5 1 +"), "<1> -1.5");
    assert_eq!(code("", "1 2 + 3.5 +"), code("", "6.5"));
    // Applying a type annotates: these literals are i64 before `+` sees them.
    assert_eq!(run("1 i64! 1 i64! +").unwrap(), [2]);
}

#[test]
fn a_name_applies_and_a_dot_applies_once_more() {
    // `sq` is `sq' .`, the symbol applied; a dot at the end of a word
    // applies what it left, `[ 1 + ].` as `[ 1 + ] .`.
    assert_eq!(
        code("sq: [ dup * ] ;", "3 sq 2.5 sq 4 [ 1 + ]. 1 i64! +"),
        code("sq: [ dup * ] ;", "3 sq' . 2.5 sq' . 4 [ 1 + ] . 1 i64! +")
    );
    assert_eq!(
        show("sq: [ dup * ] ; 3 sq 2.5 sq 4 [ 1 + ]."),
        "<3> 9 6.25 5"
    );
}

#[test]
fn a_dot_inside_a_name_divides_a_path() {
    // A path in a namespace, `io.file.read` (doc/design/TYPES.md 3.12):
    // words do not chain, and a path names a word in its namespace.
    let mut s = Session::new();
    let e = s.eval("( 3 1 2 ) sort.length").unwrap_err();
    assert_eq!(e.kind, Kind::NoWord);
    assert!(e.to_string().contains("`sort length`"), "{e}");
    assert_eq!(show("( 3 1 2 ) sort length"), "<1> 3");
    assert_eq!(show("a.bb: [ 1 ] ; a.bb"), "<1> 1");
    assert_eq!(fails("a..b: [ 1 ] ;"), Kind::Syntax);
}

#[test]
fn literals_take_their_type_from_context() {
    // An integer literal beside a float is a float.
    assert_eq!(show("inc: < f64 > [ 1 + ] ; 2.5 inc"), "<1> 3.5");
    assert_eq!(show("yy: < f64 > [ 1 + 2 + ] ; 0.5 yy"), "<1> 3.5");
    assert_eq!(run("ww: < i64 > [ 1 + ] ; 41 ww").unwrap(), [42]);
    // Money is exact cents: a decimal literal becomes cents without passing
    // through a float, and an integer literal is whole units.
    assert_eq!(
        run("fee: < money > [ 1.10 + ] ; 19.99 fee").unwrap(),
        [2109]
    );
    assert_eq!(show("fee: < money > [ 1.10 + ] ; 19.99 fee"), "<1> 21.09");
    assert_eq!(run("19.99 money!").unwrap(), [1999]);
    assert_eq!(run("19.99 money! 1 +").unwrap(), [2099]);
    // Overflow is found at compile time when the values are known, and at
    // run time when they are not.
    assert_eq!(
        fails("ww: < i64 > [ 1 + ] ; 9223372036854775807 ww"),
        Kind::Arithmetic
    );
    let mut s = Session::new();
    s.eval("ww: < i64 > [ 1 + ] ; 9223372036854775807").unwrap();
    assert_eq!(
        s.eval("ww").unwrap_err().kind,
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
    assert_eq!(fails("bb: < money > [ 1.105 + ] ;"), Kind::Literal);
    // A decimal literal is never an integer.
    assert_eq!(fails("1.5 i64!"), Kind::Literal);
    assert_eq!(fails("bb: < i64 > [ 1.5 + ] ;"), Kind::NoWord);
    // No implicit promotion between types: no clause is no word.
    assert_eq!(fails("bb: < i64 f64 > [ + ] ;"), Kind::NoWord);
    assert_eq!(fails("bb: < i64 money > [ + ] ;"), Kind::NoWord);
    // A type left unapplied at the end, and a word with no definition.
    assert_eq!(fails("< i64 >"), Kind::Mismatch);
    assert_eq!(fails("bb: < i64 > [ ] ; bb"), Kind::Mismatch);
    assert_eq!(fails("1 foo"), Kind::NoWord);
    // A definition that fails is not made.
    let mut s = Session::new();
    s.eval("bb: < i64 > [ 1 + ] ;").unwrap();
    assert!(s.eval("bb: < i64 > [ 1.5 + ] ;").is_err());
    s.eval("1 bb").unwrap();
    assert_eq!(s.machine.stack, [2]);
}

#[test]
fn words_are_evaluated_for_each_use() {
    // `sq` says nothing of its input's type, and each use works its types
    // out there.
    let sq = "sq: [ dup * ] ; ";
    assert_eq!(
        run(&format!("{sq} ss: < i64 > [ sq ] ; 7 ss")).unwrap(),
        [49]
    );
    assert_eq!(
        show(&format!("{sq} ss: < f64 > [ sq ] ; 2.5 ss")),
        "<1> 6.25"
    );
    // Applied to a literal, it folds: `nine` is the constant 9.
    assert_eq!(
        code(&format!("{sq} nine: [ 3 sq ] ;"), "nine"),
        code("", "9")
    );
    // Words apply words.
    assert_eq!(
        show("inc: [ 1 + ] ; inc2: [ inc inc ] ; xx: < f64 > [ inc2 ] ; 0.5 xx"),
        "<1> 2.5"
    );
    // A signature types what its word is given: here a literal becomes money.
    assert_eq!(
        run("fee: < money > [ 1.10 + ] ; total: [ 19.99 fee ] ; total").unwrap(),
        [2109]
    );
    // A word that applies itself, with no case that finishes without doing
    // so, has no types for its results (tests/recursion.rs).
    assert_eq!(fails("rec: [ rec ] ; rec"), Kind::Mismatch);
}

#[test]
fn brackets_annotate_and_stack_words_move_judgments() {
    // A bracket after values types them, the deepest first.
    assert_eq!(run("mm: [ 1 2 < money money > ! + ] ; mm").unwrap(), [300]);
    assert_eq!(fails("1 2 < money i64 > ! +"), Kind::NoWord);
    assert_eq!(run("ss: < i64 > [ 10 swap -+ ] ; 3 ss").unwrap(), [7]);
    // A literal dropped leaves no code.
    assert_eq!(code("kk: [ 1 2 drop ] ;", "kk"), code("", "1"));
    // `dup` copies a literal as a literal, so `1.5 dup +` folds exactly.
    assert_eq!(show("1.5 dup +"), "<1> 3.0");
    // Products of decimals stay exact: 1.10 × 1.10 is 1.2100, 121 cents.
    assert_eq!(run("1.10 1.10 * money!").unwrap(), [121]);
    assert_eq!(show("aa: < f64 f64 > [ * ] ; 2.0 3.5 aa"), "<1> 7.0");
    assert_eq!(run("10 3 -+").unwrap(), [7]);
    // Money is added, not multiplied.
    assert_eq!(fails("bb: < money money > [ * ] ;"), Kind::NoWord);
    // Types are values: `.` builds them and applies them.
    assert_eq!(run("( 7 8 9 ) < 3 i64 vec > ! length").unwrap(), [3]);
    assert_eq!(fails("( 7 8 9 ) < 4 i64 vec > !"), Kind::Mismatch);
    assert_eq!(show("( 7 8 9 ) < i64 ary > !"), "<1> ( 7 8 9 )");
}

#[test]
fn known_values_have_no_code_until_needed() {
    // Values at run time, from an earlier piece of source, and literals
    // placed under them only when an operation needs both.
    let mut s = Session::new();
    s.eval("5").unwrap();
    s.eval("1 swap -+").unwrap();
    assert_eq!(s.machine.stack, [(-4i64) as u64]);
    s.eval("10 20 rot").unwrap();
    assert_eq!(s.show(), "<3> 10 20 -4");
    s.eval("+ +").unwrap();
    assert_eq!(s.show(), "<1> 26");
    // `1 2 swap` moves nothing at run time: the code is two literals.
    assert_eq!(code("", "1 2 swap"), code("", "2 1"));
}

#[test]
fn structured_types_follow_values_through_containers() {
    // The map's type says its values are arrays of i64, so `at` gives an
    // array, and `at` again an i64.
    let h = "hh: < string i64 ary map > [ \"k\" at 1 at 1 + ] ;";
    assert_eq!(run(&format!("{h} {{ \"k\" ( 41 2 ) }} hh")).unwrap(), [42]);
    // Literals take their type from what the container holds.
    let fl = "fl: < string f64 ary map > [ \"k\" at 1 at 1 + ] ;";
    assert_eq!(show(&format!("{fl} {{ \"k\" ( 1.5 ) }} fl")), "<1> 2.5");
    let m = "mm: < string money map > [ \"fee\" at 1.10 + ] ;";
    assert_eq!(
        run(&format!("{m} {{ \"fee\" 19.99 money! }} mm")).unwrap(),
        [2109]
    );
    // Equal types built apart are one type; different ones do not mix.
    assert_eq!(
        run("cc: < i64 ary i64 ary > [ concat length ] ; ( 1 2 ) ( 3 ) cc").unwrap(),
        [3]
    );
    assert_eq!(fails("cc: < i64 ary f64 ary > [ concat ] ;"), Kind::NoWord);
    assert_eq!(fails("kk: < i64 ary > [ \"x\" at ] ;"), Kind::NoWord);
    // Strings, with spaces, and their own operations.
    assert_eq!(
        show("ss: < string > [ \"!\" concat ] ; \"hi\" ss"),
        "<1> \"hi!\""
    );
    assert_eq!(run("\"a b c\" length").unwrap(), [5]);
    assert_eq!(fails("ss: < string > [ 1 + ] ;"), Kind::NoWord);
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
        run("vv: < 3 i64 vec > [ length ] ; ( 7 8 9 ) vv").unwrap(),
        [3]
    );
    assert_eq!(
        code("vv: < 3 i64 vec > [ length ] ;", "( 7 8 9 ) vv"),
        code("", "( 7 8 9 ) drop 3")
    );
    // Two vecs concatenate to a vec as long as both.
    assert_eq!(
        run("cc: < 2 i64 vec 3 i64 vec > [ concat length ] ; ( 1 2 ) ( 3 4 5 ) cc").unwrap(),
        [5]
    );
    // A literal index past the end is refused at compile time.
    assert_eq!(fails("bb: < 3 i64 vec > [ 5 at ] ;"), Kind::Mismatch);
    // A value left in a bracket is a value pattern, so it cannot be an
    // output (tests/choice.rs).
    assert_eq!(fails("bb: < >< 2 > [ ] ;"), Kind::Mismatch);
}

#[test]
fn signatures_promise_outputs() {
    // After `--`, the outputs, which are checked.
    let fee = "fee: < money >< money > [ 1.10 + ] ;";
    assert_eq!(run(&format!("{fee} 19.99 fee")).unwrap(), [2109]);
    assert_eq!(fails("bb: < i64 >< f64 > [ 1 + ] ;"), Kind::Mismatch);
    assert_eq!(fails("bb: < i64 >< i64 i64 > [ 1 + ] ;"), Kind::Mismatch);
    // A literal result takes the type promised.
    assert_eq!(show("two: < >< f64 > [ 2 ] ; two"), "<1> 2.0");
}

#[test]
fn clauses_are_chosen_by_types() {
    let d = "dd: < i64 > [ 2 * ] ; dd: < string > [ dup concat ] ;";
    assert_eq!(
        show(&format!(
            "{d} aa: < i64 > [ dd ] ; bb: < string > [ dd ] ; 21 aa \"ab\" bb"
        )),
        "<2> 42 \"abab\""
    );
    // The most specific clause wins; a type variable matches anything.
    let first = "first: < a ary > [ 1 at ] ; first: < i64 ary > [ 2 at ] ;";
    assert_eq!(
        show(&format!("{first} ( 5 6 ) first ( 1.5 2.5 ) first")),
        "<2> 6 1.5"
    );
    // Equally specific clauses tie.
    let w = "ww: < i64 a > [ drop drop 1 ] ; ww: < a i64 > [ drop drop 2 ] ;";
    assert_eq!(
        fails(&format!("{w} xx: < i64 i64 > [ ww ] ;")),
        Kind::Mismatch
    );
    // A literal prefers a clause of its default type, then one it converts to.
    let fee = "fee: < money > [ 1.10 + ] ; fee: < i64 > [ 1 + ] ;";
    assert_eq!(run(&format!("{fee} 5 fee 5.5 fee")).unwrap(), [6, 660]);
    // A family with no clause for the values is no word.
    assert_eq!(fails("inc: < f64 > [ 1 + ] ; 1 i64! inc"), Kind::NoWord);
    // A clause with the same inputs as one before replaces it.
    assert_eq!(
        run("ff: < i64 > [ 1 + ] ; ff: < i64 > [ 2 + ] ; 1 ff").unwrap(),
        [3]
    );
}

#[test]
fn array_literals_share_one_type_or_are_tuples() {
    assert_eq!(show("( 1 2 3 )"), "<1> ( 1 2 3 )");
    assert_eq!(show("( 1 2.5 )"), "<1> ( 1.0 2.5 )");
    assert_eq!(run("( ( 1 2 ) ( 3 ) ) length").unwrap(), [2]);
    assert_eq!(show("( ( 1 2 ) ( 3 ) )"), "<1> ( ( 1 2 ) ( 3 ) )");
    assert_eq!(fails("( )"), Kind::Mismatch);
    // Elements of different types are a tuple (doc/design/TYPES.md 2.5).
    assert_eq!(show("( 1 \"a\" )"), "<1> ( 1 \"a\" )");
    assert_eq!(show("( 1 i64! \"a\" ) length"), "<1> 2");
    // An element may not take values from outside its literal.
    assert_eq!(fails("1 ( 2 + )"), Kind::Mismatch);
}

#[test]
fn over_and_rot_move_judgments() {
    assert_eq!(run("1 2 3 rot").unwrap(), [2, 3, 1]);
    assert_eq!(run("1 2 over").unwrap(), [1, 2, 1]);
    assert_eq!(show("oo: < f64 i64 > [ over ] ; 1.5 2 oo"), "<3> 1.5 2 1.5");
}

#[test]
fn dip_sets_a_value_aside() {
    // `x q dip`, written `<>`: q applied below x, which is put back on top,
    // out of q's reach.
    assert_eq!(run("1 2 [ dup ] <>").unwrap(), [1, 1, 2]);
    assert_eq!(run("1 2 3 [ + ] dip").unwrap(), [3, 3]);
    assert_eq!(fails("1 [ dup ] dip"), Kind::Mismatch);
    // Words over it, in the core vocabulary.
    assert_eq!(run("5 [ 1 + ] keep").unwrap(), [6, 5]);
    assert_eq!(run("3 [ dup * ] [ 1 + ] bi").unwrap(), [9, 4]);
    // A word's effect counts dip's quotation and one more.
    assert_eq!(show("small: [ [ 10 ] <> gt? ] ; 3 small?"), "<1> true");
    // At run time x waits on the scratch stack, and `over` and `rot`, so
    // defined, are still the machine's one operation each.
    use Primitive::*;
    assert_eq!(
        code("5", "[ 1 + ] keep"),
        [
            Op::Prim(Dup),
            Op::Prim(ScratchPush),
            Op::Lit(1),
            Op::Prim(IAdd),
            Op::Prim(ScratchPop),
            Op::Return
        ]
    );
    assert_eq!(code("1 2", "over"), [Op::Prim(Over), Op::Return]);
    assert_eq!(code("1 2 3", "rot"), [Op::Prim(Rot), Op::Return]);
}

#[test]
fn errors_say_where() {
    let mut s = Session::new();
    let e = s.eval("1 2 +'\n  \"a\" +").unwrap_err();
    assert_eq!(e.kind, Kind::NoWord);
    assert_eq!(e.to_string(), "2:7: no word `+` for +' string");
    let e = s.eval("ff: < i64 > [ \"x\" + ] ;").unwrap_err();
    assert_eq!(
        e.to_string(),
        "1:19: no word `+` for i64 string\n  in `ff`, defined at 1:1"
    );
}

#[test]
fn printing() {
    let mut s = Session::new();
    s.eval("\"héllo\\n;\" print 42 print 2.5 print").unwrap();
    assert_eq!(String::from_utf8(s.take_output()).unwrap(), "héllo\n422.5");
    assert_eq!(s.show(), "<0>");
}
