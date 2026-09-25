//! Bounded, dependency-free microbenchmarks for the current diagnostic runner.
//! These ablations are not an optimized backend or a language-wide benchmark.
use super::*;
use std::hint::black_box;
use std::time::Instant;

const SAMPLES: usize = 7;
const CASES: usize = 1024;

fn input(i: usize) -> (i64, bool) {
    ((i % 997 + 1) as i64, i & 1 == 0)
}

#[inline(never)]
fn direct(x: i64, condition: bool) -> i64 {
    x.checked_mul(3)
        .unwrap()
        .checked_add(if condition { 1 } else { 2 })
        .unwrap()
}

#[inline(never)]
fn identity(x: i64, condition: bool) -> i64 {
    x + i64::from(condition)
}

fn expected(cases: usize) -> i64 {
    (0..cases)
        .map(|i| {
            let (x, b) = input(i);
            direct(x, b)
        })
        .sum()
}

fn make(i: usize, mode: Diagnostics, padding: usize) -> Net {
    let (x, condition) = black_box(input(i));
    let mut n = conditional_fixture_with_input(Some(condition), Producer::Finite, true, x);
    n.diagnostics = mode;
    // Legitimate quiescent components, not dead slots: one Int per free boundary.
    for j in 0..padding {
        let value = n.add(Kind::Int(j as i64));
        let boundary = n.boundary("passive-padding");
        n.link(End::Port(value, 0), boundary);
    }
    n
}

fn evaluate(n: &mut Net, order: Order) -> i64 {
    assert_eq!(black_box(&mut *n).run(order, 128), Halt::Quiescent);
    match n.terminal("result") {
        Some(Kind::Answer(v)) => v,
        other => panic!("unexpected result: {other:?}"),
    }
}

fn report(label: &str, cases: usize, samples: usize, mut batch: impl FnMut() -> f64) {
    // One full warm-up batch; timing never includes compilation or stdout.
    black_box(batch());
    let mut times: Vec<_> = (0..samples).map(|_| batch()).collect();
    times.sort_by(f64::total_cmp);
    println!(
        "{label}: ns/case min={:.1} median={:.1} max={:.1}; cases/sample={cases}; samples={samples}",
        times[0],
        times[samples / 2],
        times[samples - 1]
    );
}

fn graph_batch(
    mode: Diagnostics,
    order: Order,
    reduce_only: bool,
    padding: usize,
    cases: usize,
) -> f64 {
    let want = expected(cases);
    let mut checksum = 0i64;
    let elapsed = if reduce_only {
        let mut nets: Vec<_> = (0..cases).map(|i| make(i, mode, padding)).collect();
        let start = Instant::now();
        for n in &mut nets {
            checksum += evaluate(n, order);
        }
        let elapsed = start.elapsed();
        // Final independent audit is outside timing even in Lean mode.
        for n in &nets {
            n.audit();
            assert_eq!(n.rules.len(), 18);
            assert_eq!(n.live(), 1 + padding);
        }
        elapsed
    } else {
        let start = Instant::now();
        for i in 0..cases {
            let mut n = make(i, mode, padding);
            checksum += evaluate(&mut n, order);
            // Destruction included, including retained diagnostic history.
            drop(n);
        }
        start.elapsed()
    };
    assert_eq!(black_box(checksum), want);
    elapsed.as_secs_f64() * 1e9 / cases as f64
}

pub(super) fn run() {
    assert!(!cfg!(debug_assertions), "bench requires --release");
    println!("Conditional Join: runtime x and bool, x*3 + (bool ? 1 : 2); 18 interactions/case");
    println!(
        "Full=trace+audits; NoTrace=audits without trace; Lean=no trace/rule-validation/full-graph-audits during reduction"
    );
    println!(
        "All modes retain local wiring assertions, rule-name history, scans, allocation, and actual erasure; run cap128."
    );
    for (label, f) in [
        ("direct-checked-arithmetic", direct as fn(i64, bool) -> i64),
        ("input-call-control", identity as fn(i64, bool) -> i64),
    ] {
        let cases = 1_000_000;
        let want: i64 = (0..cases)
            .map(|i| {
                let (x, b) = input(i);
                f(x, b)
            })
            .sum();
        report(label, cases, SAMPLES, || {
            let start = Instant::now();
            let mut sum = 0i64;
            for i in 0..cases {
                let (x, b) = black_box(input(i));
                sum += black_box(f)(x, b);
            }
            let elapsed = start.elapsed();
            assert_eq!(black_box(sum), want);
            elapsed.as_secs_f64() * 1e9 / cases as f64
        });
    }
    report("construction+drop-only", CASES, SAMPLES, || {
        let start = Instant::now();
        for i in 0..CASES {
            drop(black_box(make(i, Diagnostics::Full, 0)));
        }
        start.elapsed().as_secs_f64() * 1e9 / CASES as f64
    });
    for order in [Order::Oldest, Order::Youngest] {
        for mode in [Diagnostics::Full, Diagnostics::NoTrace, Diagnostics::Lean] {
            for reduce_only in [false, true] {
                let scope = if reduce_only {
                    "reduction-only"
                } else {
                    "build+reduce+drop"
                };
                report(
                    &format!("{mode:?}/{order:?}/{scope}"),
                    CASES,
                    SAMPLES,
                    || graph_batch(mode, order, reduce_only, 0, CASES),
                );
            }
        }
    }
    for padding in [0, 100, 1000] {
        report(
            &format!("Lean/Oldest/reduction-only/passive={padding}"),
            128,
            SAMPLES,
            || graph_batch(Diagnostics::Lean, Order::Oldest, true, padding, 128),
        );
    }
    for mode in [Diagnostics::Full, Diagnostics::NoTrace, Diagnostics::Lean] {
        let mut n = make(0, mode, 0);
        evaluate(&mut n, Order::Oldest);
        println!(
            "retained/{mode:?}: live={}, allocated-slots={}, trace-strings={}, trace-text-bytes={}, rule-history={}",
            n.live(),
            n.agents.len(),
            n.trace.len(),
            n.trace.iter().map(String::len).sum::<usize>(),
            n.rules.len()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn benchmark_modes_preserve_rules_results_and_residual_graphs() {
        for producer in [Producer::Finite, Producer::Unknown, Producer::Spin] {
            for condition in [None, Some(false), Some(true)] {
                for invoked in [false, true] {
                    for order in [Order::Oldest, Order::Youngest] {
                        let initial = conditional_fixture(condition, producer, invoked);
                        let mut reference = initial.clone();
                        let halt = reference.run(order, 128);
                        for mode in [Diagnostics::NoTrace, Diagnostics::Lean] {
                            let mut n = initial.clone();
                            n.diagnostics = mode;
                            assert_eq!(n.run(order, 128), halt);
                            n.audit();
                            assert_eq!(n.agents, reference.agents);
                            assert_eq!(n.boundaries, reference.boundaries);
                            assert_eq!(n.retired, reference.retired);
                            assert_eq!(n.rules, reference.rules);
                            assert!(n.trace.is_empty());
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn benchmark_runtime_inputs_match_direct_arithmetic() {
        for mode in [Diagnostics::Full, Diagnostics::NoTrace, Diagnostics::Lean] {
            for i in 0..32 {
                let (x, b) = input(i);
                let mut n = make(i, mode, 0);
                assert_eq!(evaluate(&mut n, Order::Oldest), direct(x, b));
                n.audit();
                assert_eq!(n.rules.len(), 18);
            }
        }
    }
}
