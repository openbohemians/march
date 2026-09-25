//! Bounded, dependency-free conventional-engine microbenchmark.
//!
//! Run in release mode, preferably CPU-pinned, with an external timeout:
//! `timeout --kill-after=2s 30s cargo run --offline --release --example fast_bench -- 20000`
//! All timings include the same varied-input loop, black boxes and checksum.
//! These tiny scalar workloads are not an application-performance prediction.

use march_research::fast::{Binary, Context, Executor, Literal, Op, Program, Value, WordId};
use march_research::{Atom, Bindings, Cid, Node, Reducer, Store};
use std::hint::black_box;
use std::time::Instant;

const BATCHES: usize = 7;
const BUDGET: usize = 10_000;

#[derive(Clone, Copy, Debug)]
enum Workload {
    Square,
    Quad,
    Conditional,
    SharedCall,
    ContextFamily,
    Chain64,
    SourceSquare,
}

impl Workload {
    fn expected(self, input: i64, condition: bool) -> i64 {
        match self {
            Self::Square | Self::SourceSquare => input.checked_mul(input).unwrap(),
            Self::Quad => {
                let square = input.checked_mul(input).unwrap();
                square.checked_mul(square).unwrap()
            }
            Self::Conditional => input
                .checked_mul(3)
                .unwrap()
                .checked_add(if condition { 1 } else { 2 })
                .unwrap(),
            Self::SharedCall => {
                let square = input.checked_mul(input).unwrap();
                square.checked_add(square).unwrap()
            }
            Self::ContextFamily => {
                if condition {
                    input.checked_mul(input).unwrap()
                } else {
                    input.checked_add(input).unwrap()
                }
            }
            Self::Chain64 => {
                let mut value = input;
                for _ in 0..64 {
                    value = value.checked_add(3).unwrap();
                }
                value
            }
        }
    }

    fn build(self) -> (Program, WordId) {
        let mut program = Program::new();
        let square = program
            .add_word(1, vec![Op::Arg(0), Op::Binary(Binary::Mul, 0, 0)], vec![1])
            .unwrap();
        let word = match self {
            Self::Square => square,
            Self::Quad => program
                .add_word(
                    1,
                    vec![
                        Op::Arg(0),
                        Op::Binary(Binary::Mul, 0, 0),
                        Op::Binary(Binary::Mul, 1, 1),
                    ],
                    vec![2],
                )
                .unwrap(),
            Self::Conditional => program
                .add_word(
                    2,
                    vec![
                        Op::Arg(0),
                        Op::Arg(1),
                        Op::Const(Literal::Int(3)),
                        Op::Binary(Binary::Mul, 0, 2),
                        Op::Const(Literal::Int(1)),
                        Op::Const(Literal::Int(2)),
                        Op::Binary(Binary::Add, 3, 4),
                        Op::Binary(Binary::Add, 3, 5),
                        Op::Select {
                            condition: 1,
                            when_true: 6,
                            when_false: 7,
                        },
                    ],
                    vec![8],
                )
                .unwrap(),
            Self::SharedCall => program
                .add_word(
                    1,
                    vec![
                        Op::Arg(0),
                        Op::Call {
                            word: square,
                            arguments: vec![0],
                        },
                        Op::Project { call: 1, output: 0 },
                        Op::Binary(Binary::Add, 2, 2),
                    ],
                    vec![3],
                )
                .unwrap(),
            Self::ContextFamily => {
                let guard = program
                    .add_word(1, vec![Op::Context("square".into())], vec![0])
                    .unwrap();
                let otherwise = program
                    .add_word(1, vec![Op::Const(Literal::Bool(true))], vec![0])
                    .unwrap();
                let twice = program
                    .add_word(1, vec![Op::Arg(0), Op::Binary(Binary::Add, 0, 0)], vec![1])
                    .unwrap();
                program
                    .add_family(1, 1, vec![(guard, square), (otherwise, twice)])
                    .unwrap()
            }
            Self::Chain64 => {
                let mut ops = vec![Op::Arg(0), Op::Const(Literal::Int(3))];
                let mut previous = 0;
                for _ in 0..64 {
                    let next = ops.len();
                    ops.push(Op::Binary(Binary::Add, previous, 1));
                    previous = next;
                }
                program.add_word(1, ops, vec![previous]).unwrap()
            }
            Self::SourceSquare => {
                march_research::fast::source::compile(&mut program, ": square dup * ; square")
                    .unwrap()
            }
        };
        (program, word)
    }

    fn inputs(self) -> usize {
        if matches!(self, Self::Conditional) {
            2
        } else {
            1
        }
    }

    fn reference_root(self, store: &mut Store, input: i64, condition: bool) -> Cid {
        let x = store.intern(Node::Const(Atom::Int(input)));
        match self {
            Self::Square => store.intern(Node::Mul(x, x)),
            Self::Quad => {
                let square = store.intern(Node::Mul(x, x));
                store.intern(Node::Mul(square, square))
            }
            Self::Conditional => {
                let three = store.intern(Node::Const(Atom::Int(3)));
                let one = store.intern(Node::Const(Atom::Int(1)));
                let two = store.intern(Node::Const(Atom::Int(2)));
                let shared = store.intern(Node::Mul(x, three));
                let when_true = store.intern(Node::Add(shared, one));
                let when_false = store.intern(Node::Add(shared, two));
                let condition = store.intern(Node::Const(Atom::Bool(condition)));
                store.intern(Node::If {
                    condition,
                    when_true,
                    when_false,
                })
            }
            Self::SharedCall => {
                let parameter = store.intern(Node::Param(0));
                let body = store.intern(Node::Mul(parameter, parameter));
                let function = store.intern(Node::Quote { params: 1, body });
                let call = store.intern(Node::Apply {
                    function,
                    arguments: vec![x],
                });
                store.intern(Node::Add(call, call))
            }
            _ => unreachable!("no reference baseline for this workload"),
        }
    }

    // Cold CAS graph construction + fresh reducer + result lookup + destruction.
    // This deliberately includes interning and is NOT a runtime-only comparison.
    fn reference(self, input: i64, condition: bool) -> i64 {
        let mut store = Store::new();
        let root = self.reference_root(&mut store, input, condition);
        reference_reduce(&mut store, root)
    }
}

fn reference_reduce(store: &mut Store, root: Cid) -> i64 {
    let result = Reducer::with_budget(store, &Bindings::new(), BUDGET)
        .run(root)
        .unwrap();
    match store.get(result.root) {
        Some(Node::Const(Atom::Int(number))) => *number,
        other => panic!("unexpected reference result: {other:?}"),
    }
}

fn scalar(values: &[Value]) -> i64 {
    match values {
        [Value::Int(number)] => *number,
        other => panic!("unexpected conventional result: {other:?}"),
    }
}

fn input(index: usize, batch: usize) -> (i64, bool) {
    (
        ((index * 17 + batch * 23) % 127) as i64 - 63,
        index.is_multiple_of(2),
    )
}

fn measure(
    label: &str,
    iterations: usize,
    workload: Workload,
    mut evaluate: impl FnMut(i64, bool) -> i64,
) {
    // Warm up the very same execution path; construction has its own row below.
    for index in 0..256 {
        let (x, condition) = input(index, 0);
        assert_eq!(
            evaluate(black_box(x), black_box(condition)),
            workload.expected(x, condition)
        );
    }
    let mut times = [0.0; BATCHES];
    for (batch, elapsed) in times.iter_mut().enumerate() {
        let expected = (0..iterations).fold(0i64, |sum, index| {
            let (x, condition) = input(index, batch);
            sum.wrapping_add(workload.expected(x, condition))
        });
        let start = Instant::now();
        let mut checksum = 0i64;
        for index in 0..iterations {
            let (x, condition) = input(index, batch);
            checksum =
                checksum.wrapping_add(black_box(evaluate(black_box(x), black_box(condition))));
        }
        *elapsed = start.elapsed().as_secs_f64() * 1e9 / iterations as f64;
        assert_eq!(
            black_box(checksum),
            expected,
            "checksum mismatch for {label}"
        );
    }
    times.sort_by(f64::total_cmp);
    println!(
        "{label:32} median {:10.1} ns/op  range {:10.1}..{:10.1}  n={iterations}",
        times[3], times[0], times[6]
    );
}

fn measure_compile(workload: Workload, iterations: usize) {
    let mut times = [0.0; BATCHES];
    for elapsed in &mut times {
        let start = Instant::now();
        for _ in 0..iterations {
            let (program, word) = black_box(workload).build();
            black_box((program, word));
        }
        *elapsed = start.elapsed().as_secs_f64() * 1e9 / iterations as f64;
    }
    times.sort_by(f64::total_cmp);
    println!(
        "{:<32} median {:10.1} ns/op  range {:10.1}..{:10.1}  n={iterations}",
        "cold Program build+drop", times[3], times[0], times[6]
    );
}

fn recursion_retention() {
    let mut program = Program::new();
    march_research::fast::source::compile(
        &mut program,
        ": zero 0 = ; : always drop true ; : base drop 0 ; \
         : step 1 - recur 1 1 ; \
         family countdown 1 1 zero base always step ;",
    )
    .unwrap();
    let word = program.lookup("countdown").unwrap();
    println!("\nRecursion workspace (optimized run versus selective control; no timing):");
    for depth in [100usize, 1_000, 10_000] {
        let mut executor = Executor::new(&program);
        executor.cell_limit = 500_000;
        executor.argument_limit = 100_000;
        let budget = depth * 100 + 100;
        let result = executor
            .run(word, &[Literal::Int(depth as i64)], &Context::new(), budget)
            .unwrap();
        assert_eq!(scalar(&result), 0, "countdown depth {depth}");
        println!(
            "tail depth={depth} result=0 budget={budget} stats={:?}",
            executor.stats()
        );
        let handles = executor
            .start(word, &[Literal::Int(depth as i64)], &Context::new(), budget)
            .unwrap();
        assert_eq!(executor.force(handles[0]).unwrap(), Value::Int(0));
        println!("lazy depth={depth} result=0 stats={:?}", executor.stats());
    }
}

fn main() {
    assert!(!cfg!(debug_assertions), "benchmark requires --release");
    let mut arguments = std::env::args().skip(1);
    let iterations = arguments.next().map_or(20_000, |n| {
        n.parse::<usize>().expect("integer iteration count")
    });
    assert!(arguments.next().is_none(), "usage: fast_bench [iterations]");
    assert!(
        (100..=200_000).contains(&iterations),
        "iteration count must be 100..=200000"
    );
    println!(
        "Seven batches, varied inputs, checked i64 arithmetic; per-invocation budget {BUDGET}."
    );
    println!(
        "Fresh executor rows exclude Program compilation. CAS cold includes graph construction; warm prebuilds inputs and retains interned results but resets reducer memo each invocation."
    );
    for workload in [
        Workload::Square,
        Workload::Quad,
        Workload::Conditional,
        Workload::SharedCall,
        Workload::ContextFamily,
        Workload::Chain64,
        Workload::SourceSquare,
    ] {
        println!("\n{workload:?}");
        let (program, word) = workload.build();
        let context = Context::new();
        // Two immutable contexts are created before timing, then selected by
        // the same varied Boolean that selects the direct-Rust branch.
        let context_false = Context::from([("square".into(), Literal::Bool(false))]);
        let context_true = Context::from([("square".into(), Literal::Bool(true))]);
        let invocation_context = |condition: bool| {
            if matches!(workload, Workload::ContextFamily) {
                if condition {
                    &context_true
                } else {
                    &context_false
                }
            } else {
                &context
            }
        };
        let mut executor = Executor::new(&program);
        let mut output = Vec::with_capacity(1);
        measure(
            "direct checked Rust",
            iterations,
            workload,
            |x, condition| workload.expected(x, condition),
        );
        measure(
            "fresh Executor + run + drop",
            iterations,
            workload,
            |x, condition| {
                let args = [Literal::Int(x), Literal::Bool(condition)];
                let mut executor = Executor::new(&program);
                scalar(
                    &executor
                        .run(
                            word,
                            &args[..workload.inputs()],
                            invocation_context(condition),
                            BUDGET,
                        )
                        .unwrap(),
                )
            },
        );
        measure(
            "reused Executor::run_into",
            iterations,
            workload,
            |x, condition| {
                let args = [Literal::Int(x), Literal::Bool(condition)];
                executor
                    .run_into(
                        word,
                        &args[..workload.inputs()],
                        invocation_context(condition),
                        BUDGET,
                        &mut output,
                    )
                    .unwrap();
                scalar(&output)
            },
        );
        println!("last invocation stats: {:?}", executor.stats());
        measure_compile(workload, (iterations / 20).clamp(100, 1_000));
        if matches!(
            workload,
            Workload::ContextFamily | Workload::Chain64 | Workload::SourceSquare
        ) {
            continue;
        }
        measure(
            "reference CAS cold end-to-end",
            (iterations / 100).clamp(100, 1_000),
            workload,
            |x, condition| workload.reference(x, condition),
        );
        let mut store = Store::new();
        let mut roots = Vec::with_capacity(254);
        for x in -63..=63 {
            for condition in [false, true] {
                roots.push(workload.reference_root(&mut store, x, condition));
            }
        }
        // Populate immutable result nodes before timing. The fresh Reducer below
        // still revisits/reduces the original input graph, not a cached answer.
        for root in &roots {
            black_box(reference_reduce(&mut store, *root));
        }
        measure(
            "reference CAS warm reduce",
            (iterations / 10).clamp(100, 5_000),
            workload,
            |x, condition| {
                reference_reduce(
                    &mut store,
                    roots[(x + 63) as usize * 2 + usize::from(condition)],
                )
            },
        );
    }
    recursion_retention();
}
