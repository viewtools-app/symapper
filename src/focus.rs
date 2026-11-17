//! Resolve focus values and build deterministic ranking personalization.

use std::collections::{BTreeMap, BTreeSet};

use crate::tags::{FileTags, TagKind};
use std::path::{Component, Path, PathBuf};

pub const CONTENT_WEIGHT: f64 = 0.75;
pub const COMMON_SHARE: f64 = 0.5;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Focus {
    pub files: BTreeSet<String>,
    pub idents: BTreeSet<String>,
    /// Identifiers contributed only by explicit file stems, never content terms.
    pub stem_idents: BTreeSet<String>,
}

/// Resolve only paths present in the mapped file list; preserve other values as identifiers.
pub fn resolve(values: &[String], toplevel: &Path, cwd: &Path, paths: &[String]) -> Focus {
    let known: BTreeSet<_> = paths.iter().map(String::as_str).collect();
    let root = normalize(
        &toplevel
            .canonicalize()
            .unwrap_or_else(|_| toplevel.to_owned()),
    );
    let cwd = normalize(&cwd.canonicalize().unwrap_or_else(|_| cwd.to_owned()));
    let lexical_root = normalize(toplevel);
    let mut focus = Focus::default();
    let mut explicit_idents = BTreeSet::new();
    for value in values {
        let path = Path::new(value);
        let candidates = if path.is_absolute() {
            let absolute = normalize(path);
            let rebased = absolute
                .strip_prefix(&lexical_root)
                .map(|suffix| root.join(suffix))
                .unwrap_or_else(|_| absolute.clone());
            [absolute, rebased]
        } else {
            [root.join(path), cwd.join(path)]
        };
        let matched = candidates
            .iter()
            .filter_map(|candidate| relative(candidate, &root))
            .find(|candidate| known.contains(candidate.as_str()));
        if let Some(file) = matched {
            if let Some(stem) = Path::new(&file).file_stem().and_then(|s| s.to_str()) {
                focus.idents.insert(stem.to_owned());
                focus.stem_idents.insert(stem.to_owned());
            }
            focus.files.insert(file);
        } else {
            focus.idents.insert(value.clone());
            explicit_idents.insert(value.clone());
        }
    }
    focus
        .stem_idents
        .retain(|ident| !explicit_idents.contains(ident));
    focus
}

fn relative(candidate: &Path, toplevel: &Path) -> Option<String> {
    normalize(candidate)
        .strip_prefix(toplevel)
        .ok()?
        .to_str()
        .map(str::to_owned)
}

fn normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if matches!(
                    normalized.components().next_back(),
                    Some(Component::Normal(_))
                ) {
                    normalized.pop();
                } else if !normalized.has_root() {
                    normalized.push(component.as_os_str());
                }
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

/// Split separators and case boundaries, retaining digits in their word.
pub fn words(value: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut word = String::new();
    let chars: Vec<_> = value.chars().collect();
    for (index, &ch) in chars.iter().enumerate() {
        let separator = matches!(ch, '_' | '-' | '.' | '/') || ch.is_whitespace();
        let boundary = ch.is_uppercase()
            && index > 0
            && (chars[index - 1].is_lowercase()
                || chars[index - 1].is_numeric()
                || (chars[index - 1].is_uppercase()
                    && chars.get(index + 1).is_some_and(|next| next.is_lowercase())));
        if (separator || boundary) && !word.is_empty() {
            result.push(std::mem::take(&mut word));
        }
        if !separator {
            word.extend(ch.to_lowercase());
        }
    }
    if !word.is_empty() {
        result.push(word);
    }
    result
}

fn case_boundary(previous: Option<char>, ch: char, next: Option<char>) -> bool {
    ch.is_uppercase()
        && previous.is_some_and(|prev| {
            prev.is_lowercase()
                || prev.is_numeric()
                || (prev.is_uppercase() && next.is_some_and(char::is_lowercase))
        })
}

/// Stream content words with the same case rules as words, splitting all punctuation.
pub fn content_words(text: &str) -> impl Iterator<Item = String> + '_ {
    let mut chars = text.chars().peekable();
    let mut previous = None;
    let mut word = String::new();
    std::iter::from_fn(move || {
        while let Some(ch) = chars.next() {
            let separator = !ch.is_alphanumeric();
            let boundary = case_boundary(previous, ch, chars.peek().copied());
            previous = Some(ch);
            if (separator || boundary) && !word.is_empty() {
                let result = std::mem::take(&mut word);
                if !separator {
                    word.extend(ch.to_lowercase());
                }
                return Some(result);
            }
            if !separator {
                word.extend(ch.to_lowercase());
            }
        }
        (!word.is_empty()).then(|| std::mem::take(&mut word))
    })
}

/// Prepared content terms, using the indices of Focus::idents for scoring.
pub struct ContentTerms {
    terms: Vec<(usize, Vec<String>)>,
}

impl ContentTerms {
    pub fn new(focus: &Focus) -> Self {
        Self {
            terms: focus
                .idents
                .iter()
                .enumerate()
                .filter(|(_, term)| !focus.stem_idents.contains(*term))
                .filter_map(|(index, term)| {
                    let words: Vec<_> = content_words(term).collect();
                    (!words.is_empty()).then_some((index, words))
                })
                .collect(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }

    pub fn matches(&self, text: &str) -> BTreeSet<usize> {
        let lower = text.to_ascii_lowercase();
        self.terms
            .iter()
            .filter_map(|(index, term)| {
                if !lower.contains(&term[0]) {
                    return None;
                }
                // Store only a term-sized window, never the file's whole word stream.
                let mut window = std::collections::VecDeque::with_capacity(term.len());
                for word in content_words(text) {
                    if window.len() == term.len() {
                        window.pop_front();
                    }
                    window.push_back(word);
                    if window.len() == term.len() && window.iter().eq(term) {
                        return Some(*index);
                    }
                }
                None
            })
            .collect()
    }
}

/// Empty terms match nothing; other terms require a contiguous word run.
pub fn term_matches(term: &[String], name: &[String]) -> bool {
    !term.is_empty() && name.windows(term.len()).any(|run| run == term)
}

/// Share mass in proportion to distinct matching focus terms, or uniformly if all zero.
/// Also return explicit file indices for suppression and unnormalized file scores.
pub fn personalization(
    paths: &[String],
    tags: &[FileTags],
    focus: &Focus,
    content: &[BTreeSet<usize>],
) -> (Vec<f64>, BTreeSet<usize>, Vec<f64>) {
    let terms: Vec<_> = focus
        .idents
        .iter()
        .map(|term| (term, words(term)))
        .collect();
    let mut definitions: BTreeMap<&str, Vec<Vec<String>>> = BTreeMap::new();
    for file in tags {
        for tag in &file.tags {
            if tag.kind == TagKind::Def {
                definitions
                    .entry(&file.path)
                    .or_default()
                    .push(words(&tag.name));
            }
        }
    }
    let mut content_counts = vec![0; terms.len()];
    for matches in content.iter().take(paths.len()) {
        for &index in matches {
            if let Some(count) = content_counts.get_mut(index) {
                *count += 1;
            }
        }
    }
    let mut files = BTreeSet::new();
    let mut scores = Vec::with_capacity(paths.len());
    for (index, path) in paths.iter().enumerate() {
        if focus.files.contains(path) {
            files.insert(index);
            scores.push(terms.len().max(1) as f64);
            continue;
        }
        let path_ref = Path::new(path);
        let filename = path_ref.file_name().and_then(|s| s.to_str());
        let mut names: Vec<_> = path_ref
            .parent()
            .into_iter()
            .flat_map(Path::components)
            .filter_map(|component| component.as_os_str().to_str().map(words))
            .collect();
        if let Some(stem) = path_ref.file_stem().and_then(|s| s.to_str()) {
            names.push(words(stem));
        }
        if let Some(defs) = definitions.get(path.as_str()) {
            names.extend(defs.iter().cloned());
        }
        let score = terms
            .iter()
            .enumerate()
            .map(|(term_index, (term, term_words))| {
                let route = !term_words.is_empty()
                    && (path == *term
                        || filename == Some(term.as_str())
                        || names.iter().any(|name| term_matches(term_words, name)));
                if route {
                    1.0
                } else if !focus.stem_idents.contains(*term)
                    && content
                        .get(index)
                        .is_some_and(|matches| matches.contains(&term_index))
                    && content_counts[term_index] as f64 <= COMMON_SHARE * paths.len() as f64
                {
                    CONTENT_WEIGHT
                } else {
                    0.0
                }
            })
            .sum();
        scores.push(score);
    }
    let total: f64 = scores.iter().sum();
    let vector = if total == 0.0 {
        crate::rank::uniform(paths.len())
    } else {
        scores.iter().map(|score| score / total).collect()
    };
    (vector, files, scores)
}
