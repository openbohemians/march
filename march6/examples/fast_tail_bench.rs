//! Same-code comparison: bounded scalar tail loop vs selective lazy evaluator.
//! `timeout --kill-after=2s 30s cargo run --offline --release --example fast_tail_bench`
use march_research::fast::{Context, Executor, Literal, Program, Value, source};
use std::{hint::black_box, time::Instant};

fn main() {
    assert!(!cfg!(debug_assertions), "use --release");
    let mut p = Program::new();
    source::compile(
        &mut p,
        ": zero 0 = ; : always drop true ; : base drop 0 ;
        : step 1 - recur 1 1 ; family countdown 1 1 zero base always step ;",
    )
    .unwrap();
    let word = p.lookup("countdown").unwrap();
    let context = Context::new();
    for depth in [100usize, 1000, 10_000] {
        let repeats = (10_000 / depth).max(2);
        for tail in [false, true] {
            let mut e = Executor::new(&p);
            let mut out = Vec::new();
            let mut samples = Vec::new();
            // Warm capacities once; no invocation memo survives a run/start.
            for batch in 0..8 {
                let start = Instant::now();
                for iteration in 0..repeats {
                    let n = black_box(depth + iteration % 2);
                    let args = [Literal::Int(n as i64)];
                    let budget = n * 100 + 100;
                    if tail {
                        e.run_into(word, &args, &context, budget, &mut out).unwrap();
                        assert_eq!(black_box(&out[..]), &[Value::Int(0)]);
                    } else {
                        let handles = e.start(word, &args, &context, budget).unwrap();
                        assert_eq!(black_box(e.force(handles[0]).unwrap()), Value::Int(0));
                    }
                }
                if batch != 0 {
                    samples.push(start.elapsed().as_secs_f64() * 1e6 / repeats as f64);
                }
            }
            samples.sort_by(f64::total_cmp);
            println!(
                "{} depth={depth} median_us={:.3} range_us={:.3}..{:.3} repeats={repeats} stats={:?}",
                if tail { "tail" } else { "lazy" },
                samples[3],
                samples[0],
                samples[6],
                e.stats()
            );
        }
    }
}
