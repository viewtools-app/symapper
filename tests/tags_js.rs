use symapper::{
    lang::{self, Lang},
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
fn javascript_and_jsx_exact_tags() {
    use TagKind::{Def, Ref};
    let source = b"class View {\n render() {\n  function nested() {}\n  helper(); api.load(); new Model(); new ns.Factory();\n  return <Panel><Icon.Small /><div /></Panel>;\n }\n}\nexport const Button = () => <View />;\nconst make = function() {};\nfunction* stream() {}\nfunction helper() {}\n";
    for path in ["sample.js", "sample.jsx"] {
        let lang = lang::from_path(path).unwrap();
        assert_eq!(lang, Lang::JavaScript);
        assert_eq!(
            tags::parse(lang, source),
            vec![
                tag("View", Def, 0, &[]),
                tag("render", Def, 1, &[0]),
                tag("nested", Def, 2, &[0, 1]),
                tag("Factory", Ref, 3, &[]),
                tag("Model", Ref, 3, &[]),
                tag("helper", Ref, 3, &[]),
                tag("load", Ref, 3, &[]),
                tag("Panel", Ref, 4, &[]),
                tag("Small", Ref, 4, &[]),
                tag("Button", Def, 7, &[]),
                tag("View", Ref, 7, &[]),
                tag("make", Def, 8, &[]),
                tag("stream", Def, 9, &[]),
                tag("helper", Def, 10, &[]),
            ]
        );
    }
}

#[test]
fn javascript_syntax_error_keeps_valid_tags() {
    assert_eq!(
        tags::parse(Lang::JavaScript, b"function valid() {}\nfunction broken(\n"),
        vec![tag("valid", TagKind::Def, 0, &[])]
    );
}
