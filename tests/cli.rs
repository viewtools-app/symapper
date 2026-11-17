mod common;

use common::{Fixture, run_bin};

#[test]
fn help_exits_zero() {
    let dir = tempfile::tempdir().unwrap();
    let (code, stdout, _) = run_bin(&["--help"], dir.path());
    assert_eq!(code, 0);
    assert!(stdout.contains("Usage: symapper"));
}

#[test]
fn version_prints_crate_version() {
    let dir = tempfile::tempdir().unwrap();
    let (code, stdout, _) = run_bin(&["--version"], dir.path());
    assert_eq!(code, 0);
    assert_eq!(stdout, format!("symapper {}\n", env!("CARGO_PKG_VERSION")));
}

#[test]
fn unknown_flag_exits_two() {
    let dir = tempfile::tempdir().unwrap();
    let (code, _, stderr) = run_bin(&["--unknown"], dir.path());
    assert_eq!(code, 2);
    assert!(!stderr.is_empty());
}

#[test]
fn zero_tokens_exits_two() {
    let dir = tempfile::tempdir().unwrap();
    let (code, _, stderr) = run_bin(&[".", "--tokens", "0"], dir.path());
    assert_eq!(code, 2);
    assert!(stderr.contains("--tokens"));
}

#[test]
fn non_git_directory_exits_two() {
    let dir = tempfile::tempdir().unwrap();
    let (code, stdout, stderr) = run_bin(&["."], dir.path());
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("symapper:"));
}

#[test]
fn missing_path_exits_two() {
    let dir = tempfile::tempdir().unwrap();
    let (code, stdout, stderr) = run_bin(&["missing"], dir.path());
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("missing"));
}

#[test]
fn fixture_repo_exits_zero() {
    let fixture = Fixture::new();
    let (code, stdout, stderr) = run_bin(
        &[
            ".",
            "--focus",
            "src",
            "--focus",
            "main",
            "--tokens",
            "20",
            "--exclude",
            "*.lock",
            "--exclude",
            "vendor/**",
            "--no-cache",
        ],
        fixture.path(),
    );
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.is_empty());
    assert_eq!(stderr, "symapper: no supported source files\n");
}
