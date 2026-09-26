//! Dictionary-container probe, not a claim about whole-compiler throughput.
//! Run: cargo run --offline --release --example fast_dictionary_bench
use std::{collections::BTreeMap, hint::black_box, time::Instant};

fn measure(label: &str, repeats: usize, mut f: impl FnMut()) {
    let mut samples = Vec::new();
    for batch in 0..8 {
        let started = Instant::now();
        for _ in 0..repeats {
            f();
        }
        if batch != 0 {
            samples.push(started.elapsed().as_secs_f64() * 1e6 / repeats as f64);
        }
    }
    samples.sort_by(f64::total_cmp);
    println!(
        "{label}: median_us={:.3} range_us={:.3}..{:.3} repeats={repeats}",
        samples[3], samples[0], samples[6]
    );
}

fn main() {
    assert!(!cfg!(debug_assertions), "use --release");
    for count in [32, 1000, 10_000] {
        let keys: Vec<_> = (0..count)
            .map(|i| format!("namespace.word-{i:06}"))
            .collect();
        let tree: BTreeMap<_, _> = keys
            .iter()
            .cloned()
            .enumerate()
            .map(|(i, k)| (k, i))
            .collect();
        let hamt: imbl::HashMap<_, _> = tree.iter().map(|(k, &v)| (k.clone(), v)).collect();
        let repeats = (100_000 / count).max(10);
        println!("bindings={count}; snapshot/update includes dropping the edited copy");
        measure("BTreeMap snapshot + rebind", repeats, || {
            let mut next = black_box(&tree).clone();
            next.insert(black_box(keys[count / 2].clone()), count);
            black_box(next);
        });
        measure("imbl HAMT snapshot + rebind", repeats, || {
            let mut next = black_box(&hamt).clone();
            next.insert(black_box(keys[count / 2].clone()), count);
            black_box(next);
        });
        measure("BTreeMap 1000 lookups", 100, || {
            for i in 0..1000 {
                black_box(tree.get(black_box(&keys[(i * 97) % count])));
            }
        });
        measure("imbl HAMT 1000 lookups", 100, || {
            for i in 0..1000 {
                black_box(hamt.get(black_box(&keys[(i * 97) % count])));
            }
        });
        assert_eq!(tree.get(&keys[count / 2]), Some(&(count / 2)));
        assert_eq!(hamt.get(&keys[count / 2]), Some(&(count / 2)));
    }
}
