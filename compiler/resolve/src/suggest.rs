//! "Did you mean" suggestions.

/// Levenshtein distance, small inputs only.
pub fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut cur = vec![i + 1];
        for (j, &cb) in b.iter().enumerate() {
            let sub = prev[j] + usize::from(ca != cb);
            cur.push(sub.min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[b.len()]
}

/// Closest candidate within a distance proportional to the name length.
pub fn best<'a>(name: &str, candidates: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    let max = match name.chars().count() {
        0..=2 => 0,
        3..=5 => 1,
        _ => 2,
    };
    candidates
        .filter(|c| *c != name)
        .map(|c| (distance(name, c), c))
        .filter(|(d, _)| *d <= max)
        .min()
        .map(|(_, c)| c)
}

#[cfg(test)]
mod tests {
    #[test]
    fn distances() {
        assert_eq!(super::distance("kitten", "sitting"), 3);
        assert_eq!(super::best("prnt", ["print", "panic"].into_iter()), Some("print"));
        assert_eq!(super::best("ab", ["ac"].into_iter()), None);
    }
}
