//! Namespaces (doc/design/SYMBOLIC-FORTH.md 16, 17): maps all the way down,
//! a dot in a name dividing the path. Each source is expanded once read:
//! each name it applies becomes its path where it was written, looked up in
//! its namespace, then among the namespace's import, then in the core. A
//! symbol is global text.
use march8::{Kind, Level, Session};

fn show(src: &str) -> String {
    let mut s = Session::new();
    if let Err(e) = s.eval(src) {
        panic!("{src}: {e}");
    }
    s.show()
}

fn error(src: &str) -> (Kind, String) {
    let e = Session::new().eval(src).expect_err(src);
    (e.kind, e.msg.clone())
}

const MATH: &str = "math.pi: 3 ; math.sq: dup * ; math.area: sq pi * ;";

#[test]
fn a_path_names_a_word_in_its_namespace() {
    assert_eq!(show(&format!("{MATH} 2 math.area math.pi")), "<2> 12 3");
    // `;` at a path makes the namespace on the way.
    assert_eq!(show("a.b.cc: 7 ; a.b.cc"), "<1> 7");
    let (kind, msg) = error(&format!("{MATH} math.tau"));
    assert_eq!(kind, Kind::NoWord);
    assert_eq!(msg, "no word `tau` in `math`");
}

#[test]
fn a_word_looks_up_names_in_its_namespace_where_written() {
    // `sq` in a quotation that `map` runs is still `math.sq`, and `+'` the
    // core's.
    let src = format!("{MATH} math.sum-sq: [ sq ] map 0 +' fold ; ( 1 2 3 ) math.sum-sq");
    assert_eq!(show(&src), "<1> 14");
    // A word defined later is found when applied.
    assert_eq!(show("ns.ff: gg ; ns.gg: 5 ; ns.ff"), "<1> 5");
    // So by a typed word, whose check, made as it is defined, waits for it.
    assert_eq!(show("ff: < i64 >. gg + ; gg: 7 ; 1 ff"), "<1> 8");
    assert_eq!(show("ns.ff: < i64 >. gg + ; ns.gg: 7 ; 1 ns.ff"), "<1> 8");
    let (kind, msg) = error("ff: < i64 >. gg ; 1 ff gg: 7 ;");
    assert_eq!(kind, Kind::NoWord);
    assert!(msg.contains("no word `gg` yet"), "{msg}");
    // A word of the program's root is not the core: it is imported.
    let (_, msg) = error("helper: 1 ; ns.ff: helper ; ns.ff");
    assert!(
        msg.contains("no word `helper` in `ns`, its import, or the core"),
        "{msg}"
    );
    assert_eq!(
        show("helper: 1 ; ns.import#: [| helper |] ; ns.ff: helper ; ns.ff"),
        "<1> 1"
    );
    // A namespace's own word counts wherever its source defines it, before
    // the core's where they fit as well (Thomas, 2026-10-24).
    assert_eq!(
        show("ns.ff: + ; ns.+: < i64 i64 >. * ; 2 i64! 3 i64! ns.ff"),
        "<1> 6"
    );
}

#[test]
fn a_symbol_is_global_text() {
    // Symbols are global (Thomas, 2026-10-20): `dbl'` is `dbl'` wherever it
    // is written, so handed to `map`, it is the root's `dbl`. The
    // namespace's is its path, or a quotation's, expanded where written.
    let (kind, msg) = error("ns.dbl: dup + ; ns.ff: ( 1 2 ) dbl' map ; ns.ff");
    assert_eq!(kind, Kind::NoWord);
    assert!(msg.contains("no word `dbl`"), "{msg}");
    assert_eq!(
        show("ns.dbl: dup + ; ns.ff: ( 1 2 ) ns.dbl' map ; ns.ff"),
        "<1> ( 2 4 )"
    );
    assert_eq!(
        show("ns.dbl: dup + ; ns.ff: ( 1 2 ) [ dbl ] map ; ns.ff"),
        "<1> ( 2 4 )"
    );
    let times = "ns.+: < i64 i64 >. * ;";
    assert_eq!(
        show(&format!("{times} ns.ff: ( 2 3 4 ) +' reduce ; ns.ff")),
        "<1> 9"
    );
    assert_eq!(
        show(&format!("{times} ns.ff: ( 2 3 4 ) [ + ] reduce ; ns.ff")),
        "<1> 24"
    );
    // It compares as text wherever written.
    assert_eq!(show("ns.ff: Open' ; ns.ff Open' eq?"), "<1> true");
}

#[test]
fn a_meta_op_is_no_name() {
    // The reader knows the meta-ops and nothing looks them up, so nothing
    // defines them (doc/design/SYMBOLIC-FORTH.md 16).
    for src in [
        "!: < i64 >. drop 7 ;",
        "';' [ drop ] ;",
        "'.' [ drop ] ;",
        "'?' [ drop ] ;",
    ] {
        let (kind, msg) = error(src);
        assert_eq!(kind, Kind::Syntax);
        assert!(msg.contains("is a meta-op, which is no name"), "{msg}");
    }
}

#[test]
fn an_import_makes_names_local() {
    // Each path by its last part, and `math.*` all that `math` exports.
    assert_eq!(
        show(&format!("{MATH} import#: [| math.* |] ; 2 sq pi *")),
        "<1> 12"
    );
    assert_eq!(
        show(&format!("{MATH} import#: [| math.pi |] ; pi")),
        "<1> 3"
    );
    // A namespace's own import, for its words.
    let app = "io.get: 7 ; app.import#: [| io.get |] ; app.main: get 1 + ;";
    assert_eq!(show(&format!("{app} app.main")), "<1> 8");
    // An import holds for all of its namespace, wherever it is written.
    assert_eq!(
        show(&format!("{MATH} pi import#: [| math.pi |] ;")),
        "<1> 3"
    );
}

#[test]
fn imports_are_looked_up_in_order() {
    // Imports are ordered, the first one's coming first, and a name of the
    // namespace's own before any import's.
    let two = "a.ff: 1 ; b.ff: 2 ;";
    assert_eq!(show(&format!("{two} import#: [| a.* b.* |] ; ff")), "<1> 1");
    assert_eq!(show(&format!("{two} import#: [| b.* a.* |] ; ff")), "<1> 2");
    assert_eq!(
        show("math.pi: 3 ; pi: 4 ; import#: [| math.pi |] ; pi"),
        "<1> 4"
    );
    // Clause by clause: where none of a word's clauses fits, the lookup goes
    // on to the next import's.
    let typed = "a.ff: < i64 >. drop 1 ; b.ff: < string >. drop 2 ;";
    assert_eq!(
        show(&format!("{typed} import#: [| a.* b.* |] ; 5 ff \"x\" ff")),
        "<2> 1 2"
    );
    assert_eq!(
        show(&format!(
            "{typed} ns.import#: [| a.* b.* |] ; ns.gg: ff ; \"x\" ns.gg"
        )),
        "<1> 2"
    );
    // A path says which.
    assert_eq!(
        show("math.pi: 3 ; pi: 4 ; import#: [| math.pi |] ; math.pi"),
        "<1> 3"
    );
}

#[test]
fn a_namespace_has_one_import() {
    let (kind, msg) = error("import#: [| a.x |] ; import#: [| b.y |] ;");
    assert_eq!(kind, Kind::Syntax);
    assert!(msg.contains("the root has its `import#` already"), "{msg}");
}

#[test]
fn reaching_what_is_not_exported_is_a_severe_warning() {
    let math = "math.pi: 3 ; math.ee: 2 ; math.export#: [| pi |] ;";
    let mut s = Session::new();
    s.eval(&format!("{math} import#: [| math.* |] ; pi math.ee"))
        .unwrap();
    assert_eq!(s.show(), "<2> 3 2");
    let w = s.take_warnings();
    assert_eq!(w.len(), 1);
    assert_eq!(w[0].level, Level::Severe);
    assert!(
        w[0].msg
            .contains("`math.ee` is reached from outside `math`"),
        "{}",
        w[0].msg
    );
    // `math.*` brings only what `math` exports.
    let (kind, _) = error(&format!("{math} import#: [| math.* |] ; ee"));
    assert_eq!(kind, Kind::NoWord);
    // A namespace's children see its unexported words.
    let mut s = Session::new();
    s.eval(&format!("{math} math.test.tt: math.ee ; math.test.tt"))
        .unwrap();
    assert!(s.take_warnings().is_empty());
}

#[test]
fn the_core_names_its_words_by_namespace() {
    // Reading and case styles are words of the core's namespaces (TYPES.md
    // 3.12), not families chosen by symbols.
    assert_eq!(
        show("\"Cargo.toml\" io.file.read lines first \"Hello World\" case.snake"),
        "<2> \"[package]\" \"hello_world\""
    );
    assert_eq!(
        show(
            "\"helloWorld\" case.kebab \"hello_world\" case.camel \"HTTPServer error\" case.pascal"
        ),
        "<3> \"hello-world\" \"helloWorld\" \"HttpServerError\""
    );
    // Imported, a style is its last name, and anyone may add one.
    assert_eq!(
        show("case.shout: upper \"!\" concat ; import#: [| case.* |] ; \"hi\" snake shout"),
        "<1> \"HI!\""
    );
    // One alone, too.
    assert_eq!(
        show("import#: [| case.lower |] ; \"Hi\" lower ( \"A\" ) lower"),
        "<2> \"hi\" ( \"a\" )"
    );
    assert_eq!(error("\"Hi\" lower").0, Kind::NoWord);
}

#[test]
fn the_core_is_an_import() {
    // The core is `core`, which every namespace imports, its words looked
    // up after the namespace's own: a word of the program's of the same name
    // is tried first, and `core.dup` is still the core's.
    assert_eq!(show("dup: 7 ; 1 dup 1 core.dup"), "<4> 1 7 1 1");
    assert_eq!(show("m.dup: 9 ; m.ff: dup ; 1 m.ff"), "<2> 1 9");
    // The core's words mean what they mean in the core, and a symbol
    // applied from the stack is found from the root.
    assert_eq!(show("dup: 7 ; 1 2 over"), "<3> 1 2 1");
    assert_eq!(show("dup: 7 ; 1 dup' ."), "<2> 1 7");
    // A name is looked up clause by clause, so where none of the program's
    // clauses fits, the lookup goes on to the core's, and the program writes
    // nothing into the core (Thomas, 2026-10-24).
    let bools = "length: < bool >. drop 1 ; true length \"abc\" length";
    assert_eq!(show(bools), "<2> 1 3");
    let (kind, msg) = error("length: < bool >. drop 1 ; 2.0 sqrt length");
    assert_eq!(kind, Kind::NoWord);
    assert!(msg.starts_with("no word `length` for f64"), "{msg}");
    // Messages name the core's words as the root finds them.
    let (kind, msg) = error("\"a\" 1 +");
    assert_eq!(kind, Kind::NoWord);
    assert!(msg.starts_with("no word `+` for"), "{msg}");
    let (_, msg) = error("+: 7 ; \"a\" 1 core.+");
    assert!(msg.starts_with("no word `core.+` for"), "{msg}");
    let (_, msg) = error("core.foo");
    assert_eq!(msg, "no word `foo` in `core`");
    // A source applies its own word wherever it is written.
    let (kind, msg) = error("i64: < i64 f64 or > ;");
    assert_eq!(kind, Kind::NoWord);
    assert!(msg.contains("the core's is `core.i64`"), "{msg}");
}
