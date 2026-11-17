mod common;

use common::{Fixture, git_cmd};
use symapper::{Error, Options, list_files};

fn options(fixture: &Fixture) -> Options {
    Options {
        repo: fixture.path().to_path_buf(),
        focus: vec![],
        tokens: 1500,
        exclude: vec![],
        no_cache: true,
        cache_dir: None,
    }
}

fn paths(opts: &Options) -> Vec<String> {
    list_files(opts)
        .unwrap()
        .sources
        .into_iter()
        .map(|f| f.rel)
        .collect()
}

#[test]
fn untracked_file_is_not_listed() {
    let f = Fixture::new();
    f.write("tracked.rs", "fn tracked() {}");
    f.add_all();
    f.write("untracked.rs", "fn hidden() {}");
    assert_eq!(paths(&options(&f)), ["tracked.rs"]);
}

#[test]
fn exclude_glob_drops_files() {
    let f = Fixture::new();
    f.write("gen/generated.rs", "");
    f.write("main.rs", "");
    f.add_all();
    let mut opts = options(&f);
    opts.exclude.push("gen/**".into());
    assert_eq!(paths(&opts), ["main.rs"]);
    opts.exclude.push("[".into());
    assert!(matches!(list_files(&opts), Err(Error::Usage(_))));
    assert!(matches!(symapper::map(&opts), Err(Error::Usage(_))));
}

#[cfg(unix)]
#[test]
fn outside_symlink_is_skipped() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("outside.rs"), "fn outside() {}").unwrap();
    symlink(outside.path().join("outside.rs"), f.path().join("link.rs")).unwrap();
    f.add_all();
    assert!(paths(&options(&f)).is_empty());
}

#[cfg(unix)]
#[test]
fn inside_symlink_keeps_own_path() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    f.write("source.rs", "fn source() {}");
    symlink("source.rs", f.path().join("alias.rs")).unwrap();
    f.add_all();
    let files = list_files(&options(&f)).unwrap().sources;
    assert_eq!(
        files.iter().map(|f| f.rel.as_str()).collect::<Vec<_>>(),
        ["alias.rs", "source.rs"]
    );
    assert_eq!(
        files[0].abs,
        f.path().canonicalize().unwrap().join("source.rs")
    );
    assert_eq!(files[0].size, 14);
    assert!(files[0].mtime_ns > 0);
}

#[test]
fn oversized_file_is_skipped() {
    let f = Fixture::new();
    f.write("large.rs", &"x".repeat(1_048_577));
    f.write("limit.rs", &"x".repeat(1_048_576));
    f.add_all();
    assert_eq!(paths(&options(&f)), ["limit.rs"]);
}

#[test]
fn nul_in_first_eight_kib_is_skipped() {
    let f = Fixture::new();
    f.write("binary.rs", "fn binary() {}\0");
    f.write("later.rs", &format!("{}\0", "x".repeat(8192)));
    f.add_all();
    assert_eq!(paths(&options(&f)), ["later.rs"]);
}

#[cfg(target_os = "linux")]
#[test]
fn non_utf8_path_is_skipped() {
    use std::os::unix::ffi::OsStrExt;
    let f = Fixture::new();
    let name = std::ffi::OsStr::from_bytes(b"bad\xff.rs");
    std::fs::write(f.path().join(name), "fn bad() {}").unwrap();
    f.add_all();
    assert!(paths(&options(&f)).is_empty());
}

#[cfg(unix)]
#[test]
fn hostile_fsmonitor_never_runs() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    f.write("main.rs", "");
    f.add_all();
    f.write("monitor", "#!/bin/sh\ntouch \"$(dirname \"$0\")/marker\"\n");
    let script = f.path().join("monitor");
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(
        git_cmd(f.path())
            .args(["config", "core.fsmonitor"])
            .arg(&script)
            .status()
            .unwrap()
            .success()
    );
    assert_eq!(paths(&options(&f)), ["main.rs"]);
    assert!(!f.path().join("marker").exists());
}

#[test]
fn git_dir_environment_is_ignored_by_binary() {
    let f = Fixture::new();
    f.write("main.rs", "");
    f.add_all();
    let other = tempfile::tempdir().unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_symapper"))
        .arg(f.path())
        .env("GIT_DIR", other.path())
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn subdirectory_lists_from_repo_root() {
    let f = Fixture::new();
    f.write("root.rs", "");
    f.write("nested/child.py", "");
    f.add_all();
    let mut opts = options(&f);
    opts.repo = f.path().join("nested");
    assert_eq!(paths(&opts), ["nested/child.py", "root.rs"]);
}

#[test]
fn root_files_detected_before_content_filters_and_sorted() {
    let f = Fixture::new();
    let roots = [
        ".forgejo/workflows/build.yml",
        ".github/workflows/ci.yml",
        "Cargo.toml",
        "Dockerfile",
        "LICENSE-MIT",
        "Makefile",
        "Package.swift",
        "README.md",
        "compose.dev.yaml",
        "compose.yml",
        "docker-compose.test.yml",
        "docker-compose.yaml",
        "go.mod",
        "package.json",
        "pyproject.toml",
    ];
    for name in roots {
        f.write(name, "\0");
    }
    for name in [
        "nested/README.md",
        ".github/workflows/deep/ci.yml",
        ".github/workflows/ci.yaml",
        "other.txt",
    ] {
        f.write(name, "");
    }
    f.add_all();
    let mut opts = options(&f);
    assert_eq!(list_files(&opts).unwrap().root_files, roots);
    opts.exclude.push("README*".into());
    assert!(
        !list_files(&opts)
            .unwrap()
            .root_files
            .contains(&"README.md".into())
    );
}

#[test]
fn missing_and_non_regular_sources_are_skipped() {
    let f = Fixture::new();
    f.write("deleted.rs", "");
    f.write("directory.rs", "");
    f.add_all();
    std::fs::remove_file(f.path().join("deleted.rs")).unwrap();
    std::fs::remove_file(f.path().join("directory.rs")).unwrap();
    std::fs::create_dir(f.path().join("directory.rs")).unwrap();
    assert!(paths(&options(&f)).is_empty());
}

#[test]
fn language_extensions_are_case_sensitive() {
    use symapper::lang::{Lang, from_path};
    for (extensions, lang) in [
        (&["ts", "mts", "cts", "d.ts"][..], Lang::TypeScript),
        (&["tsx"][..], Lang::Tsx),
        (&["js", "jsx", "mjs", "cjs"][..], Lang::JavaScript),
        (&["py", "pyi"][..], Lang::Python),
        (&["go"][..], Lang::Go),
        (&["rs"][..], Lang::Rust),
        (&["swift"][..], Lang::Swift),
    ] {
        for ext in extensions {
            assert_eq!(from_path(&format!("src/file.{ext}")), Some(lang));
        }
    }
    for name in ["file.RS", "file.txt", "no_extension"] {
        assert_eq!(from_path(name), None);
    }
}

#[cfg(unix)]
#[test]
fn unsafe_symlink_targets_are_skipped() {
    use std::os::unix::fs::symlink;
    for case in ["untracked", "git", "directory"] {
        let f = Fixture::new();
        let outside = tempfile::tempdir().unwrap();
        match case {
            "untracked" => {
                symlink("local.secrets", f.path().join("settings.py")).unwrap();
                f.add_all();
                f.write("local.secrets", "def MARKER_SECRET_1(): pass\n");
            }
            "git" => {
                symlink(".git/config", f.path().join("gitcfg.py")).unwrap();
                f.add_all();
                let config = f.path().join(".git/config");
                let mut text = std::fs::read_to_string(&config).unwrap();
                text.push_str("\n[secret]\n    token = MARKER_SECRET_1\n");
                std::fs::write(config, text).unwrap();
            }
            _ => {
                f.write("d/x.py", "def original(): pass\n");
                f.add_all();
                std::fs::write(outside.path().join("x.py"), "def MARKER_SECRET_1(): pass\n")
                    .unwrap();
                std::fs::remove_dir_all(f.path().join("d")).unwrap();
                symlink(outside.path(), f.path().join("d")).unwrap();
            }
        }
        let (code, stdout, _) = common::run_bin(&["."], f.path());
        assert_eq!(code, 0);
        assert!(!stdout.contains("MARKER_SECRET_1"), "{case}: {stdout}");
        assert!(paths(&options(&f)).is_empty(), "{case}");
    }
}
