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
        ranking_v010: false,
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
        ranking_v010: false,
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

#[test]
fn focus_keeps_matching_workflow_after_definition_blocks() {
    let repo = Fixture::new();
    repo.write("README.md", "Example\n");
    repo.write(
        "Cargo.toml",
        "[package]\nname = \"example\"\nversion = \"0.1.0\"\n",
    );
    repo.write(".github/workflows/test.yml", "name: test\n");
    repo.write("a.rs", "fn test_target() {}\n");
    repo.write("b.rs", "fn other() {}\n");
    repo.add_all();
    for focused in [false, true] {
        let mut args = vec![".", "--tokens", "2000", "--no-cache"];
        if focused {
            args.extend(["--focus", "test"]);
        }
        let (code, text, stderr) = run_bin(&args, repo.path());
        assert_eq!(code, 0, "{stderr}");
        assert!(stderr.is_empty(), "{stderr}");
        let roots = ".github/workflows/test.yml\nCargo.toml\nREADME.md\n";
        if focused {
            assert!(text.starts_with("a.rs:\n"), "{text}");
            let root_at = text.find(roots).unwrap();
            for path in ["a.rs:\n", "b.rs:\n"] {
                assert!(text.find(path).unwrap() < root_at, "{text}");
            }
            assert_eq!(text.matches(".github/workflows/test.yml").count(), 1);
        } else {
            assert!(text.starts_with(roots), "{text}");
        }
    }
}

#[test]
fn match_strength_output_is_byte_identical_across_runs() {
    let repo = Fixture::new();
    repo.write("src/a.rs", "fn motion() {}\n");
    repo.write("src/motion/b.rs", "fn directory() {}\n");
    repo.write("src/c.rs", "// motion\nfn mentioner() {}\n");
    repo.write("src/d.rs", "// fix\nfn unrelated() {}\n");
    repo.write("src/e.rs", "fn plain() {}\n");
    repo.add_all();
    let args = [
        ".",
        "--focus",
        "motion",
        "--focus",
        "fix",
        "--focus",
        "src",
        "--tokens",
        "1000",
        "--no-cache",
    ];
    let first = run_bin(&args, repo.path());
    assert_eq!(first.0, 0, "{}", first.2);
    assert!(first.2.is_empty(), "{}", first.2);
    assert_eq!(first_block(&first.1), "src/a.rs:");
    let second = run_bin(&args, repo.path());
    assert_eq!(first.0, second.0);
    assert_eq!(first.1.as_bytes(), second.1.as_bytes());
    assert_eq!(first.2.as_bytes(), second.2.as_bytes());
}

#[test]
fn ranking_switch_selects_legacy_roots_and_rejects_unknown_values() {
    let repo = multilingual();
    let args = [".", "--no-cache", "--focus", "plain", "--tokens", "1024"];
    let (code, legacy, stderr) =
        common::run_bin_with_ranking(&args, repo.path(), std::ffi::OsStr::new(""), Some("v010"));
    assert_eq!(code, 0, "{stderr}");
    assert!(legacy.starts_with("README.md\n"), "{legacy}");
    let (code, normal, stderr) = run_bin(&args, repo.path());
    assert_eq!(code, 0, "{stderr}");
    assert!(normal.starts_with("plain.rs:\n"), "{normal}");
    let (code, stdout, stderr) =
        common::run_bin_with_ranking(&args, repo.path(), std::ffi::OsStr::new(""), Some("bogus"));
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("SYMAPPER_RANKING"));
}

/// Export complete tracked fixture repositories for the published 0.1.0 binary.
#[test]
#[ignore]
fn dump_fixtures_for_v010_goldens() {
    let Some(out) = std::env::var_os("SYMAPPER_FIXTURE_OUT") else {
        return;
    };
    let out = std::path::PathBuf::from(out);
    std::fs::create_dir_all(&out).unwrap();
    fn copy_tree(source: &std::path::Path, target: &std::path::Path) {
        std::fs::create_dir(target).unwrap();
        for entry in std::fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            let destination = target.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_tree(&entry.path(), &destination);
            } else {
                std::fs::copy(entry.path(), destination).unwrap();
            }
        }
    }
    let mut cases = String::new();
    for (name, case, repo, focus) in [
        ("no_focus", "plain_focus", multilingual(), "--focus plain"),
        (
            "python_go_swift",
            "python_go_swift",
            python_go_swift(),
            "--focus greet --focus View",
        ),
    ] {
        copy_tree(repo.path(), &out.join(name));
        for budget in [300, 1024] {
            cases.push_str(&format!(
                "v010_{case}_{budget}\t{name}\t--tokens {budget} --no-cache {focus}\n"
            ));
        }
    }
    std::fs::write(out.join("cases.tsv"), cases).unwrap();
}

fn v010_fixture(name: &str) -> Fixture {
    match name {
        "no_focus" => multilingual(),
        "python_go_swift" => python_go_swift(),
        _ => panic!("unknown v010 fixture: {name}"),
    }
}

#[test]
fn v010_output_matches_published_goldens() {
    for line in include_str!("golden/v010_cases.tsv").lines() {
        let columns: Vec<_> = line.split('\t').collect();
        assert_eq!(columns.len(), 3);
        let [case, fixture, args] = columns[..] else {
            unreachable!()
        };
        let repo = v010_fixture(fixture);
        let args: Vec<_> = std::iter::once(".")
            .chain(args.split_whitespace())
            .collect();
        let (code, out, stderr) = common::run_bin_with_ranking(
            &args,
            repo.path(),
            std::ffi::OsStr::new(""),
            Some("v010"),
        );
        assert_eq!(code, 0, "{case}: {stderr}");
        assert!(stderr.is_empty(), "{case}: {stderr}");
        let golden = std::fs::read_to_string(format!(
            "{}/tests/golden/{case}.txt",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap();
        assert_eq!(out, golden, "{case}");
    }
}

#[test]
fn focused_output_contains_every_legacy_path_at_the_same_budget() {
    for line in include_str!("golden/v010_cases.tsv").lines() {
        let columns: Vec<_> = line.split('\t').collect();
        let [case, fixture, args] = columns[..] else {
            panic!("invalid case: {line}")
        };
        let repo = v010_fixture(fixture);
        for budget in [300, 1024, 1500] {
            let value = budget.to_string();
            let mut args: Vec<_> = std::iter::once(".")
                .chain(args.split_whitespace())
                .collect();
            let index = args.iter().position(|arg| *arg == "--tokens").unwrap();
            args[index + 1] = &value;
            let (code, legacy, stderr) = common::run_bin_with_ranking(
                &args,
                repo.path(),
                std::ffi::OsStr::new(""),
                Some("v010"),
            );
            assert_eq!(code, 0, "{case}: {stderr}");
            assert!(stderr.is_empty(), "{case}: {stderr}");
            let (code, out, stderr) = run_bin(&args, repo.path());
            assert_eq!(code, 0, "{case}: {stderr}");
            assert!(stderr.is_empty(), "{case}: {stderr}");
            assert!(tokens(&out) <= budget, "{case}, budget={budget}");
            for path in legacy
                .lines()
                .map(|line| line.strip_suffix(':').unwrap_or(line))
                .filter(|line| repo.path().join(line).is_file())
            {
                assert_eq!(
                    out.lines()
                        .filter(|line| *line == path || *line == format!("{path}:"))
                        .count(),
                    1,
                    "{case}, budget={budget}: missing or duplicate {path}"
                );
            }
        }
    }
}

#[test]
fn default_map_restores_a_legacy_path_missing_without_floor() {
    use std::collections::{BTreeMap, BTreeSet};
    use symapper::{collect, focus, graph, rank, render};

    let repo = Fixture::new();
    repo.write("Package.swift", "struct Manifest {}\n");
    for index in 0..8 {
        repo.write(
            &format!("needle_{index}.rs"),
            &format!("fn needle_{index}_{}() {{}}\n", "long_signature".repeat(8)),
        );
    }
    repo.add_all();
    let mut opts = Options {
        repo: repo.path().into(),
        focus: vec!["needle".into()],
        tokens: 60,
        exclude: vec![],
        no_cache: true,
        ranking_v010: false,
        cache_dir: None,
    };

    // Reproduce the 006 pipeline with an empty floor, using only this fixture's sources.
    let collected = collect(&opts).unwrap();
    let paths: Vec<_> = collected
        .files
        .iter()
        .map(|file| file.path.clone())
        .collect();
    let resolved = focus::resolve(&opts.focus, repo.path(), repo.path(), &paths);
    let graph = graph::build(&collected.files, &resolved.idents);
    let sources: BTreeMap<_, _> = graph
        .paths
        .iter()
        .map(|path| {
            (
                path.as_str(),
                std::fs::read_to_string(repo.path().join(path)).unwrap(),
            )
        })
        .collect();
    let terms = focus::ContentTerms::new(&resolved);
    let content: Vec<_> = graph
        .paths
        .iter()
        .map(|path| terms.matches(&sources[path.as_str()]))
        .collect();
    let (vector, focus_files, scores) =
        focus::personalization(&graph.paths, &collected.files, &resolved, &content);
    let ranks = rank::pagerank(graph.paths.len(), &graph.groups, &vector);
    let ranked = rank::focus_order(
        rank::split(&graph, &ranks, &focus_files),
        &scores,
        rank::MATCHED_DEFS_PER_FILE,
    );
    let empty_floor = render::render_with_files(
        &render::Input {
            floor: &BTreeSet::new(),
            late_roots: true,
            root_files: &collected.root_files,
            graph: &graph,
            files: &collected.files,
            file_ranks: &ranks,
            file_scores: &scores,
            ranked: &ranked,
            focus_files: &focus_files,
        },
        opts.tokens,
        &mut |path| sources.get(path).cloned(),
        &mut BTreeMap::new(),
    )
    .0;
    let printed_paths = |text: &str| -> BTreeSet<String> {
        text.lines()
            .map(|line| line.strip_suffix(':').unwrap_or(line))
            .filter(|path| sources.contains_key(path))
            .map(String::from)
            .collect()
    };
    opts.ranking_v010 = true;
    let legacy = map(&opts).unwrap().text;
    opts.ranking_v010 = false;
    let default = map(&opts).unwrap().text;
    assert!(printed_paths(&legacy).contains("Package.swift"), "{legacy}");
    assert!(
        !printed_paths(&empty_floor).contains("Package.swift"),
        "{empty_floor}"
    );
    assert!(
        printed_paths(&default).contains("Package.swift"),
        "{default}"
    );
    assert!(printed_paths(&legacy).is_subset(&printed_paths(&default)));
    assert!(tokens(&default) <= opts.tokens);
}
