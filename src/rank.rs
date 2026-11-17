//! Pure weighted PageRank and allocation of file ranks to definitions.

use std::collections::{BTreeMap, BTreeSet};

use crate::graph::{Graph, Group};

const DAMPING: f64 = 0.85;
const MAX_ITERATIONS: usize = 100;
const TOLERANCE: f64 = 1e-6;

#[derive(Clone, Debug, PartialEq)]
pub struct DefRank {
    pub file: usize,
    pub ident: usize,
    pub rank: f64,
}

pub fn uniform(n: usize) -> Vec<f64> {
    if n == 0 {
        Vec::new()
    } else {
        vec![1.0 / n as f64; n]
    }
}

fn out_weights(n: usize, groups: &[Group]) -> Vec<f64> {
    let mut weights = vec![0.0; n];
    for group in groups {
        if group.referencers.is_empty() {
            for &node in &group.definers {
                weights[node] += 0.1;
            }
        } else {
            for source in &group.referencers {
                weights[source.from] += source.weight * group.definers.len() as f64;
            }
        }
    }
    weights
}

/// Visit each destination once, sharing the incoming mass calculation per group.
fn distribute(group: &Group, ranks: &[f64], weights: &[f64], mut add: impl FnMut(usize, f64)) {
    if group.referencers.is_empty() {
        for &node in &group.definers {
            add(node, ranks[node] * 0.1 / weights[node]);
        }
    } else {
        let mass: f64 = group
            .referencers
            .iter()
            .map(|source| ranks[source.from] * source.weight / weights[source.from])
            .sum();
        for &to in &group.definers {
            add(to, mass);
        }
    }
}

/// Compute ranks from uniform initial mass, using fixed-order power iteration.
///
/// The personalization vector must have n finite, nonnegative entries summing to
/// one. Groups must have positive finite weights and valid node indices; graphs
/// produced by graph::build satisfy these requirements.
pub fn pagerank(n: usize, groups: &[Group], personalization: &[f64]) -> Vec<f64> {
    iterate(n, groups, personalization, TOLERANCE).0
}

fn iterate(
    n: usize,
    groups: &[Group],
    personalization: &[f64],
    tolerance: f64,
) -> (Vec<f64>, usize) {
    if n == 0 {
        return (Vec::new(), 0);
    }
    assert_eq!(personalization.len(), n);
    let weights = out_weights(n, groups);
    let mut ranks = uniform(n);
    for iteration in 1..=MAX_ITERATIONS {
        let dangling: f64 = ranks
            .iter()
            .zip(&weights)
            .filter(|(_, weight)| **weight == 0.0)
            .map(|(rank, _)| rank)
            .sum();
        let mut next: Vec<_> = personalization
            .iter()
            .map(|p| (1.0 - DAMPING) * p + DAMPING * dangling * p)
            .collect();
        for group in groups {
            distribute(group, &ranks, &weights, |to, mass| {
                next[to] += DAMPING * mass;
            });
        }
        let delta: f64 = ranks
            .iter()
            .zip(&next)
            .map(|(old, new)| (old - new).abs())
            .sum();
        ranks = next;
        if delta < tolerance {
            return (ranks, iteration);
        }
    }
    (ranks, MAX_ITERATIONS)
}

/// Distribute each source file's rank over its outgoing edges by weight.
/// Dangling sources have no definitions to credit. Focus destinations are omitted.
/// Ties are resolved by destination path, then identifier name.
pub fn split(graph: &Graph, ranks: &[f64], focus_files: &BTreeSet<usize>) -> Vec<DefRank> {
    assert_eq!(ranks.len(), graph.paths.len());
    let weights = out_weights(graph.paths.len(), &graph.groups);
    let mut definitions = BTreeMap::new();
    for group in &graph.groups {
        distribute(group, ranks, &weights, |to, mass| {
            if !focus_files.contains(&to) {
                *definitions.entry((to, group.ident)).or_insert(0.0) += mass;
            }
        });
    }
    let mut definitions: Vec<_> = definitions
        .into_iter()
        .map(|((file, ident), rank)| DefRank { file, ident, rank })
        .collect();
    definitions.sort_by(|a, b| {
        b.rank
            .total_cmp(&a.rank)
            .then_with(|| graph.paths[a.file].cmp(&graph.paths[b.file]))
            .then_with(|| graph.idents[a.ident].cmp(&graph.idents[b.ident]))
    });
    definitions
}

pub const MATCHED_DEFS_PER_FILE: usize = 1;

/// Promote at most cap definitions per matched file, preserving prior order on ties.
pub fn focus_order(ranked: Vec<DefRank>, scores: &[f64], cap: usize) -> Vec<DefRank> {
    if cap == 0 || !scores.iter().any(|&score| score > 0.0) {
        return ranked;
    }
    let mut counts = vec![0; scores.len()];
    let mut prefix = Vec::new();
    let mut tail = Vec::new();
    for item in ranked {
        if scores.get(item.file).is_some_and(|&score| score > 0.0) && counts[item.file] < cap {
            counts[item.file] += 1;
            prefix.push(item);
        } else {
            tail.push(item);
        }
    }
    // Stable sort retains the current definition order for equal file scores.
    prefix.sort_by(|a, b| scores[b.file].total_cmp(&scores[a.file]));
    prefix.extend(tail);
    prefix
}

#[cfg(test)]
mod tests {
    use super::*;

    // Singleton groups preserve the original weighted-edge test fixtures.
    fn edge(from: usize, to: usize, ident: usize, weight: f64) -> Group {
        Group {
            ident,
            definers: vec![to],
            referencers: vec![crate::graph::Referencer { from, weight }],
        }
    }

    #[test]
    fn three_node_pagerank_matches_closed_form_and_is_repeatable() {
        let edges = [edge(0, 1, 0, 1.0), edge(1, 0, 0, 1.0), edge(2, 0, 0, 1.0)];
        // Uniform teleportation contributes 0.05 to each node. Thus c = 0.05,
        // b = 0.05 + 0.85a, a = 0.05 + 0.85(b + c).
        // Substitution gives a = 0.135 / 0.2775 = 18/37, b = 343/740.
        let ranks = pagerank(3, &edges, &uniform(3));
        for (actual, expected) in ranks.iter().zip([18.0 / 37.0, 343.0 / 740.0, 0.05]) {
            assert!((actual - expected).abs() < 1e-6);
        }
        assert!((ranks.iter().sum::<f64>() - 1.0).abs() < 1e-12);
        assert_eq!(
            ranks.iter().map(|r| r.to_bits()).collect::<Vec<_>>(),
            pagerank(3, &edges, &uniform(3))
                .iter()
                .map(|r| r.to_bits())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn dangling_mass_uses_personalization_and_empty_graph_is_safe() {
        assert_eq!(pagerank(3, &[], &[0.75, 0.25, 0.0]), [0.75, 0.25, 0.0]);
        let ranks = pagerank(2, &[edge(0, 1, 0, 1.0)], &[1.0, 0.0]);
        // a = 0.15 + 0.85b; b = 0.85a, hence a = 20/37, b = 17/37.
        assert!((ranks[0] - 20.0 / 37.0).abs() < 1e-6);
        assert!((ranks[1] - 17.0 / 37.0).abs() < 1e-6);
        assert!(pagerank(0, &[], &[]).is_empty());
        assert!(uniform(0).is_empty());
    }

    #[test]
    fn weighted_parallel_edges_share_outgoing_mass() {
        let edges = [
            edge(0, 1, 0, 1.0),
            edge(0, 1, 1, 2.0),
            edge(0, 2, 0, 1.0),
            edge(1, 0, 0, 1.0),
            edge(2, 0, 0, 1.0),
        ];
        let ranks = pagerank(3, &edges, &[1.0, 0.0, 0.0]);
        assert!((ranks[1] / ranks[2] - 3.0).abs() < 1e-12);
        assert!((ranks.iter().sum::<f64>() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn iteration_cap_and_l1_stop_rule() {
        let edges = [edge(0, 1, 0, 1.0), edge(1, 0, 0, 1.0)];
        // Zero tolerance disables early convergence so the hard cap is exercised.
        let (ranks, iterations) = iterate(2, &edges, &[1.0, 0.0], 0.0);
        assert_eq!(iterations, 100);
        // Starting at uniform: a_t = 20/37 + (1/2 - 20/37) * (-0.85)^t.
        assert!(
            (ranks[0] - (20.0 / 37.0 + (0.5 - 20.0 / 37.0) * (-0.85_f64).powi(100))).abs() < 1e-14
        );
        let (ranks, iterations) = iterate(2, &edges, &[1.0, 0.0], 1e-6);
        let expected_iteration = (1_i32..=100)
            .find(|&t| 0.15 * 0.85_f64.powi(t - 1) < 1e-6)
            .unwrap();
        assert_eq!(iterations, expected_iteration as usize);
        assert_eq!(ranks, pagerank(2, &edges, &[1.0, 0.0]));
    }

    #[test]
    fn split_conserves_mass_aggregates_filters_and_sorts() {
        let graph = Graph {
            paths: vec!["a".into(), "b".into(), "c".into()],
            idents: vec!["alpha".into(), "beta".into()],
            groups: vec![
                edge(0, 1, 0, 1.0),
                edge(0, 2, 1, 3.0),
                edge(1, 1, 0, 1.0),
                edge(2, 0, 0, 1.0),
                edge(2, 0, 1, 1.0),
            ],
        };
        let ranks = split(&graph, &[0.4, 0.3, 0.3], &BTreeSet::new());
        assert!((ranks.iter().map(|d| d.rank).sum::<f64>() - 1.0).abs() < 1e-12);
        assert_eq!((ranks[0].file, ranks[0].ident), (1, 0));
        assert!((ranks[0].rank - 0.4).abs() < 1e-12);
        assert_eq!((ranks[1].file, ranks[1].ident), (2, 1));
        assert_eq!((ranks[2].file, ranks[2].ident), (0, 0));
        assert_eq!((ranks[3].file, ranks[3].ident), (0, 1));
        let focused = split(&graph, &[0.4, 0.3, 0.3], &BTreeSet::from([1]));
        assert_eq!(focused, ranks[1..]);
        for source in 0..3 {
            let mut mass = [0.0; 3];
            mass[source] = 1.0;
            assert!(
                (split(&graph, &mass, &BTreeSet::new())
                    .iter()
                    .map(|d| d.rank)
                    .sum::<f64>()
                    - 1.0)
                    .abs()
                    < 1e-12
            );
        }
        let tied = Graph {
            paths: vec!["a".into(), "b".into()],
            idents: vec!["alpha".into(), "beta".into()],
            groups: vec![edge(0, 0, 0, 1.0), edge(0, 0, 1, 1.0), edge(1, 1, 0, 1.0)],
        };
        let tied_ranks = split(&tied, &[0.5, 0.25], &BTreeSet::new());
        assert_eq!(
            tied_ranks
                .iter()
                .map(|d| (d.file, d.ident))
                .collect::<Vec<_>>(),
            [(0, 0), (0, 1), (1, 0)]
        );
        assert!(
            split(
                &Graph {
                    paths: vec![],
                    idents: vec![],
                    groups: vec![]
                },
                &[],
                &BTreeSet::new()
            )
            .is_empty()
        );
    }
}

#[cfg(test)]
mod focus_tests {
    use super::*;

    #[test]
    fn matched_prefix_caps_files_preserves_ties_and_tail_order() {
        let ranked: Vec<_> = [2, 0, 1, 0, 2, 1, 0, 3]
            .into_iter()
            .enumerate()
            .map(|(ident, file)| DefRank {
                file,
                ident,
                rank: 1.0,
            })
            .collect();
        let order = focus_order(ranked.clone(), &[1.0, 2.0, 1.0, 0.0], 1);
        assert_eq!(
            order.iter().map(|item| item.ident).collect::<Vec<_>>(),
            [2, 0, 1, 3, 4, 5, 6, 7]
        );
        assert_eq!(focus_order(ranked.clone(), &[], 8), ranked);
        assert_eq!(focus_order(ranked.clone(), &[0.0; 4], 8), ranked);
        assert_eq!(focus_order(ranked.clone(), &[1.0; 4], 0), ranked);
    }
}
