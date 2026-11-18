pub mod cache;
pub mod cli;
pub mod files;
pub mod focus;
pub mod git;
pub mod graph;
pub mod lang;
pub mod rank;
pub mod render;
pub mod tags;

use rayon::prelude::*;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::PathBuf;

pub struct Options {
    pub repo: PathBuf,
    pub focus: Vec<String>,
    pub tokens: usize,
    pub exclude: Vec<String>,
    pub no_cache: bool,
    pub ranking_v010: bool,
    pub cache_dir: Option<std::ffi::OsString>,
}

#[derive(Debug)]
pub enum Error {
    Usage(String),
    NotARepo(String),
    Runtime(String),
}

impl Error {
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::Usage(_) | Self::NotARepo(_) => 2,
            Self::Runtime(_) => 1,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage(message) | Self::NotARepo(message) | Self::Runtime(message) => {
                f.write_str(message)
            }
        }
    }
}

impl std::error::Error for Error {}

pub struct MapOutput {
    pub text: String,
    pub note: Option<String>,
}

pub fn map(opts: &Options) -> Result<MapOutput, Error> {
    let collected = collect(opts)?;
    if collected.files.is_empty() {
        return Ok(MapOutput {
            text: String::new(),
            note: Some("symapper: no supported source files".into()),
        });
    }
    let paths = collected
        .files
        .iter()
        .map(|file| file.path.clone())
        .collect::<Vec<_>>();
    let cwd = std::env::current_dir()
        .map_err(|e| Error::Runtime(format!("could not read current directory: {e}")))?;
    let focus = focus::resolve(&opts.focus, &collected.toplevel, &cwd, &paths);
    let graph = graph::build(&collected.files, &focus.idents);
    let sources: BTreeMap<_, _> = collected
        .sources
        .iter()
        .map(|source| (source.rel.as_str(), source))
        .collect();
    let read = |path: &str| sources.get(path).and_then(|file| tags::read_text(file));
    let content = read_content(&graph.paths, &focus::ContentTerms::new(&focus), &read);
    let mut source_cache = BTreeMap::new();
    let mut reader = |path: &str| content.read_for_render(path, &read);
    if opts.ranking_v010 {
        let (text, _) = map_v010(
            &collected,
            &graph,
            &focus,
            &content.matches,
            opts.tokens,
            &mut reader,
            &mut source_cache,
        );
        return Ok(MapOutput { text, note: None });
    }
    let floor = if opts.focus.is_empty() {
        BTreeSet::new()
    } else {
        map_v010(
            &collected,
            &graph,
            &focus,
            &content.matches,
            opts.tokens,
            &mut reader,
            &mut source_cache,
        )
        .1
    };
    let (vector, focus_files, scores) =
        focus::personalization(&graph.paths, &collected.files, &focus, &content.matches);
    let ranks = rank::pagerank(graph.paths.len(), &graph.groups, &vector);
    let ranked = rank::split(&graph, &ranks, &focus_files);
    let ranked = rank::focus_order(ranked, &scores, rank::MATCHED_DEFS_PER_FILE);
    let (text, _) = render::render_with_files(
        &render::Input {
            floor: &floor,
            late_roots: !opts.focus.is_empty(),
            root_files: &collected.root_files,
            graph: &graph,
            files: &collected.files,
            ranked: &ranked,
            file_ranks: &ranks,
            file_scores: &scores,
            focus_files: &focus_files,
        },
        opts.tokens,
        &mut reader,
        &mut source_cache,
    );
    Ok(MapOutput { text, note: None })
}

/// Run the frozen 0.1.0 ranking pipeline with roots first.
fn map_v010(
    collected: &Collected,
    graph: &graph::Graph,
    focus: &focus::Focus,
    content: &[BTreeSet<usize>],
    tokens: usize,
    reader: &mut dyn FnMut(&str) -> Option<String>,
    source_cache: &mut BTreeMap<usize, Option<render::CachedSource>>,
) -> (String, BTreeSet<usize>) {
    let (vector, focus_files, scores) =
        focus::personalization_v010(&graph.paths, &collected.files, focus, content);
    let ranks = rank::pagerank(graph.paths.len(), &graph.groups, &vector);
    let ranked = rank::split(graph, &ranks, &focus_files);
    let ranked = rank::focus_order(ranked, &scores, rank::MATCHED_DEFS_PER_FILE);
    render::render_with_files(
        &render::Input {
            floor: &BTreeSet::new(),
            late_roots: false,
            root_files: &collected.root_files,
            graph,
            files: &collected.files,
            ranked: &ranked,
            file_ranks: &ranks,
            file_scores: &scores,
            focus_files: &focus_files,
        },
        tokens,
        reader,
        source_cache,
    )
}

#[derive(Default)]
struct ContentReads {
    matches: Vec<BTreeSet<usize>>,
    failed: BTreeSet<String>,
}

impl ContentReads {
    fn read_for_render(
        &self,
        path: &str,
        read: &impl Fn(&str) -> Option<String>,
    ) -> Option<String> {
        if self.failed.contains(path) {
            None
        } else {
            read(path)
        }
    }
}

fn read_content(
    paths: &[String],
    terms: &focus::ContentTerms,
    read: &(impl Fn(&str) -> Option<String> + Sync),
) -> ContentReads {
    if terms.is_empty() {
        return ContentReads::default();
    }
    let results: Vec<_> = paths
        .par_iter()
        .map(|path| read(path).map(|text| terms.matches(&text)))
        .collect();
    let mut content = ContentReads::default();
    for (path, matches) in paths.iter().zip(results) {
        match matches {
            Some(matches) => content.matches.push(matches),
            None => {
                content.matches.push(BTreeSet::new());
                content.failed.insert(path.clone());
            }
        }
    }
    content
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Stats {
    /// Number of files successfully parsed in this run.
    pub parsed: usize,
    pub reused: usize,
}

pub struct Collected {
    pub toplevel: PathBuf,
    pub sources: Vec<files::SourceFile>,
    pub files: Vec<tags::FileTags>,
    pub root_files: Vec<String>,
    pub stats: Stats,
}

/// Collect deterministic tags, reusing unchanged files from the optional cache.
pub fn collect(opts: &Options) -> Result<Collected, Error> {
    let paths = repo_paths(opts)?;
    let listed = files::list(&paths.toplevel, &paths.common_dir, &opts.exclude)?;
    let cache_path = if opts.no_cache {
        None
    } else {
        match cache::cache_path(&paths.common_dir, opts.cache_dir.as_deref()) {
            Ok(path) => Some(path),
            Err(reason) => {
                eprintln!("symapper: SYMAPPER_CACHE_DIR ignored: {reason}");
                None
            }
        }
    };
    let mut cached: BTreeMap<_, _> = cache_path
        .as_deref()
        .and_then(cache::load)
        .unwrap_or_default()
        .into_iter()
        .map(|entry| (entry.path.clone(), entry))
        .collect();
    let mut files = Vec::with_capacity(listed.sources.len());
    let mut misses = Vec::new();
    for source in &listed.sources {
        if let Some(entry) = cached.remove(&source.rel)
            && entry.mtime_ns == source.mtime_ns
            && entry.size == source.size
        {
            files.push(entry);
        } else {
            misses.push(source);
        }
    }
    let parsed: Vec<_> = misses
        .par_iter()
        .filter_map(|source| match tags::parse_file(source) {
            Ok(file) => Some(file),
            Err(e) => {
                eprintln!("symapper: skipping {}: {e}", source.rel);
                None
            }
        })
        .collect();
    let stats = Stats {
        parsed: parsed.len(),
        reused: files.len(),
    };
    files.extend(parsed);
    files.sort_by(|a, b| a.path.cmp(&b.path));
    let gone = cached.keys().any(|path| !listed.excluded.contains(path));
    if let Some(cache_path) = cache_path
        && (!misses.is_empty() || gone)
    {
        let mut saved = files.clone();
        saved.extend(
            cached
                .into_values()
                .filter(|entry| listed.excluded.contains(&entry.path)),
        );
        saved.sort_by(|a, b| a.path.cmp(&b.path));
        if let Err(e) = cache::save(&cache_path, &saved) {
            eprintln!("symapper: cannot write cache: {e}");
        }
    }
    Ok(Collected {
        toplevel: paths.toplevel,
        sources: listed.sources,
        files,
        root_files: listed.root_files,
        stats,
    })
}

pub fn list_files(opts: &Options) -> Result<files::FileList, Error> {
    let paths = repo_paths(opts)?;
    files::list(&paths.toplevel, &paths.common_dir, &opts.exclude)
}

fn repo_paths(opts: &Options) -> Result<git::RepoPaths, Error> {
    if !opts.repo.is_dir() {
        return Err(Error::Usage(format!(
            "not a directory: {}",
            opts.repo.display()
        )));
    }
    git::repo_paths(&opts.repo)
}

#[cfg(test)]
mod focus_content_tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn no_focus_or_stem_only_focus_makes_no_content_reads() {
        let paths = vec!["a.rs".into()];
        let calls = AtomicUsize::new(0);
        let read = |_: &str| {
            calls.fetch_add(1, Ordering::Relaxed);
            Some("mod".into())
        };
        for focus in [
            focus::Focus::default(),
            focus::Focus {
                idents: BTreeSet::from(["mod".into()]),
                stem_idents: BTreeSet::from(["mod".into()]),
                ..focus::Focus::default()
            },
        ] {
            let content = read_content(&paths, &focus::ContentTerms::new(&focus), &read);
            assert!(content.matches.is_empty());
            assert!(content.failed.is_empty());
        }
        assert_eq!(calls.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn content_reads_keep_path_order_and_failed_reads_are_not_retried_for_render() {
        let focus = focus::Focus {
            idents: BTreeSet::from(["needle".into()]),
            ..focus::Focus::default()
        };
        let paths = ["z.rs", "broken.rs", "a.rs"].map(String::from);
        let calls = AtomicUsize::new(0);
        let read = |path: &str| {
            calls.fetch_add(1, Ordering::Relaxed);
            match path {
                "broken.rs" => None,
                "z.rs" => Some("// needle".into()),
                _ => Some("fn alpha() {}".into()),
            }
        };
        let content = read_content(&paths, &focus::ContentTerms::new(&focus), &read);
        assert_eq!(
            content.matches,
            [BTreeSet::from([0]), BTreeSet::new(), BTreeSet::new()]
        );
        assert_eq!(content.failed, BTreeSet::from(["broken.rs".into()]));
        assert_eq!(calls.load(Ordering::Relaxed), 3);
        assert!(content.read_for_render("broken.rs", &read).is_none());
        assert_eq!(calls.load(Ordering::Relaxed), 3);
        assert!(content.read_for_render("z.rs", &read).is_some());
        assert_eq!(calls.load(Ordering::Relaxed), 4);
    }
}
