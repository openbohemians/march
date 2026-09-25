//! Command-line entry point for the deliberately provisional conventional spike.

use march_research::fast::{Context, Executor, Literal, Program, source};
use std::{env, fs, io::Read, process};

fn usage() -> &'static str {
    "usage: march-fast [--budget N] [--context key=value] [--arg value]\n\
     [--save-image PATH] (--eval SOURCE | FILE | --load-image PATH)\n\
     values: signed integers, true, false, or unit; default budget: 100000\n\
     images contain code/dictionary only, not context or suspended execution"
}

fn literal(text: &str) -> Result<Literal, String> {
    match text {
        "true" => Ok(Literal::Bool(true)),
        "false" => Ok(Literal::Bool(false)),
        "unit" => Ok(Literal::Unit),
        _ => text
            .parse::<i64>()
            .map(Literal::Int)
            .map_err(|_| format!("invalid literal '{text}'")),
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let mut input = None;
    let mut load_image = None;
    let mut save_image = None;
    let mut context = Context::new();
    let mut arguments = Vec::new();
    let mut budget = 100_000;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                println!("{}", usage());
                return Ok(());
            }
            "--eval" | "-e" => {
                if input.is_some() {
                    return Err("provide exactly one source input".into());
                }
                input = Some(args.next().ok_or("--eval requires source text")?);
            }
            "--context" => {
                let entry = args.next().ok_or("--context requires key=value")?;
                let (key, value) = entry
                    .split_once('=')
                    .ok_or("--context requires key=value")?;
                if key.is_empty() {
                    return Err("context key cannot be empty".into());
                }
                context.insert(key.to_string(), literal(value)?);
            }
            "--load-image" => {
                if load_image.is_some() {
                    return Err("provide only one --load-image".into());
                }
                load_image = Some(args.next().ok_or("--load-image requires a path")?);
            }
            "--save-image" => {
                if save_image.is_some() {
                    return Err("provide only one --save-image".into());
                }
                save_image = Some(args.next().ok_or("--save-image requires a path")?);
            }
            "--arg" => arguments.push(literal(&args.next().ok_or("--arg requires a value")?)?),
            "--budget" => {
                budget = args
                    .next()
                    .ok_or("--budget requires an integer")?
                    .parse()
                    .map_err(|_| "--budget requires a nonnegative integer")?;
            }
            value if value.starts_with('-') => {
                return Err(format!("unknown option '{value}'\n{}", usage()));
            }
            file => {
                if input.is_some() {
                    return Err("provide exactly one source input".into());
                }
                let mut source = String::new();
                fs::File::open(file)
                    .map_err(|e| format!("{file}: {e}"))?
                    .take(4 * 1024 * 1024 + 1)
                    .read_to_string(&mut source)
                    .map_err(|e| format!("{file}: {e}"))?;
                input = Some(source);
            }
        }
    }
    let (program, word) = match (input, load_image) {
        (Some(input), None) => {
            let mut program = Program::new();
            let word = source::compile(&mut program, &input).map_err(|e| e.to_string())?;
            (program, word)
        }
        (None, Some(path)) => {
            // Read at most one byte beyond the format limit, even if a file is
            // replaced or grows between opening and validation.
            let mut bytes = Vec::new();
            fs::File::open(&path)
                .map_err(|e| format!("{path}: {e}"))?
                .take(32 * 1024 * 1024 + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| format!("{path}: {e}"))?;
            Program::from_image(&bytes).map_err(|e| e.to_string())?
        }
        (None, None) => return Err(usage().into()),
        (Some(_), Some(_)) => return Err("choose source or --load-image, not both".into()),
    };
    if let Some(path) = save_image {
        let bytes = program.to_image(word).map_err(|e| e.to_string())?;
        fs::write(&path, &bytes).map_err(|e| format!("{path}: {e}"))?;
        eprintln!(
            "saved code image {}",
            march_research::Cid::digest(b"march-fast-image-v1", &bytes)
        );
    }
    let values = Executor::new(&program)
        .run(word, &arguments, &context, budget)
        .map_err(|e| e.to_string())?;
    println!("{values:?}");
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("march-fast: {error}");
        process::exit(1);
    }
}
