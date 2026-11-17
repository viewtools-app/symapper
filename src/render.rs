//! Pure rendering of ranked definitions within an estimated token budget.

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    graph::Graph,
    rank::DefRank,
    tags::{FileTags, TagKind},
};

/// In-memory tags and ranks; file indices use the graph's path order.
/// Ranked items must be ordered best first, as returned by rank::split.
pub struct Input<'a> {
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
/// A block contains an unindented "path:" header followed by source rows, with
/// "..." between non-adjacent rows. Blocks have no blank separator. Nonempty
/// output ends in exactly one newline. Rows are trimmed, then cut to 100 chars.
///
/// After the blocks, append remaining paths by descending focus score, then file
/// rank, then path, stopping at the first path that exceeds the budget.
/// Printed blocks, focus files and roots already printed are excluded.
///
/// Source is supplied by the caller: this module performs no I/O. Each requested
/// file (including missing sources) is memoized across all bisection candidates.
/// Focus files are excluded defensively even if present in the ranked input.
pub fn render(
    input: &Input<'_>,
    budget_tokens: usize,
    source: &mut dyn FnMut(&str) -> Option<String>,
) -> String {
    let mut roots: Vec<_> = input
        .root_files
        .iter()
        .filter(|path| {
            !input
                .focus_files
                .iter()
                .any(|&file| input.graph.paths.get(file) == Some(*path))
        })
        .cloned()
        .collect();
    roots.sort();
    roots.dedup();
    let mut root_text = roots_text(&roots);
    while tokens(&root_text) > budget_tokens {
        roots.pop();
        root_text = roots_text(&roots);
    }

    let mut sources = BTreeMap::new();
    let mut best = root_text.clone();
    let mut best_blocks = BTreeSet::new();
    let mut low = 0;
    let mut high = input.ranked.len();
    while low <= high {
        let n = low + (high - low) / 2;
        let (candidate, blocks) = render_prefix(input, n, &root_text, source, &mut sources);
        let count = tokens(&candidate);
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
    let mut chars = best.chars().count();
    for file in remaining {
        let path = &input.graph.paths[file];
        let next_chars = chars + path.chars().count() + 1;
        if next_chars / 4 > budget_tokens {
            break;
        }
        best.push_str(path);
        best.push('\n');
        chars = next_chars;
    }
    best
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
    sources: &mut BTreeMap<usize, Option<String>>,
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
    let mut out = roots.to_string();
    for file in order {
        let path = &input.graph.paths[file];
        let Some(text) = sources.entry(file).or_insert_with(|| source(path)).as_ref() else {
            continue;
        };
        let lines: Vec<_> = text.split('\n').collect();
        let mut previous = None;
        let mut block = String::new();
        for &row in &rows_by_file[&file] {
            let Some(line) = lines.get(row as usize) else {
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

    struct Fixture {
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
    fn root_files_first_and_sorted() {
        assert_eq!(
            Fixture::new(&["a.rs"]).run(200),
            "Cargo.toml\nREADME.md\na.rs:\nfn item() {}\n"
        );
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
    fn token_estimate_counts_characters() {
        assert_eq!(tokens("界界界界界界界"), 1);
    }
}
