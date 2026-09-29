use march7::{Driver, Image};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .ok_or("usage: march7 IMAGE [--eval SOURCE | FILE] [--save IMAGE]")?;
    let image = Image::decode(&std::fs::read(path)?)?;
    let mut d = Driver::boot(&image)?;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--eval" => d.evaluate(&args.next().ok_or("missing source")?)?,
            "--save" => std::fs::write(
                args.next().ok_or("missing output")?,
                d.snapshot()?.encode()?,
            )?,
            _ => d.evaluate(&std::fs::read_to_string(arg)?)?,
        }
    }
    println!("{:?}", d.machine.stack);
    Ok(())
}
