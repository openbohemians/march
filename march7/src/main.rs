use march7::{Driver, Error, Image};

/// Runs `step`, turning an exhausted step budget into advice.
fn guard(fuel: u64, step: Result<(), Error>) -> Result<(), Box<dyn std::error::Error>> {
    match step {
        Err(Error::Fuel) => Err(format!(
            "step budget of {fuel} exhausted (a runaway loop, or raise it with --fuel N)"
        )
        .into()),
        other => Ok(other?),
    }
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let usage = "usage: march7 IMAGE [--fuel N] [--eval SOURCE | FILE] [--save IMAGE] [--system IMAGE]\n       march7 fmt < SOURCE > FORMATTED";
    let path = args.next().ok_or(usage)?;
    // `fmt` rewrites `\name` escapes as symbols, from stdin to stdout.
    if path == "fmt" {
        let mut source = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut source)?;
        print!("{}", march7::symbols::Symbols::standard().format(&source));
        return Ok(());
    }
    let image = Image::decode(&std::fs::read(path)?)?;
    let mut d = Driver::boot(&image)?;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--fuel" => d.fuel = args.next().ok_or("missing fuel")?.parse()?,
            "--eval" => guard(d.fuel, d.evaluate(&args.next().ok_or("missing source")?))?,
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
            _ => guard(d.fuel, d.evaluate(&std::fs::read_to_string(arg)?))?,
        }
    }
    println!("{:?}", d.machine.stack);
    Ok(())
}
