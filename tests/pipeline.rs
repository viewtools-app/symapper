mod common;

use common::Fixture;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};
use symapper::{Options, cache, collect};

fn fixture() -> Fixture {
    let repo = Fixture::new();
    repo.write("z.rs", "fn zebra() {}\n");
    repo.write("a.rs", "fn alpha() { zebra(); }\n");
    repo.write("README.md", "Example\n");
    repo.add_all();
    repo
}

fn options(repo: &Fixture) -> Options {
    Options {
        repo: repo.path().to_owned(),
        focus: vec![],
        tokens: 1500,
        exclude: vec![],
        no_cache: false,
        cache_dir: None,
    }
}

fn cache_file(repo: &Fixture) -> PathBuf {
    cache::cache_path(&repo.path().join(".git"), None).unwrap()
}

fn old_mtime(path: &Path) {
    fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000))
        .unwrap();
}

#[test]
fn collect_without_cache_override_writes_default_cache() {
    let repo = fixture();
    let opts = options(&repo);
    assert!(opts.cache_dir.is_none());
    let result = collect(&opts).unwrap();
    let path = repo.path().join(".git/symapper-cache.bin");
    assert!(
        path.is_file(),
        "collect must use the default cache location"
    );
    assert_eq!(cache::load(&path).unwrap(), result.files);
}

#[test]
fn second_run_reuses_every_file_without_rewriting_cache() {
    let repo = fixture();
    let first = collect(&options(&repo)).unwrap();
    assert_eq!((first.stats.parsed, first.stats.reused), (2, 0));
    assert_eq!(first.root_files, ["README.md"]);
    assert_eq!(
        first
            .files
            .iter()
            .map(|f| f.path.as_str())
            .collect::<Vec<_>>(),
        ["a.rs", "z.rs"]
    );
    assert!(first.files.iter().all(|f| !f.tags.is_empty()));
    old_mtime(&cache_file(&repo));
    let before = snapshot(repo.path());
    let second = collect(&options(&repo)).unwrap();
    assert_eq!((second.stats.parsed, second.stats.reused), (0, 2));
    assert_eq!(first.files, second.files);
    assert_eq!(before, snapshot(repo.path()));
}

#[test]
fn touching_one_file_reparses_only_it() {
    let repo = fixture();
    collect(&options(&repo)).unwrap();
    old_mtime(&repo.path().join("a.rs"));
    let result = collect(&options(&repo)).unwrap();
    assert_eq!((result.stats.parsed, result.stats.reused), (1, 1));
}

#[test]
fn changed_size_invalidates_even_with_matching_mtime() {
    let repo = fixture();
    collect(&options(&repo)).unwrap();
    let path = repo.path().join("a.rs");
    let mtime = fs::metadata(&path).unwrap().modified().unwrap();
    repo.write("a.rs", "fn changed_and_longer() {}\n");
    fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(mtime)
        .unwrap();
    let result = collect(&options(&repo)).unwrap();
    assert_eq!((result.stats.parsed, result.stats.reused), (1, 1));
    assert!(
        result.files[0]
            .tags
            .iter()
            .any(|t| t.name == "changed_and_longer")
    );
}

#[test]
fn corrupt_cache_is_rebuilt_and_valid() {
    let repo = fixture();
    collect(&options(&repo)).unwrap();
    fs::write(cache_file(&repo), b"corrupt").unwrap();
    let result = collect(&options(&repo)).unwrap();
    assert_eq!((result.stats.parsed, result.stats.reused), (2, 0));
    assert_eq!(cache::load(&cache_file(&repo)).unwrap(), result.files);
}

#[test]
fn no_cache_does_not_read_or_write_existing_cache() {
    let repo = fixture();
    collect(&options(&repo)).unwrap();
    old_mtime(&cache_file(&repo));
    let before = snapshot(repo.path());
    let mut opts = options(&repo);
    opts.no_cache = true;
    let result = collect(&opts).unwrap();
    assert_eq!((result.stats.parsed, result.stats.reused), (2, 0));
    assert_eq!(before, snapshot(repo.path()));
}

#[test]
fn no_cache_leaves_absent_cache_absent() {
    let repo = fixture();
    let before = snapshot(repo.path());
    let mut opts = options(&repo);
    opts.no_cache = true;
    collect(&opts).unwrap();
    assert!(!cache_file(&repo).exists());
    assert_eq!(before, snapshot(repo.path()));
}

#[test]
fn gone_entries_are_dropped_and_excluded_entries_are_kept_without_parsing() {
    let repo = fixture();
    collect(&options(&repo)).unwrap();
    fs::remove_file(repo.path().join("z.rs")).unwrap();
    let result = collect(&options(&repo)).unwrap();
    assert_eq!((result.stats.parsed, result.stats.reused), (0, 1));
    assert_eq!(cache::load(&cache_file(&repo)).unwrap(), result.files);
    let kept = result.files;
    old_mtime(&cache_file(&repo));
    let before = snapshot(repo.path());
    let mut opts = options(&repo);
    opts.exclude.push("*.rs".into());
    let result = collect(&opts).unwrap();
    assert_eq!((result.stats.parsed, result.stats.reused), (0, 0));
    assert!(result.files.is_empty());
    assert_eq!(cache::load(&cache_file(&repo)).unwrap(), kept);
    assert_eq!(before, snapshot(repo.path()));
}

type Snapshot = BTreeMap<PathBuf, (u64, SystemTime, Vec<u8>)>;

// Include file lengths, modification times and contents, including git state.
fn snapshot(root: &Path) -> Snapshot {
    fn walk(root: &Path, dir: &Path, result: &mut Snapshot) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(root, &path, result);
            } else {
                let meta = fs::metadata(&path).unwrap();
                result.insert(
                    path.strip_prefix(root).unwrap().to_owned(),
                    (
                        meta.len(),
                        meta.modified().unwrap(),
                        fs::read(path).unwrap(),
                    ),
                );
            }
        }
    }
    let mut result = BTreeMap::new();
    walk(root, root, &mut result);
    result
}

#[test]
fn map_changes_only_cache_in_git_dir() {
    let repo = fixture();
    let before = snapshot(repo.path());
    let (code, stdout, stderr) = common::run_bin(&["."], repo.path());
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.starts_with("README.md\n"));
    let mut after = snapshot(repo.path());
    assert!(after.remove(Path::new(".git/symapper-cache.bin")).is_some());
    assert_eq!(before, after);
}

#[test]
fn foreign_cache_file_is_untouched_and_cache_is_written() {
    let repo = fixture();
    let old_cache = repo.path().join(".git/other-tool-cache.bin");
    fs::write(&old_cache, b"foreign cache").unwrap();
    let (code, _, stderr) = common::run_bin(&["."], repo.path());
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(fs::read(old_cache).unwrap(), b"foreign cache");
    assert_eq!(cache::load(&cache_file(&repo)).unwrap().len(), 2);
}

#[test]
fn shared_env_directory_keeps_repo_caches_separate_and_reuses_them() {
    let dir = tempfile::tempdir().unwrap();
    for _ in 0..2 {
        let repo = fixture();
        let before = snapshot(repo.path());
        let (code, stdout, stderr) =
            common::run_bin_with_env(&["."], repo.path(), dir.path().as_os_str());
        assert_eq!(code, 0, "{stderr}");
        assert!(stderr.is_empty(), "{stderr}");
        assert!(!stdout.is_empty());
        assert_eq!(before, snapshot(repo.path()));
        let files = snapshot(dir.path());
        let (code, again, stderr) =
            common::run_bin_with_env(&["."], repo.path(), dir.path().as_os_str());
        assert_eq!(code, 0, "{stderr}");
        assert_eq!(stdout, again);
        assert_eq!(files, snapshot(dir.path()));
    }
    let files = snapshot(dir.path());
    assert_eq!(files.len(), 2);
    for path in files.keys() {
        let name = path.to_str().unwrap();
        let hash = name
            .strip_prefix("symapper-")
            .unwrap()
            .strip_suffix(".bin")
            .unwrap();
        assert_eq!(hash.len(), 16);
        assert!(
            hash.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        );
        assert_eq!(cache::load(&dir.path().join(path)).unwrap().len(), 2);
    }
}

#[test]
fn invalid_env_directory_disables_cache_with_one_note() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("file");
    fs::write(&file, b"unchanged").unwrap();
    for value in [Path::new("relative"), &dir.path().join("missing"), &file] {
        let repo = fixture();
        let (code, _, stderr) = common::run_bin(&["."], repo.path());
        assert_eq!(code, 0, "{stderr}");
        fs::write(cache_file(&repo), b"corrupt cache must not be read").unwrap();
        let before = snapshot(repo.path());
        let env_before = snapshot(dir.path());
        let (code, stdout, stderr) =
            common::run_bin_with_env(&["."], repo.path(), value.as_os_str());
        assert_eq!(code, 0, "{stderr}");
        assert!(!stdout.is_empty());
        assert_eq!(stderr.lines().count(), 1, "{stderr}");
        assert!(
            stderr.starts_with("symapper: SYMAPPER_CACHE_DIR ignored: "),
            "{stderr}"
        );
        assert_eq!(before, snapshot(repo.path()));
        assert_eq!(env_before, snapshot(dir.path()));
    }
}

#[test]
fn no_cache_disables_env_cache_reads_and_writes() {
    let repo = fixture();
    let dir = tempfile::tempdir().unwrap();
    let before = snapshot(repo.path());
    let (code, _, stderr) =
        common::run_bin_with_env(&[".", "--no-cache"], repo.path(), dir.path().as_os_str());
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(snapshot(dir.path()).len(), 0);
    let (code, _, stderr) = common::run_bin_with_env(&["."], repo.path(), dir.path().as_os_str());
    assert_eq!(code, 0, "{stderr}");
    let path = fs::read_dir(dir.path())
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    fs::write(path, b"corrupt cache must not be read").unwrap();
    let cached = snapshot(dir.path());
    let (code, stdout, stderr) =
        common::run_bin_with_env(&[".", "--no-cache"], repo.path(), dir.path().as_os_str());
    assert_eq!(code, 0, "{stderr}");
    assert!(stderr.is_empty(), "{stderr}");
    assert!(!stdout.is_empty());
    assert_eq!(cached, snapshot(dir.path()));
    assert_eq!(before, snapshot(repo.path()));
}

#[test]
fn cache_write_failure_is_nonfatal() {
    let repo = fixture();
    fs::create_dir(cache_file(&repo)).unwrap();
    let result = collect(&options(&repo)).unwrap();
    assert_eq!(result.stats.parsed, 2);
}

#[cfg(unix)]
#[test]
fn unreadable_tracked_file_is_skipped_without_aborting_map() {
    use std::os::unix::fs::PermissionsExt;
    let repo = fixture();
    repo.write("secret.rs", "fn MARKER_SECRET_1() {}\n");
    repo.add_all();
    collect(&options(&repo)).unwrap();
    let path = repo.path().join("secret.rs");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).unwrap();
    if fs::read(&path).is_ok() {
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        return;
    }
    let (code, stdout, stderr) = common::run_bin(&["."], repo.path());
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(code, 0, "{stderr}");
    assert!(stderr.contains("symapper: skipping secret.rs:"), "{stderr}");
    assert!(stdout.contains("fn alpha()"), "{stdout}");
    assert!(!stdout.contains("MARKER_SECRET_1"));
    assert!(
        cache::load(&cache_file(&repo))
            .unwrap()
            .iter()
            .all(|f| f.path != "secret.rs")
    );
}

#[test]
fn excluding_then_restoring_a_file_reuses_its_cache_entry() {
    let repo = fixture();
    collect(&options(&repo)).unwrap();
    old_mtime(&cache_file(&repo));
    let before = snapshot(repo.path());
    let mut opts = options(&repo);
    opts.exclude.push("z.rs".into());
    let excluded = collect(&opts).unwrap();
    assert_eq!((excluded.stats.parsed, excluded.stats.reused), (0, 1));
    assert_eq!(excluded.files[0].path, "a.rs");
    assert_eq!(before, snapshot(repo.path()));
    let restored = collect(&options(&repo)).unwrap();
    assert_eq!((restored.stats.parsed, restored.stats.reused), (0, 2));
}

#[test]
fn cache_rewrites_keep_excluded_entries_when_other_files_change_or_disappear() {
    for remove in [false, true] {
        let repo = fixture();
        collect(&options(&repo)).unwrap();
        if remove {
            fs::remove_file(repo.path().join("a.rs")).unwrap();
        } else {
            repo.write("a.rs", "fn changed_and_longer() {}\n");
        }
        let mut opts = options(&repo);
        opts.exclude.push("z.rs".into());
        let result = collect(&opts).unwrap();
        assert_eq!(result.stats.parsed, usize::from(!remove));
        assert!(result.files.iter().all(|file| file.path != "z.rs"));
        let saved = cache::load(&cache_file(&repo)).unwrap();
        let expected = if remove {
            vec!["z.rs"]
        } else {
            vec!["a.rs", "z.rs"]
        };
        assert_eq!(
            saved
                .iter()
                .map(|file| file.path.as_str())
                .collect::<Vec<_>>(),
            expected
        );
        let restored = collect(&options(&repo)).unwrap();
        assert_eq!(restored.stats.parsed, 0);
        assert_eq!(restored.stats.reused, expected.len());
    }
}

#[cfg(unix)]
#[test]
fn symlink_cache_is_ignored_and_target_is_untouched() {
    let repo = fixture();
    collect(&options(&repo)).unwrap();
    let target_dir = tempfile::tempdir().unwrap();
    let target = target_dir.path().join("valid-cache.bin");
    fs::rename(cache_file(&repo), &target).unwrap();
    let original = fs::read(&target).unwrap();
    std::os::unix::fs::symlink(&target, cache_file(&repo)).unwrap();
    let (code, stderr) = common::run_bin_with_timeout(&["."], repo.path());
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(stderr, "symapper: ignoring cache: not a regular file\n");
    assert_eq!(fs::read(&target).unwrap(), original);
    assert!(fs::symlink_metadata(cache_file(&repo)).unwrap().is_file());
}

#[cfg(unix)]
#[test]
fn fifo_cache_is_ignored_without_blocking() {
    let repo = fixture();
    assert!(
        std::process::Command::new("mkfifo")
            .arg(cache_file(&repo))
            .status()
            .unwrap()
            .success()
    );
    let (code, stderr) = common::run_bin_with_timeout(&["."], repo.path());
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(stderr, "symapper: ignoring cache: not a regular file\n");
}

#[test]
fn oversized_cache_is_ignored_without_reading_it_all() {
    let repo = fixture();
    fs::File::create(cache_file(&repo))
        .unwrap()
        .set_len(256 * 1024 * 1024 + 1)
        .unwrap();
    let (code, stderr) = common::run_bin_with_timeout(&["."], repo.path());
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(stderr, "symapper: ignoring cache: too large\n");
}

#[cfg(unix)]
#[test]
fn unreadable_file_with_content_focus_emits_exactly_one_note() {
    use std::os::unix::fs::PermissionsExt;
    let repo = fixture();
    repo.write("secret.rs", "// needle\nfn private_item() {}\n");
    repo.add_all();
    collect(&options(&repo)).unwrap();
    let path = repo.path().join("secret.rs");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).unwrap();
    if fs::read(&path).is_ok() {
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        return;
    }
    for args in [
        vec![".", "--focus", "needle"],
        vec![".", "--focus", "needle", "--no-cache"],
    ] {
        let (code, stdout, stderr) = common::run_bin(&args, repo.path());
        assert_eq!(code, 0, "{stderr}");
        assert_eq!(stderr.lines().count(), 1, "{stderr}");
        assert!(
            stderr.starts_with("symapper: skipping secret.rs:"),
            "{stderr}"
        );
        assert!(stdout.contains("fn alpha()"), "{stdout}");
        assert!(!stdout.contains("private_item"));
    }
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
}
