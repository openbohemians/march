//! One/two-consumer lazy-stream retention, with/without explicit collection.
//! Run release, with an external wall-time cap. No scalar tail-loop fast path.
use march_research::fast::{Context, Executor, Handle, Literal, Program, Value, source};
use std::{hint::black_box, time::Instant};

fn advance(e: &mut Executor<'_>, cursor: Handle, expected: i64) -> Handle {
    let Value::Pair(head, tail) = e.force(cursor).unwrap() else {
        panic!("pair")
    };
    assert_eq!(black_box(e.force(head).unwrap()), Value::Int(expected));
    tail
}

fn main() {
    assert!(!cfg!(debug_assertions), "use --release");
    let mut p = Program::new();
    let word = source::compile(&mut p, ": from dup 1 + recur 1 1 pair ; from").unwrap();
    for count in [100usize, 1_000, 10_000, 100_000, 1_000_000] {
        for lag in [0usize, 32] {
            for every in [0usize, 64] {
                if count > 10_000 && every == 0 {
                    continue;
                }
                let mut e = Executor::new(&p);
                if every != 0 {
                    e.cell_limit = 1024;
                    e.argument_limit = 256;
                }
                let mut cursor = e
                    .start(
                        word,
                        &[Literal::Int(0)],
                        &Context::new(),
                        count * 100 + 1000,
                    )
                    .unwrap()[0];
                let mut slow = cursor;
                let mut peak_vector_bytes = e.storage().vector_bytes;
                let started = Instant::now();
                for i in 0..count {
                    cursor = advance(&mut e, cursor, i as i64);
                    if lag != 0 && i >= lag {
                        slow = advance(&mut e, slow, (i - lag) as i64);
                    }
                    if every != 0 && (i + 1) % every == 0 {
                        peak_vector_bytes = peak_vector_bytes.max(e.storage().vector_bytes);
                        if lag == 0 {
                            cursor = e.collect(&[cursor], 20_000).unwrap()[0];
                        } else {
                            let roots = e.collect(&[cursor, slow], 20_000).unwrap();
                            cursor = roots[0];
                            slow = roots[1];
                        }
                    }
                }
                peak_vector_bytes = peak_vector_bytes.max(e.storage().vector_bytes);
                if every != 0 {
                    let roots = if lag == 0 {
                        vec![cursor]
                    } else {
                        vec![cursor, slow]
                    };
                    let roots = e.collect(&roots, 20_000).unwrap();
                    cursor = roots[0];
                    if lag != 0 {
                        slow = roots[1];
                    }
                }
                assert_eq!(e.stats().primitive_ops, count - 1);
                println!(
                    "count={count} lag={lag} collect_every={every} elapsed_us={:.3} primitives={} collections={} peak_cells={} peak_vector_bytes={peak_vector_bytes} storage={:?}",
                    started.elapsed().as_secs_f64() * 1e6,
                    e.stats().primitive_ops,
                    e.stats().collections,
                    e.stats().peak_cells,
                    e.storage()
                );
                black_box((cursor, slow));
            }
        }
    }
}
