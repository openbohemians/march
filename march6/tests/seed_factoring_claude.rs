//! Adversarial B0d factoring tests from @march-claude.
//!
//! The concatenative factoring law: cut any contiguous fragment, define it
//! under a new name, and replace the fragment with that name.  Values and
//! error classes must not change.  These tests apply that transformation at
//! random positions inside generated programs, including inside definition
//! bodies and around calls to other words, and replay every pause boundary
//! through an image.  Fragments are limited to the seed's current single
//! integer result.

use march_research::{
    Atom, Cid, Image, Node, ReduceError, Store,
    seed::{Seed, Syntax, resume},
};
use std::collections::BTreeMap;

const WORK: usize = 50_000_000;
const MAX: &str = "9223372036854775807";
const MIN: &str = "-9223372036854775808";

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }
}

// ---------------------------------------------------------------------------
// Harness.

fn field(store: &Store, record: Cid, name: &str) -> Cid {
    let Some(Node::Record(fields)) = store.get(record) else {
        panic!("expected record: {}", store.format(record));
    };
    fields
        .iter()
        .find(|(key, _)| key == name)
        .unwrap_or_else(|| panic!("missing {name}"))
        .1
}

/// Observable outcome: the final stack values (content IDs, comparable across
/// stores), a reader error state, or a nucleus error class.
#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    Values(Vec<Cid>),
    ReaderError(String),
    Nucleus(String),
}

fn outcome_of(store: &Store, state: Cid) -> Outcome {
    if let Some(Node::Const(Atom::Text(message))) = store.get(field(store, state, "error")) {
        return Outcome::ReaderError(message.clone());
    }
    let mut values = Vec::new();
    let mut cursor = field(store, state, "stack");
    while let Some(Node::Pair(cell, rest)) = store.get(cursor) {
        values.push(field(store, *cell, "value"));
        cursor = *rest;
    }
    Outcome::Values(values)
}

fn nucleus(error: ReduceError) -> Outcome {
    match error {
        ReduceError::IntegerOverflow(_) => Outcome::Nucleus("overflow".into()),
        other => panic!("unexpected nucleus error {other:?}"),
    }
}

fn start(source: &str, syntax: Syntax) -> (Store, Seed, Cid) {
    let mut store = Store::new();
    let seed = Seed::build(&mut store, syntax);
    let text = store.intern(Node::Const(Atom::Text(source.into())));
    let state = seed.state(&mut store, text);
    (store, seed, state)
}

fn evaluate(source: &str, syntax: Syntax) -> Outcome {
    let (mut store, seed, state) = start(source, syntax);
    match resume(&mut store, seed.runner, state, u32::MAX, WORK) {
        Ok(reduction) => outcome_of(&store, reduction.root),
        Err(error) => nucleus(error),
    }
}

// ---------------------------------------------------------------------------
// Generated programs.

#[derive(Clone, Debug)]
enum Word {
    Literal(&'static str),
    Dup,
    Drop,
    Swap,
    Add,
    Mul,
    Call(String),
}

impl Word {
    fn spelling(&self) -> &str {
        match self {
            Self::Literal(text) => text,
            Self::Dup => "dup",
            Self::Drop => "drop",
            Self::Swap => "swap",
            Self::Add => "+",
            Self::Mul => "*",
            Self::Call(name) => name,
        }
    }
}

const LITERALS: [&str; 7] = ["0", "1", "-1", "2", "3", MAX, MIN];

/// Stack effect with inferred inputs: (inputs, outputs).
fn effect(words: &[Word], arity: &BTreeMap<String, usize>) -> (usize, usize) {
    let (mut inputs, mut depth) = (0, 0);
    for word in words {
        let (needed, produced) = match word {
            Word::Literal(_) => (0, 1),
            Word::Dup => (1, 2),
            Word::Drop => (1, 0),
            Word::Swap => (2, 2),
            Word::Add | Word::Mul => (2, 1),
            Word::Call(name) => (arity[name], 1),
        };
        if depth < needed {
            inputs += needed - depth;
            depth = needed;
        }
        depth = depth - needed + produced;
    }
    (inputs, depth)
}

fn random_word(rng: &mut Rng, callable: &[String]) -> Word {
    match rng.below(9) {
        0 | 1 => Word::Literal(LITERALS[rng.below(LITERALS.len())]),
        2 => Word::Dup,
        3 => Word::Drop,
        4 => Word::Swap,
        5 => Word::Add,
        6 => Word::Mul,
        _ if !callable.is_empty() => Word::Call(callable[rng.below(callable.len())].clone()),
        _ => Word::Add,
    }
}

/// A body with exactly one result, as the seed's quotations require.
fn single_result_body(
    rng: &mut Rng,
    callable: &[String],
    arity: &BTreeMap<String, usize>,
) -> Vec<Word> {
    let mut body = (0..1 + rng.below(6))
        .map(|_| random_word(rng, callable))
        .collect::<Vec<_>>();
    let (_, mut depth) = effect(&body, arity);
    while depth > 1 {
        body.push(Word::Add);
        depth -= 1;
    }
    if depth == 0 {
        body.push(Word::Literal(LITERALS[rng.below(LITERALS.len())]));
    }
    body
}

#[derive(Clone)]
struct Program {
    /// Definitions in dependency order; each body calls only earlier names.
    definitions: Vec<(String, Vec<Word>)>,
    /// Top-level words, preceded by enough literal inputs.
    prefix: Vec<Word>,
    main: Vec<Word>,
}

impl Program {
    fn arity(&self) -> BTreeMap<String, usize> {
        let mut arity = BTreeMap::new();
        for (name, body) in &self.definitions {
            let (inputs, _) = effect(body, &arity);
            arity.insert(name.clone(), inputs);
        }
        arity
    }

    fn render(&self, syntax: Syntax) -> String {
        let mut text = String::new();
        for (name, body) in &self.definitions {
            let body = body
                .iter()
                .map(Word::spelling)
                .collect::<Vec<_>>()
                .join(" ");
            match syntax {
                Syntax::NameFirst => text.push_str(&format!("{name} : ( {body} ) ; ")),
                Syntax::Forth => text.push_str(&format!(": {name} {body} ; ")),
            }
        }
        for word in self.prefix.iter().chain(&self.main) {
            text.push_str(word.spelling());
            text.push(' ');
        }
        text
    }
}

fn generate(rng: &mut Rng) -> Program {
    let mut definitions = Vec::new();
    let mut arity = BTreeMap::new();
    let mut callable = Vec::new();
    for index in 0..rng.below(4) {
        let body = single_result_body(rng, &callable, &arity);
        let name = format!("w{index}");
        let (inputs, _) = effect(&body, &arity);
        arity.insert(name.clone(), inputs);
        callable.push(name.clone());
        definitions.push((name, body));
    }
    let main = (0..1 + rng.below(8))
        .map(|_| random_word(rng, &callable))
        .collect::<Vec<_>>();
    let (inputs, _) = effect(&main, &arity);
    let prefix = (0..inputs)
        .map(|_| Word::Literal(LITERALS[rng.below(LITERALS.len())]))
        .collect();
    Program {
        definitions,
        prefix,
        main,
    }
}

/// Cut a random single-result fragment out of the main sequence or a
/// definition body, define it under a fresh name just before its container,
/// and replace the fragment with that name.  None if the chosen span cannot
/// be a current seed quotation.
fn factor(rng: &mut Rng, program: &Program, fresh: &str) -> Option<Program> {
    let arity = program.arity();
    let target = rng.below(program.definitions.len() + 1);
    let body = if target == program.definitions.len() {
        &program.main
    } else {
        &program.definitions[target].1
    };
    let start = rng.below(body.len());
    let end = start + 1 + rng.below(body.len() - start);
    let fragment = body[start..end].to_vec();
    if effect(&fragment, &arity).1 != 1 {
        return None;
    }
    let mut replaced = body[..start].to_vec();
    replaced.push(Word::Call(fresh.into()));
    replaced.extend_from_slice(&body[end..]);

    let mut factored = program.clone();
    if target == program.definitions.len() {
        factored.main = replaced;
        factored.definitions.push((fresh.into(), fragment));
    } else {
        factored.definitions[target].1 = replaced;
        factored
            .definitions
            .insert(target, (fresh.into(), fragment));
    }
    Some(factored)
}

// ---------------------------------------------------------------------------
// The factoring law.

#[test]
fn cut_name_replace_anywhere_preserves_values_and_errors() {
    let mut rng = Rng(0xfac7_0a11_5eed_0001);
    let (mut values, mut overflows, mut inside_definitions) = (0, 0, 0);
    let mut applied = 0;
    while applied < 80 {
        let program = generate(&mut rng);
        // Factor repeatedly: each step must preserve the original outcome.
        let original = [Syntax::NameFirst, Syntax::Forth]
            .map(|syntax| evaluate(&program.render(syntax), syntax));
        assert_eq!(
            original[0],
            original[1],
            "{}",
            program.render(Syntax::NameFirst)
        );
        match &original[0] {
            Outcome::Values(_) => values += 1,
            Outcome::Nucleus(_) => overflows += 1,
            Outcome::ReaderError(error) => panic!(
                "generated program failed ({error}): {}",
                program.render(Syntax::NameFirst)
            ),
        }
        let mut current = program;
        for step in 0..3 {
            let before = current.definitions.len();
            let Some(factored) = factor(&mut rng, &current, &format!("f{applied}x{step}")) else {
                continue;
            };
            if factored.definitions.len() > before
                && factored
                    .definitions
                    .last()
                    .is_some_and(|(name, _)| !name.starts_with('f'))
            {
                inside_definitions += 1;
            }
            for (index, syntax) in [Syntax::NameFirst, Syntax::Forth].into_iter().enumerate() {
                let source = factored.render(syntax);
                assert_eq!(
                    evaluate(&source, syntax),
                    original[index],
                    "{syntax:?}\n  original: {}\n  factored: {source}",
                    current.render(syntax)
                );
            }
            applied += 1;
            current = factored;
        }
    }
    assert!(
        values > 20 && overflows > 5,
        "values {values}, overflows {overflows}"
    );
    assert!(
        inside_definitions > 6,
        "only {inside_definitions} in-definition factorings"
    );
}

#[test]
fn inlining_a_call_is_the_inverse_transformation() {
    // Replace every call with its callee's body text: the program flattens to
    // primitives, and nothing observable may change.
    let mut rng = Rng(0x011e_ab1e_5eed_0002);
    let mut compared = 0;
    for _ in 0..40 {
        let program = generate(&mut rng);
        if program.definitions.is_empty() {
            continue;
        }
        let bodies = program
            .definitions
            .iter()
            .cloned()
            .collect::<BTreeMap<_, _>>();
        fn expand(words: &[Word], bodies: &BTreeMap<String, Vec<Word>>) -> Vec<Word> {
            words
                .iter()
                .flat_map(|word| match word {
                    Word::Call(name) => expand(&bodies[name], bodies),
                    other => vec![other.clone()],
                })
                .collect()
        }
        let flat = Program {
            definitions: Vec::new(),
            prefix: program.prefix.clone(),
            main: expand(&program.main, &bodies),
        };
        for syntax in [Syntax::NameFirst, Syntax::Forth] {
            assert_eq!(
                evaluate(&flat.render(syntax), syntax),
                evaluate(&program.render(syntax), syntax),
                "{syntax:?}: {} versus {}",
                program.render(syntax),
                flat.render(syntax)
            );
        }
        compared += 1;
    }
    assert!(compared > 20);
}

// ---------------------------------------------------------------------------
// Pauses never observe; replay through images.

#[test]
fn every_pause_boundary_replays_through_an_image_without_observing() {
    let mut rng = Rng(0xba5e_1a1d_5eed_0003);
    let mut overflowing = 0;
    for sample in 0..8 {
        // Bias toward dropped or demanded extreme arithmetic.
        let mut program = generate(&mut rng);
        program.main.extend([
            Word::Literal(MAX),
            Word::Literal("1"),
            Word::Add,
            if sample % 2 == 0 {
                Word::Drop
            } else {
                Word::Dup
            },
        ]);
        let arity = program.arity();
        let (inputs, _) = effect(&program.main, &arity);
        program.prefix = (0..inputs).map(|_| Word::Literal("1")).collect();
        let source = program.render(Syntax::NameFirst);
        let tokens = source.split_whitespace().count() as u32;
        let direct = evaluate(&source, Syntax::NameFirst);
        if matches!(direct, Outcome::Nucleus(_)) {
            overflowing += 1;
        }
        let (mut store, seed, initial) = start(&source, Syntax::NameFirst);
        for boundary in 0..tokens {
            // A pause before EOF must never observe pending arithmetic.
            let partial = resume(&mut store, seed.runner, initial, boundary, WORK)
                .unwrap_or_else(|error| {
                    panic!("sample {sample}: pause at {boundary} observed: {error:?}\n  {source}")
                })
                .root;
            let image = Image::from_store(&store, &[seed.runner, partial]).unwrap();
            let (image, mut loaded) = Image::parse(image.as_bytes()).unwrap();
            let resumed = match resume(&mut loaded, image.roots[0], image.roots[1], u32::MAX, WORK)
            {
                Ok(reduction) => outcome_of(&loaded, reduction.root),
                Err(error) => nucleus(error),
            };
            assert_eq!(
                resumed, direct,
                "sample {sample} boundary {boundary}: {source}"
            );
        }
    }
    assert!(overflowing >= 3, "only {overflowing} overflowing samples");
}

// ---------------------------------------------------------------------------
// Observation boundaries worth pinning down.

#[test]
fn a_dropped_overflow_is_harmless_inline_and_factored() {
    for (inline, factored) in [
        (
            format!("{MAX} 1 + drop 0"),
            format!("f : ( {MAX} 1 + drop 0 ) ; f"),
        ),
        // An unused argument is not demanded.
        (
            format!("{MAX} 1 + 5 swap drop"),
            format!("g : ( 5 swap drop ) ; {MAX} 1 + g"),
        ),
        (
            format!("{MIN} -1 * drop 0 {MIN} -1 * drop 0 + 7 +"),
            format!("h : ( {MIN} -1 * drop 0 ) ; k : ( h h + 7 + ) ; k"),
        ),
    ] {
        let expected = evaluate(&inline, Syntax::NameFirst);
        assert!(
            matches!(expected, Outcome::Values(_)),
            "{inline}: {expected:?}"
        );
        assert_eq!(
            evaluate(&factored, Syntax::NameFirst),
            expected,
            "{factored}"
        );
    }
}

#[test]
fn a_constant_definition_is_an_observation_boundary_not_a_factoring() {
    // `name : fragment ;` without parentheses binds a computed constant, and
    // `;` observes it.  Moving a fragment into a constant is therefore not a
    // factoring: it can surface an error that the inline program never
    // demands.  This pins the documented boundary so any change is deliberate.
    let inline = format!("{MAX} 1 + drop 0");
    assert!(matches!(
        evaluate(&inline, Syntax::NameFirst),
        Outcome::Values(_)
    ));
    let as_constant = format!("c : {MAX} 1 + ; c drop 0");
    assert_eq!(
        evaluate(&as_constant, Syntax::NameFirst),
        Outcome::Nucleus("overflow".into())
    );
    let as_quotation = format!("c : ( {MAX} 1 + ) ; c drop 0");
    assert_eq!(
        evaluate(&as_quotation, Syntax::NameFirst),
        evaluate(&inline, Syntax::NameFirst)
    );
    // Even an unused constant is observed at its definition.
    let unused = format!("c : {MAX} 1 + ; 0");
    assert_eq!(
        evaluate(&unused, Syntax::NameFirst),
        Outcome::Nucleus("overflow".into())
    );
}

#[test]
fn identical_calls_share_one_demand() {
    // Two calls with equal arguments are one graph, so the second adds only
    // reader work, independent of the body's size.  A call with a different
    // argument must pay for the body again.
    let steps = |source: &str| {
        let (mut store, seed, state) = start(source, Syntax::NameFirst);
        resume(&mut store, seed.runner, state, u32::MAX, WORK)
            .unwrap()
            .stats
            .steps
    };
    let mut extra_same = Vec::new();
    let mut extra_different = Vec::new();
    for size in [100, 400] {
        let definition = format!("big : ( {}) ; ", "1 + ".repeat(size));
        let base = steps(&format!("{definition}0"));
        let one = steps(&format!("{definition}5 big")) - base;
        extra_same.push(steps(&format!("{definition}5 big 5 big +")) - base - one);
        extra_different.push(steps(&format!("{definition}5 big 6 big +")) - base - one);
    }
    assert_eq!(
        extra_same[0], extra_same[1],
        "a repeated identical call grew with the body: {extra_same:?}"
    );
    assert!(
        extra_different[1] > extra_different[0] + 300,
        "a different argument should recompute the body: {extra_different:?}"
    );
}
