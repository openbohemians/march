//! Whole programs (examples/), and the words they needed: `put`, `empty`.
use march8::{Kind, Session};
use std::process::Command;

fn show(src: &str) -> String {
    let mut s = Session::new();
    if let Err(e) = s.eval(src) {
        panic!("{src}: {e}");
    }
    s.show_typed()
}

#[test]
fn word_frequencies() {
    let out = Command::new(env!("CARGO_BIN_EXE_march8"))
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/examples/wordfreq.march"
        ))
        .output()
        .unwrap();
    let out = String::from_utf8(out.stdout).unwrap();
    assert_eq!(
        out,
        "and 2\nbat 1\ncat 1\nhat 1\nmat 1\non 1\nsat 1\nthe 4\n--\n\
         the 4\nand 2\nbat 1\ncat 1\nhat 1\nmat 1\non 1\nsat 1\n<0>\n"
    );
}

#[test]
fn put_sets_and_empty_starts() {
    assert_eq!(
        show("string i64 map. empty. \"a\" 1 put. \"b\" 2 put. \"a\" 3 put."),
        "<1> { \"a\" 3 \"b\" 2 } < string i64 map >"
    );
    assert_eq!(show("string ary. empty. length."), "<1> 0 < i64 >");
    let mut s = Session::new();
    assert_eq!(s.eval("i64 empty.").unwrap_err().kind, Kind::NoWord);
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
    let plain = bytes("1000 range. 0 [ 26 mod. 1 +. +. ] fold.");
    let literal =
        bytes("1000 range. 0 [ \"abcdefghijklmnopqrstuvwxyz\" swap. 26 mod. 1 +. at. +. ] fold.");
    assert!(literal - plain < 26 * 10, "{plain} and {literal} bytes");
}
