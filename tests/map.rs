mod common;

use common::{Fixture, run_bin};
use symapper::{Options, map, render::tokens};

fn multilingual() -> Fixture {
    let repo = Fixture::new();
    repo.write("README.md", "Example\n");
    repo.write("plain.rs", "fn plain() {}\n");
    repo.write("other.py", "def other():\n    pass\n");
    repo.write("private.ts", "function _private() {}\n");
    for index in 0..6 {
        repo.write(
            &format!("shared{index}.go"),
            "package demo\nfunc shared() {}\n",
        );
    }
    let mut source = String::from("fn caller() { plain(); other(); _private(); shared(); }\n");
    for index in 0..400 {
        source.push_str(&format!(
            "fn item{index:03}() {{ plain(); other(); _private(); shared(); }}\n"
        ));
    }
    repo.write("calls.rs", &source);
    repo.add_all();
    repo
}

fn first_block(text: &str) -> &str {
    text.lines().find(|line| line.ends_with(':')).unwrap()
}

#[test]
fn multilingual_budget_cache_and_relative_output() {
    let repo = multilingual();
    for budget in [200, 1500] {
        let value = budget.to_string();
        let args = [".", "--tokens", &value];
        let (code, stdout, stderr) = run_bin(&args, repo.path());
        assert_eq!(code, 0, "{stderr}");
        assert!(stderr.is_empty(), "{stderr}");
        let count = tokens(&stdout);
        assert!(
            (budget * 85 / 100..=budget).contains(&count),
            "{budget}: {count}"
        );
        assert!(stdout.starts_with("README.md\n"));
        assert!(!stdout.contains(repo.path().to_str().unwrap()));
        assert!(!stdout.contains(repo.path().file_name().unwrap().to_str().unwrap()));
        assert_eq!(
            (code, stdout.clone(), stderr.clone()),
            run_bin(&args, repo.path())
        );
        assert_eq!(
            (code, stdout, stderr),
            run_bin(&[".", "--tokens", &value, "--no-cache"], repo.path())
        );
    }
}

#[test]
fn identifier_focus_changes_top_file_and_penalties_lower_definitions() {
    let repo = multilingual();
    let mut opts = Options {
        repo: repo.path().to_owned(),
        focus: vec![],
        tokens: 20_000,
        exclude: vec![],
        no_cache: true,
        cache_dir: None,
    };
    let normal = map(&opts).unwrap().text;
    let plain = normal.find("plain.rs:\n").unwrap();
    assert!(plain < normal.find("private.ts:\n").unwrap());
    for index in 0..6 {
        assert!(plain < normal.find(&format!("shared{index}.go:\n")).unwrap());
    }
    opts.focus = vec!["plain".into()];
    let focused = map(&opts).unwrap().text;
    assert_ne!(first_block(&normal), first_block(&focused));
    assert_eq!(first_block(&focused), "plain.rs:");
    opts.focus = vec!["calls.rs".into()];
    assert!(!map(&opts).unwrap().text.contains("calls.rs:\n"));
}

#[test]
fn readme_only_and_filtered_repos_print_only_the_note() {
    let repo = Fixture::new();
    repo.write("README.md", "Example\n");
    repo.add_all();
    let expected = (
        0,
        String::new(),
        "symapper: no supported source files\n".into(),
    );
    assert_eq!(run_bin(&["."], repo.path()), expected);
    repo.write("main.rs", "fn main() {}\n");
    repo.add_all();
    assert_eq!(run_bin(&[".", "--exclude", "*.rs"], repo.path()), expected);
}

#[test]
fn caller_only_file_appears_as_a_name_after_definition_blocks() {
    let repo = Fixture::new();
    repo.write("definition.py", "def target():\n    pass\n");
    repo.write("caller.py", "target()\n");
    repo.add_all();
    let (code, stdout, stderr) = run_bin(&[".", "--no-cache"], repo.path());
    assert_eq!(code, 0, "{stderr}");
    assert!(
        stdout.contains("definition.py:\ndef target():\n"),
        "{stdout}"
    );
    assert!(stdout.ends_with("caller.py\n"), "{stdout}");
    assert!(!stdout.contains("caller.py:"));
}

#[test]
fn prose_focus_ranks_definition_words_and_directory_components() {
    let repo = Fixture::new();
    repo.write("a.rs", "struct Unrelated;\n");
    repo.write("z.rs", "struct SettingsSummary;\n");
    repo.write("SheetLayout.swift", "struct SheetLayout {}\n");
    repo.write("push/one.rs", "fn one() {}\n");
    repo.write("push/nested/two.rs", "fn two() {}\n");
    repo.add_all();
    let mut opts = Options {
        repo: repo.path().to_owned(),
        focus: vec!["settings".into(), "summary".into()],
        tokens: 20_000,
        exclude: vec![],
        no_cache: true,
        cache_dir: None,
    };
    assert_eq!(first_block(&map(&opts).unwrap().text), "z.rs:");
    opts.focus = vec!["sheet".into()];
    assert_eq!(first_block(&map(&opts).unwrap().text), "SheetLayout.swift:");
    opts.focus = vec!["push".into()];
    let text = map(&opts).unwrap().text;
    assert_eq!(first_block(&text), "push/nested/two.rs:");
    assert!(text.find("push/one.rs:\n").unwrap() < text.find("a.rs:\n").unwrap());
}

fn python_go_swift() -> Fixture {
    let repo = Fixture::new();
    repo.write("README.md", "Example\n");
    repo.write("lib.py", "def greet():\n    return 1\n");
    repo.write("main.go", "package demo\nfunc main() { greet() }\n");
    repo.write("View.swift", "struct View {\n    func greet() {}\n}\n");
    repo.add_all();
    repo
}

#[test]
fn no_focus_output_matches_golden() {
    for (name, repo) in [
        ("no_focus", multilingual()),
        ("python_go_swift", python_go_swift()),
    ] {
        for budget in [1024, 300] {
            let value = budget.to_string();
            let (code, stdout, stderr) =
                run_bin(&[".", "--tokens", &value, "--no-cache"], repo.path());
            assert_eq!(code, 0, "{stderr}");
            assert!(stderr.is_empty(), "{stderr}");
            assert!(!stdout.contains(repo.path().to_str().unwrap()));
            assert!(!stdout.lines().any(|line| line.starts_with('/')));
            let path = format!(
                "{}/tests/golden/{name}_{budget}.txt",
                env!("CARGO_MANIFEST_DIR")
            );
            let expected =
                std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{path}: {error}"));
            let first_difference = stdout
                .split_inclusive('\n')
                .zip(expected.split_inclusive('\n'))
                .position(|(actual, golden)| actual != golden)
                .unwrap_or_else(|| {
                    stdout
                        .split_inclusive('\n')
                        .count()
                        .min(expected.split_inclusive('\n').count())
                })
                + 1;
            assert!(
                stdout.as_bytes() == expected.as_bytes(),
                "{name}_{budget}: first differing line {first_difference}"
            );
        }
    }
}

#[test]
fn content_only_blocks_and_names_precede_unmatched_and_are_repeatable() {
    let repo = Fixture::new();
    repo.write("a.rs", "fn anchor() {}\n");
    repo.write("b.py", "anchor()\n");
    repo.write("z.rs", "// reducedMotion\nfn zebra() {}\n");
    repo.write("zz.py", "# reduced_motion\nanchor()\n");
    repo.add_all();
    let args = [
        ".",
        "--focus",
        "reduced-motion",
        "--no-cache",
        "--tokens",
        "1000",
    ];
    let result = run_bin(&args, repo.path());
    assert_eq!(result.0, 0, "{}", result.2);
    assert!(result.2.is_empty(), "{}", result.2);
    assert_eq!(first_block(&result.1), "z.rs:");
    assert!(result.1.find("zz.py\n").unwrap() < result.1.find("b.py\n").unwrap());
    assert_eq!(result, run_bin(&args, repo.path()));
    let (code, text, stderr) = run_bin(
        &[".", "--focus", "a.rs", "--focus", "reduced-motion"],
        repo.path(),
    );
    assert_eq!(code, 0, "{stderr}");
    assert!(!text.lines().any(|line| line == "a.rs" || line == "a.rs:"));
    assert_eq!(first_block(&text), "z.rs:");
}

#[test]
fn large_matched_file_leaves_budget_for_small_matched_files() {
    let repo = Fixture::new();
    let mut large = String::from("// needle\n");
    for index in 0..40 {
        large.push_str(&format!("fn item{index:02}() {{}}\n"));
    }
    repo.write("needle_big.rs", &large);
    repo.write("b.rs", "// needle\nfn bravo() {}\n");
    repo.write("c.rs", "// needle\nfn charlie() {}\n");
    for name in ["d", "e", "f"] {
        repo.write(&format!("{name}.rs"), &format!("fn {name}() {{}}\n"));
    }
    repo.add_all();
    let (code, text, stderr) = run_bin(
        &[".", "--focus", "needle", "--tokens", "70", "--no-cache"],
        repo.path(),
    );
    assert_eq!(code, 0, "{stderr}");
    assert!(stderr.is_empty(), "{stderr}");
    assert!(text.contains("needle_big.rs:\n"), "{text}");
    assert!(text.contains("b.rs:\n"), "{text}");
    assert!(text.contains("c.rs:\n"), "{text}");
    assert!(tokens(&text) <= 70);
}

#[test]
fn content_matched_low_rank_file_prints_block_before_high_rank_unmatched() {
    let repo = Fixture::new();
    repo.write("hub.rs", "fn hub() {}\n");
    repo.write("lonely.rs", "// needle\nfn lonely() { hub(); }\n");
    for index in 0..6 {
        repo.write(
            &format!("caller{index}.rs"),
            &format!("fn caller{index}() {{ hub(); }}\n"),
        );
    }
    repo.add_all();
    let (code, text, stderr) = run_bin(
        &[".", "--focus", "needle", "--tokens", "70", "--no-cache"],
        repo.path(),
    );
    assert_eq!(code, 0, "{stderr}");
    assert!(stderr.is_empty(), "{stderr}");
    assert_eq!(first_block(&text), "lonely.rs:", "{text}");
    assert!(
        text.find("lonely.rs:\n").unwrap() < text.find("hub.rs:\n").unwrap(),
        "{text}"
    );
    assert!(tokens(&text) <= 70);
}
