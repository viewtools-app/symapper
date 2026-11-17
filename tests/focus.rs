mod common;

use common::Fixture;
use std::collections::BTreeSet;
use symapper::focus::{ContentTerms, Focus, content_words, personalization, resolve};

fn paths() -> Vec<String> {
    ["lib/main.py", "src/app/main.rs", "src/other.rs"]
        .into_iter()
        .map(String::from)
        .collect()
}

#[test]
fn path_forms_resolve_to_the_same_file() {
    let fixture = Fixture::new();
    fixture.write("src/app/main.rs", "fn main() {}\n");
    let cwd = fixture.path().join("src");
    let absolute = fixture.path().join("src/app/main.rs");
    for value in [
        "src/app/main.rs",
        "./src/app/main.rs",
        "app/main.rs",
        absolute.to_str().unwrap(),
        "app/../app/./main.rs",
    ] {
        let focus = resolve(&[value.into()], fixture.path(), &cwd, &paths());
        assert_eq!(focus.files, BTreeSet::from(["src/app/main.rs".into()]));
        assert_eq!(focus.idents, BTreeSet::from(["main".into()]));
    }
    let focus = resolve(
        &["src/app/main.rs".into()],
        fixture.path(),
        fixture.path(),
        &paths(),
    );
    assert_eq!(focus.files.len(), 1);
}

#[test]
fn unknown_and_outside_values_remain_identifiers() {
    let fixture = Fixture::new();
    let outside = tempfile::tempdir().unwrap();
    let outside_file = outside.path().join("main.rs");
    std::fs::write(&outside_file, "fn main() {}").unwrap();
    for value in [
        "unknown".to_string(),
        "../main.rs".into(),
        outside_file.to_str().unwrap().into(),
        "src/app/missing.rs".into(),
    ] {
        let focus = resolve(
            std::slice::from_ref(&value),
            fixture.path(),
            fixture.path(),
            &paths(),
        );
        assert!(focus.files.is_empty());
        assert_eq!(focus.idents, BTreeSet::from([value]));
    }
}

#[test]
fn lexical_paths_resolve_without_existing_files() {
    let fixture = Fixture::new();
    let value = fixture.path().join("src/app/../app/main.rs");
    let focus = resolve(
        &[value.to_str().unwrap().into()],
        fixture.path(),
        fixture.path(),
        &paths(),
    );
    assert_eq!(focus.files, BTreeSet::from(["src/app/main.rs".into()]));
}

#[test]
fn identifiers_match_path_filename_and_stem() {
    for (ident, expected) in [
        ("main", vec![0.5, 0.5, 0.0]),
        ("main.rs", vec![0.0, 1.0, 0.0]),
        ("src/app/main.rs", vec![0.0, 1.0, 0.0]),
    ] {
        let focus = Focus {
            stem_idents: BTreeSet::new(),
            files: BTreeSet::new(),
            idents: BTreeSet::from([ident.into()]),
        };
        let (vector, indices, _) = personalization(&paths(), &[], &focus, &[]);
        assert_eq!(vector, expected);
        assert!(indices.is_empty());
    }
}

#[test]
fn explicit_files_and_ident_matches_share_mass_without_duplicates() {
    let focus = Focus {
        stem_idents: BTreeSet::new(),
        files: BTreeSet::from(["src/app/main.rs".into(), "src/other.rs".into()]),
        idents: BTreeSet::from(["main".into()]),
    };
    let (vector, indices, _) = personalization(&paths(), &[], &focus, &[]);
    assert_eq!(vector, symapper::rank::uniform(3));
    assert_eq!(indices, BTreeSet::from([1, 2]));
}

#[test]
fn empty_and_unmatched_focus_are_uniform() {
    for focus in [
        Focus::default(),
        Focus {
            stem_idents: BTreeSet::new(),
            files: BTreeSet::new(),
            idents: BTreeSet::from(["missing".into()]),
        },
    ] {
        assert_eq!(
            personalization(&paths(), &[], &focus, &[]),
            (symapper::rank::uniform(3), BTreeSet::new(), vec![0.0; 3])
        );
        assert_eq!(
            personalization(&[], &[], &focus, &[]),
            (Vec::new(), BTreeSet::new(), Vec::new())
        );
    }
}

#[cfg(unix)]
#[test]
fn canonical_paths_match_a_symlinked_repo_root() {
    let fixture = Fixture::new();
    fixture.write("src/app/main.rs", "fn main() {}\n");
    let alias_dir = tempfile::tempdir().unwrap();
    let alias = alias_dir.path().join("repo");
    std::os::unix::fs::symlink(fixture.path(), &alias).unwrap();
    let absolute = fixture
        .path()
        .canonicalize()
        .unwrap()
        .join("src/app/main.rs");
    let focus = resolve(
        &[absolute.to_str().unwrap().into()],
        &alias,
        &alias.join("src"),
        &paths(),
    );
    assert_eq!(focus.files, BTreeSet::from(["src/app/main.rs".into()]));
}

#[cfg(unix)]
#[test]
fn tracked_symlink_focus_preserves_the_link_path() {
    let fixture = Fixture::new();
    fixture.write("real.py", "def real(): pass\n");
    std::os::unix::fs::symlink("real.py", fixture.path().join("alias.py")).unwrap();
    fixture.add_all();
    let paths = vec!["alias.py".into(), "real.py".into()];
    for name in ["alias.py", "real.py"] {
        let absolute = fixture.path().join(name);
        for value in [name, absolute.to_str().unwrap()] {
            let focus = resolve(&[value.into()], fixture.path(), fixture.path(), &paths);
            assert_eq!(focus.files, BTreeSet::from([name.into()]));
        }
    }
}

#[test]
fn words_split_separators_case_boundaries_and_keep_digits() {
    for (name, expected) in [
        ("SheetLayout", vec!["sheet", "layout"]),
        ("HTTPServer", vec!["http", "server"]),
        ("askID", vec!["ask", "id"]),
        ("session-context", vec!["session", "context"]),
        ("autoCompactWindow", vec!["auto", "compact", "window"]),
        (
            "v2_parser3 HTTP2Server",
            vec!["v2", "parser3", "http2", "server"],
        ),
        ("a_b-c.d/e f", vec!["a", "b", "c", "d", "e", "f"]),
        ("_ -./ ", vec![]),
    ] {
        assert_eq!(symapper::focus::words(name), expected, "{name}");
    }
}

#[test]
fn word_runs_score_components_and_definitions_by_distinct_term() {
    use symapper::tags::{FileTags, Tag, TagKind};
    let paths = [
        "SheetLayout.swift",
        "a.rs",
        "push/a.rs",
        "push/nested/b.rs",
        "z.rs",
    ]
    .map(String::from);
    // Tag input order deliberately differs from graph path order.
    let files = vec![
        FileTags {
            path: "a.rs".into(),
            mtime_ns: 0,
            size: 0,
            tags: vec![
                Tag {
                    name: "SettingsSummary".into(),
                    kind: TagKind::Def,
                    line: 0,
                    scopes: vec![]
                };
                2
            ],
        },
        FileTags {
            path: "z.rs".into(),
            mtime_ns: 0,
            size: 0,
            tags: vec![Tag {
                name: "SettingsSummary".into(),
                kind: TagKind::Ref,
                line: 0,
                scopes: vec![],
            }],
        },
    ];
    for (terms, expected) in [
        (vec!["sheet"], vec![1.0, 0.0, 0.0, 0.0, 0.0]),
        (vec!["settings", "summary"], vec![0.0, 1.0, 0.0, 0.0, 0.0]),
        (vec!["push"], vec![0.0, 0.0, 0.5, 0.5, 0.0]),
        (vec!["settings summary"], vec![0.0, 1.0, 0.0, 0.0, 0.0]),
        (vec!["summary settings"], symapper::rank::uniform(5)),
        (vec!["_-/"], symapper::rank::uniform(5)),
    ] {
        let focus = Focus {
            stem_idents: BTreeSet::new(),
            files: BTreeSet::new(),
            idents: terms.into_iter().map(String::from).collect(),
        };
        assert_eq!(personalization(&paths, &files, &focus, &[]).0, expected);
    }
    let focus = Focus {
        stem_idents: BTreeSet::new(),
        files: BTreeSet::from(["z.rs".into()]),
        idents: ["settings", "summary", "push"].map(String::from).into(),
    };
    let (vector, explicit, _) = personalization(&paths, &files, &focus, &[]);
    assert_eq!(
        vector,
        vec![0.0, 2.0 / 7.0, 1.0 / 7.0, 1.0 / 7.0, 3.0 / 7.0]
    );
    assert_eq!(explicit, BTreeSet::from([4]));
}

#[test]
fn explicit_file_without_terms_still_receives_mass() {
    let focus = Focus {
        stem_idents: BTreeSet::new(),
        files: BTreeSet::from(["src/other.rs".into()]),
        idents: BTreeSet::new(),
    };
    assert_eq!(
        personalization(&paths(), &[], &focus, &[]),
        (
            vec![0.0, 0.0, 1.0],
            BTreeSet::from([2]),
            vec![0.0, 0.0, 1.0]
        )
    );
}

#[test]
fn content_words_and_runs_use_all_separators_and_case_boundaries() {
    assert_eq!(
        content_words("HTTPServer v2_parser3:askID#SheetLayout").collect::<Vec<_>>(),
        [
            "http", "server", "v2", "parser3", "ask", "id", "sheet", "layout"
        ]
    );
    for term in ["reduced-motion", "reduced:motion", "reduced#motion"] {
        let focus = Focus {
            idents: BTreeSet::from([term.into()]),
            ..Focus::default()
        };
        let terms = ContentTerms::new(&focus);
        for text in [
            "reducedMotion",
            "REDUCED_MOTION",
            "reduced motion",
            "reduced-motion",
        ] {
            assert_eq!(terms.matches(text), BTreeSet::from([0]), "{term}: {text}");
        }
        for text in [
            "reduced extra motion",
            "unreduced motion",
            "reduced motionless",
            "",
        ] {
            assert!(terms.matches(text).is_empty(), "{term}: {text}");
        }
    }
    let focus = Focus {
        idents: BTreeSet::from(["a a b".into()]),
        ..Focus::default()
    };
    assert_eq!(
        ContentTerms::new(&focus).matches("a a a b"),
        BTreeSet::from([0])
    );
}

#[test]
fn content_only_scores_less_than_route_and_common_terms_are_suppressed() {
    let paths = ["motion.rs", "a.rs", "b.rs", "c.rs"].map(String::from);
    let focus = Focus {
        idents: BTreeSet::from(["motion".into()]),
        ..Focus::default()
    };
    let content = vec![
        BTreeSet::from([0]),
        BTreeSet::from([0]),
        BTreeSet::new(),
        BTreeSet::new(),
    ];
    let (vector, files, scores) = personalization(&paths, &[], &focus, &content);
    let w = symapper::focus::CONTENT_WEIGHT;
    assert!(w > 0.0 && w < 1.0);
    assert_eq!(scores, [1.0, w, 0.0, 0.0]);
    assert_eq!(vector, [1.0 / (1.0 + w), w / (1.0 + w), 0.0, 0.0]);
    assert!(files.is_empty());
    let common = vec![BTreeSet::from([0]); 4];
    assert_eq!(
        personalization(&paths, &[], &focus, &common).2,
        [1.0, 0.0, 0.0, 0.0]
    );
    let missing = Focus {
        idents: BTreeSet::from(["common".into()]),
        ..Focus::default()
    };
    let result = personalization(&paths, &[], &missing, &common);
    assert_eq!(result.0, symapper::rank::uniform(4));
    assert_eq!(result.2, [0.0; 4]);
}

#[test]
fn explicit_file_stems_are_route_only_unless_also_supplied_as_terms() {
    let repo = Fixture::new();
    let paths = ["a.rs", "b.rs", "src/x/mod.rs"].map(String::from);
    let focus = resolve(&["src/x/mod.rs".into()], repo.path(), repo.path(), &paths);
    assert_eq!(focus.stem_idents, BTreeSet::from(["mod".into()]));
    assert!(ContentTerms::new(&focus).is_empty());
    let content = vec![BTreeSet::from([0]), BTreeSet::new(), BTreeSet::new()];
    assert_eq!(
        personalization(&paths, &[], &focus, &content).2,
        [0.0, 0.0, 1.0]
    );
    for values in [
        vec!["mod".into(), "src/x/mod.rs".into()],
        vec!["src/x/mod.rs".into(), "mod".into()],
    ] {
        let focus = resolve(&values, repo.path(), repo.path(), &paths);
        assert!(focus.stem_idents.is_empty());
        assert_eq!(
            ContentTerms::new(&focus).matches("mod"),
            BTreeSet::from([0])
        );
    }
}
