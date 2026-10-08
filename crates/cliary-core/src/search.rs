//! Weighted lexical retrieval over Catalog facts. No inferred tool capabilities.
use crate::{LocalizedName, Tool};
use std::collections::{BTreeMap, BTreeSet};

type Terms = BTreeMap<String, f64>;
fn han(c: char) -> bool {
    matches!(c as u32, 0x3400..=0x4dbf | 0x4e00..=0x9fff | 0x20000..=0x323af)
}
fn stop(word: &str) -> bool {
    matches!(
        word,
        "a" | "an"
            | "the"
            | "to"
            | "for"
            | "of"
            | "in"
            | "on"
            | "by"
            | "with"
            | "and"
            | "or"
            | "is"
            | "it"
            | "i"
            | "my"
            | "me"
            | "please"
            | "want"
            | "would"
            | "like"
            | "which"
            | "how"
            | "can"
            | "could"
            | "tool"
            | "tools"
            | "command"
            | "line"
            | "工具"
            | "命令"
            | "程序"
            | "一个"
            | "帮我"
            | "看看"
            | "一下"
            | "想要"
            | "可以"
            | "使用"
            | "处理"
            | "查看"
    )
}
fn word(value: &str) -> String {
    // Small, conservative English plural normalization; exact identities use raw text.
    if value.len() > 4 && value.ends_with("ies") {
        format!("{}y", &value[..value.len() - 3])
    } else if value.len() > 3
        && value.ends_with('s')
        && !value.ends_with("ss")
        && !value.ends_with("us")
    {
        value[..value.len() - 1].into()
    } else {
        value.into()
    }
}
fn latin(value: &str) -> Vec<String> {
    value
        .split(|c: char| !c.is_alphanumeric() || han(c))
        .filter(|s| !s.is_empty() && !stop(s))
        .map(word)
        .collect()
}
fn put(terms: &mut Terms, term: String, weight: f64) {
    if !term.is_empty() && !stop(&term) {
        terms
            .entry(term)
            .and_modify(|v| *v = v.max(weight))
            .or_insert(weight);
    }
}
fn field(terms: &mut Terms, value: &str, weight: f64, curated: bool) {
    let lower = value.to_lowercase();
    let words = latin(&lower);
    for w in &words {
        put(terms, w.clone(), weight);
    }
    if curated && words.len() > 1 {
        put(terms, words.join(" "), weight * 1.5);
    }
    for run in lower.split(|c: char| !han(c)).filter(|s| !s.is_empty()) {
        if curated && run.chars().count() > 1 {
            put(terms, run.into(), weight * 1.5);
        }
        let chars: Vec<_> = run.chars().collect();
        for pair in chars.windows(2) {
            put(
                terms,
                pair.iter().collect(),
                weight * if curated { 0.5 } else { 1.0 },
            );
        }
    }
}
fn document(tool: &Tool, categories: &[LocalizedName], tags: &[LocalizedName]) -> Terms {
    let mut terms = Terms::new();
    for name in std::iter::once(&tool.id)
        .chain(std::iter::once(&tool.name))
        .chain(&tool.aliases)
        .chain(&tool.executables)
    {
        field(&mut terms, name, 10.0, true);
    }
    for value in tool.keywords.values().flatten() {
        field(&mut terms, value, 6.0, true);
    }
    for value in tool.description.values().chain(tool.summary.values()) {
        field(&mut terms, value, 2.0, false);
    }
    for (ids, names, weight) in [(&tool.tags, tags, 4.0), (&tool.categories, categories, 0.6)] {
        for id in ids {
            field(&mut terms, id, weight, false);
            if let Some(name) = names.iter().find(|name| name.id == *id) {
                for value in name.name.values() {
                    field(&mut terms, value, weight, true);
                }
                for value in name.keywords.values().flatten() {
                    field(&mut terms, value, weight, true);
                }
            }
        }
    }
    terms
}
/// Field weights and inverse document frequency reduce broad-category noise.
/// Each term counts once; repeated synonyms do not accumulate term frequency.
pub(crate) fn rank(
    query: &str,
    tools: Vec<Tool>,
    categories: &[LocalizedName],
    tags: &[LocalizedName],
) -> Vec<Tool> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }
    let docs: Vec<_> = tools
        .iter()
        .map(|t| document(t, categories, tags))
        .collect();
    let mut frequencies = BTreeMap::<String, usize>::new();
    for doc in &docs {
        for term in doc.keys() {
            *frequencies.entry(term.clone()).or_default() += 1;
        }
    }
    let normalized = latin(&q).join(" ");
    let words: BTreeSet<_> = latin(&q).into_iter().take(32).collect();
    let mut matched: Vec<_> = frequencies
        .keys()
        .filter(|term| {
            if term.chars().any(han) {
                q.contains(term.as_str())
            } else if term.contains(' ') {
                format!(" {normalized} ").contains(&format!(" {term} "))
            } else {
                words.contains(*term)
            }
        })
        .cloned()
        .collect();
    // Prefer the longest meaningful phrases over their overlapping fragments.
    let longest = matched.clone();
    matched.retain(|term| {
        !longest.iter().any(|other| {
            other != term
                && if term.chars().any(han) {
                    other.contains(term.as_str())
                } else {
                    format!(" {other} ").contains(&format!(" {term} "))
                }
        })
    });
    let mut scored = Vec::new();
    let n = tools.len() as f64;
    for (tool, doc) in tools.into_iter().zip(docs) {
        let names: Vec<_> = std::iter::once(&tool.id)
            .chain(std::iter::once(&tool.name))
            .chain(&tool.aliases)
            .chain(&tool.executables)
            .map(|s| s.to_lowercase())
            .collect();
        let exact = names.contains(&q);
        let prefix = !q.contains(char::is_whitespace)
            && q.chars().count() >= 2
            && names.iter().any(|name| name.starts_with(&q));
        let score: f64 = matched
            .iter()
            .filter_map(|term| {
                doc.get(term).map(|weight| {
                    weight * (1.0 + ((n + 1.0) / (frequencies[term] as f64 + 1.0)).ln())
                })
            })
            .sum();
        if exact || prefix || score > 0.0 {
            scored.push((
                tool,
                if exact {
                    2
                } else if prefix {
                    1
                } else {
                    0
                },
                score,
            ));
        }
    }
    scored.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then(b.2.total_cmp(&a.2))
            .then(a.0.id.cmp(&b.0.id))
    });
    scored.into_iter().map(|(tool, _, _)| tool).collect()
}
