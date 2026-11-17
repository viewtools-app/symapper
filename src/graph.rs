//! Deterministic, identifier-weighted file graphs built only from in-memory tags.

use std::collections::{BTreeMap, BTreeSet};

use crate::focus::{term_matches, words};
use crate::tags::{FileTags, TagKind};

/// One weighted source entry per file referencing an identifier.
#[derive(Clone, Debug, PartialEq)]
pub struct Referencer {
    pub from: usize,
    pub weight: f64,
}

/// A shared identifier's complete bipartite graph, stored without pairwise edges.
/// Empty referencers mean each definer has only a fixed 0.1 self edge.
#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    pub ident: usize,
    pub definers: Vec<usize>,
    pub referencers: Vec<Referencer>,
}

/// Node and identifier indices follow lexicographically sorted paths and names.
/// Groups follow identifier order; definers and referencers follow node order.
#[derive(Clone, Debug, PartialEq)]
pub struct Graph {
    pub paths: Vec<String>,
    pub idents: Vec<String>,
    pub groups: Vec<Group>,
}

pub fn ident_mul(name: &str, defined_in: usize, focus_idents: &BTreeSet<String>) -> f64 {
    let terms: Vec<_> = focus_idents
        .iter()
        .map(|term| (term.to_lowercase(), words(term)))
        .collect();
    ident_mul_prepared(name, defined_in, &terms)
}

fn ident_mul_prepared(name: &str, defined_in: usize, terms: &[(String, Vec<String>)]) -> f64 {
    let mut multiplier = 1.0;
    let name_words = words(name);
    let lowercase = name.to_lowercase();
    if terms
        .iter()
        .any(|(term, parts)| !parts.is_empty() && *term == lowercase)
    {
        multiplier *= 10.0;
    } else if terms
        .iter()
        .any(|(_, parts)| term_matches(parts, &name_words))
    {
        multiplier *= 3.0;
    }
    let chars: Vec<_> = name.chars().collect();
    let snake = chars
        .windows(3)
        .any(|w| w[0].is_alphabetic() && w[1] == '_' && w[2].is_alphabetic());
    let camel = name.bytes().any(|c| c.is_ascii_lowercase())
        && name.bytes().any(|c| c.is_ascii_uppercase());
    if chars.len() >= 8 && (snake || name.contains('-') || camel) {
        multiplier *= 10.0;
    }
    if name.starts_with('_') {
        multiplier *= 0.1;
    }
    if defined_in > 5 {
        multiplier *= 0.1;
    }
    multiplier
}

pub fn build(files: &[FileTags], focus_idents: &BTreeSet<String>) -> Graph {
    let paths: Vec<_> = files
        .iter()
        .map(|file| file.path.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let nodes: BTreeMap<_, _> = paths
        .iter()
        .enumerate()
        .map(|(index, path)| (path.as_str(), index))
        .collect();
    let mut defines: BTreeMap<&str, BTreeSet<usize>> = BTreeMap::new();
    let mut references: BTreeMap<&str, BTreeMap<usize, usize>> = BTreeMap::new();
    for file in files {
        let node = nodes[file.path.as_str()];
        for tag in &file.tags {
            match tag.kind {
                TagKind::Def => {
                    defines.entry(&tag.name).or_default().insert(node);
                }
                TagKind::Ref => {
                    *references
                        .entry(&tag.name)
                        .or_default()
                        .entry(node)
                        .or_default() += 1;
                }
            }
        }
    }
    let idents: Vec<_> = defines.keys().map(|name| (*name).to_owned()).collect();
    let terms: Vec<_> = focus_idents
        .iter()
        .map(|term| (term.to_lowercase(), words(term)))
        .collect();
    let groups = defines
        .iter()
        .enumerate()
        .map(|(ident, (name, definers))| {
            let multiplier = ident_mul_prepared(name, definers.len(), &terms);
            let referencers = references
                .get(name)
                .into_iter()
                .flat_map(|sources| sources.iter())
                .map(|(&from, &count)| Referencer {
                    from,
                    weight: multiplier * (count as f64).sqrt(),
                })
                .collect();
            Group {
                ident,
                definers: definers.iter().copied().collect(),
                referencers,
            }
        })
        .collect();
    Graph {
        paths,
        idents,
        groups,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tags::{Tag, TagKind};

    fn file(path: &str, defs: &[&str], refs: &[&str]) -> FileTags {
        FileTags {
            path: path.into(),
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

    #[test]
    fn multipliers_and_product() {
        let focus = BTreeSet::from(["plain".into(), "_privateLongName".into()]);
        for (name, count, expected) in [
            ("plain", 1, 10.0),
            ("PLAIN", 1, 10.0),
            ("plainThing", 1, 30.0),
            ("plainer", 1, 1.0),
            ("ordinary", 1, 1.0),
            ("long_name", 1, 10.0),
            ("long-name", 1, 10.0),
            ("longName", 1, 10.0),
            ("aB", 1, 1.0),
            ("abcdef_g", 1, 10.0),
            ("abcdef_1", 1, 1.0),
            ("abcde__f", 1, 1.0),
            ("_short", 1, 0.1),
            ("short", 5, 1.0),
            ("short", 6, 0.1),
            ("_privateLongName", 6, 1.0),
        ] {
            assert!(
                (ident_mul(name, count, &focus) - expected).abs() < 1e-12,
                "{name}"
            );
        }
    }

    #[test]
    fn focus_word_runs_are_contiguous_and_whole_match_wins() {
        let focus = BTreeSet::from(["sheet layout".into(), "layout".into(), "_-/".into()]);
        assert_eq!(ident_mul("SheetLayout", 1, &focus), 30.0);
        assert_eq!(
            ident_mul(
                "SheetOtherLayout",
                1,
                &BTreeSet::from(["sheet layout".into()])
            ),
            10.0
        );
        assert_eq!(ident_mul("layout", 1, &focus), 10.0);
        assert_eq!(
            ident_mul("ordinary", 1, &BTreeSet::from(["_-/".into()])),
            1.0
        );
    }

    #[test]
    fn graph_weights_self_loops_and_deterministic_order() {
        let files = vec![
            file(
                "z.rs",
                &["foo", "foo", "alone"],
                &["foo", "foo", "foo", "foo", "external"],
            ),
            file("a.rs", &["foo"], &["foo"]),
            file("m.rs", &[], &[]),
        ];
        let graph = build(&files, &BTreeSet::new());
        assert_eq!(graph.paths, ["a.rs", "m.rs", "z.rs"]);
        assert_eq!(graph.idents, ["alone", "foo"]);
        let mut edges = Vec::new();
        for group in &graph.groups {
            for &to in &group.definers {
                if group.referencers.is_empty() {
                    edges.push((to, to, group.ident, 0.1));
                } else {
                    for source in &group.referencers {
                        edges.push((source.from, to, group.ident, source.weight));
                    }
                }
            }
        }
        edges.sort_by_key(|&(from, to, ident, _)| (from, to, ident));
        assert_eq!(
            edges,
            [
                (0, 0, 1, 1.0),
                (0, 2, 1, 1.0),
                (2, 0, 1, 2.0),
                (2, 2, 0, 0.1),
                (2, 2, 1, 2.0)
            ]
        );
        let mut reversed = files;
        reversed.reverse();
        for file in &mut reversed {
            file.tags.reverse();
        }
        assert_eq!(graph, build(&reversed, &BTreeSet::new()));
        let focused = build(&reversed, &BTreeSet::from(["foo".into()]));
        assert_eq!(focused.groups[1].referencers[0].weight, 10.0);
        assert_eq!(focused.groups[1].referencers[1].weight, 20.0);
    }

    #[test]
    fn unreferenced_definitions_get_fixed_self_edges() {
        let graph = build(
            &[
                file("b", &["_privateLongName"], &[]),
                file("a", &["_privateLongName"], &[]),
            ],
            &BTreeSet::from(["_privateLongName".into()]),
        );
        assert_eq!(
            graph.groups,
            [Group {
                ident: 0,
                definers: vec![0, 1],
                referencers: vec![],
            }]
        );
        assert!(build(&[], &BTreeSet::new()).paths.is_empty());
    }
}
