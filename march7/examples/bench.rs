use march7::{Driver, Image};
use std::hint::black_box;
use std::time::Instant;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bytes = std::fs::read("seed.image")?;
    let t = Instant::now();
    let image = Image::decode(&bytes)?;
    let mut d = Driver::boot(&image)?;
    println!("load + validate + link + boot: {:?}", t.elapsed());
    let t = Instant::now();
    d.evaluate(": square dup u* ; quote square")?;
    let token = d.machine.stack.pop().ok_or("missing token")?;
    println!("compile + lookup square: {:?}", t.elapsed());
    let n = 50_000;
    let t = Instant::now();
    let mut sum = 0u64;
    for i in 0..n {
        d.machine.stack.push(black_box(i));
        d.machine.run(token, 100)?;
        sum = sum.wrapping_add(d.machine.stack.pop().ok_or("missing result")?);
    }
    println!(
        "warm square: {:.1} ns/call; checksum={sum}",
        t.elapsed().as_nanos() as f64 / n as f64
    );
    println!("stats: {:?}", d.machine.stats);
    Ok(())
}
