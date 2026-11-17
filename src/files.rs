use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use globset::{Glob, GlobSetBuilder};

use crate::{
    Error, git,
    lang::{self, Lang},
};

pub const MAX_SOURCE_BYTES: u64 = 1_048_576;

pub struct SourceFile {
    pub rel: String,
    pub abs: PathBuf,
    pub lang: Lang,
    pub size: u64,
    pub mtime_ns: u128,
}

pub struct FileList {
    pub sources: Vec<SourceFile>,
    pub root_files: Vec<String>,
    pub excluded: BTreeSet<String>,
}

pub fn list(toplevel: &Path, common_dir: &Path, exclude: &[String]) -> Result<FileList, Error> {
    let mut builder = GlobSetBuilder::new();
    for pattern in exclude {
        builder.add(
            Glob::new(pattern).map_err(|e| Error::Usage(format!("invalid exclude glob: {e}")))?,
        );
    }
    let excludes = builder
        .build()
        .map_err(|e| Error::Usage(format!("invalid exclude globs: {e}")))?;
    let root = toplevel
        .canonicalize()
        .map_err(|e| Error::Runtime(format!("could not resolve repo root: {e}")))?;
    let common_dir = common_dir
        .canonicalize()
        .map_err(|e| Error::Runtime(format!("could not resolve git common dir: {e}")))?;
    let tracked: BTreeSet<String> = git::ls_files(toplevel)?
        .into_iter()
        .filter_map(|raw| String::from_utf8(raw).ok())
        .collect();
    let mut sources = Vec::new();
    let mut root_files = Vec::new();
    let mut excluded = BTreeSet::new();
    for rel in &tracked {
        if excludes.is_match(rel) {
            excluded.insert(rel.clone());
            continue;
        }
        let abs = match toplevel.join(rel).canonicalize() {
            Ok(path) => path,
            Err(e) => {
                eprintln!("symapper: skipping {rel}: {e}");
                continue;
            }
        };
        let canonical_rel = abs
            .strip_prefix(&root)
            .ok()
            .and_then(|path| path.to_str())
            .map(|path| path.replace(std::path::MAIN_SEPARATOR, "/"));
        if abs.starts_with(&common_dir)
            || !canonical_rel.is_some_and(|path| tracked.contains(&path))
        {
            continue;
        }
        let meta = match fs::metadata(&abs) {
            Ok(meta) => meta,
            Err(e) => {
                eprintln!("symapper: skipping {rel}: {e}");
                continue;
            }
        };
        if is_root_file(rel) && meta.is_file() {
            root_files.push(rel.clone());
        }
        if !meta.is_file() || meta.len() > MAX_SOURCE_BYTES {
            continue;
        }
        let Some(lang) = lang::from_path(rel) else {
            continue;
        };
        let mut prefix = Vec::with_capacity(8192);
        if let Err(e) = File::open(&abs).and_then(|f| f.take(8192).read_to_end(&mut prefix)) {
            eprintln!("symapper: skipping {rel}: {e}");
            continue;
        }
        if prefix.contains(&0) {
            continue;
        }
        let mtime_ns = match meta
            .modified()
            .map_err(|e| e.to_string())
            .and_then(|time| time.duration_since(UNIX_EPOCH).map_err(|e| e.to_string()))
        {
            Ok(duration) => duration.as_nanos(),
            Err(e) => {
                eprintln!("symapper: skipping {rel}: {e}");
                continue;
            }
        };
        sources.push(SourceFile {
            rel: rel.clone(),
            abs,
            lang,
            size: meta.len(),
            mtime_ns,
        });
    }
    sources.sort_by(|a, b| a.rel.cmp(&b.rel));
    sources.dedup_by(|a, b| a.rel == b.rel);
    root_files.sort();
    root_files.dedup();
    Ok(FileList {
        sources,
        root_files,
        excluded,
    })
}

fn is_root_file(rel: &str) -> bool {
    if !rel.contains('/') {
        return rel.starts_with("README")
            || rel.starts_with("LICENSE")
            || matches!(
                rel,
                "package.json"
                    | "Cargo.toml"
                    | "go.mod"
                    | "pyproject.toml"
                    | "Package.swift"
                    | "Makefile"
                    | "Dockerfile"
            )
            || ((rel.starts_with("compose") || rel.starts_with("docker-compose"))
                && (rel.ends_with(".yml") || rel.ends_with(".yaml")));
    }
    [".github/workflows/", ".forgejo/workflows/"]
        .iter()
        .any(|prefix| {
            rel.strip_prefix(prefix)
                .is_some_and(|name| !name.contains('/') && name.ends_with(".yml"))
        })
}
