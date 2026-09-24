//! Adversarial B0a reflection tests from @march-claude.
//!
//! These complement `reflection.rs`.  They probe the description boundary
//! with generated code (a describe/intern round-trip property), a matrix of
//! malformed descriptions, budget sweeps, runtime-generated code that tries to
//! capture capabilities, image reload determinism, and randomized mutations.
//! Every failure must be a structured error, must leave no reflected node
//! behind (a pre-normalized description leaves the store exactly as it was),
//! and must never panic.

use march_research::{
    Atom, Bindings, Cid, Clause, Image, Node, ReduceError, Reducer, ReflectError, Store,
};
use std::collections::BTreeMap;

const BUDGET: usize = 1_000_000;

// ---------------------------------------------------------------------------
// Deterministic generator.

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

    fn chance(&mut self, percent: usize) -> bool {
        self.below(100) < percent
    }
}

// ---------------------------------------------------------------------------
// Description builders.

fn int(store: &mut Store, value: i64) -> Cid {
    store.intern(Node::Const(Atom::Int(value)))
}

fn text(store: &mut Store, value: &str) -> Cid {
    store.intern(Node::Const(Atom::Text(value.into())))
}

fn unit(store: &mut Store) -> Cid {
    store.intern(Node::Const(Atom::Unit))
}

fn record(store: &mut Store, fields: Vec<(&str, Cid)>) -> Cid {
    store.intern(Node::Record(
        fields
            .into_iter()
            .map(|(name, value)| (name.to_string(), value))
            .collect(),
    ))
}

fn op(store: &mut Store, operation: &str, fields: Vec<(&str, Cid)>) -> Cid {
    let operation = text(store, operation);
    let mut all = vec![("op", operation)];
    all.extend(fields);
    record(store, all)
}

fn list(store: &mut Store, values: Vec<Cid>) -> Cid {
    let mut tail = unit(store);
    for value in values.into_iter().rev() {
        tail = store.intern(Node::Pair(value, tail));
    }
    tail
}

/// Intern the reflective root first, so that a failed run can be checked for
/// leaving the store exactly as it found it.
fn prepare(store: &mut Store, description: Cid) -> (Cid, usize) {
    let root = store.intern(Node::Intern(description));
    (root, store.len())
}

fn reflect_with(store: &mut Store, root: Cid, budget: usize) -> Result<Cid, ReduceError> {
    Reducer::with_budget(store, &Bindings::new(), budget)
        .run(root)
        .map(|reduction| reduction.root)
}

fn is_code(store: &Store, cid: Cid) -> bool {
    matches!(
        store.get(cid),
        Some(Node::Quote { .. } | Node::Family { .. })
    )
}

// ---------------------------------------------------------------------------
// Generated closed code and a test-only encoder (E2).

#[derive(Clone, Copy)]
struct Scope {
    parameters: u16,
    recur: bool,
}

fn gen_code(store: &mut Store, rng: &mut Rng, depth: usize) -> Cid {
    if rng.chance(50) {
        gen_quote(store, rng, depth)
    } else {
        gen_family(store, rng, depth)
    }
}

fn gen_quote(store: &mut Store, rng: &mut Rng, depth: usize) -> Cid {
    let parameters = rng.below(3) as u16;
    let body = gen_expr(
        store,
        rng,
        depth,
        Scope {
            parameters,
            recur: false,
        },
    );
    store.intern(Node::Quote {
        params: parameters,
        body,
    })
}

fn gen_family(store: &mut Store, rng: &mut Rng, depth: usize) -> Cid {
    let parameters = 1 + rng.below(2) as u16;
    let mut clauses = Vec::new();
    for _ in 0..1 + rng.below(3) {
        let guard = gen_guard(store, rng, depth.min(2), parameters);
        let body = gen_expr(
            store,
            rng,
            depth,
            Scope {
                parameters,
                recur: true,
            },
        );
        clauses.push(Clause { guard, body });
    }
    store.intern(Node::Family {
        parameters,
        clauses,
    })
}

fn gen_leaf(store: &mut Store, rng: &mut Rng, parameters: u16) -> Cid {
    match rng.below(5) {
        0 if parameters > 0 => store.intern(Node::Param(rng.below(parameters.into()) as u16)),
        0 | 1 => int(store, rng.below(7) as i64 - 3),
        2 => store.intern(Node::Const(Atom::Bool(rng.chance(50)))),
        3 => text(store, ["a", "b", "tick"][rng.below(3)]),
        _ => unit(store),
    }
}

/// Guards stay inside the pure subset: no code, calls, effects, or recursion.
fn gen_guard(store: &mut Store, rng: &mut Rng, depth: usize, parameters: u16) -> Cid {
    if depth == 0 || rng.chance(30) {
        return gen_leaf(store, rng, parameters);
    }
    let left = gen_guard(store, rng, depth - 1, parameters);
    let right = gen_guard(store, rng, depth - 1, parameters);
    match rng.below(4) {
        0 => store.intern(Node::Eq(left, right)),
        1 => store.intern(Node::Add(left, right)),
        2 => store.intern(Node::Pair(left, right)),
        _ => {
            let condition = gen_leaf(store, rng, parameters);
            store.intern(Node::If {
                condition,
                when_true: left,
                when_false: right,
            })
        }
    }
}

fn gen_expr(store: &mut Store, rng: &mut Rng, depth: usize, scope: Scope) -> Cid {
    if depth == 0 || rng.chance(20) {
        return gen_leaf(store, rng, scope.parameters);
    }
    let next = depth - 1;
    match rng.below(14) {
        0 => {
            let (a, b) = (
                gen_expr(store, rng, next, scope),
                gen_expr(store, rng, next, scope),
            );
            store.intern(Node::Add(a, b))
        }
        1 => {
            let (a, b) = (
                gen_expr(store, rng, next, scope),
                gen_expr(store, rng, next, scope),
            );
            store.intern(Node::Mul(a, b))
        }
        2 => {
            let (a, b) = (
                gen_expr(store, rng, next, scope),
                gen_expr(store, rng, next, scope),
            );
            store.intern(Node::Eq(a, b))
        }
        3 => {
            let condition = gen_expr(store, rng, next, scope);
            let when_true = gen_expr(store, rng, next, scope);
            let when_false = gen_expr(store, rng, next, scope);
            store.intern(Node::If {
                condition,
                when_true,
                when_false,
            })
        }
        4 => {
            // A deliberately shared child exercises DAG preservation.
            let shared = gen_expr(store, rng, next, scope);
            store.intern(Node::Pair(shared, shared))
        }
        5 => {
            let pair = gen_expr(store, rng, next, scope);
            if rng.chance(50) {
                store.intern(Node::First(pair))
            } else {
                store.intern(Node::Second(pair))
            }
        }
        6 => {
            let names = ["x", "y", "z"];
            let count = rng.below(4);
            let fields = (0..count)
                .map(|index| (names[index].to_string(), gen_expr(store, rng, next, scope)))
                .collect();
            store.intern(Node::Record(fields))
        }
        7 => {
            let record = gen_expr(store, rng, next, scope);
            if rng.chance(50) {
                store.intern(Node::Get {
                    record,
                    field: "x".into(),
                })
            } else {
                let value = gen_expr(store, rng, next, scope);
                store.intern(Node::Put {
                    record,
                    field: "y".into(),
                    value,
                })
            }
        }
        8 => {
            let function = gen_quote(store, rng, next);
            let Some(Node::Quote { params, .. }) = store.get(function).cloned() else {
                unreachable!()
            };
            let arguments = (0..params)
                .map(|_| gen_expr(store, rng, next, scope))
                .collect();
            store.intern(Node::Apply {
                function,
                arguments,
            })
        }
        9 => {
            let family = gen_family(store, rng, next);
            let Some(Node::Family { parameters, .. }) = store.get(family).cloned() else {
                unreachable!()
            };
            let arguments = (0..parameters)
                .map(|_| gen_expr(store, rng, next, scope))
                .collect();
            store.intern(Node::Dispatch { family, arguments })
        }
        10 if scope.recur => {
            let arguments = (0..scope.parameters)
                .map(|_| gen_expr(store, rng, next, scope))
                .collect();
            store.intern(Node::Recur(arguments))
        }
        11 if scope.parameters > 0 => {
            // Effects may consume a parameter; they may not capture a world.
            let token = store.intern(Node::Param(0));
            let message = text(store, "tick");
            store.intern(Node::Emit { token, message })
        }
        12 => gen_code(store, rng, next),
        _ => gen_leaf(store, rng, scope.parameters),
    }
}

/// Test-side inverse of reflection.  Nested code is randomly described either
/// structurally or through `embed`; both must yield the same CID.
/// `mutate_at` corrupts exactly one description record (E10).
struct Describer<'a> {
    rng: &'a mut Rng,
    memo: BTreeMap<Cid, Cid>,
    visited: usize,
    mutate_at: Option<usize>,
}

impl Describer<'_> {
    fn describe(&mut self, store: &mut Store, cid: Cid, root: bool) -> Cid {
        if let Some(done) = self.memo.get(&cid) {
            return *done;
        }
        let node = store.get(cid).cloned().expect("generated node");
        let description = match node {
            Node::Const(Atom::Int(value)) => {
                let value = int(store, value);
                op(store, "int", vec![("value", value)])
            }
            Node::Const(Atom::Bool(value)) => {
                let value = store.intern(Node::Const(Atom::Bool(value)));
                op(store, "bool", vec![("value", value)])
            }
            Node::Const(Atom::Text(value)) => {
                let value = text(store, &value);
                op(store, "text", vec![("value", value)])
            }
            Node::Const(Atom::Unit) => op(store, "unit", vec![]),
            Node::Param(index) => {
                let index = int(store, index.into());
                op(store, "param", vec![("index", index)])
            }
            Node::Add(a, b) => self.binary(store, "add", "left", a, "right", b),
            Node::Mul(a, b) => self.binary(store, "multiply", "left", a, "right", b),
            Node::Eq(a, b) => self.binary(store, "equal", "left", a, "right", b),
            Node::Pair(a, b) => self.binary(store, "pair", "first", a, "second", b),
            Node::If {
                condition,
                when_true,
                when_false,
            } => {
                let condition = self.describe(store, condition, false);
                let when_true = self.describe(store, when_true, false);
                let when_false = self.describe(store, when_false, false);
                op(
                    store,
                    "if",
                    vec![
                        ("condition", condition),
                        ("true", when_true),
                        ("false", when_false),
                    ],
                )
            }
            Node::First(pair) => {
                let pair = self.describe(store, pair, false);
                op(store, "first", vec![("pair", pair)])
            }
            Node::Second(pair) => {
                let pair = self.describe(store, pair, false);
                op(store, "second", vec![("pair", pair)])
            }
            Node::Record(fields) => {
                // Field order in the description list is not semantic.
                let mut entries = Vec::new();
                for (name, value) in fields {
                    let name = text(store, &name);
                    let value = self.describe(store, value, false);
                    entries.push(store.intern(Node::Pair(name, value)));
                }
                if entries.len() > 1 && self.rng.chance(50) {
                    entries.reverse();
                }
                let fields = list(store, entries);
                op(store, "record", vec![("fields", fields)])
            }
            Node::Get { record, field } => {
                let record = self.describe(store, record, false);
                let field = text(store, &field);
                op(store, "get", vec![("record", record), ("field", field)])
            }
            Node::Put {
                record,
                field,
                value,
            } => {
                let record = self.describe(store, record, false);
                let field = text(store, &field);
                let value = self.describe(store, value, false);
                op(
                    store,
                    "put",
                    vec![("record", record), ("field", field), ("value", value)],
                )
            }
            Node::Quote { params, body } if root || self.rng.chance(60) => {
                let parameters = int(store, params.into());
                let body = self.describe(store, body, false);
                op(
                    store,
                    "quote",
                    vec![("parameters", parameters), ("body", body)],
                )
            }
            Node::Family {
                parameters,
                clauses,
            } if root || self.rng.chance(60) => {
                let mut described = Vec::new();
                for clause in clauses {
                    let guard = self.describe(store, clause.guard, false);
                    let body = self.describe(store, clause.body, false);
                    described.push(store.intern(Node::Pair(guard, body)));
                }
                let parameters = int(store, parameters.into());
                let clauses = list(store, described);
                op(
                    store,
                    "family",
                    vec![("parameters", parameters), ("clauses", clauses)],
                )
            }
            Node::Quote { .. } | Node::Family { .. } => op(store, "embed", vec![("value", cid)]),
            Node::Apply {
                function,
                arguments,
            } => {
                let function = self.describe(store, function, false);
                let arguments = self.describe_list(store, arguments);
                op(
                    store,
                    "apply",
                    vec![("function", function), ("arguments", arguments)],
                )
            }
            Node::Dispatch { family, arguments } => {
                let family = self.describe(store, family, false);
                let arguments = self.describe_list(store, arguments);
                op(
                    store,
                    "dispatch",
                    vec![("family", family), ("arguments", arguments)],
                )
            }
            Node::Recur(arguments) => {
                let arguments = self.describe_list(store, arguments);
                op(store, "recur", vec![("arguments", arguments)])
            }
            Node::Emit { token, message } => {
                let token = self.describe(store, token, false);
                let message = self.describe(store, message, false);
                op(store, "emit", vec![("token", token), ("message", message)])
            }
            other => panic!("generator produced an undescribable node {other:?}"),
        };
        let description = self.maybe_mutate(store, description);
        self.memo.insert(cid, description);
        description
    }

    fn binary(
        &mut self,
        store: &mut Store,
        operation: &str,
        left_name: &str,
        left: Cid,
        right_name: &str,
        right: Cid,
    ) -> Cid {
        let left = self.describe(store, left, false);
        let right = self.describe(store, right, false);
        op(
            store,
            operation,
            vec![(left_name, left), (right_name, right)],
        )
    }

    fn describe_list(&mut self, store: &mut Store, values: Vec<Cid>) -> Cid {
        let described = values
            .into_iter()
            .map(|value| self.describe(store, value, false))
            .collect();
        list(store, described)
    }

    fn maybe_mutate(&mut self, store: &mut Store, description: Cid) -> Cid {
        let index = self.visited;
        self.visited += 1;
        if self.mutate_at != Some(index) {
            return description;
        }
        mutate(store, self.rng, description)
    }
}

/// Apply one adversarial corruption to a description record.
fn mutate(store: &mut Store, rng: &mut Rng, description: Cid) -> Cid {
    let Some(Node::Record(mut fields)) = store.get(description).cloned() else {
        unreachable!("descriptions are records")
    };
    let junk_values = [
        int(store, -1),
        int(store, 65_536),
        int(store, i64::MIN),
        int(store, i64::MAX),
        text(store, "0"),
        unit(store),
        store.intern(Node::Const(Atom::Bool(true))),
        store.intern(Node::Const(Atom::Trace(Vec::new()))),
        text(store, &description.to_string()),
    ];
    let junk = junk_values[rng.below(junk_values.len())];
    match rng.below(6) {
        // Drop a non-`op` field, or `op` itself.
        0 => {
            let index = rng.below(fields.len());
            fields.remove(index);
        }
        // Add an unexpected field.
        1 => fields.push(("extra".into(), junk)),
        // Replace the operation name, including surface words and near misses.
        2 => {
            let names = [
                "dup", ":", ";", "hole", "trace", "Int", "int ", "", "ref", "cid",
            ];
            let name = text(store, names[rng.below(names.len())]);
            for (field, value) in &mut fields {
                if field == "op" {
                    *value = name;
                }
            }
        }
        // Make `op` a non-text value.
        3 => {
            for (field, value) in &mut fields {
                if field == "op" {
                    *value = junk;
                }
            }
        }
        // Replace some field value with junk (wrong type, range, or a Trace).
        4 => {
            let index = rng.below(fields.len());
            fields[index].1 = junk;
        }
        // Make a list improper, or replace a child with a non-record.
        _ => {
            let index = rng.below(fields.len());
            let improper = store.intern(Node::Pair(junk, junk));
            fields[index].1 = improper;
        }
    }
    store.intern(Node::Record(fields))
}

#[test]
fn e2_generated_closed_code_round_trips_through_descriptions() {
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    for sample in 0..300 {
        let mut store = Store::new();
        let code = gen_code(&mut store, &mut rng, 4);
        // The generator must only produce valid code.
        assert_eq!(
            reflect_with(&mut store, code, BUDGET),
            Ok(code),
            "sample {sample}: generated code is not a valid value"
        );
        let description = Describer {
            rng: &mut rng,
            memo: BTreeMap::new(),
            visited: 0,
            mutate_at: None,
        }
        .describe(&mut store, code, true);
        let (root, _) = prepare(&mut store, description);
        assert_eq!(
            reflect_with(&mut store, root, BUDGET),
            Ok(code),
            "sample {sample}: {} did not round-trip",
            store.format(code)
        );
    }
}

#[test]
fn e2_shared_subdescriptions_preserve_dag_sharing_exactly() {
    let mut store = Store::new();
    let parameter = {
        let index = int(&mut store, 0);
        op(&mut store, "param", vec![("index", index)])
    };
    let sum = op(
        &mut store,
        "add",
        vec![("left", parameter), ("right", parameter)],
    );
    let product = op(&mut store, "multiply", vec![("left", sum), ("right", sum)]);
    let parameters = int(&mut store, 1);
    let quote = op(
        &mut store,
        "quote",
        vec![("parameters", parameters), ("body", product)],
    );

    let p0 = store.intern(Node::Param(0));
    let hand_sum = store.intern(Node::Add(p0, p0));
    let hand_product = store.intern(Node::Mul(hand_sum, hand_sum));
    let expected = store.intern(Node::Quote {
        params: 1,
        body: hand_product,
    });
    let (root, before) = prepare(&mut store, quote);
    assert_eq!(reflect_with(&mut store, root, BUDGET), Ok(expected));
    // Everything the reflection produced already existed: no duplicates.
    assert_eq!(store.len(), before);
}

// ---------------------------------------------------------------------------
// Malformed descriptions (E4) and authority (E5).

fn assert_rejected_atomically(
    store: &mut Store,
    description: Cid,
    label: &str,
    expected: impl Fn(&ReduceError) -> bool,
) {
    let (root, before) = prepare(store, description);
    let result = reflect_with(store, root, BUDGET);
    match &result {
        Err(error) if expected(error) => {}
        other => panic!("{label}: unexpected result {other:?}"),
    }
    assert_eq!(
        store.len(),
        before,
        "{label}: rejected reflection left nodes"
    );
}

fn quote_of(store: &mut Store, parameters: i64, body: Cid) -> Cid {
    let parameters = int(store, parameters);
    op(
        store,
        "quote",
        vec![("parameters", parameters), ("body", body)],
    )
}

#[test]
fn e4_malformed_description_matrix_is_rejected_without_residue() {
    let mut store = Store::new();
    let seven = int(&mut store, 7);
    let valid_int = op(&mut store, "int", vec![("value", seven)]);
    let junk_text = text(&mut store, "7");
    let world = store.intern(Node::Const(Atom::Trace(Vec::new())));

    let reflection = |check: fn(&ReflectError) -> bool| move |error: &ReduceError| matches!(error, ReduceError::Reflection(inner) if check(inner));

    // Missing `op`.
    let no_op = record(&mut store, vec![("value", seven)]);
    let body = quote_of(&mut store, 0, no_op);
    assert_rejected_atomically(
        &mut store,
        body,
        "missing op",
        reflection(|e| matches!(e, ReflectError::MissingField { .. })),
    );

    // `op` is not text.
    let bad_op = record(&mut store, vec![("op", seven), ("value", seven)]);
    let body = quote_of(&mut store, 0, bad_op);
    assert_rejected_atomically(
        &mut store,
        body,
        "non-text op",
        reflection(|e| matches!(e, ReflectError::ExpectedText { .. })),
    );

    // Extra field on an otherwise valid description.
    let extra = op(
        &mut store,
        "int",
        vec![("value", seven), ("comment", junk_text)],
    );
    let body = quote_of(&mut store, 0, extra);
    assert_rejected_atomically(
        &mut store,
        body,
        "extra field",
        reflection(|e| matches!(e, ReflectError::UnexpectedFields { .. })),
    );

    // Missing required field.
    let missing = op(&mut store, "add", vec![("left", valid_int)]);
    let body = quote_of(&mut store, 0, missing);
    assert_rejected_atomically(
        &mut store,
        body,
        "missing field",
        reflection(|e| matches!(e, ReflectError::MissingField { .. })),
    );

    // Wrong primitive types.
    let text_int = op(&mut store, "int", vec![("value", junk_text)]);
    let body = quote_of(&mut store, 0, text_int);
    assert_rejected_atomically(
        &mut store,
        body,
        "int from text",
        reflection(|e| matches!(e, ReflectError::ExpectedInteger { .. })),
    );
    let text_arity = op(
        &mut store,
        "quote",
        vec![("parameters", junk_text), ("body", valid_int)],
    );
    assert_rejected_atomically(
        &mut store,
        text_arity,
        "arity from text",
        reflection(|e| matches!(e, ReflectError::ExpectedInteger { .. })),
    );

    // Boundary indices: only 0..=65535 fit, and must be in scope.
    for bad in [-1, 65_536, i64::MIN, i64::MAX] {
        let index = int(&mut store, bad);
        let param = op(&mut store, "param", vec![("index", index)]);
        let body = quote_of(&mut store, 1, param);
        assert_rejected_atomically(
            &mut store,
            body,
            "param index range",
            reflection(|e| matches!(e, ReflectError::IntegerOutOfRange { .. })),
        );
    }
    let index = int(&mut store, 65_535);
    let param = op(&mut store, "param", vec![("index", index)]);
    let body = quote_of(&mut store, 1, param);
    assert_rejected_atomically(&mut store, body, "param out of scope", |e| {
        matches!(e, ReduceError::ParameterOutOfRange { .. })
    });

    // A description child that is not a record, including a raw Trace.
    for child in [seven, junk_text, world] {
        let add = op(
            &mut store,
            "add",
            vec![("left", child), ("right", valid_int)],
        );
        let body = quote_of(&mut store, 0, add);
        assert_rejected_atomically(
            &mut store,
            body,
            "non-record child",
            reflection(|e| matches!(e, ReflectError::ExpectedDescriptionRecord(_))),
        );
    }

    // Improper and wrongly shaped lists.
    let improper = store.intern(Node::Pair(valid_int, seven));
    let q0 = {
        let body = op(&mut store, "unit", vec![]);
        quote_of(&mut store, 0, body)
    };
    let apply = op(
        &mut store,
        "apply",
        vec![("function", q0), ("arguments", improper)],
    );
    let body = quote_of(&mut store, 0, apply);
    assert_rejected_atomically(
        &mut store,
        body,
        "improper argument list",
        reflection(|e| matches!(e, ReflectError::ExpectedList(_))),
    );
    let not_pair = list(&mut store, vec![seven]);
    let bad_record = op(&mut store, "record", vec![("fields", not_pair)]);
    let body = quote_of(&mut store, 0, bad_record);
    assert_rejected_atomically(
        &mut store,
        body,
        "record entry not a pair",
        reflection(|e| matches!(e, ReflectError::ExpectedPair { .. })),
    );
    let numeric_name = store.intern(Node::Pair(seven, valid_int));
    let entries = list(&mut store, vec![numeric_name]);
    let bad_record = op(&mut store, "record", vec![("fields", entries)]);
    let body = quote_of(&mut store, 0, bad_record);
    assert_rejected_atomically(
        &mut store,
        body,
        "record field name not text",
        reflection(|e| matches!(e, ReflectError::ExpectedText { .. })),
    );

    // Duplicate target-record fields, nested inside a family clause body:
    // `Node::canonicalize` would panic if this ever reached `Store::intern`.
    let name = text(&mut store, "same");
    let first = store.intern(Node::Pair(name, valid_int));
    let unit_description = op(&mut store, "unit", vec![]);
    let second = store.intern(Node::Pair(name, unit_description));
    let entries = list(&mut store, vec![first, second]);
    let duplicate = op(&mut store, "record", vec![("fields", entries)]);
    let truth = {
        let value = store.intern(Node::Const(Atom::Bool(true)));
        op(&mut store, "bool", vec![("value", value)])
    };
    let clause = store.intern(Node::Pair(truth, duplicate));
    let clauses = list(&mut store, vec![clause]);
    let parameters = int(&mut store, 1);
    let family = op(
        &mut store,
        "family",
        vec![("parameters", parameters), ("clauses", clauses)],
    );
    assert_rejected_atomically(
        &mut store,
        family,
        "duplicate target record field",
        reflection(|e| matches!(e, ReflectError::DuplicateRecordField(_))),
    );

    // Surface words and graph-only spellings are not operations.
    for word in [":", ";", "dup", "hole", "trace", "ref", "cid", "Int"] {
        let bogus = op(&mut store, word, vec![]);
        let body = quote_of(&mut store, 0, bogus);
        assert_rejected_atomically(
            &mut store,
            body,
            "unknown operation",
            reflection(|e| matches!(e, ReflectError::UnknownOperation(_))),
        );
    }
}

#[test]
fn e4_scope_and_purity_violations_are_rejected_atomically() {
    let mut store = Store::new();
    let unit_description = op(&mut store, "unit", vec![]);
    let no_arguments = unit(&mut store);
    let recur = op(&mut store, "recur", vec![("arguments", no_arguments)]);

    // Recursion in a quotation, and in a quotation nested in a family body.
    let quote = quote_of(&mut store, 0, recur);
    assert_rejected_atomically(&mut store, quote, "recur in quote", |e| {
        matches!(e, ReduceError::UnboundRecursion)
    });
    let truth = {
        let value = store.intern(Node::Const(Atom::Bool(true)));
        op(&mut store, "bool", vec![("value", value)])
    };
    let clause = store.intern(Node::Pair(truth, quote));
    let clauses = list(&mut store, vec![clause]);
    let zero = int(&mut store, 0);
    let family = op(
        &mut store,
        "family",
        vec![("parameters", zero), ("clauses", clauses)],
    );
    assert_rejected_atomically(&mut store, family, "recur under nested quote", |e| {
        matches!(e, ReduceError::UnboundRecursion)
    });

    // Reflection, application, dispatch, and recursion are not pure guards.
    let inner = quote_of(&mut store, 0, unit_description);
    let reflective_guard = op(&mut store, "intern", vec![("description", inner)]);
    let apply_guard = op(
        &mut store,
        "apply",
        vec![("function", inner), ("arguments", no_arguments)],
    );
    for guard in [reflective_guard, apply_guard, recur] {
        let clause = store.intern(Node::Pair(guard, unit_description));
        let clauses = list(&mut store, vec![clause]);
        let family = op(
            &mut store,
            "family",
            vec![("parameters", zero), ("clauses", clauses)],
        );
        assert_rejected_atomically(&mut store, family, "impure guard", |e| {
            matches!(
                e,
                ReduceError::ImpureGuard(_) | ReduceError::UnboundRecursion
            )
        });
    }
}

#[test]
fn e5_knowing_a_cid_grants_no_reference() {
    let mut store = Store::new();
    let unit_description = op(&mut store, "unit", vec![]);
    let held = {
        let body = store.intern(Node::Const(Atom::Unit));
        store.intern(Node::Quote { params: 0, body })
    };
    // The exact CID of existing code, spelled as data, is still just data.
    let spelled = text(&mut store, &held.to_string());
    let embed_text = op(&mut store, "embed", vec![("value", spelled)]);
    assert_rejected_atomically(&mut store, embed_text, "embed spelled CID", |e| {
        matches!(
            e,
            ReduceError::Reflection(ReflectError::ExpectedCodeValue(_))
        )
    });
    let spelled_description = op(&mut store, "text", vec![("value", spelled)]);
    let quote = quote_of(&mut store, 0, spelled_description);
    let (root, _) = prepare(&mut store, quote);
    let reflected = reflect_with(&mut store, root, BUDGET).unwrap();
    let Some(Node::Quote { body, .. }) = store.get(reflected).cloned() else {
        panic!("expected a quotation");
    };
    assert_eq!(
        store.get(body),
        Some(&Node::Const(Atom::Text(held.to_string())))
    );

    // Non-code values cannot be embedded, whatever they are.
    let world = store.intern(Node::Const(Atom::Trace(Vec::new())));
    let seven = int(&mut store, 7);
    let pair = store.intern(Node::Pair(seven, seven));
    for value in [world, seven, pair, unit_description] {
        let embed = op(&mut store, "embed", vec![("value", value)]);
        assert_rejected_atomically(&mut store, embed, "embed non-code", |e| {
            matches!(
                e,
                ReduceError::Reflection(ReflectError::ExpectedCodeValue(_))
            )
        });
    }
}

// ---------------------------------------------------------------------------
// Runtime-generated code: capture of data versus capabilities.

/// `Quote(1, Intern({op: quote, parameters: 0, body: {op: kind, value: $0}}))`
/// generates code at runtime from its argument.
fn generator(store: &mut Store, kind: &str) -> Cid {
    let p0 = store.intern(Node::Param(0));
    let leaf = op(store, kind, vec![("value", p0)]);
    let zero = int(store, 0);
    let description = op(store, "quote", vec![("parameters", zero), ("body", leaf)]);
    let intern = store.intern(Node::Intern(description));
    store.intern(Node::Quote {
        params: 1,
        body: intern,
    })
}

fn apply(store: &mut Store, function: Cid, argument: Cid) -> Result<Cid, ReduceError> {
    let root = store.intern(Node::Apply {
        function,
        arguments: vec![argument],
    });
    reflect_with(store, root, BUDGET)
}

#[test]
fn runtime_generated_code_may_capture_data_but_not_capabilities() {
    let mut store = Store::new();
    let int_generator = generator(&mut store, "int");
    let embed_generator = generator(&mut store, "embed");

    // Data becomes a constant of the generated code: partial evaluation.
    let seven = int(&mut store, 7);
    let expected = store.intern(Node::Quote {
        params: 0,
        body: seven,
    });
    assert_eq!(apply(&mut store, int_generator, seven), Ok(expected));

    // A world passed at runtime cannot become a constant or an embed.
    let world = store.intern(Node::Const(Atom::Trace(Vec::new())));
    assert!(matches!(
        apply(&mut store, int_generator, world),
        Err(ReduceError::Reflection(
            ReflectError::ExpectedInteger { .. }
        ))
    ));
    assert!(matches!(
        apply(&mut store, embed_generator, world),
        Err(ReduceError::Reflection(ReflectError::ExpectedCodeValue(_)))
    ));
    // Ordinary substitution artifacts may remain, but no code capturing the
    // world was ever interned: interning it now must create a new node.
    let before = store.len();
    store.intern(Node::Quote {
        params: 0,
        body: world,
    });
    assert_eq!(store.len(), before + 1, "a world-capturing quote exists");

    // A spelled CID passed at runtime is not a reference either.
    let spelled = text(&mut store, &expected.to_string());
    assert!(matches!(
        apply(&mut store, embed_generator, spelled),
        Err(ReduceError::Reflection(ReflectError::ExpectedCodeValue(_)))
    ));

    // A held code value passed at runtime may be embedded: the argument was
    // handed over, so the capability discipline allows it.
    let wrapped = store.intern(Node::Quote {
        params: 0,
        body: expected,
    });
    assert_eq!(apply(&mut store, embed_generator, expected), Ok(wrapped));
}

// ---------------------------------------------------------------------------
// Budget sweep and rollback (E8).

#[test]
fn e8_every_budget_either_succeeds_identically_or_leaves_no_residue() {
    let mut rng = Rng(0x0123_4567_89ab_cdef);
    for sample in 0..6 {
        let mut generated = Store::new();
        let code = gen_code(&mut generated, &mut rng, 3);
        let image = Image::from_store(&generated, &[code]).unwrap();
        let describe_seed = rng.next();
        // Rebuild the identical code and description in a fresh store for
        // every budget, so each attempt starts from the same state.
        let fresh = |budget: usize| {
            let (_, mut store) = Image::parse(image.as_bytes()).unwrap();
            let mut describe_rng = Rng(describe_seed);
            let description = Describer {
                rng: &mut describe_rng,
                memo: BTreeMap::new(),
                visited: 0,
                mutate_at: None,
            }
            .describe(&mut store, code, true);
            let (root, before) = prepare(&mut store, description);
            let result = reflect_with(&mut store, root, budget);
            (result, store.len(), before)
        };
        let (full, _, _) = fresh(BUDGET);
        assert_eq!(full, Ok(code), "sample {sample}: unbounded reflection");
        let mut succeeded = false;
        for budget in 0..400 {
            let (result, after, before) = fresh(budget);
            match result {
                Ok(root) => {
                    assert_eq!(root, code, "sample {sample} budget {budget}");
                    succeeded = true;
                }
                Err(ReduceError::BudgetExhausted { .. }) => {
                    assert!(
                        !succeeded,
                        "sample {sample}: budget {budget} failed after a smaller one succeeded"
                    );
                    assert_eq!(after, before, "sample {sample} budget {budget} left nodes");
                }
                Err(other) => panic!("sample {sample} budget {budget}: {other:?}"),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Image reload determinism (E9).

#[test]
fn e9_reflection_is_identical_after_an_image_reload() {
    let mut rng = Rng(0xfeed_face_cafe_beef);
    for sample in 0..40 {
        let mut original = Store::new();
        let code = gen_code(&mut original, &mut rng, 3);
        let description = Describer {
            rng: &mut rng,
            memo: BTreeMap::new(),
            visited: 0,
            mutate_at: None,
        }
        .describe(&mut original, code, true);
        let root = original.intern(Node::Intern(description));

        // Save only the suspended reflective root: the expected code value is
        // not necessarily reachable from it.
        let image = Image::from_store(&original, &[root]).unwrap();
        let (_, mut reloaded) = Image::parse(image.as_bytes()).unwrap();

        let here = reflect_with(&mut original, root, BUDGET).unwrap();
        let there = reflect_with(&mut reloaded, root, BUDGET).unwrap();
        assert_eq!(here, code, "sample {sample}");
        assert_eq!(there, code, "sample {sample}");
        assert_eq!(
            Image::from_store(&original, &[here]).unwrap().cid(),
            Image::from_store(&reloaded, &[there]).unwrap().cid(),
            "sample {sample}: reflected images differ"
        );
    }
}

// ---------------------------------------------------------------------------
// Randomized malformed value graphs (E10).

#[test]
fn e10_mutated_descriptions_never_panic_and_fail_atomically() {
    let mut rng = Rng(0xdead_beef_0bad_f00d);
    let mut rejected = 0;
    for sample in 0..400 {
        let mut store = Store::new();
        let code = gen_code(&mut store, &mut rng, 3);
        let mutate_at = Some(rng.below(12));
        let description = Describer {
            rng: &mut rng,
            memo: BTreeMap::new(),
            visited: 0,
            mutate_at,
        }
        .describe(&mut store, code, true);
        let (root, before) = prepare(&mut store, description);
        match reflect_with(&mut store, root, BUDGET) {
            Ok(result) => {
                assert!(is_code(&store, result), "sample {sample}: non-code result");
                // Accepted results are ordinary valid code values.
                assert_eq!(
                    reflect_with(&mut store, result, BUDGET),
                    Ok(result),
                    "sample {sample}"
                );
            }
            Err(ReduceError::BudgetExhausted { .. }) => {
                panic!("sample {sample}: budget exhausted with an ample budget")
            }
            Err(_) => {
                rejected += 1;
                assert_eq!(store.len(), before, "sample {sample}: residue after error");
            }
        }
    }
    // The mutations must actually exercise the error paths.
    assert!(
        rejected > 200,
        "only {rejected} of 400 mutations were rejected"
    );
}
