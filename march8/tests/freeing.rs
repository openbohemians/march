//! Freeing by a count of holders (doc/design/REPRESENTATIONS.md 4): a copy
//! holds a value once more and a drop once less, and a value is freed when
//! none is left, with what it holds.
use march8::Session;

fn session(pieces: &[&str]) -> Session {
    let mut s = Session::new();
    for p in pieces {
        if let Err(e) = s.eval(p) {
            panic!("{p}: {e}");
        }
    }
    s
}

#[test]
fn what_nothing_holds_is_freed() {
    // 80 MB made, past the machine's 64, each array let go of as soon as
    // its length is read: no more than one is held at a time.
    let s = session(&["10000 range 0 [ drop 1000 range length + ] fold"]);
    assert_eq!(s.show(), "<1> 10000000");
    assert!(s.machine.stats.allocated_bytes > 64 << 20);
    assert!(s.machine.stats.peak_live_bytes < 2 << 20);
    // Strings in arrays go with the arrays.
    let s = session(&[
        "\"the cat sat on the mat\" \" and\" concat",
        "20000 range 0 [ drop over words length + ] fold",
    ]);
    assert_eq!(s.show(), "<2> \"the cat sat on the mat and\" 140000");
    assert!(s.machine.stats.peak_live_bytes < 2 << 20);
}

#[test]
fn each_holder_keeps_its_own() {
    // A version made of a value another holds shares it: each keeps its own.
    let show = |src: &str| session(&[src]).show();
    assert_eq!(
        show("( \"a\" \"b\" ) dup \"c\" -1 insert swap"),
        "<2> ( \"a\" \"b\" \"c\" ) ( \"a\" \"b\" )"
    );
    assert_eq!(
        show("< string i64 map > empty \"a\" 1 put dup \"a\" 2 put swap"),
        "<2> { \"a\" 2 } { \"a\" 1 }"
    );
    assert_eq!(
        show("{ name: \"a\" toks: ( \"b\" ) } dup toks' ( \"c\" ) put swap"),
        "<2> { name' \"a\" toks' ( \"c\" ) } { name' \"a\" toks' ( \"b\" ) }"
    );
    assert_eq!(
        show("\"x y\" words [ w | w w concat ] ."),
        "<1> ( \"x\" \"y\" \"x\" \"y\" )"
    );
    // A string made as the program runs, not a literal, which is kept for
    // good: held by the array it is in and by the stack.
    assert_eq!(
        session(&[
            "\"a\" \"b\"",
            "concat dup < string ary > empty swap -1 insert swap"
        ])
        .show(),
        "<2> ( \"ab\" ) \"ab\""
    );
}

#[test]
fn a_failed_run_puts_back_what_it_took() {
    // The session holds its stack while a run may fail, so what the run let
    // go of comes back whole.
    let mut s = session(&["\"x y\" \"z\"", "concat"]);
    assert!(s.eval("words length 0 mod").is_err());
    assert_eq!(s.show(), "<1> \"x yz\"");
    // A handler holds its inputs, so they come back to it whole.
    let mut s = session(&[
        "ff: < string >. words 5 at ; ff: < string k trap >. drop ;",
        "\"a b\" \"c\"",
    ]);
    s.eval("concat ff").unwrap();
    assert_eq!(s.show(), "<1> \"a bc\"");
}

#[test]
fn an_append_to_an_array_alone_is_in_place() {
    // Appended where nothing else holds it, an array grows its flat tail,
    // joined to it once where it is next needed whole: shared, put in the
    // middle of, compared, sorted or joined, it is the array it was.
    let grown = "10 range < i64 ary > empty [ -1 insert ] fold";
    let show = |src: &str| session(&[&format!("{grown} {src}")]).show();
    assert_eq!(show(""), "<1> ( 1 2 3 4 5 6 7 8 9 10 )");
    assert_eq!(show("dup 11 -1 insert length swap length"), "<2> 11 10");
    assert_eq!(show("0 0 insert 1 3 slice"), "<1> ( 0 1 2 )");
    assert_eq!(show("10 range eq?"), "<1> true");
    assert_eq!(show("reverse 1 at"), "<1> 10");
    assert_eq!(show("dup concat length"), "<1> 20");
    // Strings appended are held by the array, and go with it.
    let s =
        session(&["1000 range < string ary > empty [ drop \"x\" \"y\" concat -1 insert ] fold"]);
    let shown = s.show();
    assert!(shown.starts_with("<1> ( \"xy\" \"xy\""), "{shown}");
    assert!(shown.ends_with("\"xy\" … 984 more )"), "{shown}");
    let s = session(&[
        "1000 range < string ary > empty [ drop \"x\" \"y\" concat -1 insert ] fold length",
    ]);
    assert_eq!(s.show(), "<1> 1000");
    let (bytes, _) = s.machine.live();
    assert!(bytes < (1 << 20) + (64 << 10), "{bytes}");
}

#[test]
fn lower_frees_as_it_goes() {
    // The first compiler in March, on itself: 16.8 MB held at its peak when
    // nothing was freed, now a few hundred kilobytes and the working
    // memory's megabyte.
    let dir = env!("CARGO_MANIFEST_DIR");
    let src = std::fs::read_to_string(format!("{dir}/compiler/lower.march")).unwrap();
    let (defs, run) = src.trim_end().rsplit_once('\n').unwrap();
    assert!(run.starts_with("io.stdin.read"), "{run}");
    let mut s = Session::new();
    s.fuel = 100_000_000_000;
    s.eval(defs).unwrap();
    let path = format!("{dir}/compiler/lower.march");
    s.eval(&run.replace("io.stdin.read", &format!("{path:?} io.file.read")))
        .unwrap();
    let out = String::from_utf8(s.take_output()).unwrap();
    let want = std::process::Command::new(env!("CARGO_BIN_EXE_march8"))
        .arg("lower")
        .stdin(std::fs::File::open(&path).unwrap())
        .output()
        .unwrap()
        .stdout;
    assert!(out.as_bytes() == want, "lowered as `march8 lower` does");
    assert!(
        s.machine.stats.peak_live_bytes < 2 << 20,
        "{}",
        s.machine.stats.peak_live_bytes
    );
    let (bytes, _) = s.machine.live();
    assert!(bytes < (1 << 20) + (64 << 10), "{bytes}");
}
