//! Command search: fuzzy matching of action names.

use super::registry::Registry;

/// How well `query` matches `name`: None if it does not, higher is better.
/// Every query letter must appear in order. Matches at word starts and runs
/// of consecutive letters score more, and shorter names win ties.
pub fn score(query: &str, name: &str) -> Option<i32> {
    let q: Vec<char> = query.to_lowercase().chars().filter(|c| !c.is_whitespace()).collect();
    if q.is_empty() {
        return Some(0);
    }
    let n: Vec<char> = name.to_lowercase().chars().collect();
    let mut score = 0;
    let mut qi = 0;
    let mut prev_match = false;
    for (i, &c) in n.iter().enumerate() {
        if qi < q.len() && c == q[qi] {
            let word_start = i == 0 || !n[i - 1].is_alphanumeric();
            score += 1 + if word_start { 8 } else { 0 } + if prev_match { 4 } else { 0 };
            qi += 1;
            prev_match = true;
        } else {
            prev_match = false;
        }
    }
    (qi == q.len()).then(|| score * 10 - n.len() as i32)
}

/// Action ids matching `query`, best first.
pub fn find(reg: &Registry, query: &str) -> Vec<&'static str> {
    let mut hits: Vec<(i32, usize, &'static str)> = reg
        .all()
        .iter()
        .enumerate()
        .filter(|(_, a)| a.id != "file.recent")
        .filter_map(|(i, a)| score(query, a.name).map(|s| (s, i, a.id)))
        .collect();
    hits.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    hits.into_iter().map(|(_, _, id)| id).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_must_appear_in_order() {
        assert!(score("pp", "Push/Pull").is_some());
        assert!(score("zom", "Zoom Extents").is_some());
        assert!(score("xz", "Zoom Extents").is_none());
    }

    #[test]
    fn word_starts_and_exact_names_rank_first() {
        let reg = Registry::new();
        assert_eq!(find(&reg, "line").first(), Some(&"tool.line"));
        assert_eq!(find(&reg, "pp").first(), Some(&"tool.push"));
        assert_eq!(find(&reg, "zoom ext").first(), Some(&"cam.extents"));
        assert!(find(&reg, "").len() > 100, "empty query lists everything");
    }
}
