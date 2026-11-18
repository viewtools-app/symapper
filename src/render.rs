//! Pure rendering of ranked definitions within an estimated token budget.

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    graph::Graph,
    rank::DefRank,
    tags::{FileTags, TagKind},
};

/// Source text and line starts, computed once per successful source read.
pub struct CachedSource {
    text: String,
    line_starts: Vec<usize>,
}

impl CachedSource {
    fn new(text: String) -> Self {
        let line_starts = std::iter::once(0)
            .chain(text.match_indices('\n').map(|(offset, _)| offset + 1))
            .collect();
        Self { text, line_starts }
    }

    fn row(&self, row: usize) -> Option<&str> {
        let &start = self.line_starts.get(row)?;
        let end = self
            .line_starts
            .get(row + 1)
            .map_or(self.text.len(), |next| next - 1);
        Some(&self.text[start..end])
    }
}

/// In-memory tags and ranks; file indices use the graph's path order.
/// Ranked items must be ordered best first, as returned by rank::split.
pub struct Input<'a> {
    /// Required graph files; an empty set preserves the original rendering.
    pub floor: &'a BTreeSet<usize>,
    pub late_roots: bool,
    pub root_files: &'a [String],
    pub graph: &'a Graph,
    pub files: &'a [FileTags],
    pub file_ranks: &'a [f64],
    pub file_scores: &'a [f64],
    pub ranked: &'a [DefRank],
    pub focus_files: &'a BTreeSet<usize>,
}

/// Estimate tokens using Unicode characters, including layout characters.
pub fn tokens(text: &str) -> usize {
    text.chars().count() / 4
}

/// Print sorted root paths, then file blocks ordered by their best ranked item.
/// With late roots, mapped sources leave the root group and roots follow blocks.
/// A block contains an unindented "path:" header followed by source rows, with
/// "..." between non-adjacent rows. Blocks have no blank separator. Nonempty
/// output ends in exactly one newline. Rows are trimmed, then cut to 100 chars.
///
/// After the blocks, append remaining paths by descending focus score, then file
/// rank, then path. Stop at the first path that exceeds the budget unless floor
/// paths still wait; then skip non-fitting non-floor paths to preserve that reserve.
/// Printed blocks, focus files and roots already printed are excluded.
/// A nonempty floor preserves the plain output if it already includes all required
/// files; otherwise reserve bare floor paths before selecting blocks and other names.
/// Explicit focus files remain excluded even if listed in the floor.
///
/// Source is supplied by the caller: this module performs no I/O. Each requested
/// file (including missing sources) is memoized across all bisection candidates.
/// Focus files are excluded defensively even if present in the ranked input.
pub fn render(
    input: &Input<'_>,
    budget_tokens: usize,
    source: &mut dyn FnMut(&str) -> Option<String>,
) -> String {
    render_with_files(input, budget_tokens, source, &mut BTreeMap::new()).0
}

/// Render and return all printed graph file indices, reusing a caller-owned source cache.
///
/// A nonempty floor must contain only files printed by a render of the same input
/// at the same budget (as returned by `map_v010`). This makes its bare paths fit.
/// An infeasible floor can exceed the budget; debug builds assert this precondition.
pub fn render_with_files(
    input: &Input<'_>,
    budget_tokens: usize,
    source: &mut dyn FnMut(&str) -> Option<String>,
    sources: &mut BTreeMap<usize, Option<CachedSource>>,
) -> (String, BTreeSet<usize>) {
    if !input.floor.is_empty() {
        let plain_input = Input {
            floor: &BTreeSet::new(),
            ..*input
        };
        let plain = render_with_files(&plain_input, budget_tokens, source, sources);
        if input
            .floor
            .difference(input.focus_files)
            .all(|file| plain.1.contains(file))
        {
            return plain;
        }
    }

    let mut roots: Vec<_> = input
        .root_files
        .iter()
        .filter(|path| {
            !input
                .focus_files
                .iter()
                .any(|&file| input.graph.paths.get(file) == Some(*path))
        })
        .filter(|path| !input.late_roots || !input.graph.paths.contains(path))
        .cloned()
        .collect();
    roots.sort();
    roots.dedup();
    let mut root_text = roots_text(&roots);
    let no_blocks = BTreeSet::new();
    while !roots.is_empty()
        && (root_text.chars().count() + floor_reserve(input, &no_blocks, &roots)) / 4
            > budget_tokens
    {
        roots.pop();
        root_text = roots_text(&roots);
    }
    debug_assert!(
        (root_text.chars().count() + floor_reserve(input, &no_blocks, &roots)) / 4 <= budget_tokens,
        "the legacy floor paths must fit within the same budget"
    );

    let mut best = root_text.clone();
    let mut best_blocks = BTreeSet::new();
    let mut low = 0;
    let mut high = input.ranked.len();
    while low <= high {
        let n = low + (high - low) / 2;
        let (candidate, blocks) = render_prefix(input, n, &root_text, source, sources);
        let count = (candidate.chars().count() + floor_reserve(input, &blocks, &roots)) / 4;
        if count <= budget_tokens {
            best = candidate;
            best_blocks = blocks;
            if count as f64 >= 0.85 * budget_tokens as f64 || n == input.ranked.len() {
                break;
            }
            low = n + 1;
        } else if n == 0 {
            break;
        } else {
            high = n - 1;
        }
    }
    let mut remaining: Vec<_> = (0..input.graph.paths.len())
        .filter(|file| {
            !best_blocks.contains(file)
                && !input.focus_files.contains(file)
                && !roots.contains(&input.graph.paths[*file])
        })
        .collect();
    remaining.sort_by(|&a, &b| {
        input
            .file_scores
            .get(b)
            .unwrap_or(&0.0)
            .total_cmp(input.file_scores.get(a).unwrap_or(&0.0))
            .then_with(|| input.file_ranks[b].total_cmp(&input.file_ranks[a]))
            .then_with(|| input.graph.paths[a].cmp(&input.graph.paths[b]))
    });
    best_blocks.extend(
        input
            .graph
            .paths
            .iter()
            .enumerate()
            .filter_map(|(file, path)| roots.contains(path).then_some(file)),
    );
    let mut chars = best.chars().count();
    let mut reserved = floor_reserve(input, &best_blocks, &roots);
    for file in remaining {
        let path = &input.graph.paths[file];
        let line_chars = path.chars().count() + 1;
        let is_floor = input.floor.contains(&file);
        if is_floor {
            reserved -= line_chars;
        }
        let next_chars = chars + line_chars;
        if !is_floor && (next_chars + reserved) / 4 > budget_tokens {
            if reserved > 0 {
                continue;
            }
            break;
        }
        best_blocks.insert(file);
        best.push_str(path);
        best.push('\n');
        chars = next_chars;
    }
    (best, best_blocks)
}

/// Reserve bare path lines, counting characters before the caller divides by four.
fn floor_reserve(input: &Input<'_>, blocks: &BTreeSet<usize>, roots: &[String]) -> usize {
    input
        .floor
        .iter()
        .filter(|file| !input.focus_files.contains(file) && !blocks.contains(file))
        .filter_map(|&file| input.graph.paths.get(file))
        .filter(|path| !roots.contains(path))
        .map(|path| path.chars().count() + 1)
        .sum()
}

fn roots_text(roots: &[String]) -> String {
    if roots.is_empty() {
        String::new()
    } else {
        format!("{}\n", roots.join("\n"))
    }
}

fn render_prefix(
    input: &Input<'_>,
    n: usize,
    roots: &str,
    source: &mut dyn FnMut(&str) -> Option<String>,
    sources: &mut BTreeMap<usize, Option<CachedSource>>,
) -> (String, BTreeSet<usize>) {
    let mut blocks = BTreeSet::new();
    let mut order = Vec::new();
    let mut rows_by_file: BTreeMap<usize, BTreeSet<u32>> = BTreeMap::new();
    for item in &input.ranked[..n] {
        if input.focus_files.contains(&item.file) {
            continue;
        }
        let (Some(file), Some(name), Some(path)) = (
            input.files.get(item.file),
            input.graph.idents.get(item.ident),
            input.graph.paths.get(item.file),
        ) else {
            continue;
        };
        if &file.path != path {
            continue;
        }
        let mut rows = BTreeSet::new();
        for tag in &file.tags {
            if tag.kind == TagKind::Def && &tag.name == name {
                rows.insert(tag.line);
                rows.extend(&tag.scopes);
            }
        }
        if rows.is_empty() {
            continue;
        }
        if !rows_by_file.contains_key(&item.file) {
            order.push(item.file);
        }
        rows_by_file.entry(item.file).or_default().extend(rows);
    }
    let mut out = if input.late_roots {
        String::new()
    } else {
        roots.to_string()
    };
    for file in order {
        let path = &input.graph.paths[file];
        let Some(text) = sources
            .entry(file)
            .or_insert_with(|| source(path).map(CachedSource::new))
            .as_ref()
        else {
            continue;
        };
        let mut previous = None;
        let mut block = String::new();
        for &row in &rows_by_file[&file] {
            let Some(line) = text.row(row as usize) else {
                continue;
            };
            if previous.is_some_and(|prev| row > prev + 1) {
                block.push_str("...\n");
            }
            block.extend(line.trim_end().chars().take(100));
            block.push('\n');
            previous = Some(row);
        }
        if !block.is_empty() {
            blocks.insert(file);
            out.push_str(path);
            out.push_str(":\n");
            out.push_str(&block);
        }
    }
    if input.late_roots {
        out.push_str(roots);
    }
    if !out.is_empty() {
        out.truncate(out.trim_end_matches('\n').len());
        out.push('\n');
    }
    (out, blocks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tags::{Tag, TagKind};
    use std::collections::BTreeMap;

    #[test]
    fn cached_source_rows_match_split_indexing() {
        for text in ["", "a", "a\n", "a\n\nb", "x\r\ny\r\n", "é\nü€\n"] {
            let source = CachedSource::new(text.to_string());
            let rows = text.split('\n').count();
            for row in 0..=rows + 1 {
                assert_eq!(
                    source.row(row),
                    text.split('\n').nth(row),
                    "{text:?}, row {row}"
                );
            }
        }
    }

    struct Fixture {
        floor: BTreeSet<usize>,
        late_roots: bool,
        roots: Vec<String>,
        graph: Graph,
        files: Vec<FileTags>,
        ranked: Vec<DefRank>,
        file_ranks: Vec<f64>,
        file_scores: Vec<f64>,
        focus: BTreeSet<usize>,
        source: BTreeMap<String, String>,
    }
    impl Fixture {
        fn new(paths: &[&str]) -> Self {
            let paths: Vec<String> = paths.iter().map(|s| s.to_string()).collect();
            Self {
                floor: BTreeSet::new(),
                late_roots: false,
                roots: vec!["README.md".into(), "Cargo.toml".into()],
                graph: Graph {
                    paths: paths.clone(),
                    idents: vec!["item".into()],
                    groups: vec![],
                },
                files: paths
                    .iter()
                    .map(|p| FileTags {
                        path: p.clone(),
                        mtime_ns: 0,
                        size: 0,
                        tags: vec![Tag {
                            name: "item".into(),
                            kind: TagKind::Def,
                            line: 0,
                            scopes: vec![],
                        }],
                    })
                    .collect(),
                ranked: (0..paths.len())
                    .map(|file| DefRank {
                        file,
                        ident: 0,
                        rank: 1.0,
                    })
                    .collect(),
                file_ranks: vec![1.0; paths.len()],
                file_scores: vec![],
                focus: BTreeSet::new(),
                source: paths
                    .into_iter()
                    .map(|p| (p, "fn item() {}   \n".into()))
                    .collect(),
            }
        }
        fn run(&self, budget: usize) -> String {
            let mut calls = BTreeSet::new();
            render(
                &Input {
                    floor: &self.floor,
                    late_roots: self.late_roots,
                    root_files: &self.roots,
                    graph: &self.graph,
                    files: &self.files,
                    ranked: &self.ranked,
                    file_ranks: &self.file_ranks,
                    file_scores: &self.file_scores,
                    focus_files: &self.focus,
                },
                budget,
                &mut |path| {
                    assert!(calls.insert(path.to_string()), "source fetched twice");
                    self.source.get(path).cloned()
                },
            )
        }
    }
    #[test]
    fn printed_files_and_cache_are_shared_across_renders() {
        let mut fixture = Fixture::new(&["a.rs", "b.rs", "Package.swift", "missing.rs"]);
        fixture.roots.push("Package.swift".into());
        fixture.ranked.truncate(1);
        fixture.source.remove("missing.rs");
        let input = Input {
            floor: &fixture.floor,
            late_roots: false,
            root_files: &fixture.roots,
            graph: &fixture.graph,
            files: &fixture.files,
            ranked: &fixture.ranked,
            file_ranks: &fixture.file_ranks,
            file_scores: &fixture.file_scores,
            focus_files: &fixture.focus,
        };
        let mut cache = BTreeMap::new();
        let mut calls = 0;
        let mut source = |path: &str| {
            calls += 1;
            fixture.source.get(path).cloned()
        };
        for budget in [200, 300] {
            let (text, printed) = render_with_files(&input, budget, &mut source, &mut cache);
            assert_eq!(text, fixture.run(budget));
            assert_eq!(printed, BTreeSet::from([0, 1, 2, 3]));
        }
        assert_eq!(calls, 1);
    }

    #[test]
    fn shared_cache_remembers_failed_sources() {
        let fixture = Fixture::new(&["a.rs"]);
        let input = Input {
            floor: &fixture.floor,
            late_roots: false,
            root_files: &fixture.roots,
            graph: &fixture.graph,
            files: &fixture.files,
            ranked: &fixture.ranked,
            file_ranks: &fixture.file_ranks,
            file_scores: &fixture.file_scores,
            focus_files: &fixture.focus,
        };
        let mut cache = BTreeMap::new();
        let mut calls = 0;
        let mut source = |_: &str| {
            calls += 1;
            None
        };
        for _ in 0..2 {
            let (_, printed) = render_with_files(&input, 200, &mut source, &mut cache);
            assert_eq!(printed, BTreeSet::from([0]));
        }
        assert_eq!(calls, 1);
    }

    #[test]
    fn printed_set_tracks_only_paths_that_fit() {
        let mut fixture = Fixture::new(&["Package.swift", "a.rs", "b.rs"]);
        fixture.roots.push("Package.swift".into());
        for late_roots in [false, true] {
            fixture.late_roots = late_roots;
            let input = Input {
                floor: &fixture.floor,
                late_roots,
                root_files: &fixture.roots,
                graph: &fixture.graph,
                files: &fixture.files,
                ranked: &fixture.ranked,
                file_ranks: &fixture.file_ranks,
                file_scores: &fixture.file_scores,
                focus_files: &fixture.focus,
            };
            let mut cache = BTreeMap::new();
            for budget in 0..100 {
                let (text, printed) = render_with_files(
                    &input,
                    budget,
                    &mut |path| fixture.source.get(path).cloned(),
                    &mut cache,
                );
                assert_eq!(text, fixture.run(budget));
                let expected = fixture
                    .graph
                    .paths
                    .iter()
                    .enumerate()
                    .filter_map(|(file, path)| {
                        text.lines()
                            .any(|line| line == path || line == format!("{path}:"))
                            .then_some(file)
                    })
                    .collect();
                assert_eq!(
                    printed, expected,
                    "budget={budget}, late_roots={late_roots}"
                );
            }
        }
    }

    #[test]
    fn root_files_first_and_sorted() {
        assert_eq!(
            Fixture::new(&["a.rs"]).run(200),
            "Cargo.toml\nREADME.md\na.rs:\nfn item() {}\n"
        );
    }
    #[test]
    fn late_roots_follow_blocks_before_names() {
        let mut f = Fixture::new(&["block.rs", "name.rs", "focus.rs"]);
        f.late_roots = true;
        f.ranked.truncate(1);
        f.focus.insert(2);
        f.roots.extend(["README.md".into(), "focus.rs".into()]);
        assert_eq!(
            f.run(200),
            "block.rs:\nfn item() {}\nCargo.toml\nREADME.md\nname.rs\n"
        );
    }
    #[test]
    fn late_roots_mapped_sources_print_once_at_normal_position() {
        let mut f = Fixture::new(&["a.rs", "Package.swift"]);
        f.late_roots = true;
        f.roots.push("Package.swift".into());
        assert_eq!(
            f.run(200),
            "a.rs:\nfn item() {}\nPackage.swift:\nfn item() {}\nCargo.toml\nREADME.md\n"
        );
        f.ranked.truncate(1);
        assert_eq!(
            f.run(200),
            "a.rs:\nfn item() {}\nCargo.toml\nREADME.md\nPackage.swift\n"
        );
    }
    #[test]
    fn late_roots_reserve_the_same_block_budget() {
        let mut f = Fixture::new(&["a.rs", "b.rs", "c.rs", "d.rs"]);
        let mut saw_partial = false;
        for budget in 0..100 {
            f.late_roots = false;
            let early = f.run(budget);
            f.late_roots = true;
            let late = f.run(budget);
            let count = |text: &str| text.lines().filter(|line| line.ends_with(':')).count();
            assert_eq!(count(&early), count(&late), "budget {budget}");
            assert_eq!(tokens(&early), tokens(&late), "budget {budget}");
            assert!(tokens(&late) <= budget);
            saw_partial |= (1..4).contains(&count(&late));
        }
        assert!(saw_partial);
    }
    #[test]
    fn late_roots_trim_sorted_deduplicated_paths_from_end() {
        let mut f = Fixture::new(&[]);
        f.late_roots = true;
        f.roots = vec!["z.md".into(), "b.md".into(), "a.md".into(), "b.md".into()];
        assert_eq!(f.run(4), "a.md\nb.md\nz.md\n");
        assert_eq!(f.run(2), "a.md\nb.md\n");
        assert_eq!(f.run(1), "a.md\n");
        assert_eq!(f.run(0), "");
    }
    #[test]
    fn files_follow_best_rank() {
        let mut f = Fixture::new(&["a.rs", "b.rs"]);
        f.ranked.reverse();
        let out = f.run(200);
        assert!(out.find("b.rs:").unwrap() < out.find("a.rs:").unwrap());
    }
    #[test]
    fn scope_rows_and_all_matching_definitions() {
        let mut f = Fixture::new(&["a.rs"]);
        f.files[0].tags[0].line = 1;
        f.files[0].tags[0].scopes = vec![0, 0];
        f.files[0].tags.push(Tag {
            name: "item".into(),
            kind: TagKind::Def,
            line: 2,
            scopes: vec![0],
        });
        f.source.insert(
            "a.rs".into(),
            "impl Thing {\nfn item() {}\nfn item(x: i32) {}\n".into(),
        );
        assert!(
            f.run(200)
                .ends_with("a.rs:\nimpl Thing {\nfn item() {}\nfn item(x: i32) {}\n")
        );
    }
    #[test]
    fn gaps_have_ellipsis_only_between_rows() {
        let mut f = Fixture::new(&["a.rs"]);
        f.files[0].tags[0].line = 3;
        f.files[0].tags[0].scopes = vec![1];
        f.source
            .insert("a.rs".into(), "hidden\nscope\nhidden\ndef\n".into());
        assert!(f.run(200).ends_with("a.rs:\nscope\n...\ndef\n"));
    }
    #[test]
    fn multibyte_lines_cut_at_100_chars() {
        let mut f = Fixture::new(&["a.rs"]);
        f.source
            .insert("a.rs".into(), format!("{}  \n", "界".repeat(110)));
        assert!(
            f.run(200)
                .ends_with(&format!("a.rs:\n{}\n", "界".repeat(100)))
        );
    }
    #[test]
    fn focus_files_are_absent() {
        let mut f = Fixture::new(&["a.rs", "b.rs"]);
        f.focus.insert(0);
        let out = f.run(200);
        assert!(!out.contains("a.rs"));
        assert!(out.contains("b.rs"));
    }
    #[test]
    fn bisection_fills_budgets_and_memoizes_sources() {
        let mut f = Fixture::new(&["a.rs"]);
        f.graph.idents = (0..100).map(|i| format!("item{i}")).collect();
        f.files[0].tags = (0..100)
            .map(|i| Tag {
                name: format!("item{i}"),
                kind: TagKind::Def,
                line: i,
                scopes: vec![],
            })
            .collect();
        f.ranked = (0..100)
            .map(|ident| DefRank {
                file: 0,
                ident,
                rank: 1.0,
            })
            .collect();
        f.source.insert(
            "a.rs".into(),
            (0..100).map(|_| format!("{}\n", "x".repeat(90))).collect(),
        );
        for budget in [200, 1500] {
            let n = tokens(&f.run(budget));
            assert!(n <= budget && n * 100 >= budget * 85, "{n} for {budget}");
        }
    }
    #[test]
    fn oversized_root_list_is_trimmed_from_end() {
        let f = Fixture::new(&[]);
        assert_eq!(f.run(3), "Cargo.toml\n");
        assert_eq!(f.run(0), "");
    }
    #[test]
    fn missing_source_and_empty_input() {
        let mut f = Fixture::new(&["a.rs"]);
        f.source.clear();
        assert_eq!(f.run(200), "Cargo.toml\nREADME.md\na.rs\n");
        f.roots.clear();
        assert_eq!(f.run(200), "a.rs\n");
        f.graph.paths.clear();
        assert_eq!(f.run(200), "");
    }
    #[test]
    fn name_list_follows_blocks_with_path_ties() {
        let mut f = Fixture::new(&["block.rs", "z.rs", "b.rs", "a.rs"]);
        f.ranked.truncate(1);
        assert_eq!(
            f.run(200),
            "Cargo.toml\nREADME.md\nblock.rs:\nfn item() {}\na.rs\nb.rs\nz.rs\n"
        );
    }
    #[test]
    fn name_list_orders_by_descending_file_rank() {
        let mut f = Fixture::new(&["a.rs", "b.rs", "z.rs"]);
        f.roots.clear();
        f.ranked.clear();
        f.file_ranks = vec![0.1, 0.2, 0.7];
        assert_eq!(f.run(200), "z.rs\nb.rs\na.rs\n");
    }
    #[test]
    fn name_list_excludes_focus_and_printed_roots() {
        let mut f = Fixture::new(&["README.md", "focus.rs", "other.rs"]);
        f.ranked.clear();
        f.focus.insert(1);
        assert_eq!(f.run(200), "Cargo.toml\nREADME.md\nother.rs\n");
    }
    #[test]
    fn name_list_stops_at_first_path_that_exceeds_budget() {
        let mut f = Fixture::new(&["a.rs", "b_very_long_path.rs", "c.rs"]);
        f.roots.clear();
        f.ranked.clear();
        assert_eq!(f.run(2), "a.rs\n");
        for budget in 0..30 {
            let out = f.run(budget);
            assert!(tokens(&out) <= budget);
            assert!(out.is_empty() || (out.ends_with("\n") && !out.ends_with("\n\n")));
        }
    }
    #[test]
    fn definitions_that_do_not_fit_still_get_a_name_line() {
        let mut f = Fixture::new(&["a.rs"]);
        f.roots.clear();
        assert_eq!(f.run(2), "a.rs\n");
    }

    #[test]
    fn name_list_orders_by_focus_score_then_rank_then_path() {
        let mut f = Fixture::new(&["a.rs", "b.rs", "c.rs", "d.rs", "e.rs"]);
        f.roots.clear();
        f.ranked.clear();
        f.file_ranks = vec![0.8, 0.1, 0.2, 0.2, 0.9];
        f.file_scores = vec![0.0, 1.0, 1.0, 1.0, 0.5];
        assert_eq!(f.run(200), "c.rs\nd.rs\nb.rs\ne.rs\na.rs\n");
        f.file_scores = vec![0.0; 5];
        assert_eq!(f.run(200), "e.rs\na.rs\nc.rs\nd.rs\nb.rs\n");
    }
    #[test]
    fn floor_skips_names_only_until_the_last_floor_path() {
        let mut f = Fixture::new(&[
            "a.rs",
            "b_very_long_path.rs",
            "c.rs",
            "d_very_long_path.rs",
            "e.rs",
        ]);
        f.roots.clear();
        f.ranked.clear();
        assert_eq!(f.run(4), "a.rs\n");
        f.floor.insert(2);
        let out = f.run(4);
        assert_eq!(out, "a.rs\nc.rs\n");
        assert!(tokens(&out) <= 4);
    }

    #[test]
    fn floor_cuts_blocks_before_dropping_paths() {
        let mut f = Fixture::new(&["a.rs", "b.rs", "c.rs"]);
        f.roots.clear();
        f.late_roots = true;
        let plain = f.run(9);
        assert!(!plain.lines().any(|line| line == "c.rs" || line == "c.rs:"));
        f.floor.insert(2);
        let out = f.run(9);
        assert!(out.lines().any(|line| line == "c.rs"), "{out}");
        assert!(!out.contains("c.rs:"));
        assert!(
            out.lines().filter(|line| line.ends_with(':')).count()
                < plain.lines().filter(|line| line.ends_with(':')).count()
        );
        assert!(tokens(&out) <= 9);
        assert_eq!(out, f.run(9));
        for path in &f.graph.paths {
            assert_eq!(
                out.lines()
                    .filter(|line| *line == path || *line == format!("{path}:"))
                    .count(),
                1
            );
        }
    }

    #[test]
    fn floor_keeps_mapped_roots_with_late_roots() {
        let mut f = Fixture::new(&["a.rs", "b.rs", "Package.swift"]);
        f.roots = vec!["Package.swift".into()];
        f.late_roots = true;
        f.file_scores = vec![2.0, 1.0, 0.0];
        assert!(!f.run(9).contains("Package.swift"));
        f.floor.insert(2);
        let out = f.run(9);
        assert!(out.lines().any(|line| line == "Package.swift"), "{out}");
        assert!(tokens(&out) <= 9);
    }

    #[test]
    fn floor_already_met_preserves_plain_output_and_ignores_focus_files() {
        let mut f = Fixture::new(&["a.rs", "b.rs", "c.rs", "focus.rs"]);
        f.focus.insert(3);
        f.late_roots = true;
        for scores in [vec![], vec![0.0, 1.0, 2.0, 3.0]] {
            f.file_scores = scores;
            for budget in 0..100 {
                f.floor.clear();
                let plain = f.run(budget);
                f.floor = f
                    .graph
                    .paths
                    .iter()
                    .enumerate()
                    .filter_map(|(file, path)| {
                        plain
                            .lines()
                            .any(|line| line == path || line == format!("{path}:"))
                            .then_some(file)
                    })
                    .collect();
                f.floor.insert(3);
                assert_eq!(plain, f.run(budget), "budget={budget}");
                assert!(!plain.contains("focus.rs"));
            }
        }
    }

    #[test]
    fn floor_budget_sweep_reserves_paths_and_trims_roots() {
        let paths: Vec<_> = std::iter::once("Package.swift".to_string())
            .chain((0..40).map(|i| format!("src/界_item_{i:02}.rs")))
            .collect();
        let mut f = Fixture::new(&paths.iter().map(String::as_str).collect::<Vec<_>>());
        f.roots = (0..30)
            .map(|i| format!(".github/workflows/job_{i:02}.yml"))
            .collect();
        f.roots.extend([
            "Package.swift".into(),
            "README.md".into(),
            "README.md".into(),
        ]);
        for text in f.source.values_mut() {
            *text = format!("{}\n", "界".repeat(90));
        }
        let mut changed = 0;
        for budget in 50..=2000 {
            f.late_roots = false;
            f.floor.clear();
            f.ranked.sort_by_key(|item| item.file);
            let legacy = f.run(budget);
            let floor: BTreeSet<_> = f
                .graph
                .paths
                .iter()
                .enumerate()
                .filter_map(|(file, path)| {
                    legacy
                        .lines()
                        .any(|line| line == path || line == format!("{path}:"))
                        .then_some(file)
                })
                .collect();
            f.late_roots = true;
            f.ranked.reverse();
            let plain = f.run(budget);
            f.floor = floor.clone();
            let input = Input {
                floor: &f.floor,
                late_roots: f.late_roots,
                root_files: &f.roots,
                graph: &f.graph,
                files: &f.files,
                ranked: &f.ranked,
                file_ranks: &f.file_ranks,
                file_scores: &f.file_scores,
                focus_files: &f.focus,
            };
            let (out, printed) = render_with_files(
                &input,
                budget,
                &mut |path| f.source.get(path).cloned(),
                &mut BTreeMap::new(),
            );
            assert!(
                tokens(&out) <= budget,
                "budget={budget}, tokens={}",
                tokens(&out)
            );
            assert!(
                floor.is_subset(&printed),
                "missing floor at budget={budget}"
            );
            for file in &floor {
                let path = &f.graph.paths[*file];
                assert_eq!(
                    out.lines()
                        .filter(|line| *line == path || *line == format!("{path}:"))
                        .count(),
                    1,
                    "budget={budget}, path={path}"
                );
            }
            // Check every graph path and root, including non-floor blocks and names.
            for path in f.graph.paths.iter().chain(&f.roots) {
                assert!(
                    out.lines()
                        .filter(|line| *line == path || *line == format!("{path}:"))
                        .count()
                        <= 1,
                    "budget={budget}, path={path}"
                );
            }
            assert_eq!(out, f.run(budget));
            changed += usize::from(out != plain);
        }
        assert!(changed > 0, "sweep must exercise the reserve pass");
    }

    #[test]
    fn token_estimate_counts_characters() {
        assert_eq!(tokens("界界界界界界界"), 1);
    }
}
