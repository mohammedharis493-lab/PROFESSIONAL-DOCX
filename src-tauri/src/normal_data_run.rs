//! Native comparison execution: exact approved-root source bytes -> strict CSV
//! normalization -> deterministic calculation -> atomic append-only run.
//! Frontend callers supply only an immutable recipe-version UUID.
use crate::{
    normal_data_comparison::{self, ComparisonConfig, ComparisonResult, PeriodBasis},
    normal_data_csv::{self, CsvLayout},
    normal_data_source_reader,
    persistence::{self, PersistenceError},
};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::Serialize;
use serde_json::json;
use std::{fmt::Write, path::Path};
use uuid::Uuid;

const MAX_RESULT_JSON_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComparisonRunRecord {
    pub normal_data_comparison_run_id: String,
    pub normal_data_comparison_recipe_version_id: String,
    pub dataset_a_source_sha256_hex: String,
    pub dataset_b_source_sha256_hex: String,
    pub result: ComparisonResult,
    pub started_at_ms: i64,
    pub completed_at_ms: i64,
}

struct RecipeSnapshot {
    dataset_a_version_id: String,
    dataset_b_version_id: String,
    period_basis: String,
    period_column_a: String,
    period_column_b: String,
    amount_columns_json: String,
    tolerance_minor_units: i64,
}

fn invalid(message: impl Into<String>) -> PersistenceError {
    PersistenceError::Configuration(message.into())
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut out, "{byte:02x}").expect("hex encoding into String cannot fail");
    }
    out
}

fn decode_hex_32(value: &str) -> Result<[u8; 32], PersistenceError> {
    if value.len() != 64 {
        return Err(invalid("comparison output digest has invalid length"));
    }
    let mut result = [0u8; 32];
    for (i, byte) in result.iter_mut().enumerate() {
        let hexpair = value.get(2 * i..2 * i + 2)
            .ok_or_else(|| invalid("invalid result digest encoding"))?;
        *byte = u8::from_str_radix(hexpair, 16)
            .map_err(|_| invalid("result digest is not hexadecimal"))?;
    }
    Ok(result)
}

fn get_recipe(
    database_path: &Path,
    recipe_version_id: &str,
) -> Result<RecipeSnapshot, PersistenceError> {
    let connection = persistence::open_configured_connection(database_path)?;
    connection.query_row(
        "SELECT dataset_a_version_id, dataset_b_version_id,
                period_basis, period_column_a, period_column_b,
                amount_columns_json, tolerance_minor_units
         FROM normal_data_comparison_recipe_versions
         WHERE normal_data_comparison_recipe_version_id = ?1",
        [recipe_version_id],
        |row| Ok(RecipeSnapshot {
            dataset_a_version_id: row.get(0)?,
            dataset_b_version_id: row.get(1)?,
            period_basis: row.get(2)?,
            period_column_a: row.get(3)?,
            period_column_b: row.get(4)?,
            amount_columns_json: row.get(5)?,
            tolerance_minor_units: row.get(6)?,
        }),
    ).optional()?.ok_or_else(|| invalid("comparison recipe version does not exist"))
}

fn confirmed_business_key(
    database_path: &Path,
    dataset_version_id: &str,
    period_column: &str,
    period_role: &str,
    numeric_columns: &[String],
) -> Result<String, PersistenceError> {
    let connection = persistence::open_configured_connection(database_path)?;
    let mut stmt = connection.prepare(
        "SELECT column_name FROM normal_data_column_semantics
         WHERE normal_data_dataset_version_id = ?1
           AND semantic_role = 'BUSINESS_KEY' AND data_type = 'TEXT'
         ORDER BY normal_data_column_semantic_id",
    )?;
    let found: Vec<String> = stmt.query_map([dataset_version_id], |row| row.get(0))?
        .collect::<Result<Vec<_>, _>>()?;
    if found.len() != 1 {
        return Err(invalid(
            "each compared dataset version needs exactly one declared TEXT business key",
        ));
    }
    let period_confirmed: bool = connection.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM normal_data_column_semantics
            WHERE normal_data_dataset_version_id = ?1
              AND column_name = ?2 COLLATE NOCASE
              AND semantic_role = ?3
              AND data_type = ?4
        )",
        params![dataset_version_id, period_column, period_role,
            if period_role == "INVOICE_DATE" { "DATE" } else { "PERIOD" }],
        |row| row.get(0),
    )?;
    if !period_confirmed {
        return Err(invalid("frozen selected period column has no matching declaration"));
    }
    for column in numeric_columns {
        let confirmed: bool = connection.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM normal_data_column_semantics
                WHERE normal_data_dataset_version_id = ?1
                  AND column_name = ?2 COLLATE NOCASE
                  AND semantic_role = 'NUMERIC_VALUE'
                  AND data_type = 'DECIMAL'
            )",
            params![dataset_version_id, column],
            |row| row.get(0),
        )?;
        if !confirmed {
            return Err(invalid(format!(
                "declared numeric column '{column}' is not available on both sources"
            )));
        }
    }
    Ok(found[0].clone())
}

fn rows_for_version(
    database_path: &Path,
    dataset_version_id: &str,
    period_column: &str,
    basis: PeriodBasis,
    period_role: &str,
    numeric_columns: &[String],
) -> Result<(Vec<normal_data_comparison::CompareRow>, [u8; 32]), PersistenceError> {
    let key = confirmed_business_key(
        database_path, dataset_version_id, period_column, period_role, numeric_columns,
    )?;
    let verified = normal_data_source_reader::read_verified_dataset(
        database_path, dataset_version_id,
    )?;
    let rows = normal_data_csv::parse_normalized_csv(
        &verified.bytes,
        &CsvLayout {
            key_column: &key,
            period_column,
            period_basis: basis,
            amount_columns: numeric_columns,
        },
    ).map_err(|error| invalid(format!(
        "verified dataset '{}' has invalid normalized CSV: {error}",
        dataset_version_id,
    )))?;
    Ok((rows, verified.sha256))
}

pub fn execute_comparison(
    database_path: &Path,
    recipe_version_id: &str,
) -> Result<ComparisonRunRecord, PersistenceError> {
    Uuid::parse_str(recipe_version_id)
        .map_err(|_| invalid("comparison recipe version is not a UUID"))?;
    let started_at_ms = persistence::now_unix_ms()?;
    let recipe = get_recipe(database_path, recipe_version_id)?;
    let (basis, period_role) = match recipe.period_basis.as_str() {
        "FILING_PERIOD" => (PeriodBasis::FilingPeriod, "FILING_PERIOD"),
        "INVOICE_MONTH" => (PeriodBasis::InvoiceMonth, "INVOICE_DATE"),
        "ACCOUNTING_PERIOD" => (PeriodBasis::AccountingPeriod, "ACCOUNTING_PERIOD"),
        _ => return Err(invalid("unsupported frozen comparison period basis")),
    };
    let amount_columns: Vec<String> = serde_json::from_str(&recipe.amount_columns_json)
        .map_err(|error| invalid(format!("invalid frozen amount-column recipe: {error}")))?;
    if amount_columns.is_empty() || amount_columns.len() > 32 {
        return Err(invalid("comparison needs 1 to 32 declared numeric columns"));
    }
    let (left, hash_a) = rows_for_version(
        database_path, &recipe.dataset_a_version_id, &recipe.period_column_a,
        basis, period_role, &amount_columns,
    )?;
    let (right, hash_b) = rows_for_version(
        database_path, &recipe.dataset_b_version_id, &recipe.period_column_b,
        basis, period_role, &amount_columns,
    )?;
    let result = normal_data_comparison::compare_rows(
        &ComparisonConfig {
            period_basis: basis,
            amount_columns,
            tolerance_minor_units: recipe.tolerance_minor_units,
        },
        &left, &right,
    ).map_err(invalid)?;
    let result_json = serde_json::to_string(&result)
        .map_err(|error| invalid(format!("comparison result cannot be encoded: {error}")))?;
    if result_json.len() > MAX_RESULT_JSON_BYTES {
        return Err(invalid("comparison result exceeds the immutable 8 MiB run-record limit"));
    }
    let digest = decode_hex_32(&result.result_sha256_hex)?;
    let completed_at_ms = persistence::now_unix_ms()?;
    let run_id = Uuid::new_v4().to_string();
    let mut connection = persistence::open_configured_connection(database_path)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    transaction.execute(
        "INSERT INTO normal_data_comparison_runs (
            normal_data_comparison_run_id, normal_data_comparison_recipe_version_id,
            dataset_a_source_sha256, dataset_b_source_sha256, result_sha256,
            result_json, started_at_ms, completed_at_ms
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            &run_id, recipe_version_id, &hash_a[..], &hash_b[..],
            &digest[..], &result_json, started_at_ms, completed_at_ms,
        ],
    )?;
    transaction.execute(
        "INSERT INTO audit_events (
            audit_event_id, event_type, entity_type, entity_id,
            related_entity_type, related_entity_id, occurred_at_ms,
            actor_id, details_json
         ) VALUES (?1, 'NORMAL_DATA_COMPARISON_RUN_COMPLETED', 'NORMAL_DATA_COMPARISON_RUN',
                   ?2, 'NORMAL_DATA_COMPARISON_RECIPE_VERSION', ?3, ?4, NULL, ?5)",
        params![
            Uuid::new_v4().to_string(), &run_id, recipe_version_id, completed_at_ms,
            json!({"source_a_sha256":hex(&hash_a),"source_b_sha256":hex(&hash_b),
                "result_sha256":result.result_sha256_hex,
                "row_count_a":left.len(),"row_count_b":right.len()}).to_string(),
        ],
    )?;
    transaction.commit()?;
    Ok(ComparisonRunRecord {
        normal_data_comparison_run_id: run_id,
        normal_data_comparison_recipe_version_id: recipe_version_id.to_string(),
        dataset_a_source_sha256_hex: hex(&hash_a),
        dataset_b_source_sha256_hex: hex(&hash_b),
        result,
        started_at_ms,
        completed_at_ms,
    })
}

pub fn list_comparison_runs(
    database_path: &Path,
    recipe_version_id: &str,
) -> Result<Vec<ComparisonRunRecord>, PersistenceError> {
    Uuid::parse_str(recipe_version_id)
        .map_err(|_| invalid("comparison recipe version is not a UUID"))?;
    let connection = persistence::open_configured_connection(database_path)?;
    let exists: bool = connection.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM normal_data_comparison_recipe_versions
            WHERE normal_data_comparison_recipe_version_id = ?1
         )", [recipe_version_id], |row| row.get(0),
    )?;
    if !exists {
        return Err(invalid("comparison recipe version does not exist"));
    }
    let mut stmt = connection.prepare(
        "SELECT normal_data_comparison_run_id, dataset_a_source_sha256,
                dataset_b_source_sha256, result_sha256, result_json,
                started_at_ms, completed_at_ms
         FROM normal_data_comparison_runs
         WHERE normal_data_comparison_recipe_version_id = ?1
         ORDER BY completed_at_ms DESC, normal_data_comparison_run_id",
    )?;
    let rows = stmt.query_map([recipe_version_id], |row| Ok((
        row.get::<_, String>(0)?,
        row.get::<_, Vec<u8>>(1)?,
        row.get::<_, Vec<u8>>(2)?,
        row.get::<_, Vec<u8>>(3)?,
        row.get::<_, String>(4)?,
        row.get::<_, i64>(5)?,
        row.get::<_, i64>(6)?,
    )))?;
    let mut runs = Vec::new();
    for row in rows {
        let (run_id, hash_a, hash_b, stored_digest, result_json, started_at_ms, completed_at_ms) = row?;
        if hash_a.len() != 32 || hash_b.len() != 32 || stored_digest.len() != 32 {
            return Err(invalid("stored immutable comparison run has invalid digest lengths"));
        }
        let result: ComparisonResult = serde_json::from_str(&result_json)
            .map_err(|error| invalid(format!("invalid immutable comparison run JSON: {error}")))?;
        if decode_hex_32(&result.result_sha256_hex)?.as_slice() != stored_digest.as_slice() {
            return Err(invalid("stored result digest does not match comparison metadata"));
        }
        runs.push(ComparisonRunRecord {
            normal_data_comparison_run_id: run_id,
            normal_data_comparison_recipe_version_id: recipe_version_id.to_string(),
            dataset_a_source_sha256_hex: hex(&hash_a),
            dataset_b_source_sha256_hex: hex(&hash_b),
            result,
            started_at_ms,
            completed_at_ms,
        });
    }
    Ok(runs)
}
