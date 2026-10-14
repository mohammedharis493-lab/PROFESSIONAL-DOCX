use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone)]
pub struct ExactReconciliationItem {
    pub stable_id: String,
    pub match_key: String,
    pub amount_minor: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactReconciliationMatch {
    pub left_stable_id: String,
    pub right_stable_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactReconciliationOutcome {
    pub matches: Vec<ExactReconciliationMatch>,
    pub unmatched_left_stable_ids: Vec<String>,
    pub unmatched_right_stable_ids: Vec<String>,
}

pub fn reconcile_exact_key_amount(
    left_items: &[ExactReconciliationItem],
    right_items: &[ExactReconciliationItem],
) -> Result<ExactReconciliationOutcome, String> {
    validate_side(left_items, "LEFT")?;
    validate_side(right_items, "RIGHT")?;

    let left_buckets = bucket_items(left_items);
    let right_buckets = bucket_items(right_items);

    let mut keys = BTreeSet::new();
    keys.extend(left_buckets.keys().cloned());
    keys.extend(right_buckets.keys().cloned());

    let mut matches = Vec::new();
    let mut unmatched_left_stable_ids = Vec::new();
    let mut unmatched_right_stable_ids = Vec::new();

    for key in keys {
        let left = left_buckets.get(&key).cloned().unwrap_or_default();
        let right = right_buckets.get(&key).cloned().unwrap_or_default();
        let pair_count = left.len().min(right.len());

        for index in 0..pair_count {
            matches.push(ExactReconciliationMatch {
                left_stable_id: left[index].clone(),
                right_stable_id: right[index].clone(),
            });
        }

        unmatched_left_stable_ids.extend(left.into_iter().skip(pair_count));
        unmatched_right_stable_ids.extend(right.into_iter().skip(pair_count));
    }

    Ok(ExactReconciliationOutcome {
        matches,
        unmatched_left_stable_ids,
        unmatched_right_stable_ids,
    })
}

fn validate_side(items: &[ExactReconciliationItem], side: &str) -> Result<(), String> {
    let mut stable_ids = BTreeSet::new();

    for item in items {
        let stable_id = item.stable_id.trim();
        if stable_id.is_empty() {
            return Err(format!("{side} reconciliation item stable ID is required."));
        }
        if !stable_ids.insert(stable_id.to_string()) {
            return Err(format!(
                "{side} reconciliation item stable ID '{stable_id}' is duplicated."
            ));
        }

        if item.match_key.trim().is_empty() {
            return Err(format!(
                "{side} reconciliation item {stable_id} has an empty match key."
            ));
        }
    }

    Ok(())
}

fn bucket_items(items: &[ExactReconciliationItem]) -> BTreeMap<(String, i64), Vec<String>> {
    let mut buckets: BTreeMap<(String, i64), Vec<String>> = BTreeMap::new();

    for item in items {
        buckets
            .entry((item.match_key.trim().to_string(), item.amount_minor))
            .or_default()
            .push(item.stable_id.trim().to_string());
    }

    for stable_ids in buckets.values_mut() {
        stable_ids.sort();
    }

    buckets
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, key: &str, amount_minor: i64) -> ExactReconciliationItem {
        ExactReconciliationItem {
            stable_id: id.to_string(),
            match_key: key.to_string(),
            amount_minor,
        }
    }

    #[test]
    fn exact_match_is_deterministic_across_input_order() {
        let left = vec![
            item("L-2", "INV-100", 10_000),
            item("L-1", "INV-100", 10_000),
            item("L-3", "INV-200", 20_000),
        ];
        let right = vec![
            item("R-2", "INV-100", 10_000),
            item("R-1", "INV-100", 10_000),
            item("R-3", "INV-300", 30_000),
        ];

        let first = reconcile_exact_key_amount(&left, &right).expect("reconcile");

        let mut reversed_left = left.clone();
        reversed_left.reverse();
        let mut reversed_right = right.clone();
        reversed_right.reverse();
        let second =
            reconcile_exact_key_amount(&reversed_left, &reversed_right).expect("reconcile");

        assert_eq!(first, second);
        assert_eq!(
            first.matches,
            vec![
                ExactReconciliationMatch {
                    left_stable_id: "L-1".to_string(),
                    right_stable_id: "R-1".to_string(),
                },
                ExactReconciliationMatch {
                    left_stable_id: "L-2".to_string(),
                    right_stable_id: "R-2".to_string(),
                },
            ]
        );
        assert_eq!(first.unmatched_left_stable_ids, vec!["L-3"]);
        assert_eq!(first.unmatched_right_stable_ids, vec!["R-3"]);
    }

    #[test]
    fn same_key_with_different_amount_does_not_match() {
        let outcome = reconcile_exact_key_amount(
            &[item("L-1", "INV-100", 10_000)],
            &[item("R-1", "INV-100", 10_001)],
        )
        .expect("reconcile");

        assert!(outcome.matches.is_empty());
        assert_eq!(outcome.unmatched_left_stable_ids, vec!["L-1"]);
        assert_eq!(outcome.unmatched_right_stable_ids, vec!["R-1"]);
    }

    #[test]
    fn duplicate_stable_ids_are_rejected() {
        let error = reconcile_exact_key_amount(
            &[
                item("L-1", "INV-100", 10_000),
                item("L-1", "INV-200", 20_000),
            ],
            &[],
        )
        .expect_err("duplicate IDs should fail");

        assert!(error.contains("duplicated"));
    }
}
