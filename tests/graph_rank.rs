use std::collections::{BTreeMap, BTreeSet};

use symapper::{
    graph, rank,
    tags::{FileTags, Tag, TagKind},
};

fn file(path: String, defs: &[&str], refs: &[&str]) -> FileTags {
    FileTags {
        path,
        mtime_ns: 0,
        size: 0,
        tags: defs
            .iter()
            .map(|name| (name, TagKind::Def))
            .chain(refs.iter().map(|name| (name, TagKind::Ref)))
            .map(|(name, kind)| Tag {
                name: (*name).into(),
                kind,
                line: 0,
                scopes: Vec::new(),
            })
            .collect(),
    }
}

// Independent copy of the old pairwise construction and rank computation.
fn pairwise_reference(
    files: &[FileTags],
    focus: &BTreeSet<String>,
    personal: &[f64],
) -> (Vec<f64>, BTreeMap<(usize, usize), f64>) {
    let paths: BTreeSet<_> = files.iter().map(|f| f.path.as_str()).collect();
    let nodes: BTreeMap<_, _> = paths.into_iter().enumerate().map(|(i, p)| (p, i)).collect();
    let mut defs: BTreeMap<&str, BTreeSet<usize>> = BTreeMap::new();
    let mut refs: BTreeMap<&str, BTreeMap<usize, usize>> = BTreeMap::new();
    for f in files {
        let node = nodes[f.path.as_str()];
        for tag in &f.tags {
            match tag.kind {
                TagKind::Def => {
                    defs.entry(&tag.name).or_default().insert(node);
                }
                TagKind::Ref => {
                    *refs.entry(&tag.name).or_default().entry(node).or_default() += 1;
                }
            }
        }
    }
    let mut edges = Vec::new();
    for (ident, (name, definers)) in defs.iter().enumerate() {
        if let Some(referencers) = refs.get(name) {
            for (&from, &count) in referencers {
                let weight = graph::ident_mul(name, definers.len(), focus) * (count as f64).sqrt();
                for &to in definers {
                    edges.push((from, to, ident, weight));
                }
            }
        } else {
            for &node in definers {
                edges.push((node, node, ident, 0.1));
            }
        }
    }
    edges.sort_by_key(|&(from, to, ident, _)| (from, to, ident));
    let n = nodes.len();
    let mut weights = vec![0.0; n];
    for &(from, _, _, weight) in &edges {
        weights[from] += weight;
    }
    let mut ranks = vec![1.0 / n as f64; n];
    for _ in 0..100 {
        let dangling: f64 = ranks
            .iter()
            .zip(&weights)
            .filter(|(_, w)| **w == 0.0)
            .map(|(r, _)| r)
            .sum();
        let mut next: Vec<_> = personal
            .iter()
            .map(|p| (1.0 - 0.85) * p + 0.85 * dangling * p)
            .collect();
        for &(from, to, _, weight) in &edges {
            next[to] += 0.85 * ranks[from] * weight / weights[from];
        }
        let delta: f64 = ranks.iter().zip(&next).map(|(a, b)| (a - b).abs()).sum();
        ranks = next;
        if delta < 1e-6 {
            break;
        }
    }
    let mut definitions = BTreeMap::new();
    for &(from, to, ident, weight) in &edges {
        *definitions.entry((to, ident)).or_insert(0.0) += ranks[from] * weight / weights[from];
    }
    (ranks, definitions)
}

#[test]
fn seeded_group_ranks_match_pairwise_reference() {
    let names = [
        "plain",
        "sharedName",
        "_private",
        "long_name",
        "focusThing",
        "rare",
    ];
    let mut state = 0x5eed_u64;
    let mut random = || {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        state >> 32
    };
    let mut files = Vec::new();
    for i in 0..40 {
        let mut defs = Vec::new();
        let mut refs = Vec::new();
        if i < 38 {
            for name in names {
                if random() % 3 == 0 {
                    defs.extend([name, name]);
                }
                for _ in 0..random() % 5 {
                    refs.push(name);
                }
            }
            if i % 7 == 0 {
                defs.push("unreferenced");
            }
            refs.push("external");
        }
        files.push(file(format!("{i:02}.rs"), &defs, &refs));
    }
    // Include a definition with no references and files with no outgoing edges.
    files.push(file("40.rs".into(), &["onlySelf"], &[]));
    let n = files.len();
    for focus in [
        BTreeSet::new(),
        BTreeSet::from(["focus".into(), "plain".into()]),
    ] {
        let graph = graph::build(&files, &focus);
        for personal in [rank::uniform(n), {
            let mut p = vec![0.0; n];
            p[0] = 0.7;
            p[n - 2] = 0.3;
            p
        }] {
            let (expected, defs) = pairwise_reference(&files, &focus, &personal);
            let actual = rank::pagerank(n, &graph.groups, &personal);
            for (i, (a, b)) in actual.iter().zip(&expected).enumerate() {
                assert!((a - b).abs() < 1e-12, "file {i}: {a} != {b}");
            }
            for excluded in [BTreeSet::new(), BTreeSet::from([0, 7, n - 1])] {
                let actual = rank::split(&graph, &actual, &excluded);
                assert_eq!(
                    actual.len(),
                    defs.keys()
                        .filter(|(file, _)| !excluded.contains(file))
                        .count()
                );
                for def in actual {
                    assert!(!excluded.contains(&def.file));
                    assert!((def.rank - defs[&(def.file, def.ident)]).abs() < 1e-12);
                }
            }
        }
    }
}

#[test]
fn shared_name_graph_storage_is_linear_and_ranking_completes() {
    let n = 3000;
    let files: Vec<_> = (0..n)
        .map(|i| file(format!("{i:04}.rs"), &["shared"], &["shared"]))
        .collect();
    let graph = graph::build(&files, &BTreeSet::new());
    let stored_entries: usize = graph
        .groups
        .iter()
        .map(|group| 1 + group.definers.len() + group.referencers.len())
        .sum();
    assert!(
        stored_entries < 10 * n,
        "stored {stored_entries} entries for {n} files"
    );
    let ranks = rank::pagerank(n, &graph.groups, &rank::uniform(n));
    assert_eq!(ranks.len(), n);
    for value in &ranks {
        assert!((value - 1.0 / n as f64).abs() < 1e-12);
    }
    let definitions = rank::split(&graph, &ranks, &BTreeSet::new());
    assert_eq!(definitions.len(), n);
    for def in definitions {
        assert!((def.rank - 1.0 / n as f64).abs() < 1e-12);
    }
}
