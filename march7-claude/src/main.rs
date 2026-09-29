use march7::{Driver, Image};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let usage =
        "usage: march7 IMAGE [--fuel N] [--eval SOURCE | FILE] [--save IMAGE] [--system IMAGE]";
    let path = args.next().ok_or(usage)?;
    let image = Image::decode(&std::fs::read(path)?)?;
    let mut d = Driver::boot(&image)?;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--fuel" => d.fuel = args.next().ok_or("missing fuel")?.parse()?,
            "--eval" => d.evaluate(&args.next().ok_or("missing source")?)?,
            "--save" => std::fs::write(
                args.next().ok_or("missing output")?,
                d.snapshot()?.encode()?,
            )?,
            // The entry token is taken from the top of the stack, where March
            // code such as `' boot` left it. The host looks up no names.
            "--system" => {
                let out = args.next().ok_or("missing output")?;
                let xt = d
                    .machine
                    .stack
                    .pop()
                    .ok_or("--system needs an entry token")?;
                std::fs::write(out, d.system_image(xt)?.encode()?)?;
            }
            _ => d.evaluate(&std::fs::read_to_string(arg)?)?,
        }
    }
    println!("{:?}", d.machine.stack);
    Ok(())
}
