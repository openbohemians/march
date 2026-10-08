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

/// Prints code, then each word it calls, once, by the start of its identity.
fn print_code(s: &Session, ops: Vec<march8::Op>) {
    let hex = |c: &march8::Cid| {
        c[..4]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    };
    let mut todo = vec![(None, ops)];
    let mut seen = std::collections::HashSet::new();
    while let Some((cid, ops)) = todo.pop() {
        match cid {
            None => println!("source:"),
            Some(c) => println!("{}:", hex(&c)),
        }
        for (i, op) in ops.iter().enumerate() {
            match op {
                march8::Op::Call(c) | march8::Op::Tail(c) => {
                    let what = if matches!(op, march8::Op::Call(_)) {
                        "Call"
                    } else {
                        "Tail"
                    };
                    println!("{i:4}  {what} {}", hex(c));
                    if seen.insert(*c)
                        && let Some(march8::Blob::Code(b)) = s.machine.blob(c)
                        && let Ok(callee) = march8::code::decode(b)
                    {
                        todo.push((Some(*c), callee));
                    }
                }
                _ => println!("{i:4}  {op:?}"),
            }
        }
    }
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let usage = "usage: march8 [--fuel N] [--eval SOURCE | --code SOURCE | FILE]...";
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
            // The code a piece of source compiles to, without running it,
            // and the code of the words it calls.
            "--code" => {
                let (ops, _) = s.compile(&args.next().ok_or("missing source")?)?;
                print_code(&s, ops);
                continue;
            }
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
