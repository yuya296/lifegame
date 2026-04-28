//! Integration tests for the `lifegame` CLI binary.

use assert_cmd::Command;
use predicates::str::contains;

#[test]
fn once_with_blinker_renders_header_and_board() {
    let output = Command::cargo_bin("lifegame")
        .unwrap()
        .args([
            "--once",
            "--pattern",
            "blinker",
            "--width",
            "5",
            "--height",
            "5",
            "--boundary",
            "toroidal",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).expect("non-UTF8 stdout");
    let lines: Vec<&str> = stdout.lines().collect();

    // Expect 1 header line + 5 board lines.
    assert!(
        lines.len() >= 6,
        "expected at least 6 lines, got {}: {:?}",
        lines.len(),
        lines
    );
    assert!(lines[0].starts_with("Gen: "), "unexpected header: {}", lines[0]);
    assert!(lines[0].contains("Alive: 3"), "blinker has 3 alive cells: {}", lines[0]);
    assert!(lines[0].contains("5x5"));
    assert!(lines[0].contains("toroidal"));

    // 5 board rows, each 5 chars wide (in chars, not bytes).
    for row in &lines[1..6] {
        assert_eq!(row.chars().count(), 5, "row width != 5: {row:?}");
    }

    // `--once` is meant to be CI / snapshot friendly: no terminal control codes.
    assert!(
        !stdout.contains('\x1b'),
        "--once must not emit ANSI escape sequences"
    );
}

#[test]
fn list_patterns_includes_known_builtins() {
    let assert = Command::cargo_bin("lifegame")
        .unwrap()
        .arg("--list-patterns")
        .assert()
        .success();

    assert
        .stdout(contains("Still life:"))
        .stdout(contains("Oscillator:"))
        .stdout(contains("Spaceship:"))
        .stdout(contains("Gun:"))
        .stdout(contains("  block"))
        .stdout(contains("  blinker"))
        .stdout(contains("  glider"))
        .stdout(contains("  gosper-glider-gun"));
}

#[test]
fn once_random_smoke() {
    Command::cargo_bin("lifegame")
        .unwrap()
        .args([
            "--once",
            "--pattern",
            "random",
            "--width",
            "8",
            "--height",
            "4",
            "--seed",
            "42",
        ])
        .assert()
        .success();
}

#[test]
fn unknown_pattern_errors() {
    let output = Command::cargo_bin("lifegame")
        .unwrap()
        .args(["--once", "--pattern", "no-such-pattern"])
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();

    let stderr = String::from_utf8(output).expect("non-UTF8 stderr");
    assert!(
        stderr.contains("--list-patterns"),
        "should suggest --list-patterns; got: {stderr}"
    );
}

#[test]
fn seed_is_deterministic() -> anyhow::Result<()> {
    let run = |seed: u64| -> anyhow::Result<Vec<u8>> {
        let out = Command::cargo_bin("lifegame")?
            .args([
                "--once",
                "--pattern",
                "random",
                "--width",
                "20",
                "--height",
                "10",
                "--seed",
            ])
            .arg(seed.to_string())
            .output()?;
        assert!(out.status.success(), "lifegame failed: {:?}", out.status);
        Ok(out.stdout)
    };
    assert_eq!(run(42)?, run(42)?, "same seed must produce identical output");
    assert_ne!(run(42)?, run(43)?, "different seeds should differ");
    Ok(())
}
