#![allow(dead_code)]

use std::path::Path;
use std::process::Command;

pub struct Fixture {
    dir: tempfile::TempDir,
}

impl Fixture {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        assert!(
            git_cmd(dir.path())
                .args(["init", "-q"])
                .status()
                .unwrap()
                .success()
        );
        Self { dir }
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    pub fn write(&self, rel: &str, content: &str) {
        let path = self.path().join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    pub fn add_all(&self) {
        assert!(
            git_cmd(self.path())
                .args(["add", "-A"])
                .status()
                .unwrap()
                .success()
        );
    }
}

pub fn git_cmd(dir: &Path) -> Command {
    let mut cmd = Command::new("git");
    cmd.current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "Test Author")
        .env("GIT_AUTHOR_EMAIL", "author@example.invalid")
        .env("GIT_COMMITTER_NAME", "Test Committer")
        .env("GIT_COMMITTER_EMAIL", "committer@example.invalid")
        .env("HOME", dir);
    cmd
}

pub fn run_bin(args: &[&str], cwd: &Path) -> (i32, String, String) {
    run_bin_with_env(args, cwd, std::ffi::OsStr::new(""))
}

pub fn run_bin_with_env(
    args: &[&str],
    cwd: &Path,
    cache_dir: &std::ffi::OsStr,
) -> (i32, String, String) {
    run_bin_with_ranking(args, cwd, cache_dir, None)
}

pub fn run_bin_with_ranking(
    args: &[&str],
    cwd: &Path,
    cache_dir: &std::ffi::OsStr,
    ranking: Option<&str>,
) -> (i32, String, String) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_symapper"));
    command.env_remove("SYMAPPER_RANKING");
    if let Some(ranking) = ranking {
        command.env("SYMAPPER_RANKING", ranking);
    }
    let output = command
        .env("SYMAPPER_CACHE_DIR", cache_dir)
        .args(args)
        .current_dir(cwd)
        .env(
            "GIT_CEILING_DIRECTORIES",
            cwd.canonicalize().unwrap().parent().unwrap(),
        )
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    (
        output.status.code().unwrap(),
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
    )
}

/// Run a fixture binary with a deadline, killing and reaping it on timeout.
pub fn run_bin_with_timeout(args: &[&str], cwd: &Path) -> (i32, String) {
    use std::process::Stdio;
    use std::time::{Duration, Instant};
    let mut child = Command::new(env!("CARGO_BIN_EXE_symapper"))
        .env("SYMAPPER_CACHE_DIR", "")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .args(args)
        .current_dir(cwd)
        .env(
            "GIT_CEILING_DIRECTORIES",
            cwd.canonicalize().unwrap().parent().unwrap(),
        )
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let start = Instant::now();
    loop {
        if child.try_wait().unwrap().is_some() {
            let output = child.wait_with_output().unwrap();
            return (
                output.status.code().unwrap(),
                String::from_utf8(output.stderr).unwrap(),
            );
        }
        if start.elapsed() >= Duration::from_secs(10) {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("binary exceeded 10 second timeout");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}
