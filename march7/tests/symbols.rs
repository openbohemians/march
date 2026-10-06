//! Symbol names (docs/SURFACE.md): `\times` in source is the word `×`, in
//! March's reader and in the formatter, which share seed/system.march's table.
#[path = "../tools/assembler.rs"]
mod assembler;
use march7::{Driver, Error, Image};

fn system() -> Image {
    let g0 = assembler::assemble(include_str!("../seed/system.asm")).unwrap();
    let mut d = Driver::boot(&g0).unwrap();
    d.fuel = 120_000_000;
    d.evaluate(include_str!("../seed/system.march")).unwrap();
    let boot = d.machine.stack.pop().unwrap();
    d.system_image(boot).unwrap()
}

fn run(image: &Image, source: &str) -> Result<Vec<u64>, Error> {
    let mut d = Driver::boot(image).unwrap();
    d.fuel = 100_000_000;
    d.evaluate(source)?;
    Ok(d.machine.stack.clone())
}

#[test]
fn an_escape_names_the_same_word_as_its_symbol() {
    let g = system();
    // Defined with the escape, used with the symbol, and the other way round.
    assert_eq!(run(&g, ": \\times u* ; 3 4 ×").unwrap(), [12]);
    assert_eq!(run(&g, ": × u* ; 3 4 \\times").unwrap(), [12]);
    // Escapes inside a word, subscripts and superscripts.
    assert_eq!(run(&g, ": x\\_1 7 ; : x\\^2 9 ; x₁ x²").unwrap(), [7, 9]);
    // Tick and the exported find see through escapes too.
    assert_eq!(run(&g, ": \\leq 5 ; ' ≤ ' \\leq eq?").unwrap(), [1]);
    // The longest run of letters is the name: \infty is not \in followed by fty.
    assert_eq!(run(&g, ": ∞ 1 ; : ∈ 2 ; \\infty \\in").unwrap(), [1, 2]);
    // An unknown name is an unknown word, as before.
    assert_eq!(run(&g, "\\nosuchname"), Err(Error::User(1)));
}

#[test]
fn the_formatter_rewrites_code_and_comments_but_not_strings() {
    let s = march7::symbols::Symbols::standard();
    assert_eq!(
        s.format(": \\times u* ; -- a \\times b"),
        ": × u* ; -- a × b"
    );
    assert_eq!(s.format("x\\_1 y\\^2 \\infty \\in"), "x₁ y² ∞ ∈");
    // Strings, unknown names and a bare backslash are left as written.
    assert_eq!(
        s.format("\"\\times\" s\" \\leq\""),
        "\"\\times\" s\" \\leq\""
    );
    assert_eq!(s.format("\\nope \\ \\\\"), "\\nope \\ \\\\");
    // `--` inside a word is not a comment; a quote inside a comment is not a
    // string.
    assert_eq!(s.format("a--b \\pi -- 5\" \\pi"), "a--b π -- 5\" π");
    // Formatting twice changes nothing more.
    let once = s.format(include_str!("../docs/surface/calc.march"));
    assert_eq!(s.format(&once), once);
}

#[test]
fn the_table_is_well_formed_for_the_reader() {
    let s = march7::symbols::Symbols::standard();
    let mut count = 0;
    for (name, symbol) in s.entries() {
        count += 1;
        // The reader rewrites into a buffer no longer than the word.
        assert!(symbol.len() <= 1 + name.len(), "{name}");
        // Names are what the reader can scan: `_` or `^` and one character,
        // or ASCII letters.
        let b = name.as_bytes();
        assert!(
            (b.len() == 2 && (b[0] == b'_' || b[0] == b'^'))
                || b.iter().all(u8::is_ascii_alphabetic),
            "{name}"
        );
    }
    assert!(count > 100);
}

#[test]
fn formatted_and_unformatted_source_compile_the_same() {
    let g = system();
    let s = march7::symbols::Symbols::standard();
    let source = ": \\times u* ; : sq\\_2 dup \\times ; 7 sq\\_2 ' sq\\_2";
    let formatted = s.format(source);
    assert_eq!(formatted, ": × u* ; : sq₂ dup × ; 7 sq₂ ' sq₂");
    let cid = |src: &str| {
        let mut d = Driver::boot(&g).unwrap();
        d.evaluate(src).unwrap();
        let xt = d.machine.stack.pop().unwrap();
        (d.machine.stack.clone(), d.machine.cid(xt).unwrap())
    };
    assert_eq!(cid(source), cid(&formatted));
}
