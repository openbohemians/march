//! Staged types, a prototype (doc/design/TYPES.md): `typed` lowers a body to
//! the explicit form, where `.` applies, and a type stage settles literals by
//! context and leaves plain code; `explicit` takes the explicit form itself.
#[path = "../tools/assembler.rs"]
mod assembler;
use march7::{Driver, Error, Image};

fn system() -> Image {
    let g0 = assembler::assemble(include_str!("../seed/system.asm")).unwrap();
    let mut d = Driver::boot(&g0).unwrap();
    d.fuel = 30_000_000;
    d.evaluate(include_str!("../seed/system.march")).unwrap();
    let boot = d.machine.stack.pop().unwrap();
    d.system_image(boot).unwrap()
}

fn run(image: &Image, source: &str) -> Result<Vec<u64>, Error> {
    let mut d = Driver::boot(image).unwrap();
    d.evaluate(source)?;
    Ok(d.machine.stack.clone())
}

fn show(image: &Image, source: &str) -> String {
    let mut d = Driver::boot(image).unwrap();
    d.evaluate(source).unwrap();
    d.show()
}

/// The code identities of the words whose tokens `source` leaves.
fn cids(image: &Image, source: &str) -> Vec<march7::Cid> {
    let mut d = Driver::boot(image).unwrap();
    d.evaluate(source).unwrap();
    let xts = d.machine.stack.clone();
    xts.iter().map(|&xt| d.machine.cid(xt).unwrap()).collect()
}

#[test]
fn literals_fold_and_default() {
    let g = system();
    assert_eq!(run(&g, "typed two 1 1 + ; two").unwrap(), [2]);
    // Folded at compile time: the word is one literal, the same code as `2`.
    let c = cids(&g, "typed two 1 1 + ; : two2 2 ; ' two ' two2");
    assert_eq!(c[0], c[1]);
    // An integer and a decimal fold exactly; a decimal left over is a float.
    assert_eq!(show(&g, "typed d 1.5 2.25 + ; d"), "<1> 3.75");
    assert_eq!(show(&g, "typed z 1 2 + 3.5 + ; z"), "<1> 6.5");
    assert_eq!(show(&g, "typed n -2.5 1 + ; n"), "<1> -1.5");
    let c = cids(&g, "typed z 1 2 + 3.5 + ; : z2 6.5 ; ' z ' z2");
    assert_eq!(c[0], c[1]);
}

#[test]
fn stage_one_lowers_to_the_explicit_form() {
    let g = system();
    // `typed` adds the `.` that `explicit` writes out: the same code.
    let c = cids(&g, "typed a 1 1 + ; explicit b 1 1 + . ; ' a ' b");
    assert_eq!(c[0], c[1]);
    let c = cids(
        &g,
        "typed a < f64 > 1 + ; explicit b < f64 > 1 + . ; ' a ' b",
    );
    assert_eq!(c[0], c[1]);
    // Applying a type annotates: these literals are i64 before `+` sees them.
    assert_eq!(run(&g, "explicit e 1 i64 . 1 i64 . + . ; e").unwrap(), [2]);
}

#[test]
fn literals_take_their_type_from_context() {
    let g = system();
    // An integer literal beside a float input is a float.
    assert_eq!(show(&g, "typed inc < f64 > 1 + ; 2.5 inc"), "<1> 3.5");
    assert_eq!(show(&g, "typed y < f64 > 1 + 2 + ; 0.5 y"), "<1> 3.5");
    assert_eq!(run(&g, "typed w < i64 > 1 + ; 41 w").unwrap(), [42]);
    // Money is exact cents: a decimal literal becomes cents without passing
    // through a float, and an integer literal is whole units.
    assert_eq!(
        run(&g, "typed fee < money > 1.10 + ; 1999 fee").unwrap(),
        [2109]
    );
    assert_eq!(run(&g, "typed p 19.99 money ; p").unwrap(), [1999]);
    assert_eq!(run(&g, "typed p 19.99 money 1 + ; p").unwrap(), [2099]);
    // Checked integer addition still traps on overflow, at run time.
    assert_eq!(
        run(&g, "typed w < i64 > 1 + ; 9223372036854775807 w"),
        Err(Error::User(10))
    );
}

#[test]
fn impossible_types_are_compile_errors() {
    let g = system();
    // Money holds cents: a third decimal digit cannot be exact.
    assert_eq!(run(&g, "typed b < money > 1.105 + ;"), Err(Error::User(41)));
    // A decimal literal is never an integer.
    assert_eq!(run(&g, "typed b < i64 > 1.5 + ;"), Err(Error::User(41)));
    // No implicit promotion between types.
    assert_eq!(run(&g, "typed b < i64 f64 > + ;"), Err(Error::User(23)));
    assert_eq!(run(&g, "typed b < i64 money > + ;"), Err(Error::User(23)));
    // A type left unapplied at the end, and a word the prototype lacks.
    assert_eq!(run(&g, "explicit b i64 ;"), Err(Error::User(23)));
    assert_eq!(run(&g, "typed b 1 foo ;"), Err(Error::User(40)));
}

#[test]
fn typed_words_are_evaluated_for_each_use() {
    let g = system();
    // `sq` says nothing of its input's type: it has no code of its own, and
    // each typed body that applies it works out its types there.
    let sq = "typed sq dup * ; ";
    assert_eq!(
        run(&g, &format!("{sq} typed s < i64 > sq ; 7 s")).unwrap(),
        [49]
    );
    assert_eq!(
        show(&g, &format!("{sq} typed s < f64 > sq ; 2.5 s")),
        "<1> 6.25"
    );
    assert_eq!(run(&g, &format!("{sq} 5 sq")), Err(Error::User(1)));
    // Applied to a literal, it folds: `nine` is the constant 9.
    let c = cids(
        &g,
        &format!("{sq} typed nine 3 sq ; : nine2 9 ; ' nine ' nine2"),
    );
    assert_eq!(c[0], c[1]);
    // Typed words apply typed words.
    assert_eq!(
        show(
            &g,
            "typed inc 1 + ; typed inc2 inc inc ; typed x < f64 > inc2 ; 0.5 x"
        ),
        "<1> 2.5"
    );
    // A signature types what its word is given: here a literal becomes money.
    assert_eq!(
        run(
            &g,
            "typed fee < money > 1.10 + ; typed total 19.99 fee ; total"
        )
        .unwrap(),
        [2109]
    );
    // A word that applies itself is not supported yet.
    assert_eq!(run(&g, "typed rec rec ;"), Err(Error::User(40)));
}

#[test]
fn brackets_annotate_and_stack_words_move_judgments() {
    let g = system();
    // A bracket after values types them, the deepest first.
    assert_eq!(run(&g, "typed m 1 2 < money money > + ; m").unwrap(), [300]);
    assert_eq!(
        run(&g, "typed m 1 2 < money i64 > + ;"),
        Err(Error::User(23))
    );
    // `swap` and `drop`; a literal dropped right after it is emitted leaves
    // no code.
    assert_eq!(run(&g, "typed s < i64 > 10 swap - ; 3 s").unwrap(), [7]);
    let c = cids(&g, "typed k 1 2 drop ; : k2 1 ; ' k ' k2");
    assert_eq!(c[0], c[1]);
    // `dup` copies a literal as a literal, so `1.5 dup +` folds exactly.
    assert_eq!(show(&g, "typed d 1.5 dup + ; d"), "<1> 3.0");
    // Products of decimals stay exact: 1.10 × 1.10 is 1.2100, 121 cents.
    assert_eq!(run(&g, "typed p 1.10 1.10 * money ; p").unwrap(), [121]);
    assert_eq!(show(&g, "typed a < f64 f64 > * ; 2.0 3.5 a"), "<1> 7.0");
    assert_eq!(run(&g, "typed s 10 3 - ; s").unwrap(), [7]);
    // Money is added, not multiplied.
    assert_eq!(run(&g, "typed b < money money > * ;"), Err(Error::User(23)));
}

#[test]
fn structured_types_follow_values_through_containers() {
    let g = system();
    // The map's type says its values are arrays of i64, so `at` gives an
    // array, and `at` again an i64.
    let h = "typed h < string i64 ary map > \"k\" at 0 at 1 + ;";
    assert_eq!(
        run(&g, &format!("{h} {{ \"k\" ( 41 2 ) }} h")).unwrap(),
        [42]
    );
    // So adding 1 lifts over the array, instead of adding 1 to a handle.
    let bad = "typed b < string i64 ary map > \"k\" at 1 + ;";
    assert_eq!(
        show(&g, &format!("{bad} {{ \"k\" ( 1 2 ) }} b")),
        "<1> ( 2 3 )"
    );
    // Literals take their type from what the container holds.
    let fl = "typed fl < string f64 ary map > \"k\" at 0 at 1 + ;";
    assert_eq!(show(&g, &format!("{fl} {{ \"k\" ( 1.5 ) }} fl")), "<1> 2.5");
    let m = "typed m < string money map > \"fee\" at 1.10 + ;";
    assert_eq!(
        run(&g, &format!("{m} {{ \"fee\" 1999 }} m")).unwrap(),
        [2109]
    );
    // Equal types built apart are one type; different ones do not mix.
    assert_eq!(
        run(
            &g,
            "typed c < i64 ary i64 ary > concat length ; ( 1 2 ) ( 3 ) c"
        )
        .unwrap(),
        [3]
    );
    assert_eq!(
        run(&g, "typed c < i64 ary f64 ary > concat ;"),
        Err(Error::User(23))
    );
    assert_eq!(
        run(&g, "typed k < i64 ary > \"x\" at ;"),
        Err(Error::User(23))
    );
    // Strings, with spaces, and their own operations.
    assert_eq!(
        show(&g, "typed s < string > \"!\" concat ; \"hi\" s"),
        "<1> \"hi!\""
    );
    assert_eq!(run(&g, "typed l \"a b c\" length ; l").unwrap(), [5]);
    assert_eq!(run(&g, "typed s < string > 1 + ;"), Err(Error::User(23)));
}

#[test]
fn a_vec_knows_its_length_at_compile_time() {
    let g = system();
    // `length` of a vec is a constant: the value is dropped.
    assert_eq!(
        run(&g, "typed v < 3 i64 vec > length ; ( 7 8 9 ) v").unwrap(),
        [3]
    );
    let c = cids(&g, "typed v < 3 i64 vec > length ; : v2 drop 3 ; ' v ' v2");
    assert_eq!(c[0], c[1]);
    // Two vecs concatenate to a vec as long as both.
    assert_eq!(
        run(
            &g,
            "typed c < 2 i64 vec 3 i64 vec > concat length ; ( 1 2 ) ( 3 4 5 ) c"
        )
        .unwrap(),
        [5]
    );
    // A literal index past the end is refused at compile time.
    assert_eq!(
        run(&g, "typed b < 3 i64 vec > 5 at ;"),
        Err(Error::User(23))
    );
    // A bracket must leave types only.
    assert_eq!(run(&g, "typed b < 2 > ;"), Err(Error::User(23)));
}

#[test]
fn headings_give_definitions_their_context() {
    let g = system();
    // A heading's bracket is each definition's signature; `--` divides
    // inputs from outputs, which are checked.
    let fee = "# < money -- money >\ntyped fee 1.10 + ;";
    assert_eq!(run(&g, &format!("{fee} 1999 fee")).unwrap(), [2109]);
    assert_eq!(
        run(&g, "# < i64 -- f64 >\ntyped b 1 + ;"),
        Err(Error::User(23))
    );
    // A heading starts a section even when the one before is empty: `inc`
    // has an f64 clause only, so applying it to an i64 is no word.
    let inc = "# < i64 >\n# < f64 >\ntyped inc 1 + ;";
    assert_eq!(show(&g, &format!("{inc} 2.5 inc")), "<1> 3.5");
    assert_eq!(
        run(&g, &format!("{inc}\n# < i64 >\ntyped u inc ;")),
        Err(Error::User(1))
    );
}

#[test]
fn clauses_are_chosen_by_types() {
    let g = system();
    let d = "# < i64 >\ntyped d 2 * ;\n# < string >\ntyped d dup concat ;\n";
    assert_eq!(
        show(
            &g,
            &format!("{d}# < i64 >\ntyped a d ;\n# < string >\ntyped b d ; 21 a \"ab\" b")
        ),
        "<2> 42 \"abab\""
    );
    // The most specific clause wins; a type variable matches anything.
    let first = "# < a ary >\ntyped first 0 at ;\n# < i64 ary >\ntyped first 1 at ;\n";
    assert_eq!(
        show(
            &g,
            &format!(
                "{first}# < i64 ary >\ntyped f first ;\n# < f64 ary >\ntyped h first ; ( 5 6 ) f ( 1.5 2.5 ) h"
            )
        ),
        "<2> 6 1.5"
    );
    // Equally specific clauses tie.
    let w = "# < i64 a >\ntyped w drop drop 1 ;\n# < a i64 >\ntyped w drop drop 2 ;\n";
    assert_eq!(
        run(&g, &format!("{w}# < i64 i64 >\ntyped x w ;")),
        Err(Error::User(23))
    );
    // A literal prefers a clause of its default type, then one it converts to.
    let fee = "# < money >\ntyped fee 1.10 + ;\n# < i64 >\ntyped fee 1 + ;\n# main\n";
    assert_eq!(
        run(&g, &format!("{fee}typed u 5 fee ; typed v 5.5 fee ; u v")).unwrap(),
        [6, 660]
    );
}

#[test]
fn guards_choose_at_run_time() {
    let g = system();
    let sign = "typed positive? 0 gt? ;\n# < i64 positive? >\ntyped sign drop 1 ;\n\
                # < i64 >\ntyped sign drop 0 ;\n# < i64 >\ntyped s sign ;";
    assert_eq!(run(&g, &format!("{sign} 5 s -3 s")).unwrap(), [1, 0]);
    // No clause whose guard holds is no word, at run time.
    let only = "typed positive? 0 gt? ;\n# < i64 positive? >\ntyped only drop 7 ;\n\
                # < i64 >\ntyped o only ;";
    assert_eq!(run(&g, &format!("{only} 5 o")).unwrap(), [7]);
    assert_eq!(run(&g, &format!("{only} -1 o")), Err(Error::User(1)));
    // A guard over two inputs: the smaller of two, with no `if`.
    let min = "# < i64 i64 lt? >\ntyped mn drop ;\n# < i64 i64 >\ntyped mn swap drop ;\n\
               # < i64 i64 >\ntyped m mn ;";
    assert_eq!(run(&g, &format!("{min} 3 5 m 5 3 m")).unwrap(), [3, 3]);
    // Clauses chosen between at run time must leave the same types.
    let bad = "typed positive? 0 gt? ;\n# < i64 positive? >\ntyped b drop 1.5 ;\n\
               # < i64 >\ntyped b drop 1 ;\n# < i64 >\ntyped c b ;";
    assert_eq!(run(&g, bad), Err(Error::User(23)));
}

#[test]
fn arithmetic_lifts_and_arrays_are_literals() {
    let g = system();
    assert_eq!(
        show(&g, "# < i64 ary >\ntyped l 1 + ; ( 1 2 3 ) l"),
        "<1> ( 2 3 4 )"
    );
    assert_eq!(
        show(&g, "# < f64 ary >\ntyped l 2 * ; ( 1.5 2.5 ) l"),
        "<1> ( 3.0 5.0 )"
    );
    assert_eq!(
        run(&g, "# < money ary >\ntyped l 2 * ;"),
        Err(Error::User(23))
    );
    // Array literals in typed bodies: one element type, literals settling on
    // it, and nesting.
    assert_eq!(show(&g, "typed a ( 1 2 3 ) ; a"), "<1> ( 1 2 3 )");
    assert_eq!(show(&g, "typed a ( 1 2.5 ) ; a"), "<1> ( 1.0 2.5 )");
    assert_eq!(show(&g, "typed a ( 1 2 ) 10 * ; a"), "<1> ( 10 20 )");
    assert_eq!(
        run(&g, "typed a ( ( 1 2 ) ( 3 ) ) length ; a").unwrap(),
        [2]
    );
    assert_eq!(run(&g, "typed a ( ) ;"), Err(Error::User(23)));
}
