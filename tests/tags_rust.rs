use symapper::{
    lang::Lang,
    tags::{self, Tag, TagKind},
};

fn tag(name: &str, kind: TagKind, line: u32, scopes: &[u32]) -> Tag {
    Tag {
        name: name.into(),
        kind,
        line,
        scopes: scopes.into(),
    }
}

#[test]
fn rust_exact_tags() {
    use TagKind::{Def, Ref};
    let source = b"struct Item;\nenum Choice { A }\ntrait Work {\n    fn run(&self);\n}\nimpl Item {\n    fn build(x: Item) -> Item {\n        helper(); x.run();\n        x\n    }\n}\nconst LIMIT: usize = 1;\nmacro_rules! make { () => {} }\nfn helper() {}\n";
    assert_eq!(
        tags::parse(Lang::Rust, source),
        vec![
            tag("Item", Def, 0, &[]),
            tag("A", Def, 1, &[]),
            tag("Choice", Def, 1, &[]),
            tag("Work", Def, 2, &[]),
            tag("run", Def, 3, &[2]),
            tag("Item", Ref, 5, &[]),
            tag("build", Def, 6, &[5]),
            tag("Item", Ref, 6, &[]),
            tag("helper", Ref, 7, &[]),
            tag("run", Ref, 7, &[]),
            tag("LIMIT", Def, 11, &[]),
            tag("make", Def, 12, &[]),
            tag("helper", Def, 13, &[]),
        ]
    );
}

#[test]
fn rust_syntax_error_keeps_valid_tags() {
    assert_eq!(
        tags::parse(Lang::Rust, b"fn valid() {}\nfn broken(\n"),
        vec![tag("valid", TagKind::Def, 0, &[])]
    );
}

#[test]
fn all_grammars_load() {
    for lang in [
        Lang::TypeScript,
        Lang::Tsx,
        Lang::JavaScript,
        Lang::Python,
        Lang::Go,
        Lang::Rust,
        Lang::Swift,
    ] {
        assert!(
            tree_sitter::Parser::new()
                .set_language(&lang.language())
                .is_ok()
        );
        lang.query();
    }
}

#[test]
fn parse_file_preserves_metadata_and_reports_io_errors() {
    let dir = tempfile::tempdir().unwrap();
    let abs = dir.path().join("sample.rs");
    std::fs::write(&abs, "fn sample() {}\n").unwrap();
    let file = symapper::files::SourceFile {
        rel: "sample.rs".into(),
        abs,
        lang: Lang::Rust,
        mtime_ns: 123,
        size: 15,
    };
    let result = tags::parse_file(&file).unwrap();
    assert_eq!(result.path, "sample.rs");
    assert_eq!(result.mtime_ns, 123);
    assert_eq!(result.size, 15);
    assert_eq!(result.tags, vec![tag("sample", TagKind::Def, 0, &[])]);
    std::fs::remove_file(&file.abs).unwrap();
    assert!(matches!(
        tags::parse_file(&file),
        Err(symapper::Error::Runtime(_))
    ));
}

#[test]
fn rust_nested_scopes_and_type_uses() {
    use TagKind::{Def, Ref};
    let source = b"fn outer() {\n    fn inner(x: (First, Second), y: [Third; 2]) {\n        helper::<First>(); x.run::<Second>(); ns::work();\n    }\n}\n";
    assert_eq!(
        tags::parse(Lang::Rust, source),
        vec![
            tag("outer", Def, 0, &[]),
            tag("inner", Def, 1, &[0]),
            tag("First", Ref, 1, &[]),
            tag("Second", Ref, 1, &[]),
            tag("Third", Ref, 1, &[]),
            tag("First", Ref, 2, &[]),
            tag("Second", Ref, 2, &[]),
            tag("helper", Ref, 2, &[]),
            tag("run", Ref, 2, &[]),
            tag("work", Ref, 2, &[]),
        ]
    );
}

#[test]
fn rust_additional_definitions_exclude_primitive_types() {
    use TagKind::Def;
    let source = b"type Count = u32;\nstatic READY: bool = true;\nmod nested {}\nunion Bits { value: u32 }\nenum State { Active }\nfn check(_: &str) -> bool { true }\n";
    let parsed = tags::parse(Lang::Rust, source);
    assert_eq!(
        parsed,
        vec![
            tag("Count", Def, 0, &[]),
            tag("READY", Def, 1, &[]),
            tag("nested", Def, 2, &[]),
            tag("Bits", Def, 3, &[]),
            tag("Active", Def, 4, &[]),
            tag("State", Def, 4, &[]),
            tag("check", Def, 5, &[]),
        ]
    );
    assert!(
        parsed
            .iter()
            .all(|tag| !matches!(tag.name.as_str(), "u32" | "str" | "bool"))
    );
}
