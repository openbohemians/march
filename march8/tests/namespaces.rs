//! Namespaces (doc/design/SYMBOLIC-FORTH.md 16, 17): maps all the way down,
//! a dot in a name dividing the path. A word's names are looked up in its
//! namespace, then among the namespace's import, then in the core.
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

const MATH: &str = "math.pi: [ 3 ] ; math.sq: [ dup * ] ; math.area: [ sq pi * ] ;";

#[test]
fn a_path_names_a_word_in_its_namespace() {
    assert_eq!(show(&format!("{MATH} 2 math.area math.pi")), "<2> 12 3");
    // `;` at a path makes the namespace on the way.
    assert_eq!(show("a.b.cc: [ 7 ] ; a.b.cc"), "<1> 7");
    let (kind, msg) = error(&format!("{MATH} math.tau"));
    assert_eq!(kind, Kind::NoWord);
    assert_eq!(msg, "no word `tau` in `math`");
}

#[test]
fn a_word_looks_up_names_in_its_namespace_where_written() {
    // `sq` in a quotation that `map` runs is still `math.sq`, and `+'` the
    // core's.
    let src = format!("{MATH} math.sum-sq: [ [ sq ] map 0 +' fold ] ; ( 1 2 3 ) math.sum-sq");
    assert_eq!(show(&src), "<1> 14");
    // A word defined later is found when applied.
    assert_eq!(show("ns.ff: [ gg ] ; ns.gg: [ 5 ] ; ns.ff"), "<1> 5");
    // A word of the program's root is not the core: it is imported.
    let (_, msg) = error("helper: [ 1 ] ; ns.ff: [ helper ] ; ns.ff");
    assert!(
        msg.contains("no word `helper` in `ns`, its import, or the core"),
        "{msg}"
    );
}

#[test]
fn an_import_makes_names_local() {
    // Each path by its last part, and `math.*` all that `math` exports.
    assert_eq!(
        show(&format!("{MATH} import#: <| math.* |> ; 2 sq pi *")),
        "<1> 12"
    );
    assert_eq!(
        show(&format!("{MATH} import#: <| math.pi |> ; pi")),
        "<1> 3"
    );
    // A namespace's own import, for its words.
    let app = "io.get: [ 7 ] ; app.import#: <| io.get |> ; app.main: [ get 1 + ] ;";
    assert_eq!(show(&format!("{app} app.main")), "<1> 8");
    // An import holds for all of its namespace, wherever it is written.
    assert_eq!(
        show(&format!("{MATH} pi import#: <| math.pi |> ;")),
        "<1> 3"
    );
}

#[test]
fn a_name_found_twice_is_an_error() {
    let (kind, msg) = error("a.ff: [ 1 ] ; b.ff: [ 2 ] ; import#: <| a.* b.* |> ; ff");
    assert_eq!(kind, Kind::NoWord);
    assert!(
        msg.contains("`ff` names both `a.ff`, imported, and `b.ff`, imported"),
        "{msg}"
    );
    let (_, msg) = error("math.pi: [ 3 ] ; pi: [ 4 ] ; import#: <| math.pi |> ; pi");
    assert!(
        msg.contains("`pi`, defined here, and `math.pi`, imported"),
        "{msg}"
    );
    // A path says which.
    assert_eq!(
        show("math.pi: [ 3 ] ; pi: [ 4 ] ; import#: <| math.pi |> ; math.pi"),
        "<1> 3"
    );
}

#[test]
fn a_namespace_has_one_import() {
    let (kind, msg) = error("import#: <| a.x |> ; import#: <| b.y |> ;");
    assert_eq!(kind, Kind::Syntax);
    assert!(msg.contains("the root has its `import#` already"), "{msg}");
}

#[test]
fn reaching_what_is_not_exported_is_a_severe_warning() {
    let math = "math.pi: [ 3 ] ; math.ee: [ 2 ] ; math.export#: <| pi |> ;";
    let mut s = Session::new();
    s.eval(&format!("{math} import#: <| math.* |> ; pi math.ee"))
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
    let (kind, _) = error(&format!("{math} import#: <| math.* |> ; ee"));
    assert_eq!(kind, Kind::NoWord);
    // A namespace's children see its unexported words.
    let mut s = Session::new();
    s.eval(&format!("{math} math.test.tt: [ math.ee ] ; math.test.tt"))
        .unwrap();
    assert!(s.take_warnings().is_empty());
}
