//! Deterministic, in-memory Normal Data row comparison.
//!
//! This is a pure calculation core. A future native-only adapter must resolve
//! registered dataset versions, re-verify their source bytes, parse rows,
//! apply explicitly confirmed semantic mappings, and persist immutable runs.
//! This module neither opens files nor accepts untrusted filesystem paths.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

const MAX_ROWS_PER_SIDE: usize = 100_000;
const MAX_AMOUNT_COLUMNS: usize = 32;
const MAX_KEY_CHARS: usize = 240;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PeriodBasis {
    FilingPeriod,
    InvoiceMonth,
    AccountingPeriod,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompareRow {
    pub business_key: String,
    pub filing_period: Option<String>,
    pub invoice_month: Option<String>,
    pub accounting_period: Option<String>,
    /// Amounts expressed as signed integer minor units at one agreed scale.
    pub numeric_values: BTreeMap<String, i64>,
}

impl CompareRow {
    fn period(&self, basis: PeriodBasis) -> Option<&str> {
        match basis {
            PeriodBasis::FilingPeriod => self.filing_period.as_deref(),
            PeriodBasis::InvoiceMonth => self.invoice_month.as_deref(),
            PeriodBasis::AccountingPeriod => self.accounting_period.as_deref(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComparisonConfig {
    pub period_basis: PeriodBasis,
    pub amount_columns: Vec<String>,
    /// A nonnegative tolerance in the same integer minor-unit scale.
    pub tolerance_minor_units: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Classification {
    PresentBoth,
    AmountDifference,
    PeriodMoved,
    OnlyA,
    OnlyB,
    DuplicateKey,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AmountDifference {
    pub column: String,
    pub a_minor_units: i64,
    pub b_minor_units: i64,
    pub b_minus_a_minor_units: i128,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComparisonEntry {
    pub business_key: String,
    pub classification: Classification,
    pub period_a: Option<String>,
    pub period_b: Option<String>,
    pub rows_a: usize,
    pub rows_b: usize,
    pub amount_differences: Vec<AmountDifference>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComparisonSummary {
    pub total_business_keys: usize,
    pub present_both: usize,
    pub amount_differences: usize,
    pub period_moved: usize,
    pub only_a: usize,
    pub only_b: usize,
    pub duplicate_keys: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComparisonResult {
    pub period_basis: PeriodBasis,
    pub amount_columns: Vec<String>,
    pub tolerance_minor_units: i64,
    pub entries: Vec<ComparisonEntry>,
    pub summary: ComparisonSummary,
    /// SHA-256 of the canonical, sorted result and effective parameters.
    /// This is a RESULT digest, not a source-version or source-file hash.
    pub result_sha256_hex: String,
}

fn validate_period(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 7
        && bytes[4] == b'-'
        && bytes[..4].iter().all(u8::is_ascii_digit)
        && bytes[5..].iter().all(u8::is_ascii_digit)
        && &value[..4] != "0000"
        && matches!(&value[5..], "01" | "02" | "03" | "04" | "05" | "06"
            | "07" | "08" | "09" | "10" | "11" | "12")
}

fn validate_rows(
    rows: &[CompareRow],
    basis: PeriodBasis,
    amount_columns: &[String],
    side: &str,
) -> Result<(), String> {
    if rows.len() > MAX_ROWS_PER_SIDE {
        return Err(format!("{side} has too many rows for a bounded comparison"));
    }
    for (index, row) in rows.iter().enumerate() {
        let key = &row.business_key;
        if key.trim().is_empty()
            || key.trim() != key
            || key.chars().count() > MAX_KEY_CHARS
            || key.chars().any(char::is_control)
        {
            return Err(format!("{side} row {index} has an invalid business key"));
        }
        let period = row.period(basis).ok_or_else(|| {
            format!("{side} row {index} is missing the explicitly selected period basis")
        })?;
        if !validate_period(period) {
            return Err(format!("{side} row {index} has an invalid YYYY-MM period"));
        }
        for column in amount_columns {
            if !row.numeric_values.contains_key(column) {
                return Err(format!(
                    "{side} row {index} is missing configured numeric column {column}"
                ));
            }
        }
    }
    Ok(())
}

/// Classify rows deterministically by exact business key. Keys are case-sensitive;
/// no implicit normalization or fuzzy matching occurs.
///
/// A key with multiple rows on either side is DUPLICATE_KEY: the engine does
/// not guess which transaction or amendment pairs with which other row.
/// Period movement takes precedence over amount differences for one-to-one
/// keys, while any changed amount fields remain visible in that entry.
pub fn compare_rows(
    config: &ComparisonConfig,
    a: &[CompareRow],
    b: &[CompareRow],
) -> Result<ComparisonResult, String> {
    if config.tolerance_minor_units < 0 {
        return Err("comparison tolerance cannot be negative".to_string());
    }
    if config.amount_columns.len() > MAX_AMOUNT_COLUMNS {
        return Err("too many numeric comparison columns".to_string());
    }
    let mut columns = config.amount_columns.clone();
    for column in &columns {
        if column.trim().is_empty()
            || column.trim() != column
            || column.chars().count() > MAX_KEY_CHARS
            || column.chars().any(char::is_control)
        {
            return Err("numeric comparison column names must be printable and nonempty".into());
        }
    }
    columns.sort();
    if columns.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err("duplicate numeric comparison columns are not allowed".to_string());
    }
    validate_rows(a, config.period_basis, &columns, "A")?;
    validate_rows(b, config.period_basis, &columns, "B")?;

    let mut grouped: BTreeMap<&str, (Vec<&CompareRow>, Vec<&CompareRow>)> = BTreeMap::new();
    for row in a {
        grouped.entry(&row.business_key).or_default().0.push(row);
    }
    for row in b {
        grouped.entry(&row.business_key).or_default().1.push(row);
    }

    let mut entries = Vec::with_capacity(grouped.len());
    let mut summary = ComparisonSummary {
        total_business_keys: grouped.len(),
        present_both: 0,
        amount_differences: 0,
        period_moved: 0,
        only_a: 0,
        only_b: 0,
        duplicate_keys: 0,
    };
    for (key, (left, right)) in grouped {
        let lone_a = left.len() == 1;
        let lone_b = right.len() == 1;
        let period_a = if lone_a {
            left[0].period(config.period_basis).map(str::to_owned)
        } else {
            None
        };
        let period_b = if lone_b {
            right[0].period(config.period_basis).map(str::to_owned)
        } else {
            None
        };
        let mut amount_differences = Vec::new();
        if lone_a && lone_b {
            for column in &columns {
                let av = left[0].numeric_values[column];
                let bv = right[0].numeric_values[column];
                let difference = i128::from(bv) - i128::from(av);
                if difference.abs() > i128::from(config.tolerance_minor_units) {
                    amount_differences.push(AmountDifference {
                        column: column.clone(),
                        a_minor_units: av,
                        b_minor_units: bv,
                        b_minus_a_minor_units: difference,
                    });
                }
            }
        }
        let classification = if left.len() > 1 || right.len() > 1 {
            summary.duplicate_keys += 1;
            Classification::DuplicateKey
        } else if left.is_empty() {
            summary.only_b += 1;
            Classification::OnlyB
        } else if right.is_empty() {
            summary.only_a += 1;
            Classification::OnlyA
        } else if period_a != period_b {
            summary.period_moved += 1;
            Classification::PeriodMoved
        } else if !amount_differences.is_empty() {
            summary.amount_differences += 1;
            Classification::AmountDifference
        } else {
            summary.present_both += 1;
            Classification::PresentBoth
        };
        entries.push(ComparisonEntry {
            business_key: key.to_string(),
            classification,
            period_a,
            period_b,
            rows_a: left.len(),
            rows_b: right.len(),
            amount_differences,
        });
    }
    let canonical = serde_json::to_vec(&(
        config.period_basis,
        &columns,
        config.tolerance_minor_units,
        &entries,
        &summary,
    ))
    .map_err(|error| format!("cannot encode deterministic comparison result: {error}"))?;
    let mut digest = Sha256::new();
    digest.update(&canonical);
    let result_sha256_hex = format!("{:x}", digest.finalize());

    Ok(ComparisonResult {
        period_basis: config.period_basis,
        amount_columns: columns,
        tolerance_minor_units: config.tolerance_minor_units,
        entries,
        summary,
        result_sha256_hex,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(
        key: &str,
        filing: &str,
        invoice: &str,
        accounting: &str,
        amount: i64,
    ) -> CompareRow {
        CompareRow {
            business_key: key.to_string(),
            filing_period: Some(filing.to_string()),
            invoice_month: Some(invoice.to_string()),
            accounting_period: Some(accounting.to_string()),
            numeric_values: BTreeMap::from([("taxable_value".to_string(), amount)]),
        }
    }

    fn config(basis: PeriodBasis, tolerance: i64) -> ComparisonConfig {
        ComparisonConfig {
            period_basis: basis,
            amount_columns: vec!["taxable_value".to_string()],
            tolerance_minor_units: tolerance,
        }
    }

    #[test]
    fn filing_period_not_invoice_month_controls_the_bucket() {
        let source = row("INV-100", "2026-08", "2026-07", "2026-07", 10_000);
        let result = compare_rows(
            &config(PeriodBasis::FilingPeriod, 0),
            &[source.clone()],
            &[source],
        )
        .expect("same filing period should match");
        assert_eq!(result.summary.present_both, 1);
        assert_eq!(result.entries[0].period_a.as_deref(), Some("2026-08"));
        assert_eq!(result.entries[0].classification, Classification::PresentBoth);
    }

    #[test]
    fn period_movement_is_not_misclassified_as_missing() {
        let a = row("INV-100", "2026-08", "2026-07", "2026-08", 10_000);
        let b = row("INV-100", "2026-09", "2026-07", "2026-08", 10_250);
        let result = compare_rows(&config(PeriodBasis::FilingPeriod, 0), &[a], &[b])
            .expect("comparison");
        assert_eq!(result.summary.period_moved, 1);
        assert_eq!(result.summary.only_a, 0);
        assert_eq!(result.summary.only_b, 0);
        assert_eq!(result.entries[0].period_a.as_deref(), Some("2026-08"));
        assert_eq!(result.entries[0].period_b.as_deref(), Some("2026-09"));
        assert_eq!(result.entries[0].amount_differences.len(), 1);
        assert_eq!(result.entries[0].amount_differences[0].b_minus_a_minor_units, 250);
    }

    #[test]
    fn tolerance_is_applied_without_floating_point_arithmetic() {
        let a = row("INV-1", "2026-08", "2026-07", "2026-08", i64::MAX);
        let b = row("INV-1", "2026-08", "2026-07", "2026-08", i64::MIN);
        let result = compare_rows(&config(PeriodBasis::FilingPeriod, 9), &[a], &[b])
            .expect("compare extreme signed values");
        assert_eq!(result.summary.amount_differences, 1);
        assert_eq!(
            result.entries[0].amount_differences[0].b_minus_a_minor_units,
            i128::from(i64::MIN) - i128::from(i64::MAX)
        );
        let a = row("INV-1", "2026-08", "2026-07", "2026-08", 10_000);
        let b = row("INV-1", "2026-08", "2026-07", "2026-08", 10_001);
        assert_eq!(
            compare_rows(&config(PeriodBasis::FilingPeriod, 1), &[a], &[b])
                .expect("comparison").summary.present_both,
            1
        );
    }

    #[test]
    fn unmatched_and_duplicate_keys_are_explicit() {
        let a = vec![
            row("only-A", "2026-08", "2026-08", "2026-08", 100),
            row("duplicate", "2026-08", "2026-08", "2026-08", 100),
            row("duplicate", "2026-09", "2026-09", "2026-09", 100),
        ];
        let b = vec![
            row("only-B", "2026-08", "2026-08", "2026-08", 100),
            row("duplicate", "2026-08", "2026-08", "2026-08", 100),
        ];
        let result = compare_rows(&config(PeriodBasis::FilingPeriod, 0), &a, &b)
            .expect("comparison");
        assert_eq!(result.summary.total_business_keys, 3);
        assert_eq!(result.summary.duplicate_keys, 1);
        assert_eq!(result.summary.only_a, 1);
        assert_eq!(result.summary.only_b, 1);
        assert_eq!(result.entries[0].business_key, "duplicate");
        assert_eq!(result.entries[0].classification, Classification::DuplicateKey);
        assert_eq!(result.entries[0].rows_a, 2);
    }

    #[test]
    fn input_row_order_does_not_change_classification_or_result_digest() {
        let first = row("Z", "2026-09", "2026-08", "2026-09", 100);
        let second = row("A", "2026-08", "2026-07", "2026-08", 200);
        let left = vec![first.clone(), second.clone()];
        let reversed = vec![second, first];
        let config = config(PeriodBasis::FilingPeriod, 0);
        let one = compare_rows(&config, &left, &[]).expect("first order");
        let two = compare_rows(&config, &reversed, &[]).expect("reverse order");
        assert_eq!(one, two);
        assert_eq!(one.result_sha256_hex.len(), 64);
    }

    #[test]
    fn selected_period_role_must_be_present_and_valid() {
        let mut value = row("K", "2026-08", "2026-07", "2026-08", 1);
        value.filing_period = None;
        assert!(compare_rows(&config(PeriodBasis::FilingPeriod, 0), &[value], &[]).is_err());
        let bad = row("K", "2026-13", "2026-07", "2026-08", 1);
        assert!(compare_rows(&config(PeriodBasis::FilingPeriod, 0), &[bad], &[]).is_err());
        let good = row("K", "2026-08", "2026-07", "2026-08", 1);
        assert!(compare_rows(&config(PeriodBasis::FilingPeriod, -1), &[good], &[]).is_err());
    }

    #[test]
    fn chosen_period_basis_is_explicit_and_changes_results() {
        let a = row("K", "2026-08", "2026-07", "2026-08", 10);
        let b = row("K", "2026-09", "2026-07", "2026-08", 10);
        let filing = compare_rows(&config(PeriodBasis::FilingPeriod, 0), &[a.clone()], &[b.clone()])
            .expect("filing comparison");
        let invoice = compare_rows(&config(PeriodBasis::InvoiceMonth, 0), &[a], &[b])
            .expect("invoice month comparison");
        assert_eq!(filing.summary.period_moved, 1);
        assert_eq!(invoice.summary.present_both, 1);
        assert_ne!(filing.result_sha256_hex, invoice.result_sha256_hex);
    }

    #[test]
    fn every_configured_amount_column_must_be_present() {
        let mut cfg = config(PeriodBasis::FilingPeriod, 0);
        cfg.amount_columns = vec!["taxable_value".into(), "tax_component".into()];
        let source = row("K", "2026-08", "2026-08", "2026-08", 10);
        assert!(compare_rows(&cfg, &[source], &[]).is_err());
        cfg.amount_columns = vec!["taxable_value".into(), "taxable_value".into()];
        assert!(compare_rows(&cfg, &[], &[]).is_err());
    }
}
