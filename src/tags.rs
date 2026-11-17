use std::cell::RefCell;
use std::fs::File;
use std::io::{self, Read};

use tree_sitter::{Parser, QueryCursor, StreamingIterator};

use crate::{
    Error,
    files::{MAX_SOURCE_BYTES, SourceFile},
    lang::Lang,
};

/// Bump whenever tag extraction in this file, Lang::scope_kinds, the extension map,
/// or a grammar crate pin changes.
pub const TAGS_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TagKind {
    Def,
    Ref,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tag {
    pub name: String,
    pub kind: TagKind,
    pub line: u32,
    pub scopes: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileTags {
    pub path: String,
    pub mtime_ns: u128,
    pub size: u64,
    pub tags: Vec<Tag>,
}

thread_local! {
    static PARSER: RefCell<Parser> = RefCell::new(Parser::new());
}

pub fn parse(lang: Lang, source: &[u8]) -> Vec<Tag> {
    PARSER.with(|parser| {
        let mut parser = parser.borrow_mut();
        if parser.set_language(&lang.language()).is_err() {
            return Vec::new();
        }
        let Some(tree) = parser.parse(source, None) else {
            return Vec::new();
        };
        let query = lang.query();
        let mut cursor = QueryCursor::new();
        let mut captures = cursor.captures(query, tree.root_node(), source);
        let mut tags = Vec::new();
        while let Some((matched, index)) = captures.next() {
            let capture = matched.captures()[*index];
            let capture_name = query.capture_names()[capture.index as usize];
            let kind = if capture_name.starts_with("name.definition.") {
                TagKind::Def
            } else if capture_name.starts_with("name.reference.") {
                TagKind::Ref
            } else {
                continue;
            };
            let Ok(name) = capture.node.utf8_text(source) else {
                continue;
            };
            if name.is_empty() {
                continue;
            }
            let line = capture.node.start_position().row as u32;
            let mut scopes = Vec::new();
            if kind == TagKind::Def {
                let mut ancestor = capture.node.parent();
                while let Some(node) = ancestor {
                    let row = node.start_position().row as u32;
                    if row != line && lang.scope_kinds().contains(&node.kind()) {
                        scopes.push(row);
                    }
                    ancestor = node.parent();
                }
                scopes.reverse();
                scopes.dedup();
            }
            tags.push(Tag {
                name: name.into(),
                kind,
                line,
                scopes,
            });
        }
        tags.sort_by(|a, b| {
            (a.line, a.kind, &a.name, &a.scopes).cmp(&(b.line, b.kind, &b.name, &b.scopes))
        });
        tags.dedup();
        tags
    })
}

pub fn parse_file(file: &SourceFile) -> Result<FileTags, Error> {
    let source = read_source(file)
        .map_err(|e| Error::Runtime(format!("could not read {}: {e}", file.rel)))?;
    Ok(FileTags {
        path: file.rel.clone(),
        mtime_ns: file.mtime_ns,
        size: file.size,
        tags: parse(file.lang, &source),
    })
}

fn read_source(file: &SourceFile) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    File::open(&file.abs)?
        .take(MAX_SOURCE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_SOURCE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "source too large",
        ));
    }
    Ok(bytes)
}

/// Read source text for rendering from a previously listed source file.
pub fn read_text(file: &SourceFile) -> Option<String> {
    match read_source(file) {
        Ok(bytes) => Some(String::from_utf8_lossy(&bytes).into_owned()),
        Err(error) => {
            eprintln!("symapper: skipping {}: {error}", file.rel);
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_growing_after_listing_is_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let abs = dir.path().join("source.rs");
        std::fs::write(&abs, b"fn sample() {}\n").unwrap();
        let file = SourceFile {
            rel: "source.rs".into(),
            abs,
            lang: Lang::Rust,
            size: 15,
            mtime_ns: 0,
        };
        std::fs::OpenOptions::new()
            .write(true)
            .open(&file.abs)
            .unwrap()
            .set_len(MAX_SOURCE_BYTES + 1)
            .unwrap();
        assert_eq!(
            read_source(&file).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        assert!(parse_file(&file).is_err());
        assert!(read_text(&file).is_none());
    }
}
