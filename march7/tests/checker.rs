//! The stack-effect checker (docs/CHECKER.md), run on a rebuilt system.
#[path = "../tools/assembler.rs"]
mod assembler;
use march7::{Driver, Error, Image};

/// Generation 1: the system compiled from `seed/system.march`.
fn system() -> Image {
    let g0 = assembler::assemble(include_str!("../seed/system.asm")).unwrap();
    let mut d = Driver::boot(&g0).unwrap();
    d.fuel = 80_000_000;
    d.evaluate(include_str!("../seed/system.march")).unwrap();
    let boot = d.machine.stack.pop().unwrap();
    d.system_image(boot).unwrap()
}

/// Defines `source`, then reports the effect of `word` as
/// (inputs, outputs) when known, or Err(reason).
fn effect(image: &Image, source: &str, word: &str) -> Result<(u64, u64), u64> {
    let mut d = Driver::boot(image).unwrap();
    d.evaluate(&format!("{source} ' {word} stack-effect"))
        .unwrap();
    let s = &d.machine.stack;
    let (a, b, known) = (s[s.len() - 3], s[s.len() - 2], s[s.len() - 1]);
    if known == 1 { Ok((a, b)) } else { Err(a) }
}

#[test]
fn straight_line_words_and_calls() {
    let g = system();
    assert_eq!(effect(&g, ": sq dup u* ;", "sq"), Ok((1, 1)));
    assert_eq!(effect(&g, ": two 1 2 ;", "two"), Ok((0, 2)));
    assert_eq!(effect(&g, ": eat drop drop ;", "eat"), Ok((2, 0)));
    assert_eq!(
        effect(&g, ": sq dup u* ; : quad sq sq ;", "quad"),
        Ok((1, 1))
    );
    assert_eq!(effect(&g, ": spin rot rot ;", "spin"), Ok((3, 3)));
}

#[test]
fn branches_loops_exits_traps_and_scratch() {
    let g = system();
    assert_eq!(
        effect(&g, ": sign dup 0= if drop 0 else drop 1 then ;", "sign"),
        Ok((1, 1))
    );
    assert_eq!(
        effect(
            &g,
            ": count 0 swap cycle dup while swap 1 u+ swap 1 u- repeat drop ;",
            "count"
        ),
        Ok((1, 1))
    );
    assert_eq!(
        effect(&g, ": f dup if drop 1 exit then drop 2 ;", "f"),
        Ok((1, 1))
    );
    // A path that traps never returns, so it does not constrain the effect.
    assert_eq!(effect(&g, ": t dup 0= if 99 trap then ;", "t"), Ok((1, 1)));
    assert_eq!(effect(&g, ": s 1 >r 2 r> ;", "s"), Ok((0, 2)));
}

#[test]
fn recursion_is_solved_from_its_base_case() {
    let g = system();
    assert_eq!(
        effect(
            &g,
            ": fact dup 1 ult? if drop 1 exit then dup 1 u- recur u* ;",
            "fact"
        ),
        Ok((1, 1))
    );
    assert_eq!(
        effect(&g, ": down dup 0= if drop exit then 1 u- recur ;", "down"),
        Ok((1, 0))
    );
    assert_eq!(effect(&g, ": forever recur ;", "forever"), Err(7));
    // The recursive path consumes a second input that the base case passes
    // through untouched, so the word takes 2 and leaves 1.
    assert_eq!(
        effect(&g, ": r dup if drop exit then drop drop 0 0 recur ;", "r"),
        Ok((2, 1))
    );
    // Here the code after `recur` reaches a value the base case never takes,
    // so no single effect fits both paths.
    assert_eq!(
        effect(&g, ": r dup if drop exit then 0 recur swap drop ;", "r"),
        Err(8)
    );
}

#[test]
fn quotations_carry_their_effect() {
    let g = system();
    assert_eq!(
        effect(&g, ": sq dup u* ; : app ' sq call ;", "app"),
        Ok((1, 1))
    );
    // A quotation whose origin is unknown makes the effect unknown.
    assert_eq!(effect(&g, ": dyn call ;", "dyn"), Err(1));
}

#[test]
fn unknown_effects_report_their_reason() {
    let g = system();
    assert_eq!(effect(&g, ": bad if 1 then ;", "bad"), Err(3));
    assert_eq!(
        effect(&g, ": grow cycle dup while dup repeat ;", "grow"),
        Err(4)
    );
    assert_eq!(effect(&g, ": g if 1 2 exit then 3 ;", "g"), Err(5));
    assert_eq!(effect(&g, ": u r> ;", "u"), Err(6));
    assert_eq!(effect(&g, ": dyn call ; : uses 1 dyn ;", "uses"), Err(2));
}

#[test]
fn system_words_have_effects_computed_from_their_code() {
    let g = system();
    assert_eq!(effect(&g, "", "dup"), Ok((1, 2)));
    assert_eq!(effect(&g, "", "find"), Ok((3, 1)));
    assert_eq!(effect(&g, "", "word"), Ok((0, 3)));
    assert_eq!(effect(&g, "", "+"), Ok((2, 1)));
    assert_eq!(effect(&g, "", "/"), Ok((2, 1)));
    // `evaluate` runs the interpreter, whose calls are dynamic.
    assert_eq!(effect(&g, "", "evaluate"), Err(2));
}

#[test]
fn checked_mode_rejects_definitions_whose_effect_is_unknown() {
    let g = system();
    let mut d = Driver::boot(&g).unwrap();
    d.evaluate("checked : sq dup u* ; : quad sq sq ; 3 quad")
        .unwrap();
    assert_eq!(d.machine.stack, [81]);
    assert_eq!(d.evaluate(": bad if 1 then ;"), Err(Error::User(103)));
    assert_eq!(d.evaluate(": dyn call ;"), Err(Error::User(101)));
    // A rejected definition is not installed.
    assert_eq!(d.evaluate("bad"), Err(Error::User(1)));
    d.evaluate("unchecked : dyn call ; checked").unwrap();
    assert_eq!(d.evaluate(": uses 1 dyn ;"), Err(Error::User(102)));
}

#[test]
fn effects_are_derived_again_after_reloading() {
    let g = system();
    let mut d = Driver::boot(&g).unwrap();
    d.evaluate(": sq dup u* ; : quad sq sq ;").unwrap();
    let saved = Image::decode(&d.snapshot().unwrap().encode().unwrap()).unwrap();
    // Effects are not stored in images; they are computed from the code.
    assert_eq!(effect(&saved, "", "quad"), Ok((1, 1)));
}
