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
fn go_exact_tags() {
    use TagKind::{Def, Ref};
    let source = b"package sample\ntype Item struct {}\ntype Worker interface {\n Run(x Item) Result\n}\ntype Count int\nconst First, Second = 1, 2\nvar Current Item\nvar (\n Other pkg.Value\n)\nfunc helper(x Item) Result {\n var local Item\n helper(local); pkg.Build(); x.Run()\n return Result{}\n}\nfunc (x *Item) Run() {\n helper(x)\n}\n";
    assert_eq!(
        tags::parse(Lang::Go, source),
        vec![
            tag("Item", Def, 1, &[]),
            tag("Worker", Def, 2, &[]),
            tag("Run", Def, 3, &[2]),
            tag("Item", Ref, 3, &[]),
            tag("Result", Ref, 3, &[]),
            tag("Count", Def, 5, &[]),
            tag("int", Ref, 5, &[]),
            tag("First", Def, 6, &[]),
            tag("Second", Def, 6, &[]),
            tag("Current", Def, 7, &[]),
            tag("Item", Ref, 7, &[]),
            tag("Other", Def, 9, &[]),
            tag("Value", Ref, 9, &[]),
            tag("helper", Def, 11, &[]),
            tag("Item", Ref, 11, &[]),
            tag("Result", Ref, 11, &[]),
            tag("Item", Ref, 12, &[]),
            tag("Build", Ref, 13, &[]),
            tag("Run", Ref, 13, &[]),
            tag("helper", Ref, 13, &[]),
            tag("Result", Ref, 14, &[]),
            tag("Run", Def, 16, &[]),
            tag("Item", Ref, 16, &[]),
            tag("helper", Ref, 17, &[]),
        ]
    );
}

#[test]
fn go_syntax_error_keeps_valid_tags() {
    assert_eq!(
        tags::parse(Lang::Go, b"package sample\nfunc valid() {}\nfunc broken(\n"),
        vec![
            tag("valid", TagKind::Def, 1, &[]),
            // The Go grammar recovers a declaration with missing parameters.
            tag("broken", TagKind::Def, 2, &[]),
        ]
    );
}

#[test]
fn go_aliases_nested_types_and_multiple_variables() {
    use TagKind::{Def, Ref};
    let source = b"package sample\ntype Alias = pkg.Item\nvar Left, Right *Item\ntype Container struct {\n Values map[Key][]*Value\n}\n";
    assert_eq!(
        tags::parse(Lang::Go, source),
        vec![
            tag("Alias", Def, 1, &[]),
            tag("Item", Ref, 1, &[]),
            tag("Left", Def, 2, &[]),
            tag("Right", Def, 2, &[]),
            tag("Item", Ref, 2, &[]),
            tag("Container", Def, 3, &[]),
            tag("Key", Ref, 4, &[]),
            tag("Value", Ref, 4, &[]),
        ]
    );
}
