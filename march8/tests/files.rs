//! Files: `file` makes a path said to be one, `read` reads it when the
//! program runs, and `embed` while compiling, so what follows folds.
use march8::{Kind, Session};

/// A file of this text, removed when dropped.
struct Temp(std::path::PathBuf);

impl Temp {
    fn new(name: &str, text: &str) -> Temp {
        let path = std::env::temp_dir().join(format!("march8-{}-{name}", std::process::id()));
        std::fs::write(&path, text).unwrap();
        Temp(path)
    }
    fn path(&self) -> String {
        self.0.to_str().unwrap().to_string()
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[test]
fn read_when_the_program_runs() {
    let f = Temp::new("read.csv", "name,qty\napple,3\npear,5\n");
    let mut s = Session::new();
    s.eval(&format!("\"{}\" file.to.read.lines.", f.path()))
        .unwrap();
    assert_eq!(s.show(), "<1> ( \"name,qty\" \"apple,3\" \"pear,5\" )");
    // A file shows as its path, applied, so it reads back.
    let mut s = Session::new();
    s.eval(&format!("\"{}\" file.to.", f.path())).unwrap();
    assert_eq!(s.show(), format!("<1> \"{}\" file", f.path()));
}

#[test]
fn embed_while_compiling() {
    // The text is known while compiling, so what follows folds: the code is
    // the constant.
    let f = Temp::new("embed.txt", "one two three");
    let mut s = Session::new();
    let src = format!("\"{}\" file.to.embed.upper.length.", f.path());
    assert_eq!(s.compile(&src).unwrap().0, s.compile("13").unwrap().0);
    s.eval(&src).unwrap();
    assert_eq!(s.show(), "<1> 13");
    // Words that make arrays run when the program does: the stage has no
    // known arrays yet.
    let mut s = Session::new();
    s.eval(&format!("\"{}\" file.to.embed.words.length.", f.path()))
        .unwrap();
    assert_eq!(s.show(), "<1> 3");
}

#[test]
fn text_is_no_file_and_files_can_be_missing() {
    let mut s = Session::new();
    assert_eq!(s.eval("\"x.txt\" read.").unwrap_err().kind, Kind::NoWord);
    let e = s.eval("\"no-such-file.txt\" file.to.read.").unwrap_err();
    assert!(
        matches!(e.kind, Kind::Run(march8::machine::Error::Io(_))),
        "{e}"
    );
    assert!(
        e.to_string().contains("cannot read \"no-such-file.txt\""),
        "{e}"
    );
    let e = s.eval("\"no-such-file.txt\" file.to.embed.").unwrap_err();
    assert_eq!(e.kind, Kind::Io);
    // `embed` needs its path while compiling.
    s.eval("\"no-such-file.txt\"").unwrap();
    assert_eq!(s.eval("file.to.embed.").unwrap_err().kind, Kind::Mismatch);
}
