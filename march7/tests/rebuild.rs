//! The system layer rebuilds itself from March source.
//!
//! Generation 0 is the assembled instruction listing. Every later generation is
//! produced by booting the previous image and compiling `seed/system.march`,
//! which leaves its `boot` token on the stack for export. From generation 1 on,
//! nothing but images and March source is involved.
#[path = "../tools/assembler.rs"]
mod assembler;
use march7::{Blob, Driver, Image};
use std::collections::BTreeSet;

const SYSTEM: &str = include_str!("../seed/system.march");

fn generation_zero() -> Image {
    assembler::assemble(include_str!("../seed/system.asm")).unwrap()
}

/// Step budget for compiling the system source in these tests. Generation 0
/// takes about 15 million steps since its `find` became hashed (2026-10-06);
/// it took 93 million when it scanned the dictionary linearly. This budget is
/// only a safety net; compile cost is tracked by
/// `compiling_the_system_stays_under_its_step_canary`.
const REBUILD_FUEL: u64 = 30_000_000;

/// Compile-cost canary: steps for a rebuilt system (generation 1) to compile
/// the system source. It is 6.68 million today, after the staged-types
/// prototype added 210 lines (the canary went from 6.5 to 7.5 million then,
/// the cost per line unchanged); 6.23 million after the second string slice;
/// 5.51 million after the byte primitives (62 to 67), down from
/// 8.90 when they took the per-byte loops out of reading words, finding
/// them, skipping comments and parsing integers. Before that, checker
/// slices, typing, arrays, strings and maps had grown it from 3.8 million,
/// in proportion to the code they added. Exceeding it means compile cost
/// grew; look at why before raising it (docs/REBUILD.md).
const COMPILE_STEP_CANARY: u64 = 7_500_000;

/// Boot `image`, compile the system source, and export the image it defines.
fn rebuild(image: &Image) -> Image {
    let mut d = Driver::boot(image).unwrap();
    d.fuel = REBUILD_FUEL;
    d.evaluate(SYSTEM).unwrap();
    let boot = d
        .machine
        .stack
        .pop()
        .expect("system source leaves boot token");
    assert!(
        d.machine.stack.is_empty(),
        "system source leaves only its entry"
    );
    d.system_image(boot).unwrap()
}

/// The SHA-256 of the image assembled from `seed/system.asm`. The listing is
/// frozen: its only job is to reproduce generation 0. Changing this value is a
/// foundation change (docs/FOUNDATION.md, "Bounded generation-zero assembler").
/// It changed once, on 2026-10-06, when the listing's dictionary became
/// hashed; the generations built from it did not change.
const GENERATION_ZERO_SHA256: &str =
    "578a24245593faf3fc12bb2c3f171459a89f945e0c46f89bb6dc776956524ad7";

#[test]
fn generation_zero_is_frozen() {
    use sha2::Digest;
    let bytes = generation_zero().encode().unwrap();
    let hash: String = sha2::Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(hash, GENERATION_ZERO_SHA256);
}

fn generations() -> (Image, Image, Image, Image) {
    let g0 = generation_zero();
    let g1 = rebuild(&g0);
    let g2 = rebuild(&Image::decode(&g1.encode().unwrap()).unwrap());
    let g3 = rebuild(&Image::decode(&g2.encode().unwrap()).unwrap());
    (g0, g1, g2, g3)
}

fn run(image: &Image, source: &str) -> Vec<u64> {
    let mut d = Driver::boot(image).unwrap();
    d.evaluate(source).unwrap();
    d.machine.stack.clone()
}

#[test]
fn system_source_reaches_a_byte_identical_fixed_point() {
    let (g0, g1, g2, g3) = generations();
    let (b0, b1, b2, b3) = (
        g0.encode().unwrap(),
        g1.encode().unwrap(),
        g2.encode().unwrap(),
        g3.encode().unwrap(),
    );
    assert_ne!(b0, b1, "generation 1 comes from source, not the listing");
    // Generation 1 is compiled by the frozen generation 0, which emits no
    // tail calls; generation 2 is compiled by the source's own compiler,
    // which does. So the fixed point is reached from generation 2 on, as a
    // bootstrapping compiler compares its second and third stages.
    assert_ne!(b1, b2, "generation 2 is the first compiled by the source");
    assert_eq!(b2, b3);
    assert_eq!(g2.entry, g3.entry);
}

#[test]
fn rebuilt_image_contains_no_generation_zero_code() {
    let (g0, g1, _, _) = generations();
    // Content addressing shares byte-identical objects between generations:
    // name strings, and leaf words such as the one-primitive `dup` wrapper.
    // No shared code object may reference anything, so no part of the
    // listing's call graph can be reached from the rebuilt system.
    let shared: BTreeSet<_> = g1
        .blobs
        .keys()
        .filter(|cid| g0.blobs.contains_key(*cid))
        .collect();
    assert!(!shared.is_empty());
    for cid in shared {
        if let Blob::Code(bytes) = &g1.blobs[cid] {
            let ops = march7::code::decode(bytes).unwrap();
            assert!(
                ops.iter().all(|op| !matches!(
                    op,
                    march7::Op::Call(_)
                        | march7::Op::Quote(_)
                        | march7::Op::Tail(_)
                        | march7::Op::Data(_)
                )),
                "shared code must be a leaf: {ops:?}"
            );
        }
    }
}

#[test]
fn export_leaves_out_compile_time_only_code() {
    // The export holds only what `boot` reaches. Everything the compiling
    // session published, including compile-time helpers such as the phase-1
    // `[` `]` `prim,` and the primitive emitters, is a strict superset.
    let mut d = Driver::boot(&generation_zero()).unwrap();
    d.fuel = REBUILD_FUEL;
    d.evaluate(SYSTEM).unwrap();
    let boot = d.machine.stack.pop().unwrap();
    let exported = d.system_image(boot).unwrap();
    let everything = d.snapshot().unwrap();
    assert!(exported.blobs.len() < everything.blobs.len());
    assert!(
        exported
            .blobs
            .keys()
            .all(|cid| everything.blobs.contains_key(cid))
    );
}

#[test]
fn rebuilt_system_compiles_and_runs_programs() {
    let (_, g1, _, _) = generations();
    assert_eq!(run(&g1, ": square dup u* ; 7 square"), [49]);
    assert_eq!(
        run(&g1, include_str!("../examples/compiler.march")),
        [49, 42]
    );
    // Control flow is ordinary March, defined in the system source.
    assert_eq!(
        run(
            &g1,
            ": abs-diff over over ult? if swap then u- ; 3 10 abs-diff 10 3 abs-diff"
        ),
        [7, 7]
    );
    assert_eq!(
        run(
            &g1,
            ": sum 0 swap cycle dup while swap over u+ swap 1 u- repeat drop ; 4 sum"
        ),
        [10]
    );
    assert_eq!(
        run(
            &g1,
            ": fact dup 1 ult? if drop 1 exit then dup 1 u- recur u* ; 10 fact"
        ),
        [3628800]
    );
    assert_eq!(run(&g1, "-5 +7"), [(-5i64) as u64, 7]);
}

#[test]
fn rebuilt_system_saves_and_reloads_its_dictionary() {
    let (_, g1, _, _) = generations();
    let mut d = Driver::boot(&g1).unwrap();
    d.evaluate(": square dup u* ; : cube dup square u* ;")
        .unwrap();
    let saved = Image::decode(&d.snapshot().unwrap().encode().unwrap()).unwrap();
    assert_eq!(run(&saved, "3 cube 4 square"), [27, 16]);
}

#[test]
fn rebuilt_system_recovers_from_failed_definitions() {
    let (_, g1, _, _) = generations();
    let mut d = Driver::boot(&g1).unwrap();
    d.evaluate(": square dup u* ;").unwrap();
    assert!(d.evaluate(": broken 1 nosuchword ;").is_err());
    assert!(d.evaluate("99999999999999999999").is_err());
    d.evaluate("7 square").unwrap();
    assert_eq!(d.machine.stack, [49]);
    let saved = Image::decode(&d.snapshot().unwrap().encode().unwrap()).unwrap();
    assert!(Driver::boot(&saved).unwrap().evaluate("broken").is_err());
}

fn signed(values: &[u64]) -> Vec<i64> {
    values.iter().map(|&n| n as i64).collect()
}

fn trap(image: &Image, source: &str) -> march7::Error {
    Driver::boot(image).unwrap().evaluate(source).unwrap_err()
}

#[test]
fn scratch_stack_and_reentrant_evaluate() {
    use march7::Error::{Stack, User};
    let (_, g1, _, _) = generations();
    assert_eq!(run(&g1, ": t 1 >r 2 >r r@ r> r> ; t"), [2, 2, 1]);
    // Returning discards whatever a word left on its scratch frame.
    assert_eq!(run(&g1, ": leave 1 >r 2 ; leave 3"), [2, 3]);
    assert_eq!(
        trap(&g1, ": leave 5 >r ; : take r> ; : both leave take ; both"),
        Stack
    );
    // A callee cannot read its caller's scratch values.
    assert_eq!(trap(&g1, ": peek r@ ; : outer 7 >r peek r> ; outer"), Stack);
    // The scratch words are compile-only.
    assert_eq!(trap(&g1, "1 >r"), User(13));
    // A word may evaluate text while its own input is being interpreted.
    assert_eq!(
        run(&g1, ": run s\" 21 2 u*\" nip evaluate ; run 5"),
        [42, 5]
    );
    // Evaluating while compiling extends the definition in progress: a macro.
    assert_eq!(
        run(
            &g1,
            ": sq-body s\" dup u*\" nip evaluate ; immediate : sq sq-body ; 7 sq"
        ),
        [49]
    );
}

#[test]
fn tail_calls_discard_the_finished_words_scratch_values() {
    use march7::{Machine, Op, Primitive};
    let mut m = Machine::empty();
    let take = m
        .define(&[Op::Prim(Primitive::ScratchPop), Op::Return])
        .unwrap();
    let take = m.cid(take).unwrap();
    let leave = m
        .define(&[Op::Lit(1), Op::Prim(Primitive::ScratchPush), Op::Tail(take)])
        .unwrap();
    assert_eq!(m.run(leave, 100), Err(march7::Error::Stack));
    let keep = m
        .define(&[
            Op::Lit(1),
            Op::Prim(Primitive::ScratchPush),
            Op::Prim(Primitive::ScratchPop),
            Op::Return,
        ])
        .unwrap();
    m.run(keep, 100).unwrap();
    assert_eq!(m.stack, [1]);
}

#[test]
fn signed_arithmetic_is_checked() {
    let (_, g1, _, _) = generations();
    assert_eq!(
        signed(&run(
            &g1,
            "-1 1 lt? 1 -1 lt? 3 -7 + 3 -7 - -7 2 / -7 2 mod 6 -7 * -5 abs 5 negate 0 5 *"
        )),
        [1, 0, -4, 10, -3, -1, -42, 5, -5, 0]
    );
    assert_eq!(
        signed(&run(&g1, "-9223372036854775808 1 -1 gt? 2 2 lte? 2 1 gte?")),
        [i64::MIN, 1, 1, 1]
    );
    use march7::Error::User;
    for (source, code) in [
        ("9223372036854775807 1 +", 10),
        ("-9223372036854775808 1 -", 10),
        ("4611686018427387904 2 *", 10),
        ("-9223372036854775808 -1 *", 10),
        ("-9223372036854775808 -1 /", 10),
        ("1 0 /", 11),
        ("5 0 mod", 11),
    ] {
        assert_eq!(trap(&g1, source), User(code), "{source}");
    }
}

#[test]
fn definitions_may_span_inputs() {
    let (_, g1, _, _) = generations();
    let mut d = Driver::boot(&g1).unwrap();
    d.evaluate(": sq").unwrap();
    d.evaluate("dup").unwrap();
    d.evaluate("u* ;").unwrap();
    d.evaluate("9 sq").unwrap();
    assert_eq!(d.machine.stack, [81]);
    // A failure mid-definition abandons only that definition.
    d.evaluate(": broken dup").unwrap();
    assert!(d.evaluate("nosuchword ;").is_err());
    d.evaluate("drop 3 sq").unwrap();
    assert_eq!(d.machine.stack, [9]);
    assert!(d.evaluate("broken").is_err());
}

#[test]
fn primitives_compile_inline() {
    let (_, g1, _, _) = generations();
    let mut d = Driver::boot(&g1).unwrap();
    d.evaluate(": sq dup u* ; ' sq").unwrap();
    let xt = d.machine.stack.pop().unwrap();
    let cid = d.machine.cid(xt).unwrap();
    let image = d.snapshot().unwrap();
    let Blob::Code(bytes) = &image.blobs[&cid] else {
        panic!("sq is code")
    };
    use march7::{Op, Primitive};
    assert_eq!(
        march7::code::decode(bytes).unwrap(),
        [
            Op::Prim(Primitive::Dup),
            Op::Prim(Primitive::Mul),
            Op::Return
        ]
    );
}

#[test]
fn names_hash_exactly_as_merkle_champ_places_keys() {
    let (_, g1, _, _) = generations();
    let names = [
        "dup",
        "",
        "namespace.word",
        "é",
        "a longer name with spaces",
    ];
    let source: String = names
        .iter()
        .map(|n| format!("s\" {n}\" hash-bytes "))
        .collect();
    let hashes = run(&g1, &source);
    let expected: Vec<u64> = names
        .iter()
        .map(|n| merkle_champ::hash_bytes(n.as_bytes()))
        .collect();
    assert_eq!(hashes, expected);
}

#[test]
fn the_hashed_dictionary_keeps_shadowing_across_many_words_and_reloads() {
    let (_, g1, _, _) = generations();
    let mut d = Driver::boot(&g1).unwrap();
    d.fuel = 100_000_000; // 3,000 definitions in one input
    let defs: String = (0..3000).map(|i| format!(": w{i} {i} ; ")).collect();
    d.evaluate(&defs).unwrap();
    d.evaluate(": w7 w7 1000 u+ ; w0 w7 w1234 w2999").unwrap();
    assert_eq!(d.machine.stack, [0, 1007, 1234, 2999]);
    let saved = Image::decode(&d.snapshot().unwrap().encode().unwrap()).unwrap();
    assert_eq!(run(&saved, "w7 w2999 dup"), [1007, 2999, 2999]);
}

#[test]
fn compiling_the_system_stays_under_its_step_canary() {
    let (_, g1, _, _) = generations();
    let mut d = Driver::boot(&g1).unwrap();
    d.fuel = REBUILD_FUEL;
    let before = d.machine.stats.steps;
    d.evaluate(SYSTEM).unwrap();
    let steps = d.machine.stats.steps - before;
    assert!(
        steps < COMPILE_STEP_CANARY,
        "compiling the system took {steps} steps (canary {COMPILE_STEP_CANARY})"
    );
}
