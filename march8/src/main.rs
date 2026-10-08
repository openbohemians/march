use march8::Session;
use std::io::Write;

/// Writes what the program has printed so far.
fn flush(s: &mut Session) {
    let out = s.take_output();
    if !out.is_empty() {
        let mut stdout = std::io::stdout();
        let _ = stdout.write_all(&out);
        if out.last() != Some(&b'\n') {
            let _ = stdout.write_all(b"\n");
        }
        let _ = stdout.flush();
    }
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let usage = "usage: march8 [--fuel N] [--eval SOURCE | FILE]...";
    let mut args = std::env::args().skip(1).peekable();
    if args.peek().is_none() {
        return Err(usage.into());
    }
    let mut s = Session::new();
    while let Some(arg) = args.next() {
        let result = match arg.as_str() {
            "--fuel" => {
                s.fuel = args.next().ok_or("missing fuel")?.parse()?;
                continue;
            }
            "--eval" => s.eval(&args.next().ok_or("missing source")?),
            "--help" | "-h" => {
                println!("{usage}");
                return Ok(());
            }
            path => s.eval(&std::fs::read_to_string(path)?),
        };
        flush(&mut s);
        result?;
    }
    println!("{}", s.show());
    Ok(())
}
