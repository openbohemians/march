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
        "1 2 +\n\"x\" [ dup\n concat ] .\n1 foo\n:types\n( 1 2 3 ) +' scan\n:q\n",
    );
    assert_eq!(
        out,
        "<1> 3\n<2> 3 \"xx\"\nError: 1:3: no word `foo`\ntypes shown\n\
         <3> 3 < i64 > \"xx\" < string > ( 1 3 6 ) < i64 ary >\n"
    );
}

#[test]
fn fmt_writes_symbols() {
    assert_eq!(
        march8(&["fmt"], "\\times x\\_1 \"\\times;\" -- \\leq\n"),
        "× x₁ \"\\times;\" -- ≤\n"
    );
    // A name's trailing `_` and `^` and digits, as subscript and superscript
    // digits, before its marks; not a string's, a comment's or a number's.
    assert_eq!(
        march8(
            &["fmt"],
            "dup_2 +^4 dup_2^3: file_read 1_000 \"x_1\" -- x_1\n"
        ),
        "dup₂ +⁴ dup₂³: file_read 1_000 \"x_1\" -- x_1\n"
    );
    // A block comment is written as it is, escapes aside.
    assert_eq!(
        march8(&["fmt"], "---\nx_1 \"a \\leq\n---.\nx_1\n"),
        "---\nx_1 \"a ≤\n---.\nx₁\n"
    );
    // A quoted symbol is written as it is, to its quote not doubled.
    assert_eq!(
        march8(&["fmt"], "'it''s \\times' \\times\n"),
        "'it''s \\times' ×\n"
    );
}

#[test]
fn a_formatted_file_reads_as_its_source() {
    use march8::read::read;
    use march8::symbols::Symbols;
    let symbols = Symbols::standard();
    let toks = |s: &str| -> Vec<_> {
        read(s, &symbols)
            .unwrap()
            .into_iter()
            .map(|t| t.tok)
            .collect()
    };
    let source = format!("{}\n1 2 dup_2 +^4\n", include_str!("../core/core.march"));
    let formatted = symbols.format(&source);
    assert!(formatted.contains("dup₂ +⁴"));
    assert_eq!(toks(&formatted), toks(&source));
}

#[test]
fn lower_writes_symbolic_forth() {
    // Every application a dot, `!` converting (doc/design/TYPES.md 2.2), and
    // a number a symbol given its kind (doc/design/SYMBOLIC-FORTH.md 6.7).
    assert_eq!(
        march8(&["lower"], "sq: dup * ;\n3 sq 2 f64!\n"),
        "sq [ dup . * . ] ;\n3 int# ! sq . 2 int# ! f64 . !\n"
    );
    // And `--symbolic` runs it.
    assert_eq!(
        march8(
            &["--symbolic", "--eval", "sq [ dup . * . ] ; 3 int# ! sq ."],
            ""
        ),
        "<1> 9\n"
    );
}

#[test]
fn the_stack_with_its_types_reads_as_march() {
    let mut s = Session::new();
    s.eval("{ \"a\" ( 1.5 ) } 2").unwrap();
    assert_eq!(
        s.show_typed(),
        "<2> { \"a\" ( 1.5 ) } < string 1 f64 vec map > 2 < i64 >"
    );
    // Text that ends inside a bracket or a string is unfinished, not wrong.
    assert_eq!(s.eval("( 1 2").unwrap_err().kind, Kind::Unfinished);
    assert_eq!(s.eval("\"abc").unwrap_err().kind, Kind::Unfinished);
}

#[test]
fn scan_gives_the_running_reductions() {
    let mut s = Session::new();
    s.eval("( 1 2 3 ) +' scan ( 1 2 3 ) -+' scan").unwrap();
    assert_eq!(s.show(), "<2> ( 1 3 6 ) ( 1 -1 -4 )");
    s.eval("( 5 6 )").unwrap();
    s.eval("+' scan").unwrap();
    assert_eq!(s.show(), "<3> ( 1 3 6 ) ( 1 -1 -4 ) ( 5 11 )");
}

#[test]
fn stdin_read_reads_standard_input() {
    // `io.stdin.read` is all of standard input, as a string.
    let out = march8(
        &["--eval", "io.stdin.read lines length"],
        "apple,3\npear,5\nfig,1\n",
    );
    assert_eq!(out, "<1> 3\n");
    let out = march8(&["--eval", "io.stdin.read words"], "one two\nthree\n");
    assert_eq!(out, "<1> ( \"one\" \"two\" \"three\" )\n");
}

#[test]
fn a_s4_file_is_low_stack() {
    // `march8 lower` writes High Stack, `.march`, as Low Stack, which a file
    // ending in `.s4` is read as, and it runs the same.
    let src = include_str!("../examples/door.march");
    let low = march8(&["lower"], src);
    let path = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("door.s4");
    std::fs::write(&path, &low).unwrap();
    let high = march8(&["examples/door.march"], "");
    assert_eq!(march8(&[path.to_str().unwrap()], ""), high);
    assert_eq!(high, "<2> Locked' Closed'\n");
    // Read as High Stack, Low Stack's names apply where a dot would.
    let path = path.with_extension("march");
    std::fs::write(&path, &low).unwrap();
    assert_ne!(march8(&[path.to_str().unwrap()], ""), high);
}
