use std::process::Command;

#[test]
fn both_seed_surfaces_are_available_from_the_cli() {
    for (command, source) in [
        ("eval", "square : ( dup * ) ; 7 square"),
        ("eval-forth", ": square dup * ; 7 square"),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_march-research"))
            .args([command, source])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(stdout.contains("stack (top first): [49]"));
        assert!(stdout.contains("image: "));
    }
}

#[test]
fn cli_returns_failure_for_invalid_source_or_arguments() {
    for args in [vec!["eval", "x :"], vec!["eval"], vec!["eval", "1", "2"]] {
        let output = Command::new(env!("CARGO_BIN_EXE_march-research"))
            .args(args)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(!output.stderr.is_empty());
    }
}
