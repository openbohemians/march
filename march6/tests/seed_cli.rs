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

#[test]
fn cli_collects_across_batches_without_forcing_discarded_work() {
    let source = format!("{}9223372036854775807 1 + drop 7", "0 drop ".repeat(31));
    let output = Command::new(env!("CARGO_BIN_EXE_march-research"))
        .args(["eval", &source])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("stack (top first): [7]"));
    assert!(stdout.contains("collection: epochs="));
    let epochs: usize = stdout
        .split("collection: epochs=")
        .nth(1)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .parse()
        .unwrap();
    assert!(epochs >= 2);
}

#[test]
fn cli_demands_overflow_even_when_last_token_exactly_fills_a_batch() {
    let source = format!("{}0 9223372036854775807 1 +", "0 drop ".repeat(30));
    assert_eq!(source.split_whitespace().count(), 64);
    let output = Command::new(env!("CARGO_BIN_EXE_march-research"))
        .args(["eval", &source])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("IntegerOverflow"));
}
