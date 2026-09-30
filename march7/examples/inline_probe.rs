//! Compare a loop using inlined primitives with the same loop calling
//! one-instruction words (the cost every primitive use had before inlining).
use march7::{Driver, Image};
fn main() {
    let path = std::env::args().nth(1).expect("image path");
    let image = Image::decode(&std::fs::read(&path).unwrap()).unwrap();
    let mut d = Driver::boot(&image).unwrap();
    d.fuel = 10_000_000_000;
    d.evaluate(": count-inline cycle dup while 1 u- repeat ;")
        .unwrap();
    d.evaluate(": d dup ; : s u- ; : count-calls cycle d while 1 s repeat ;")
        .unwrap();
    for word in ["count-calls", "count-inline"] {
        let before = d.machine.stats.steps;
        let t = std::time::Instant::now();
        d.evaluate(&format!("5000000 {word} drop")).unwrap();
        let dt = t.elapsed();
        println!(
            "{word:13} {:>9} steps  {:7.1} ms  {:5.2} ns/iteration",
            d.machine.stats.steps - before,
            dt.as_secs_f64() * 1e3,
            dt.as_nanos() as f64 / 5e6
        );
    }
}
