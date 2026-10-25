//! Whole programs (examples/), and the words they needed: `put`, `empty`.
use march8::{Kind, Session};
use std::io::Write;
use std::process::{Command, Stdio};

/// What an example prints, given its input.
fn example(name: &str, input: &str) -> String {
    let path = format!("{}/examples/{name}", env!("CARGO_MANIFEST_DIR"));
    run(&[&path], input)
}

/// What march8 prints, given its arguments and its input.
fn run(args: &[&str], input: &str) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_march8"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    String::from_utf8(child.wait_with_output().unwrap().stdout).unwrap()
}

fn show(src: &str) -> String {
    let mut s = Session::new();
    if let Err(e) = s.eval(src) {
        panic!("{src}: {e}");
    }
    s.show_typed()
}

#[test]
fn word_frequencies() {
    assert_eq!(
        example("wordfreq.march", ""),
        "and 2\nbat 1\ncat 1\nhat 1\nmat 1\non 1\nsat 1\nthe 4\n--\n\
         the 4\nand 2\nbat 1\ncat 1\nhat 1\nmat 1\non 1\nsat 1\n<0>\n"
    );
}

#[test]
fn a_door_is_a_state_machine() {
    // Run while compiling, then on events made symbols as it runs.
    assert_eq!(example("door.march", ""), "<2> Locked' Closed'\n");
    // A transition the table lacks is no word, while compiling.
    let mut s = Session::new();
    let src = include_str!("../examples/door.march");
    s.eval(src).unwrap();
    let e = s.eval("Locked Open door").unwrap_err();
    assert_eq!(e.kind, Kind::NoWord);
    assert!(e.to_string().contains("for Locked' Open'"), "{e}");
}

#[test]
fn socrates_is_mortal() {
    assert_eq!(example("syllogism.march", ""), "<3> true false true\n");
}

#[test]
fn a_csv_summary() {
    let csv = include_str!("../examples/fruit.csv");
    assert_eq!(
        example("summary.march", csv),
        "qty: total 9.0, average 3.0\n\
         price: total 3.25, average 1.0833333333333333\n<0>\n"
    );
}

#[test]
fn lower_in_march_lowers_as_march8_does() {
    // The first compiler in March, compiler/lower.march, writes High Stack
    // as Low Stack as `march8 lower` does, to the byte: the core, a
    // bracket's names and `_`s as letters, the examples, and itself.
    let lower = format!("{}/compiler/lower.march", env!("CARGO_MANIFEST_DIR"));
    let names = "f1: [ rate term | rate 1 + term ^ ] . ;\n\
                 f2: [ x | x? ] . ;\nf3: [ p | p.x p.y.z ] . ;\nf4: [ x y | y ] . ;\n\
                 f5: [ x y | ( x ) [ dup ] y ] . ;\nf6: [ x | ( _ x ) ] . ;\n\
                 f7: [ x | \"\\_ and \\_\" ] . ;\nf8: [ x | [ y | y ] . ] . ;\n\
                 f9: < a [ x | x 1 gt ] ? >. ;\nhalf: _ 2 / ;\n";
    let itself = include_str!("../compiler/lower.march");
    for src in [
        include_str!("../core/core.march"),
        names,
        include_str!("../examples/door.march"),
        include_str!("../examples/summary.march"),
        include_str!("../examples/syllogism.march"),
        include_str!("../examples/wordfreq.march"),
        itself,
    ] {
        let want = run(&["lower"], src) + "<0>\n";
        assert_eq!(run(&["--fuel", "100000000000", &lower], src), want);
    }
    // A fixed point: lowered, it lowers itself to itself.
    let low = run(&["lower"], itself);
    assert_eq!(
        run(
            &["--fuel", "100000000000", "--symbolic", "--eval", &low],
            itself
        ),
        low + "<0>\n"
    );
}

#[test]
fn put_sets_and_empty_starts() {
    assert_eq!(
        show("< string i64 map > empty \"a\" 1 put \"b\" 2 put \"a\" 3 put"),
        "<1> { \"a\" 3 \"b\" 2 } < string i64 map >"
    );
    assert_eq!(show("< string ary > empty length"), "<1> 0 < i64 >");
    let mut s = Session::new();
    assert_eq!(s.eval("< i64 > empty").unwrap_err().kind, Kind::NoWord);
}

#[test]
fn a_literal_in_a_loop_is_made_once() {
    // Each pass reads a byte of the same literal at run time: the string is
    // made once, not a thousand times, so the loop allocates about what one
    // without it does.
    let bytes = |src: &str| {
        let mut s = Session::new();
        s.eval(src).unwrap();
        s.machine.stats.allocated_bytes
    };
    let plain = bytes("1000 range 0 [ 26 mod 1 + + ] fold");
    let literal =
        bytes("1000 range 0 [ \"abcdefghijklmnopqrstuvwxyz\" swap 26 mod 1 + at + ] fold");
    assert!(literal - plain < 26 * 10, "{plain} and {literal} bytes");
}

#[test]
fn a_version_is_charged_what_it_adds() {
    // Each version shares all but its edit with the one before, so five
    // thousand of them, all kept, since nothing is freed yet, are charged
    // a few megabytes. Each charged whole, as it was, the array came to 100
    // MB, the map to 200 MB and the string to 25 MB.
    let bytes = |src: &str| {
        let mut s = Session::new();
        if let Err(e) = s.eval(src) {
            panic!("{src}: {e}");
        }
        assert_eq!(s.show_typed(), "<1> 5000 < i64 >", "{src}");
        s.machine.stats.allocated_bytes
    };
    for src in [
        "5000 range < i64 ary > empty [ -1 insert ] fold length",
        "5000 range < i64 i64 map > empty [ = put ] fold length",
        "5000 range \"\" [ drop \"a\" concat ] fold length",
    ] {
        let n = bytes(src);
        assert!(n < 12 * 1024 * 1024, "{src}: {n} bytes");
    }
}
