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
        let hexpair = value
            .get(2 * i..2 * i + 2)
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
    connection
        .query_row(
            "SELECT dataset_a_version_id, dataset_b_version_id,
                period_basis, period_column_a, period_column_b,
                amount_columns_json, tolerance_minor_units
         FROM normal_data_comparison_recipe_versions
         WHERE normal_data_comparison_recipe_version_id = ?1",
            [recipe_version_id],
            |row| {
                Ok(RecipeSnapshot {
                    dataset_a_version_id: row.get(0)?,
                    dataset_b_version_id: row.get(1)?,
                    period_basis: row.get(2)?,
                    period_column_a: row.get(3)?,
                    period_column_b: row.get(4)?,
                    amount_columns_json: row.get(5)?,
                    tolerance_minor_units: row.get(6)?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| invalid("comparison recipe version does not exist"))
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
    let found: Vec<String> = stmt
        .query_map([dataset_version_id], |row| row.get(0))?
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
        params![
            dataset_version_id,
            period_column,
            period_role,
            if period_role == "INVOICE_DATE" {
                "DATE"
            } else {
                "PERIOD"
            }
        ],
        |row| row.get(0),
    )?;
    if !period_confirmed {
        return Err(invalid(
            "frozen selected period column has no matching declaration",
        ));
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
        database_path,
        dataset_version_id,
        period_column,
        period_role,
        numeric_columns,
    )?;
    let verified =
        normal_data_source_reader::read_verified_dataset(database_path, dataset_version_id)?;
    let rows = normal_data_csv::parse_normalized_csv(
        &verified.bytes,
        &CsvLayout {
            key_column: &key,
            period_column,
            period_basis: basis,
            amount_columns: numeric_columns,
        },
    )
    .map_err(|error| {
        invalid(format!(
            "verified dataset '{}' has invalid normalized CSV: {error}",
            dataset_version_id,
        ))
    })?;
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
        database_path,
        &recipe.dataset_a_version_id,
        &recipe.period_column_a,
        basis,
        period_role,
        &amount_columns,
    )?;
    let (right, hash_b) = rows_for_version(
        database_path,
        &recipe.dataset_b_version_id,
        &recipe.period_column_b,
        basis,
        period_role,
        &amount_columns,
    )?;
    let result = normal_data_comparison::compare_rows(
        &ComparisonConfig {
            period_basis: basis,
            amount_columns,
            tolerance_minor_units: recipe.tolerance_minor_units,
        },
        &left,
        &right,
    )
    .map_err(invalid)?;
    let result_json = serde_json::to_string(&result)
        .map_err(|error| invalid(format!("comparison result cannot be encoded: {error}")))?;
    if result_json.len() > MAX_RESULT_JSON_BYTES {
        return Err(invalid(
            "comparison result exceeds the immutable 8 MiB run-record limit",
        ));
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
            &run_id,
            recipe_version_id,
            &hash_a[..],
            &hash_b[..],
            &digest[..],
            &result_json,
            started_at_ms,
            completed_at_ms,
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
            Uuid::new_v4().to_string(),
            &run_id,
            recipe_version_id,
            completed_at_ms,
            json!({"source_a_sha256":hex(&hash_a),"source_b_sha256":hex(&hash_b),
                "result_sha256":result.result_sha256_hex,
                "row_count_a":left.len(),"row_count_b":right.len()})
            .to_string(),
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
         )",
        [recipe_version_id],
        |row| row.get(0),
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
    let rows = stmt.query_map([recipe_version_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Vec<u8>>(1)?,
            row.get::<_, Vec<u8>>(2)?,
            row.get::<_, Vec<u8>>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, i64>(5)?,
            row.get::<_, i64>(6)?,
        ))
    })?;
    let mut runs = Vec::new();
    for row in rows {
        let (run_id, hash_a, hash_b, stored_digest, result_json, started_at_ms, completed_at_ms) =
            row?;
        if hash_a.len() != 32 || hash_b.len() != 32 || stored_digest.len() != 32 {
            return Err(invalid(
                "stored immutable comparison run has invalid digest lengths",
            ));
        }
        let result: ComparisonResult = serde_json::from_str(&result_json)
            .map_err(|error| invalid(format!("invalid immutable comparison run JSON: {error}")))?;
        if decode_hex_32(&result.result_sha256_hex)?.as_slice() != stored_digest.as_slice() {
            return Err(invalid(
                "stored result digest does not match comparison metadata",
            ));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{normal_data, normal_data_datasets, normal_data_provenance, normal_data_recipes};
    use rusqlite::params;
    use sha2::{Digest, Sha256};
    use std::{fs, path::PathBuf};

    #[cfg(unix)]
    fn encode_relative(path: &Path) -> (Vec<u8>, &'static str) {
        use std::os::unix::ffi::OsStrExt;
        (path.as_os_str().as_bytes().to_vec(), "unix-bytes")
    }

    #[cfg(windows)]
    fn encode_relative(path: &Path) -> (Vec<u8>, &'static str) {
        use std::os::windows::ffi::OsStrExt;
        let bytes = path
            .as_os_str()
            .encode_wide()
            .flat_map(u16::to_le_bytes)
            .collect();
        (bytes, "windows-utf16le")
    }

    struct Fixture {
        folder: PathBuf,
        path: PathBuf,
        source_b: PathBuf,
        recipe_version_id: String,
    }

    impl Fixture {
        fn new() -> Self {
            Self::with_declared_keys(true)
        }

        fn with_declared_keys(declare_keys: bool) -> Self {
            let folder =
                std::env::temp_dir().join(format!("pdox-comparison-run-{}", Uuid::new_v4()));
            fs::create_dir_all(&folder).expect("create fixture directory");
            let path = folder.join("state.sqlite");
            persistence::initialize_database(&path).expect("initialize v27 database");
            let directory = folder.join("data");
            fs::create_dir_all(&directory).expect("create data directory");
            let canonical = fs::canonicalize(&directory).expect("canonical approved root");
            let root = persistence::register_storage_root(
                &path,
                "test-comparison-root",
                &directory,
                &canonical,
            )
            .expect("approve fixture root");
            let workspace = normal_data::create_workspace(
                &path,
                normal_data::WorkspaceDefinition {
                    name: "Working data",
                    description: None,
                    client_id: None,
                    period_start: None,
                    period_end: None,
                },
            )
            .expect("create engagement-independent workspace");
            let mut versions = Vec::new();
            let sources = [
                (
                    "a.csv",
                    b"Key,Period,Amount\nINV-100,2026-08,10000\n".as_slice(),
                ),
                (
                    "b.csv",
                    b"Key,Period,Amount\nINV-100,2026-09,10250\n".as_slice(),
                ),
            ];
            for (source_index, (filename, bytes)) in sources.into_iter().enumerate() {
                let source = directory.join(filename);
                fs::write(&source, bytes).expect("write source bytes");
                let document_id = Uuid::new_v4().to_string();
                let file_instance_id = Uuid::new_v4().to_string();
                let content_version_id = Uuid::new_v4().to_string();
                let (relative_bytes, encoding) = encode_relative(Path::new(filename));
                let hash = Sha256::digest(bytes);
                let connection =
                    persistence::open_configured_connection(&path).expect("open fixture database");
                connection
                    .execute(
                        "INSERT INTO documents (
                        document_id, storage_state, display_name, created_at_ms
                    ) VALUES (?1, 'LINKED', ?2, 1)",
                        params![&document_id, filename],
                    )
                    .expect("insert document");
                connection
                    .execute(
                        "INSERT INTO file_instances (
                        file_instance_id, document_id, storage_root_id,
                        relative_path_native, path_native_encoding, relative_path_display,
                        relative_path_search, size_bytes, first_seen_at_ms,
                        last_seen_at_ms, availability_state
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, 1, 'AVAILABLE')",
                        params![
                            &file_instance_id,
                            &document_id,
                            &root.storage_root_id,
                            relative_bytes,
                            encoding,
                            filename,
                            filename,
                            bytes.len() as i64
                        ],
                    )
                    .expect("insert indexed file");
                connection
                    .execute(
                        "INSERT INTO content_versions (
                        content_version_id, document_id, file_instance_id,
                        observed_at_ms, size_bytes, sha256,
                        verification_state, source_stable_during_read
                    ) VALUES (?1, ?2, ?3, 1, ?4, ?5, 'HASH_VERIFIED', 1)",
                        params![
                            &content_version_id,
                            &document_id,
                            &file_instance_id,
                            bytes.len() as i64,
                            &hash[..]
                        ],
                    )
                    .expect("insert hash-verified observation");
                drop(connection);
                let dataset = normal_data_datasets::create_dataset(
                    &path,
                    &workspace.normal_data_workspace_id,
                    &file_instance_id,
                    filename,
                )
                .expect("bind versioned dataset source");
                for (name, role, data_type) in [
                    ("Key", "BUSINESS_KEY", "TEXT"),
                    ("Period", "FILING_PERIOD", "PERIOD"),
                    ("Amount", "NUMERIC_VALUE", "DECIMAL"),
                ] {
                    if !declare_keys && source_index == 1 && role == "BUSINESS_KEY" {
                        continue;
                    }
                    normal_data_datasets::declare_column(
                        &path,
                        &dataset.normal_data_dataset_version_id,
                        name,
                        role,
                        data_type,
                    )
                    .expect("declare source semantic column");
                }
                versions.push(dataset.normal_data_dataset_version_id);
            }
            let recipe = normal_data_recipes::create_recipe(
                &path,
                normal_data_recipes::RecipeDefinition {
                    normal_data_workspace_id: &workspace.normal_data_workspace_id,
                    name: "Filing comparison",
                    dataset_a_version_id: &versions[0],
                    dataset_b_version_id: &versions[1],
                    period_basis: "FILING_PERIOD",
                    period_column_a: "Period",
                    period_column_b: "Period",
                    amount_columns: &["Amount".to_string()],
                    tolerance_minor_units: 0,
                },
            )
            .expect("create immutable comparison recipe");
            Self {
                folder,
                path,
                source_b: directory.join("b.csv"),
                recipe_version_id: recipe.normal_data_comparison_recipe_version_id,
            }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.folder);
        }
    }

    #[test]
    fn verified_csv_movement_persists_exact_hashes_and_immutable_audit_event() {
        let fixture = Fixture::new();
        let run = execute_comparison(&fixture.path, &fixture.recipe_version_id)
            .expect("verified comparison");
        assert_eq!(run.result.summary.period_moved, 1);
        assert_eq!(run.result.entries[0].period_a.as_deref(), Some("2026-08"));
        assert_eq!(run.result.entries[0].period_b.as_deref(), Some("2026-09"));
        assert_eq!(
            run.result.entries[0].amount_differences[0].b_minus_a_minor_units,
            250
        );
        assert_eq!(run.dataset_a_source_sha256_hex.len(), 64);
        assert_eq!(run.dataset_b_source_sha256_hex.len(), 64);
        let previous =
            list_comparison_runs(&fixture.path, &fixture.recipe_version_id).expect("query history");
        assert_eq!(previous.len(), 1);
        assert_eq!(
            previous[0].normal_data_comparison_run_id,
            run.normal_data_comparison_run_id
        );
        assert_eq!(previous[0].result, run.result);
        let connection =
            persistence::open_configured_connection(&fixture.path).expect("open database");
        assert!(connection
            .execute(
                "UPDATE normal_data_comparison_runs SET result_json = '{}'
             WHERE normal_data_comparison_run_id = ?1",
                [&run.normal_data_comparison_run_id],
            )
            .is_err());
        assert!(connection
            .execute(
                "DELETE FROM normal_data_comparison_runs
             WHERE normal_data_comparison_run_id = ?1",
                [&run.normal_data_comparison_run_id],
            )
            .is_err());
        let events = persistence::count_audit_events_for_test(
            &fixture.path,
            "NORMAL_DATA_COMPARISON_RUN_COMPLETED",
            &run.normal_data_comparison_run_id,
        )
        .expect("audited execution");
        assert_eq!(events, 1);
    }

    #[test]
    fn linked_source_change_rejects_run_and_keeps_prior_history() {
        let fixture = Fixture::new();
        execute_comparison(&fixture.path, &fixture.recipe_version_id).expect("first run");
        fs::write(
            &fixture.source_b,
            b"Key,Period,Amount\nINV-100,2026-09,10200\n",
        )
        .expect("same-sized source tampering");
        assert!(execute_comparison(&fixture.path, &fixture.recipe_version_id).is_err());
        let runs = list_comparison_runs(&fixture.path, &fixture.recipe_version_id)
            .expect("immutable history remains");
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].result.summary.period_moved, 1);
    }

    #[test]
    fn unconfirmed_business_key_cannot_generate_a_run() {
        let fixture = Fixture::with_declared_keys(false);
        let connection =
            persistence::open_configured_connection(&fixture.path).expect("open database");
        let all: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM normal_data_column_semantics
             WHERE semantic_role = 'BUSINESS_KEY'",
                [],
                |row| row.get(0),
            )
            .expect("declared keys");
        assert_eq!(all, 1);
        assert!(execute_comparison(&fixture.path, &fixture.recipe_version_id).is_err());
        assert!(
            list_comparison_runs(&fixture.path, &fixture.recipe_version_id)
                .expect("no runs")
                .is_empty()
        );
    }
    #[test]
    fn historical_provenance_keeps_exact_run_identity_without_promotion_or_new_capture() {
        let fixture = Fixture::new();
        let run =
            execute_comparison(&fixture.path, &fixture.recipe_version_id).expect("verified run");
        let previous_events = persistence::count_audit_events_for_test(
            &fixture.path,
            "NORMAL_DATA_COMPARISON_RUN_COMPLETED",
            &run.normal_data_comparison_run_id,
        )
        .expect("audit event count");
        let receipt =
            normal_data_provenance::inspect_run(&fixture.path, &run.normal_data_comparison_run_id)
                .expect("historical working-data receipt");
        assert_eq!(
            receipt.normal_data_comparison_recipe_version_id,
            fixture.recipe_version_id
        );
        assert_eq!(receipt.source_a.sha256_hex, run.dataset_a_source_sha256_hex);
        assert_eq!(receipt.source_b.sha256_hex, run.dataset_b_source_sha256_hex);
        assert_eq!(receipt.result_sha256_hex, run.result.result_sha256_hex);
        assert_eq!(receipt.verification_at_execution, "HASH_VERIFIED_RECORDED");
        assert!(!receipt.current_source_bytes_checked);
        assert!(!receipt.is_controlled_evidence);
        assert!(!receipt.specialist_promotion_authorized);

        // A changed linked file cannot rewrite this historical identity or imply
        // it has been reverified in the present.
        fs::write(
            &fixture.source_b,
            b"Key,Period,Amount\nINV-100,2026-09,10200\n",
        )
        .expect("change source bytes without changing size");
        assert_eq!(
            normal_data_provenance::inspect_run(&fixture.path, &run.normal_data_comparison_run_id,)
                .expect("still inspect frozen historical run"),
            receipt
        );
        assert_eq!(
            persistence::count_audit_events_for_test(
                &fixture.path,
                "NORMAL_DATA_COMPARISON_RUN_COMPLETED",
                &run.normal_data_comparison_run_id,
            )
            .expect("no extra audit events"),
            previous_events
        );
        let connection =
            persistence::open_configured_connection(&fixture.path).expect("open receipt database");
        let evidence_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM controlled_evidence_versions",
                [],
                |row| row.get(0),
            )
            .expect("receipt did not capture originals");
        assert_eq!(evidence_count, 0);
    }

    #[test]
    fn explicit_source_preflight_matches_historical_hashes_without_capturing_or_mutating() {
        let fixture = Fixture::new();
        let run =
            execute_comparison(&fixture.path, &fixture.recipe_version_id).expect("verified run");
        let observation = normal_data_provenance::recheck_run_sources(
            &fixture.path,
            &run.normal_data_comparison_run_id,
        )
        .expect("both exact source versions are available");
        assert_eq!(
            observation.normal_data_comparison_run_id,
            run.normal_data_comparison_run_id
        );
        assert_eq!(
            observation.source_a.sha256_hex,
            run.dataset_a_source_sha256_hex
        );
        assert_eq!(
            observation.source_b.sha256_hex,
            run.dataset_b_source_sha256_hex
        );
        assert!(observation.source_a.observed_at_ms >= run.started_at_ms);
        assert!(observation.source_b.observed_at_ms >= run.started_at_ms);
        assert!(observation.exact_sources_verified_at_read);
        assert!(!observation.is_controlled_evidence);
        assert!(!observation.specialist_promotion_authorized);

        let conn = persistence::open_configured_connection(&fixture.path)
            .expect("inspect receipt side effects");
        let controlled: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM controlled_evidence_versions",
                [],
                |row| row.get(0),
            )
            .expect("count evidence versions");
        assert_eq!(controlled, 0);
        let history = list_comparison_runs(&fixture.path, &fixture.recipe_version_id)
            .expect("comparison history is immutable");
        assert_eq!(history.len(), 1);
        assert_eq!(
            persistence::count_audit_events_for_test(
                &fixture.path,
                "NORMAL_DATA_COMPARISON_RUN_COMPLETED",
                &run.normal_data_comparison_run_id,
            )
            .expect("unchanged execution audit history"),
            1
        );
    }

    #[test]
    fn live_preflight_rejects_same_size_change_but_preserves_historical_receipt() {
        let fixture = Fixture::new();
        let run =
            execute_comparison(&fixture.path, &fixture.recipe_version_id).expect("verified run");
        // The source remains the same length, defeating size-only comparisons.
        fs::write(
            &fixture.source_b,
            b"Key,Period,Amount\nINV-100,2026-09,10200\n",
        )
        .expect("tamper with same-sized source");
        assert!(normal_data_provenance::recheck_run_sources(
            &fixture.path,
            &run.normal_data_comparison_run_id
        )
        .is_err());
        // Read-only historical metadata continues to reflect the exact run;
        // it deliberately does not claim current bytes are still verified.
        let historic =
            normal_data_provenance::inspect_run(&fixture.path, &run.normal_data_comparison_run_id)
                .expect("frozen provenance remains readable");
        assert_eq!(
            historic.source_b.sha256_hex,
            run.dataset_b_source_sha256_hex
        );
        assert!(!historic.current_source_bytes_checked);
    }

    #[test]
    fn live_preflight_rejects_missing_source_and_invalid_run_id() {
        let fixture = Fixture::new();
        let run =
            execute_comparison(&fixture.path, &fixture.recipe_version_id).expect("verified run");
        assert!(normal_data_provenance::recheck_run_sources(&fixture.path, "not-a-uuid").is_err());
        assert!(normal_data_provenance::recheck_run_sources(
            &fixture.path,
            &Uuid::new_v4().to_string()
        )
        .is_err());
        fs::remove_file(&fixture.source_b).expect("delete linked file");
        assert!(normal_data_provenance::recheck_run_sources(
            &fixture.path,
            &run.normal_data_comparison_run_id
        )
        .is_err());
        assert!(normal_data_provenance::inspect_run(
            &fixture.path,
            &run.normal_data_comparison_run_id
        )
        .is_ok());
    }

    #[test]
    fn provenance_rejects_bad_ids_forged_source_hash_and_modified_result_values() {
        let fixture = Fixture::new();
        let run =
            execute_comparison(&fixture.path, &fixture.recipe_version_id).expect("verified run");
        assert!(normal_data_provenance::inspect_run(&fixture.path, "../not-a-uuid").is_err());
        assert!(
            normal_data_provenance::inspect_run(&fixture.path, &Uuid::new_v4().to_string())
                .is_err()
        );
        let connection =
            persistence::open_configured_connection(&fixture.path).expect("open receipt database");
        let forged_hash_id = Uuid::new_v4().to_string();
        connection
            .execute(
                "INSERT INTO normal_data_comparison_runs (
                normal_data_comparison_run_id, normal_data_comparison_recipe_version_id,
                dataset_a_source_sha256, dataset_b_source_sha256, result_sha256,
                result_json, started_at_ms, completed_at_ms
             )
             SELECT ?1, normal_data_comparison_recipe_version_id,
                    zeroblob(32), dataset_b_source_sha256, result_sha256, result_json,
                    started_at_ms, completed_at_ms
             FROM normal_data_comparison_runs WHERE normal_data_comparison_run_id = ?2",
                params![&forged_hash_id, &run.normal_data_comparison_run_id],
            )
            .expect("insert forged synthetic immutable record");
        assert!(normal_data_provenance::inspect_run(&fixture.path, &forged_hash_id).is_err());

        // A forged result with the original metadata digest must not produce a receipt.
        let tampered_id = Uuid::new_v4().to_string();
        let mut result = run.result.clone();
        result.summary.only_b += 1;
        let tampered_json = serde_json::to_string(&result).expect("encode forged run");
        connection
            .execute(
                "INSERT INTO normal_data_comparison_runs (
                normal_data_comparison_run_id, normal_data_comparison_recipe_version_id,
                dataset_a_source_sha256, dataset_b_source_sha256, result_sha256,
                result_json, started_at_ms, completed_at_ms
             )
             SELECT ?1, normal_data_comparison_recipe_version_id,
                    dataset_a_source_sha256, dataset_b_source_sha256, result_sha256,
                    ?2, started_at_ms, completed_at_ms
             FROM normal_data_comparison_runs WHERE normal_data_comparison_run_id = ?3",
                params![
                    &tampered_id,
                    &tampered_json,
                    &run.normal_data_comparison_run_id
                ],
            )
            .expect("insert forged synthetic output");
        assert!(normal_data_provenance::inspect_run(&fixture.path, &tampered_id).is_err());
    }
}
