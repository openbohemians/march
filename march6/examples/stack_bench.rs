//! Unoptimized strict-stack baseline versus existing optimized lazy runtime.
//! Same grounded scalar results only; this is NOT semantic equivalence for
//! dropped work, effects, or termination. Compile/setup excluded from run time.
use march_research::fast::{
    Context, Executor, Literal,
    stack::{Machine, Value},
    store::Store,
    stream,
};
use std::{hint::black_box, time::Instant};

fn measure(mut run: impl FnMut(i64) -> i64, n: usize) -> (f64, i64) {
    let start = Instant::now();
    let mut sum = 0i64;
    for i in 0..n {
        sum = sum.wrapping_add(black_box(run(black_box((i % 32) as i64))));
    }
    (start.elapsed().as_nanos() as f64 / n as f64, black_box(sum))
}
fn integer(values: &[Value]) -> i64 {
    match values {
        [Value::Int(n)] => *n,
        _ => panic!("unexpected result"),
    }
}
fn main() {
    let n = std::env::args()
        .nth(1)
        .map(|s| s.parse::<usize>().unwrap())
        .unwrap_or(20_000);
    assert!((1..=1_000_000).contains(&n));
    println!("iterations={n}; ns/invocation; setup/compilation excluded");
    for (name, source, native) in [
        ("square", "dup *", (|n: i64| n * n) as fn(i64) -> i64),
        (
            "two calls",
            ": square dup * ; square square",
            (|n: i64| n * n * n * n) as fn(i64) -> i64,
        ),
        (
            "family recursion",
            ": zero 0 eq? ; : yes drop true ; : done drop 0 ; : step 1 - recur 1 1 ; family down 1 1 zero done yes step ; down",
            (|_: i64| 0) as fn(i64) -> i64,
        ),
        (
            "store",
            "\"x\" store.put \"x\" store.get 1 + \"x\" store.put \"x\" store.get",
            (|n: i64| n + 1) as fn(i64) -> i64,
        ),
    ] {
        let build = Instant::now();
        let mut p = stream::seed().unwrap();
        let w = stream::compile(&mut p, source).unwrap();
        let build_us = build.elapsed().as_micros();
        let context = Context::new();
        let store = Store::new();
        let mut vm = Machine::new(&p);
        let mut graph = Executor::new(&p);
        // Warm both engines without including source compilation or startup.
        for _ in 0..100 {
            vm.run(w, &[Literal::Int(8)], &context, 100_000, &store)
                .unwrap();
            graph.run(w, &[Literal::Int(8)], &context, 100_000).unwrap();
        }
        let (strict_ns, strict_sum) = measure(
            |x| {
                integer(
                    &vm.run(w, &[Literal::Int(x)], &context, 100_000, &store)
                        .unwrap()
                        .0,
                )
            },
            n,
        );
        let optimized_stats = vm.stats().clone();
        vm.tail_call_optimization = false;
        let (baseline_ns, baseline_sum) = measure(
            |x| {
                integer(
                    &vm.run(w, &[Literal::Int(x)], &context, 100_000, &store)
                        .unwrap()
                        .0,
                )
            },
            n,
        );
        let baseline_peak = vm.stats().peak_continuations;
        let (graph_ns, graph_sum) = measure(
            |x| match graph
                .run(w, &[Literal::Int(x)], &context, 100_000)
                .unwrap()
                .as_slice()
            {
                [march_research::fast::Value::Int(n)] => *n,
                _ => panic!("unexpected scalar fixture result"),
            },
            n,
        );
        let (native_ns, native_sum) = measure(native, n);
        assert_eq!(strict_sum, graph_sum);
        assert_eq!(strict_sum, baseline_sum);
        assert_eq!(strict_sum, native_sum);
        println!(
            "{name}: stack={strict_ns:.1} stack_without_tail={baseline_ns:.1} legacy={graph_ns:.1} Rust={native_ns:.1}; compile={build_us}us; checksum={strict_sum}; peak_stack={} peak_continuations={}/{} tail_entries={}",
            optimized_stats.peak_stack,
            optimized_stats.peak_continuations,
            baseline_peak,
            optimized_stats.tail_calls
        );
        if name == "family recursion" {
            let mut bounded = Machine::new(&p);
            bounded.continuation_limit = 16;
            let start = Instant::now();
            let (values, _) = bounded
                .run(w, &[Literal::Int(100_000)], &context, 10_000_000, &store)
                .unwrap();
            assert_eq!(integer(&values), 0);
            println!(
                "100000-iteration countdown: {:.2} ms, peak_continuations={}, peak_stack={}",
                start.elapsed().as_secs_f64() * 1000.0,
                bounded.stats().peak_continuations,
                bounded.stats().peak_stack
            );
        }
    }
}
