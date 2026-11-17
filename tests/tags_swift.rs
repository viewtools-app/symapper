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
fn swift_exact_tags() {
    use TagKind::{Def, Ref};
    let source = b"protocol Work: Parent {\n func run()\n}\nclass Box: Base, Work {\n var value: Item\n init() {\n  helper(); value.paint()\n }\n func run() {\n  let local: Item = Item()\n }\n}\nextension Box {\n func extra() { helper() }\n}\nstruct Point {}\nenum Choice {\n case first, second\n}\nactor Store {}\ntypealias Alias = Item\nfunc helper() {}\n";
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&Lang::Swift.language()).unwrap();
    assert!(!parser.parse(source, None).unwrap().root_node().has_error());
    assert_eq!(
        tags::parse(Lang::Swift, source),
        vec![
            tag("Work", Def, 0, &[]),
            tag("Parent", Ref, 0, &[]),
            tag("run", Def, 1, &[0]),
            tag("Box", Def, 3, &[]),
            tag("Base", Ref, 3, &[]),
            tag("Work", Ref, 3, &[]),
            tag("value", Def, 4, &[3]),
            tag("Item", Ref, 4, &[]),
            tag("init", Def, 5, &[3]),
            tag("helper", Ref, 6, &[]),
            tag("paint", Ref, 6, &[]),
            tag("run", Def, 8, &[3]),
            tag("local", Def, 9, &[3, 8]),
            tag("Item", Ref, 9, &[]),
            tag("Box", Def, 12, &[]),
            tag("extra", Def, 13, &[12]),
            tag("helper", Ref, 13, &[]),
            tag("Point", Def, 15, &[]),
            tag("Choice", Def, 16, &[]),
            tag("first", Def, 17, &[16]),
            tag("second", Def, 17, &[16]),
            tag("Store", Def, 19, &[]),
            tag("Alias", Def, 20, &[]),
            tag("Item", Ref, 20, &[]),
            tag("helper", Def, 21, &[]),
        ]
    );
}

#[test]
fn swift_syntax_error_keeps_valid_tags() {
    assert_eq!(
        tags::parse(Lang::Swift, b"func valid() {}\nfunc broken(\n"),
        vec![tag("valid", TagKind::Def, 0, &[])]
    );
}

#[test]
fn swift_nested_annotations_calls_and_scopes() {
    use TagKind::{Def, Ref};
    let source = b"class Outer {\n struct Inner {\n  var values: [Item?]\n  func make(input: Input) -> Output {\n   let result = Factory()\n   input.build()\n   return result\n  }\n }\n}\n";
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&Lang::Swift.language()).unwrap();
    assert!(!parser.parse(source, None).unwrap().root_node().has_error());
    assert_eq!(
        tags::parse(Lang::Swift, source),
        vec![
            tag("Outer", Def, 0, &[]),
            tag("Inner", Def, 1, &[0]),
            tag("values", Def, 2, &[0, 1]),
            tag("Item", Ref, 2, &[]),
            tag("make", Def, 3, &[0, 1]),
            tag("Input", Ref, 3, &[]),
            tag("Output", Ref, 3, &[]),
            tag("result", Def, 4, &[0, 1, 3]),
            tag("Factory", Ref, 4, &[]),
            tag("build", Ref, 5, &[]),
        ]
    );
}
