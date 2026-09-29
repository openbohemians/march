mod assembler;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: march7-seed LISTING OUTPUT.image".into());
    }
    let source = std::fs::read_to_string(&args[0])?;
    let image = assembler::assemble(&source)?;
    std::fs::write(&args[1], image.encode()?)?;
    println!(
        "generation zero: {} blobs; assembler source: {} lines",
        image.blobs.len(),
        include_str!("assembler.rs").lines().count()
    );
    Ok(())
}
