use march8::{Kind, Session};
use std::io::{BufRead, IsTerminal, Write};

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

const USAGE: &str = "usage: march8 [--fuel N] [--types] [--eval SOURCE | --code SOURCE | FILE]...
       march8                 a REPL, reading lines from the terminal or stdin
       march8 fmt             rewrites `\\name` escapes as symbols, stdin to stdout";

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1).peekable();
    if args.peek().map(String::as_str) == Some("fmt") {
        let mut source = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut source)?;
        print!("{}", march8::symbols::Symbols::standard().format(&source));
        return Ok(());
    }
    let mut s = Session::new();
    let mut types = false;
    let mut ran = false;
    while let Some(arg) = args.next() {
        let result = match arg.as_str() {
            "--fuel" => {
                s.fuel = args.next().ok_or("missing fuel")?.parse()?;
                continue;
            }
            "--types" => {
                types = true;
                continue;
            }
            "--eval" => s.eval(&args.next().ok_or("missing source")?),
            // The code a piece of source compiles to, without running it,
            // and the code of the words it calls.
            "--code" => {
                let (ops, _) = s.compile(&args.next().ok_or("missing source")?)?;
                print_code(&s, ops);
                ran = true;
                continue;
            }
            "--help" | "-h" => {
                println!("{USAGE}");
                return Ok(());
            }
            path => s.eval(&std::fs::read_to_string(path)?),
        };
        ran = true;
        flush(&mut s);
        result?;
    }
    if !ran {
        return repl(&mut s, types);
    }
    println!("{}", if types { s.show_typed() } else { s.show() });
    Ok(())
}

/// Reads lines, compiling and running each, and shows the stack after it.
/// A line ending inside a bracket or a string continues on the next.
/// Commands start with `:`, which March does not use.
fn repl(s: &mut Session, mut types: bool) -> Result<(), Box<dyn std::error::Error>> {
    let tty = std::io::stdin().is_terminal();
    let stdin = std::io::stdin();
    let mut lines = stdin.lock().lines();
    let mut pending = String::new();
    if tty {
        println!("march8: the explicit form. :help for commands, Ctrl-D to leave.");
    }
    loop {
        if tty {
            print!("{}", if pending.is_empty() { "> " } else { "… " });
            std::io::stdout().flush()?;
        }
        let Some(line) = lines.next() else { break };
        let line = line?;
        if pending.is_empty() {
            let command = line.trim();
            match command {
                "" => continue,
                ":quit" | ":q" => break,
                ":types" => {
                    types = !types;
                    println!("types {}", if types { "shown" } else { "hidden" });
                    continue;
                }
                ":help" => {
                    println!(
                        ":types     show or hide the stack's types\n\
                         :code SRC  the code SRC compiles to, without running it\n\
                         :quit      leave"
                    );
                    continue;
                }
                c if c.starts_with(":code ") => {
                    match s.compile(&c[":code ".len()..]) {
                        Ok((ops, _)) => print_code(s, ops),
                        Err(e) => println!("Error: {e}"),
                    }
                    continue;
                }
                c if c.starts_with(':') => {
                    println!("no command `{c}`: :help lists them");
                    continue;
                }
                _ => {}
            }
        }
        pending.push_str(&line);
        pending.push('\n');
        let r = s.eval(&pending);
        if let Err(e) = &r
            && e.kind == Kind::Unfinished
        {
            continue;
        }
        pending.clear();
        flush(s);
        match r {
            Ok(()) => println!("{}", if types { s.show_typed() } else { s.show() }),
            Err(e) => println!("Error: {e}"),
        }
    }
    if !pending.is_empty() {
        println!("Error: the input ends inside a bracket or a string");
    }
    Ok(())
}
