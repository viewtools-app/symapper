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
fn typescript_exact_tags() {
    use TagKind::{Def, Ref};
    let source = b"interface Shape {}\ntype Alias = Shape;\nenum Color { Red }\nabstract class Base {\n abstract draw(x: Shape): Alias;\n}\nclass Canvas extends Base implements Shape {\n draw(x: Shape): Alias {\n  helper<Alias>(); x.paint(); new Factory(); new ns.Builder();\n  return x;\n }\n}\nfunction helper<T>() {}\nfunction* stream() {}\nexport const make = () => new Canvas();\nconst other = function() {};\nnamespace Tools {\n export function work() {}\n}\ndeclare module \"external\" {}\n";
    assert_eq!(
        tags::parse(Lang::TypeScript, source),
        vec![
            tag("Shape", Def, 0, &[]),
            tag("Alias", Def, 1, &[]),
            tag("Shape", Ref, 1, &[]),
            tag("Color", Def, 2, &[]),
            tag("Base", Def, 3, &[]),
            tag("draw", Def, 4, &[3]),
            tag("Alias", Ref, 4, &[]),
            tag("Shape", Ref, 4, &[]),
            tag("Canvas", Def, 6, &[]),
            tag("Base", Ref, 6, &[]),
            tag("Shape", Ref, 6, &[]),
            tag("draw", Def, 7, &[6]),
            tag("Alias", Ref, 7, &[]),
            tag("Shape", Ref, 7, &[]),
            tag("Alias", Ref, 8, &[]),
            tag("Builder", Ref, 8, &[]),
            tag("Factory", Ref, 8, &[]),
            tag("helper", Ref, 8, &[]),
            tag("paint", Ref, 8, &[]),
            tag("helper", Def, 12, &[]),
            tag("stream", Def, 13, &[]),
            tag("make", Def, 14, &[]),
            tag("Canvas", Ref, 14, &[]),
            tag("other", Def, 15, &[]),
            tag("Tools", Def, 16, &[]),
            tag("work", Def, 17, &[16]),
            tag("\"external\"", Def, 19, &[]),
        ]
    );
}

#[test]
fn tsx_components_and_intrinsics() {
    use TagKind::{Def, Ref};
    assert_eq!(tags::parse(Lang::Tsx, b"export const Button = () => <Icon.Small />;\nfunction App() { return <Panel><Button /><div /><span>text</span></Panel>; }\n"),
        vec![tag("Button", Def, 0, &[]), tag("Small", Ref, 0, &[]),
        tag("App", Def, 1, &[]), tag("Button", Ref, 1, &[]), tag("Panel", Ref, 1, &[])]);
}

#[test]
fn typescript_and_tsx_keep_valid_tags_after_syntax_error() {
    for lang in [Lang::TypeScript, Lang::Tsx] {
        assert_eq!(
            tags::parse(lang, b"function valid() {}\nfunction broken(\n"),
            vec![tag("valid", TagKind::Def, 0, &[])]
        );
    }
}

#[test]
fn typescript_nested_types_and_interface_scopes() {
    use TagKind::{Def, Ref};
    let source = b"interface Child extends Parent<Arg> {\n method: (x: ns.Input[]) => Result<Output>;\n}\ntype Combo = First | Second[] | [Third, Fourth];\nfunction outer() {\n function inner(x: Combo) {}\n}\n";
    let expected = vec![
        tag("Child", Def, 0, &[]),
        tag("Arg", Ref, 0, &[]),
        tag("Parent", Ref, 0, &[]),
        tag("Input", Ref, 1, &[]),
        tag("Output", Ref, 1, &[]),
        tag("Result", Ref, 1, &[]),
        tag("Combo", Def, 3, &[]),
        tag("First", Ref, 3, &[]),
        tag("Fourth", Ref, 3, &[]),
        tag("Second", Ref, 3, &[]),
        tag("Third", Ref, 3, &[]),
        tag("outer", Def, 4, &[]),
        tag("inner", Def, 5, &[4]),
        tag("Combo", Ref, 5, &[]),
    ];
    for lang in [Lang::TypeScript, Lang::Tsx] {
        assert_eq!(tags::parse(lang, source), expected);
    }
}
