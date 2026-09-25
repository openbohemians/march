use march_research::{
    Cid,
    fast::{Context, Executor, Literal, Program, Value, source},
};

fn compile(text: &str) -> (Program, usize) {
    let mut program = Program::new();
    let entry = source::compile(&mut program, text).unwrap();
    (program, entry)
}

fn run(program: &Program, entry: usize) -> Vec<Value> {
    Executor::new(program)
        .run(
            entry,
            &[],
            &Context::from([("enabled".into(), Literal::Bool(true))]),
            100_000,
        )
        .unwrap()
}

#[test]
fn roundtrip_words_quotations_multioutputs_context_pairs_and_recursion() {
    for text in [
        ": square dup * ; 7 square",
        ": both dup 1 + ; 8 both",
        ": square dup * ; quote square [ dup + ]",
        "ctx enabled 7 8 select 1 2 pair second 3 4 pair first unit",
        "7 ctx enabled [ dup * ] [ 1 + ] select apply 1 1",
        ": zero 0 = ; : one 1 = ; : always drop true ; : base dup drop ; : step dup 1 - recur 1 1 swap 2 - recur 1 1 + ; family fib 1 1 zero base one base always step ; 10 fib",
    ] {
        let (program, entry) = compile(text);
        let bytes = program.to_image(entry).unwrap();
        let (loaded, loaded_entry) = Program::from_image(&bytes).unwrap();
        assert_eq!(
            loaded.cid(loaded_entry).unwrap(),
            program.cid(entry).unwrap()
        );
        assert_eq!(loaded.to_image(loaded_entry).unwrap(), bytes);
        assert_eq!(run(&loaded, loaded_entry), run(&program, entry));
        assert_eq!(
            loaded.image_cid(loaded_entry).unwrap(),
            Cid::digest(b"march-fast-image-v1", &bytes)
        );
    }
}

#[test]
fn insertion_order_does_not_change_the_image() {
    let (a, ae) = compile(": one 1 ; : two 2 ; one two +");
    let (b, be) = compile(": two 2 ; : one 1 ; one two +");
    assert_ne!(a.lookup("one"), b.lookup("one"));
    assert_eq!(a.to_image(ae).unwrap(), b.to_image(be).unwrap());
}

#[test]
fn unreachable_history_is_excluded_but_dictionary_roots_survive() {
    let (a, ae) = compile(": old 99 ; : old 1 ; : unused 42 ; old");
    let (b, be) = compile(": old 1 ; : unused 42 ; old");
    assert!(a.len() > b.len());
    assert_eq!(a.to_image(ae).unwrap(), b.to_image(be).unwrap());
    let (loaded, _) = Program::from_image(&a.to_image(ae).unwrap()).unwrap();
    assert_eq!(
        run(&loaded, loaded.lookup("unused").unwrap()),
        [Value::Int(42)]
    );
}

#[test]
fn images_exclude_runtime_context_and_memo_state() {
    let (program, entry) = compile("ctx n dup +");
    let before = program.to_image(entry).unwrap();
    let mut executor = Executor::new(&program);
    assert_eq!(
        executor
            .run(
                entry,
                &[],
                &Context::from([("n".into(), Literal::Int(3))]),
                1000
            )
            .unwrap(),
        [Value::Int(6)]
    );
    assert_eq!(program.to_image(entry).unwrap(), before);
    let (loaded, entry) = Program::from_image(&before).unwrap();
    assert!(
        Executor::new(&loaded)
            .run(entry, &[], &Context::new(), 1000)
            .is_err()
    );
}

#[test]
fn truncated_corrupt_and_trailing_data_are_rejected() {
    let (program, entry) = compile(": square dup * ; 7 square");
    let bytes = program.to_image(entry).unwrap();
    for n in 0..bytes.len() {
        assert!(Program::from_image(&bytes[..n]).is_err(), "prefix {n}");
    }
    for index in [0, 7, 16, 56] {
        let mut bad = bytes.clone();
        bad[index] ^= 0xff;
        assert!(Program::from_image(&bad).is_err());
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(Program::from_image(&trailing).is_err());
    let mut huge = bytes;
    huge[8..16].copy_from_slice(&u64::MAX.to_le_bytes());
    assert!(Program::from_image(&huge).is_err());
}

fn number(bytes: &mut Vec<u8>, n: usize) {
    bytes.extend_from_slice(&(n as u64).to_le_bytes());
}
fn word(inputs: usize, operation: &[u8], outputs: &[usize]) -> Vec<u8> {
    let mut bytes = Vec::new();
    number(&mut bytes, inputs);
    number(&mut bytes, usize::from(!operation.is_empty()));
    bytes.extend_from_slice(operation);
    number(&mut bytes, outputs.len());
    for &output in outputs {
        number(&mut bytes, output);
    }
    bytes
}
fn image(words: &[Vec<u8>], names: &[(&[u8], usize)], entry: usize) -> Vec<u8> {
    let cids: Vec<_> = words
        .iter()
        .map(|w| Cid::digest(b"march-fast-word-v1", w))
        .collect();
    let mut bytes = b"MARCHF01".to_vec();
    number(&mut bytes, words.len());
    for (w, cid) in words.iter().zip(&cids) {
        bytes.extend_from_slice(&cid.0);
        number(&mut bytes, w.len());
        bytes.extend_from_slice(w);
    }
    number(&mut bytes, names.len());
    for &(name, target) in names {
        number(&mut bytes, name.len());
        bytes.extend_from_slice(name);
        bytes.extend_from_slice(&cids[target].0);
    }
    bytes.extend_from_slice(&cids[entry].0);
    bytes
}

#[test]
fn rehashed_hostile_word_encodings_are_rejected() {
    let mut invalid_arg = vec![0];
    number(&mut invalid_arg, 0);
    let mut forward_quote = vec![1, 3];
    forward_quote.extend_from_slice(&[0; 32]);
    let mut invalid_binary = vec![3, 255];
    number(&mut invalid_binary, 0);
    number(&mut invalid_binary, 0);
    let mut invalid_context = vec![2];
    number(&mut invalid_context, 1);
    invalid_context.push(255);
    let mut huge_recur = vec![7];
    huge_recur.extend_from_slice(&u64::MAX.to_le_bytes());
    for payload in [
        word(0, &[255], &[0]),
        word(0, &[1, 1, 2], &[0]),
        word(0, &invalid_arg, &[0]),
        word(0, &forward_quote, &[0]),
        word(0, &invalid_binary, &[0]),
        word(0, &invalid_context, &[0]),
        word(0, &huge_recur, &[0]),
        word(usize::MAX, &[1, 2], &[0]),
        word(0, &[1, 2], &[1]),
    ] {
        assert!(Program::from_image(&image(&[payload], &[], 0)).is_err());
    }
}

#[test]
fn duplicate_words_names_unknown_roots_and_unreachable_records_rejected() {
    let empty = word(0, &[], &[]);
    let unit = word(0, &[1, 2], &[0]);
    for bytes in [
        image(&[empty.clone(), empty.clone()], &[], 0),
        image(std::slice::from_ref(&empty), &[(b"x", 0), (b"x", 0)], 0),
        image(std::slice::from_ref(&empty), &[(b"z", 0), (b"a", 0)], 0),
        image(std::slice::from_ref(&empty), &[(&[255], 0)], 0),
        image(&[empty.clone(), unit], &[], 0),
    ] {
        assert!(Program::from_image(&bytes).is_err());
    }
    let mut unknown = image(&[empty], &[], 0);
    let n = unknown.len();
    unknown[n - 32..].fill(0);
    assert!(Program::from_image(&unknown).is_err());
}

#[test]
fn oversized_images_are_rejected_before_decoding() {
    assert!(Program::from_image(&vec![0; 32 * 1024 * 1024 + 1]).is_err());
}

#[test]
fn deep_dependency_chain_uses_an_explicit_worklist() {
    use march_research::fast::Op;
    let mut program = Program::new();
    let mut entry = program
        .add_word(0, vec![Op::Const(Literal::Int(7))], vec![0])
        .unwrap();
    for _ in 0..2000 {
        entry = program
            .add_word(
                0,
                vec![
                    Op::Call {
                        word: entry,
                        arguments: vec![],
                    },
                    Op::Project { call: 0, output: 0 },
                ],
                vec![1],
            )
            .unwrap();
    }
    let bytes = program.to_image(entry).unwrap();
    let (loaded, entry) = Program::from_image(&bytes).unwrap();
    assert_eq!(run(&loaded, entry), [Value::Int(7)]);
}

#[test]
fn cli_saves_and_loads_a_code_image_with_fresh_context() {
    use std::{
        fs,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory =
        std::env::temp_dir().join(format!("march-fast-image-{}-{unique}", std::process::id()));
    fs::create_dir(&directory).unwrap();
    let path = directory.join("program.march-image");
    let save = Command::new(env!("CARGO_BIN_EXE_march-fast"))
        .args([
            "--eval",
            "ctx enabled 7 8 select",
            "--context",
            "enabled=true",
            "--save-image",
        ])
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        save.status.success(),
        "{}",
        String::from_utf8_lossy(&save.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&save.stdout).trim(), "[Int(7)]");
    let load = Command::new(env!("CARGO_BIN_EXE_march-fast"))
        .arg("--load-image")
        .arg(&path)
        .args(["--context", "enabled=false"])
        .output()
        .unwrap();
    assert!(
        load.status.success(),
        "{}",
        String::from_utf8_lossy(&load.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&load.stdout).trim(), "[Int(8)]");
    let conflict = Command::new(env!("CARGO_BIN_EXE_march-fast"))
        .arg("--load-image")
        .arg(&path)
        .args(["--eval", "1"])
        .output()
        .unwrap();
    assert!(!conflict.status.success());
    let oversized_source = directory.join("oversized.march");
    fs::write(&oversized_source, vec![b' '; 4 * 1024 * 1024 + 1]).unwrap();
    let rejected = Command::new(env!("CARGO_BIN_EXE_march-fast"))
        .arg(&oversized_source)
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("4 MiB"));
    fs::remove_file(&oversized_source).unwrap();
    fs::remove_file(&path).unwrap();
    fs::remove_dir(&directory).unwrap();
}
