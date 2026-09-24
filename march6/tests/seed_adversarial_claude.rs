//! Adversarial B0c seed tests from @march-claude.
//!
//! The central test is differential: randomly generated definitions are
//! compiled by the graph-defined seed and by an independent reference compiler
//! written here (a symbolic stack that builds the expected graph directly),
//! and evaluated against a plain FORTH-style stack interpreter.  Other tests
//! check name-first/Forth equivalence, save/reload at every token boundary of
//! generated programs, totality over random token soup, budget monotonicity,
//! and the documented no-capture boundary.

use march_research::{
    Atom, Cid, Image, Node, ReduceError, Store,
    seed::{Seed, Syntax, resume},
};

const WORK: usize = 50_000_000;

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
// Seed harness.

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

fn error_of(store: &Store, state: Cid) -> Option<String> {
    match store.get(field(store, state, "error")) {
        Some(Node::Const(Atom::Unit)) => None,
        Some(Node::Const(Atom::Text(message))) => Some(message.clone()),
        other => panic!("malformed error field {other:?}"),
    }
}

fn top_int(store: &Store, state: Cid) -> i64 {
    let Some(Node::Pair(cell, _)) = store.get(field(store, state, "stack")) else {
        panic!("empty stack");
    };
    match store.get(field(store, *cell, "value")) {
        Some(Node::Const(Atom::Int(value))) => *value,
        other => panic!("top of stack is not an integer: {other:?}"),
    }
}

fn definition(store: &Store, state: Cid, name: &str) -> Cid {
    field(
        store,
        field(store, field(store, state, "dictionary"), name),
        "value",
    )
}

fn start(source: &str, syntax: Syntax) -> (Store, Seed, Cid) {
    let mut store = Store::new();
    let seed = Seed::build(&mut store, syntax);
    let text = store.intern(Node::Const(Atom::Text(source.into())));
    let state = seed.state(&mut store, text);
    (store, seed, state)
}

fn run(source: &str, syntax: Syntax) -> (Store, Result<Cid, ReduceError>) {
    let (mut store, seed, state) = start(source, syntax);
    let result = resume(&mut store, seed.runner, state, 100_000, WORK).map(|r| r.root);
    (store, result)
}

// ---------------------------------------------------------------------------
// Generated programs and independent references.

#[derive(Clone, Debug)]
enum Word {
    Literal(i64),
    Dup,
    Drop,
    Swap,
    Add,
    Mul,
    Call(usize),
}

impl Word {
    fn spelling(&self) -> String {
        match self {
            Self::Literal(value) => value.to_string(),
            Self::Dup => "dup".into(),
            Self::Drop => "drop".into(),
            Self::Swap => "swap".into(),
            Self::Add => "+".into(),
            Self::Mul => "*".into(),
            Self::Call(index) => format!("w{index}"),
        }
    }
}

struct Definition {
    body: Vec<Word>,
    inputs: u16,
}

/// Reference compiler: the documented wiring, built directly as nodes.
/// Parameter zero is the first input pulled from the caller's top of stack;
/// deeper missing inputs receive successively larger indices.  Returns the
/// quotation CID, or None if the body does not leave exactly one result.
fn reference_compile(
    store: &mut Store,
    body: &[Word],
    compiled: &[(Cid, u16)],
) -> Option<(Cid, u16)> {
    let mut stack: Vec<Cid> = Vec::new(); // top is last
    let mut inputs: u16 = 0;
    let mut ensure = |stack: &mut Vec<Cid>, needed: usize, store: &mut Store| {
        while stack.len() < needed {
            stack.insert(0, store.intern(Node::Param(inputs)));
            inputs += 1;
        }
    };
    for word in body {
        match word {
            Word::Literal(value) => stack.push(store.intern(Node::Const(Atom::Int(*value)))),
            Word::Dup => {
                ensure(&mut stack, 1, store);
                stack.push(*stack.last().unwrap());
            }
            Word::Drop => {
                ensure(&mut stack, 1, store);
                stack.pop();
            }
            Word::Swap => {
                ensure(&mut stack, 2, store);
                let len = stack.len();
                stack.swap(len - 1, len - 2);
            }
            Word::Add | Word::Mul => {
                ensure(&mut stack, 2, store);
                let a = stack.pop().unwrap();
                let b = stack.pop().unwrap();
                stack.push(store.intern(if matches!(word, Word::Add) {
                    Node::Add(b, a)
                } else {
                    Node::Mul(b, a)
                }));
            }
            Word::Call(index) => {
                let (function, arity) = compiled[*index];
                ensure(&mut stack, arity.into(), store);
                let arguments = (0..arity).map(|_| stack.pop().unwrap()).collect();
                stack.push(store.intern(Node::Apply {
                    function,
                    arguments,
                }));
            }
        }
    }
    (stack.len() == 1).then(|| {
        (
            store.intern(Node::Quote {
                params: inputs,
                body: stack[0],
            }),
            inputs,
        )
    })
}

/// Reference evaluator: direct FORTH semantics on a concrete stack (top last).
fn reference_eval(body: &[Word], definitions: &[Definition], stack: &mut Vec<i64>) -> Option<()> {
    for word in body {
        match word {
            Word::Literal(value) => stack.push(*value),
            Word::Dup => stack.push(*stack.last()?),
            Word::Drop => {
                stack.pop()?;
            }
            Word::Swap => {
                let len = stack.len();
                if len < 2 {
                    return None;
                }
                stack.swap(len - 1, len - 2);
            }
            Word::Add | Word::Mul => {
                let a = stack.pop()?;
                let b = stack.pop()?;
                stack.push(if matches!(word, Word::Add) {
                    b.checked_add(a)?
                } else {
                    b.checked_mul(a)?
                });
            }
            Word::Call(index) => reference_eval(&definitions[*index].body, definitions, stack)?,
        }
    }
    Some(())
}

fn random_word(rng: &mut Rng, defined: usize) -> Word {
    match rng.below(8) {
        0 | 1 => Word::Literal(rng.below(7) as i64 - 3),
        2 => Word::Dup,
        3 => Word::Drop,
        4 => Word::Swap,
        5 => Word::Add,
        6 => Word::Mul,
        _ if defined > 0 => Word::Call(rng.below(defined)),
        _ => Word::Add,
    }
}

/// Generate definitions whose bodies the reference compiler accepts.
fn generate_program(rng: &mut Rng, store: &mut Store) -> (Vec<Definition>, Vec<(Cid, u16)>) {
    let mut definitions = Vec::new();
    let mut compiled = Vec::new();
    for _ in 0..1 + rng.below(4) {
        loop {
            let body = (0..1 + rng.below(6))
                .map(|_| random_word(rng, definitions.len()))
                .collect::<Vec<_>>();
            if let Some((cid, inputs)) = reference_compile(store, &body, &compiled) {
                compiled.push((cid, inputs));
                definitions.push(Definition { body, inputs });
                break;
            }
        }
    }
    (definitions, compiled)
}

fn source(definitions: &[Definition], syntax: Syntax, invocation: &str) -> String {
    let mut text = String::new();
    for (index, definition) in definitions.iter().enumerate() {
        let body = definition
            .body
            .iter()
            .map(Word::spelling)
            .collect::<Vec<_>>()
            .join(" ");
        match syntax {
            Syntax::NameFirst => text.push_str(&format!("w{index} : ( {body} ) ; ")),
            Syntax::Forth => text.push_str(&format!(": w{index} {body} ; ")),
        }
    }
    text.push_str(invocation);
    text
}

#[test]
fn generated_definitions_match_the_reference_compiler_and_interpreter() {
    let mut rng = Rng(0x0ddb_a11c_0ffe_e000);
    let mut checked_values = 0;
    for sample in 0..60 {
        let mut reference_store = Store::new();
        let (definitions, compiled) = generate_program(&mut rng, &mut reference_store);
        let last = definitions.len() - 1;
        let arguments = (0..definitions[last].inputs)
            .map(|_| rng.below(7) as i64 - 3)
            .collect::<Vec<_>>();
        let invocation = arguments
            .iter()
            .map(i64::to_string)
            .chain([format!("w{last}")])
            .collect::<Vec<_>>()
            .join(" ");

        let mut expected_stack = arguments.clone();
        let expected = reference_eval(&definitions[last].body, &definitions, &mut expected_stack)
            .map(|()| *expected_stack.last().unwrap());

        let mut code_cids = Vec::new();
        for syntax in [Syntax::NameFirst, Syntax::Forth] {
            let text = source(&definitions, syntax, &invocation);
            let (mut store, result) = run(&text, syntax);
            let state = match (result, expected) {
                (Ok(state), _) => state,
                // The reference overflowed; laziness may or may not avoid it.
                (Err(ReduceError::IntegerOverflow(_)), None) => continue,
                (Err(error), _) => panic!("sample {sample} {syntax:?} `{text}`: {error:?}"),
            };
            assert_eq!(
                error_of(&store, state),
                None,
                "sample {sample} {syntax:?} `{text}`"
            );
            // Rebuild the reference graph in this store and compare identities.
            let mut rebuilt = Vec::new();
            for (index, definition) in definitions.iter().enumerate() {
                let (cid, inputs) = reference_compile(&mut store, &definition.body, &rebuilt)
                    .expect("reference accepted this body");
                assert_eq!(inputs, definition.inputs);
                assert_eq!(cid, compiled[index].0, "CID is store-independent");
                assert_eq!(
                    definition_cid(&store, state, index),
                    cid,
                    "sample {sample} {syntax:?} w{index} in `{text}`"
                );
                rebuilt.push((cid, inputs));
            }
            code_cids.push(rebuilt);
            if let Some(expected) = expected {
                assert_eq!(top_int(&store, state), expected, "sample {sample} `{text}`");
                checked_values += 1;
            }
        }
        if code_cids.len() == 2 {
            assert_eq!(
                code_cids[0], code_cids[1],
                "sample {sample}: syntaxes differ"
            );
        }
    }
    assert!(
        checked_values > 80,
        "only {checked_values} evaluations were compared"
    );
}

fn definition_cid(store: &Store, state: Cid, index: usize) -> Cid {
    definition(store, state, &format!("w{index}"))
}

// ---------------------------------------------------------------------------
// Staging over generated programs.

#[test]
fn generated_programs_resume_identically_at_every_token_boundary() {
    let mut rng = Rng(0xb0a7_5eed_0000_0001);
    for sample in 0..4 {
        let mut scratch = Store::new();
        let (definitions, _) = generate_program(&mut rng, &mut scratch);
        let last = definitions.len() - 1;
        let invocation = (0..definitions[last].inputs)
            .map(|index| (index as i64 + 1).to_string())
            .chain([format!("w{last}")])
            .collect::<Vec<_>>()
            .join(" ");
        let text = source(&definitions, Syntax::NameFirst, &invocation);
        let (mut store, seed, initial) = start(&text, Syntax::NameFirst);
        let direct = match resume(&mut store, seed.runner, initial, 100_000, WORK) {
            Ok(reduction) => reduction.root,
            Err(ReduceError::IntegerOverflow(_)) => continue,
            Err(error) => panic!("sample {sample}: {error:?}"),
        };
        for boundary in 0..=text.split_whitespace().count() as u32 {
            let partial = resume(&mut store, seed.runner, initial, boundary, WORK)
                .unwrap()
                .root;
            let image = Image::from_store(&store, &[seed.runner, partial]).unwrap();
            let (image, mut loaded) = Image::parse(image.as_bytes()).unwrap();
            let resumed = resume(&mut loaded, image.roots[0], image.roots[1], 100_000, WORK)
                .unwrap()
                .root;
            assert_eq!(
                resumed, direct,
                "sample {sample} boundary {boundary}: `{text}`"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Totality: arbitrary token soup ends in a state, never an unexpected error.

#[test]
fn token_soup_always_ends_in_a_clean_or_error_state() {
    let vocabulary = [
        ":",
        ";",
        "(",
        ")",
        "quote",
        "dup",
        "drop",
        "swap",
        "+",
        "*",
        "0",
        "1",
        "-2",
        "7",
        "9223372036854775807",
        "x",
        "y",
        "end",
        "begin",
        "((",
        "",
    ];
    let mut rng = Rng(0x500b_5eed_face_0ff5);
    let mut errors = 0;
    let mut clean = 0;
    for sample in 0..300 {
        let text = (0..rng.below(20))
            .map(|_| vocabulary[rng.below(vocabulary.len())])
            .collect::<Vec<_>>()
            .join(" ");
        for syntax in [Syntax::NameFirst, Syntax::Forth] {
            let (mut store, seed, initial) = start(&text, syntax);
            match resume(&mut store, seed.runner, initial, 100_000, WORK) {
                Ok(reduction) => {
                    let state = reduction.root;
                    match error_of(&store, state) {
                        Some(_) => {
                            errors += 1;
                            // A failed state stays failed, unchanged.
                            let again = resume(&mut store, seed.runner, state, 100, WORK)
                                .unwrap()
                                .root;
                            assert_eq!(again, state, "sample {sample} {syntax:?} `{text}`");
                        }
                        None => {
                            clean += 1;
                            // A clean finished state is back in evaluation mode
                            // with no pending definition.
                            assert_eq!(
                                store.get(field(&store, state, "mode")),
                                Some(&Node::Const(Atom::Text("eval".into()))),
                                "sample {sample} {syntax:?} `{text}`"
                            );
                            assert_eq!(
                                store.get(field(&store, state, "name")),
                                Some(&Node::Const(Atom::Unit)),
                                "sample {sample} {syntax:?} `{text}`"
                            );
                        }
                    }
                }
                // Integer overflow is the documented low-level failure.
                Err(ReduceError::IntegerOverflow(_)) => {}
                Err(other) => panic!("sample {sample} {syntax:?} `{text}`: {other:?}"),
            }
        }
    }
    assert!(errors > 100 && clean > 50, "errors {errors}, clean {clean}");
}

// ---------------------------------------------------------------------------
// Semantics claims.

#[test]
fn quotations_capture_nothing_from_the_surrounding_stack() {
    // A value already on the stack when the quotation is written stays data;
    // the quotation still takes both operands as explicit inputs.
    let (store, result) = run("5 sum : ( + ) ; 1 2 sum", Syntax::NameFirst);
    let state = result.unwrap();
    assert_eq!(error_of(&store, state), None);
    assert_eq!(top_int(&store, state), 3);
    let mut reference = Store::new();
    let (expected, inputs) = reference_compile(&mut reference, &[Word::Add], &[]).unwrap();
    assert_eq!(inputs, 2);
    assert_eq!(definition(&store, state, "sum"), expected);
    let Some(Node::Pair(_, below)) = store.get(field(&store, state, "stack")) else {
        panic!("stack");
    };
    let Some(Node::Pair(cell, rest)) = store.get(*below) else {
        panic!("stack");
    };
    assert_eq!(
        store.get(field(&store, *cell, "value")),
        Some(&Node::Const(Atom::Int(5)))
    );
    assert_eq!(store.get(*rest), Some(&Node::Const(Atom::Unit)));
}

#[test]
fn quotations_never_yield_code_or_multiple_values() {
    for text in [
        "square : ( dup * ) ; f : ( quote square ) ;",
        "f : ( 1 2 ) ;",
        "f : ( ) ;",
        "g : ( 1 ) ; f : ( g g ) ;",
        "f : ( dup ) ;",
        "f : ( drop ) ;",
    ] {
        let (store, result) = run(text, Syntax::NameFirst);
        let state = result.unwrap();
        assert!(error_of(&store, state).is_some(), "`{text}` succeeded");
    }
}

#[test]
fn budget_sweep_is_monotone_and_never_changes_the_result() {
    let text = "square : ( dup * ) ; answer : 6 7 * ; answer square";
    let (mut store, seed, initial) = start(text, Syntax::NameFirst);
    let full = resume(&mut store, seed.runner, initial, 1_000, WORK).unwrap();
    let needed = full.stats.steps;
    let mut succeeded = false;
    for budget in (0..needed + 200).step_by((needed / 60).max(1)) {
        match resume(&mut store, seed.runner, initial, 1_000, budget) {
            Ok(reduction) => {
                assert_eq!(reduction.root, full.root, "budget {budget}");
                succeeded = true;
            }
            Err(ReduceError::BudgetExhausted { .. }) => {
                assert!(!succeeded, "budget {budget} failed after a success");
            }
            Err(other) => panic!("budget {budget}: {other:?}"),
        }
    }
    assert!(succeeded);
}

#[test]
fn retained_store_nodes_per_token_are_reported() {
    // Tracked metric, not an assertion: the reference reducer keeps every
    // intermediate reader state in the persistent store.
    for calls in [25, 50, 100] {
        let text = format!("square : ( dup * ) ; {}", "2 square drop ".repeat(calls));
        let tokens = text.split_whitespace().count();
        let mut store = Store::new();
        let seed = Seed::build(&mut store, Syntax::NameFirst);
        let before = store.len();
        let source = store.intern(Node::Const(Atom::Text(text)));
        let initial = seed.state(&mut store, source);
        let result = resume(&mut store, seed.runner, initial, 100_000, WORK).unwrap();
        let live = Image::from_store(&store, &[seed.runner, result.root])
            .unwrap()
            .as_bytes()
            .len();
        eprintln!(
            "tokens={tokens} retained-nodes={} per-token={:.1} steps={} live-image-bytes={live}",
            store.len() - before,
            (store.len() - before) as f64 / tokens as f64,
            result.stats.steps,
        );
    }
}
