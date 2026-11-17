//! Pure set-diff of model ids between cache and a fresh fetch.

use std::collections::{HashMap, HashSet};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Diff {
    /// Model id never seen on this gateway before.
    pub added: Vec<String>,
    /// Was present last time, missing now.
    pub removed: Vec<String>,
    /// Was removed, now back.
    pub returned: Vec<String>,
}

/// `existing` maps model_id -> removed_at (None = currently present).
pub fn compute(existing: &HashMap<String, Option<i64>>, fetched: &HashSet<String>) -> Diff {
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut returned = Vec::new();

    for id in fetched {
        if !existing.contains_key(id) {
            added.push(id.clone());
        }
    }
    for (id, removed_at) in existing {
        match removed_at {
            None => {
                if !fetched.contains(id) {
                    removed.push(id.clone());
                }
            }
            Some(_) => {
                if fetched.contains(id) {
                    returned.push(id.clone());
                }
            }
        }
    }
    added.sort();
    removed.sort();
    returned.sort();
    Diff { added, removed, returned }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pairs: &[(&str, Option<i64>)]) -> HashMap<String, Option<i64>> {
        pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    fn set(items: &[&str]) -> HashSet<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn adds_and_removes() {
        let existing = map(&[("a", None), ("b", None), ("gone", None)]);
        let fetched = set(&["a", "b", "new1", "new2"]);
        let d = compute(&existing, &fetched);
        assert_eq!(d.added, vec!["new1", "new2"]);
        assert_eq!(d.removed, vec!["gone"]);
        assert!(d.returned.is_empty());
    }

    #[test]
    fn returned_models() {
        let existing = map(&[("a", None), ("b", Some(123))]);
        let fetched = set(&["a", "b"]);
        let d = compute(&existing, &fetched);
        assert!(d.added.is_empty());
        assert!(d.removed.is_empty());
        assert_eq!(d.returned, vec!["b"]);
    }

    #[test]
    fn unchanged_is_empty() {
        let existing = map(&[("a", None)]);
        let d = compute(&existing, &set(&["a"]));
        assert!(d.added.is_empty() && d.removed.is_empty() && d.returned.is_empty());
    }

    #[test]
    fn removed_then_still_removed_is_silent() {
        let existing = map(&[("a", None), ("b", Some(5))]);
        let d = compute(&existing, &set(&["a"]));
        assert!(d.removed.is_empty());
    }
}
