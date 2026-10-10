//! The command line: the REPL, `fmt`, and the stack shown with its types.
use march8::{Kind, Session};
use std::io::Write;
use std::process::{Command, Stdio};

/// Runs the binary with arguments and input, and returns what it printed.
fn march8(args: &[&str], input: &str) -> String {
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
    let out = child.wait_with_output().unwrap();
    String::from_utf8(out.stdout).unwrap()
}

#[test]
fn the_repl_runs_each_line() {
    // A line ending inside a bracket continues on the next; an error does
    // not end the session; commands start with `:`.
    let out = march8(
        &[],
        "1 2 +.\n\"x\" [ dup.\n concat. ] .\n1 foo.\n:types\n( 1 2 3 ) + scan.\n:q\n",
    );
    assert_eq!(
        out,
        "<1> 3\n<2> 3 \"xx\"\nError: 1:3: no word `foo`\ntypes shown\n\
         <3> 3 < i64 > \"xx\" < string > ( 1 3 6 ) < i64 ary. >\n"
    );
}

#[test]
fn fmt_writes_symbols() {
    assert_eq!(
        march8(&["fmt"], "\\times x\\_1 \"\\times;\" -- \\leq\n"),
        "× x₁ \"\\times;\" -- ≤\n"
    );
}

#[test]
fn the_stack_with_its_types_reads_as_march() {
    let mut s = Session::new();
    s.eval("{ \"a\" ( 1.5 ) } 2").unwrap();
    assert_eq!(
        s.show_typed(),
        "<2> { \"a\" ( 1.5 ) } < string 1 f64 vec. map. > 2 < i64 >"
    );
    // Text that ends inside a bracket or a string is unfinished, not wrong.
    assert_eq!(s.eval("( 1 2").unwrap_err().kind, Kind::Unfinished);
    assert_eq!(s.eval("\"abc").unwrap_err().kind, Kind::Unfinished);
}

#[test]
fn scan_gives_the_running_reductions() {
    let mut s = Session::new();
    s.eval("( 1 2 3 ) + scan. ( 1 2 3 ) -+ scan.").unwrap();
    assert_eq!(s.show(), "<2> ( 1 3 6 ) ( 1 -1 -4 )");
    s.eval("( 5 6 )").unwrap();
    s.eval("+ scan.").unwrap();
    assert_eq!(s.show(), "<3> ( 1 3 6 ) ( 1 -1 -4 ) ( 5 11 )");
}

#[test]
fn stdin_read_reads_standard_input() {
    // `stdin-read.` is all of standard input, as a string.
    let out = march8(
        &["--eval", "stdin-read.lines.length."],
        "apple,3\npear,5\nfig,1\n",
    );
    assert_eq!(out, "<1> 3\n");
    let out = march8(&["--eval", "stdin-read. words."], "one two\nthree\n");
    assert_eq!(out, "<1> ( \"one\" \"two\" \"three\" )\n");
}
