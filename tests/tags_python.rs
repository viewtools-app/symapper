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
fn python_exact_tags() {
    use TagKind::{Def, Ref};
    let source = b"X = factory()\n@decorate\nclass Child(Base, pkg.Other):\n    ignored = 1\n    @pkg.wrapper()\n    def method(self):\n        local = helper()\n        self.run()\n        def nested():\n            pass\ndef top():\n    pass\n";
    assert_eq!(
        tags::parse(Lang::Python, source),
        vec![
            tag("X", Def, 0, &[]),
            tag("factory", Ref, 0, &[]),
            tag("decorate", Ref, 1, &[]),
            tag("Child", Def, 2, &[]),
            tag("Base", Ref, 2, &[]),
            tag("Other", Ref, 2, &[]),
            tag("wrapper", Ref, 4, &[]),
            tag("method", Def, 5, &[2]),
            tag("helper", Ref, 6, &[]),
            tag("run", Ref, 7, &[]),
            tag("nested", Def, 8, &[2, 5]),
            tag("top", Def, 10, &[]),
        ]
    );
}

#[test]
fn python_syntax_error_keeps_valid_tags() {
    assert_eq!(
        tags::parse(Lang::Python, b"def valid():\n    pass\ndef broken(\n"),
        vec![tag("valid", TagKind::Def, 0, &[])]
    );
}

#[test]
fn parser_can_switch_languages_and_skip_invalid_names() {
    assert_eq!(
        tags::parse(Lang::Python, b"def one(): pass"),
        vec![tag("one", TagKind::Def, 0, &[])]
    );
    assert_eq!(
        tags::parse(Lang::Rust, b"fn two() {}"),
        vec![tag("two", TagKind::Def, 0, &[])]
    );
    assert_eq!(
        tags::parse(Lang::Python, b"def three(): pass"),
        vec![tag("three", TagKind::Def, 0, &[])]
    );
    assert!(tags::parse(Lang::Python, b"").is_empty());
    let _ = tags::parse(Lang::Python, b"\xff\n");
}

#[test]
fn python_decorators_and_module_assignments() {
    use TagKind::{Def, Ref};
    let source = b"X = build()\n@ns.decorate\n@factory()\ndef outer():\n    local = 1\n    @wrap\n    def inner():\n        ns.work()\nif True:\n    hidden = 1\n";
    assert_eq!(
        tags::parse(Lang::Python, source),
        vec![
            tag("X", Def, 0, &[]),
            tag("build", Ref, 0, &[]),
            tag("decorate", Ref, 1, &[]),
            tag("factory", Ref, 2, &[]),
            tag("outer", Def, 3, &[]),
            tag("wrap", Ref, 5, &[]),
            tag("inner", Def, 6, &[3]),
            tag("work", Ref, 7, &[]),
        ]
    );
}
