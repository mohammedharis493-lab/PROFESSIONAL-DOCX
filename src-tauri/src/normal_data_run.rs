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
    use crate::{
        normal_data, normal_data_datasets, normal_data_promotion_policy as promotion_policy,
        normal_data_provenance, normal_data_recipes,
    };
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

    struct PolicyTarget {
        engagement_id: String,
        workpaper_id: String,
        revision_id: String,
    }

    fn create_policy_target(database_path: &Path, state: &str) -> PolicyTarget {
        let client_id = Uuid::new_v4().to_string();
        let service_id = Uuid::new_v4().to_string();
        let engagement_id = Uuid::new_v4().to_string();
        let workpaper_id = Uuid::new_v4().to_string();
        let revision_id = Uuid::new_v4().to_string();
        let connection =
            persistence::open_configured_connection(database_path).expect("open target database");
        connection
            .execute(
                "INSERT INTO clients (client_id, name, created_at_ms) VALUES (?1, 'Client', 1)",
                [&client_id],
            )
            .expect("target client");
        connection
            .execute(
                "INSERT INTO service_types (service_type_id, name, normalized_name, created_at_ms)
             VALUES (?1, 'Audit', ?2, 1)",
                params![&service_id, &service_id],
            )
            .expect("target service type");
        connection
            .execute(
                "INSERT INTO engagements (
                engagement_id, client_id, service_type_id, name, status, created_at_ms
             ) VALUES (?1, ?2, ?3, 'Engagement', 'ACTIVE', 1)",
                params![&engagement_id, &client_id, &service_id],
            )
            .expect("target engagement");
        connection
            .execute(
                "INSERT INTO workpapers (
                workpaper_id, engagement_id, reference, title, workflow_state, created_at_ms
             ) VALUES (?1, ?2, ?3, 'Working paper', ?4, 1)",
                params![&workpaper_id, &engagement_id, &workpaper_id, state],
            )
            .expect("target workpaper");
        connection
            .execute(
                "INSERT INTO workpaper_revisions (
                workpaper_revision_id, workpaper_id, revision_number, created_at_ms
             ) VALUES (?1, ?2, 1, 1)",
                params![&revision_id, &workpaper_id],
            )
            .expect("target workpaper revision");
        PolicyTarget {
            engagement_id,
            workpaper_id,
            revision_id,
        }
    }

    #[derive(Default)]
    struct TestGrants {
        allowed: Vec<(promotion_policy::PromotionPermission, String)>,
    }

    impl TestGrants {
        fn allow(&mut self, permission: promotion_policy::PromotionPermission, id: &str) {
            self.allowed.push((permission, id.to_string()));
        }
    }

    impl promotion_policy::PromotionPermissionPolicy for TestGrants {
        fn is_allowed(
            &self,
            _principal_id: &str,
            permission: promotion_policy::PromotionPermission,
            resource_id: &str,
        ) -> bool {
            self.allowed
                .iter()
                .any(|grant| grant.0 == permission && grant.1 == resource_id)
        }
    }

    fn full_test_grants(workspace_id: &str, target: &PolicyTarget) -> TestGrants {
        use promotion_policy::PromotionPermission as Permission;
        let mut grants = TestGrants::default();
        grants.allow(Permission::ReadNormalDataWorkspace, workspace_id);
        grants.allow(
            Permission::AttachEvidenceToEngagement,
            &target.engagement_id,
        );
        grants.allow(Permission::ModifyWorkpaperRevision, &target.revision_id);
        grants
    }

    #[test]
    fn promotion_policy_never_trusts_an_actor_string_or_partial_permissions() {
        use promotion_policy::{PromotionIntent, PromotionPermission as Permission};
        let fixture = Fixture::new();
        let run =
            execute_comparison(&fixture.path, &fixture.recipe_version_id).expect("comparison run");
        let source =
            normal_data_provenance::inspect_run(&fixture.path, &run.normal_data_comparison_run_id)
                .expect("history");
        let target = create_policy_target(&fixture.path, "DRAFT");
        let intent = PromotionIntent {
            run_id: &run.normal_data_comparison_run_id,
            expected_workspace_id: &source.normal_data_workspace_id,
            target_engagement_id: &target.engagement_id,
            target_workpaper_id: &target.workpaper_id,
            target_workpaper_revision_id: &target.revision_id,
        };
        let principal = promotion_policy::VerifiedPrincipal::fixture(&Uuid::new_v4().to_string());
        let grants = full_test_grants(&source.normal_data_workspace_id, &target);
        assert!(promotion_policy::check_policy(&fixture.path, &intent, None, &grants).is_err());
        assert!(promotion_policy::check_policy(
            &fixture.path,
            &intent,
            Some(&principal),
            &promotion_policy::DenyAllPromotionPermissions,
        )
        .is_err());
        let mut partial = TestGrants::default();
        partial.allow(
            Permission::ReadNormalDataWorkspace,
            &source.normal_data_workspace_id,
        );
        partial.allow(
            Permission::AttachEvidenceToEngagement,
            &target.engagement_id,
        );
        assert!(
            promotion_policy::check_policy(&fixture.path, &intent, Some(&principal), &partial,)
                .is_err()
        );
        // Only test-only, explicitly granted identity passes this internal
        // structural policy; production exposes neither a principal constructor
        // nor a target attachment command.
        promotion_policy::check_policy(&fixture.path, &intent, Some(&principal), &grants)
            .expect("fully scoped fixture-only policy");

        let conn = persistence::open_configured_connection(&fixture.path).expect("DB");
        for table in ["workpaper_evidence_links", "controlled_evidence_versions"] {
            let count: i64 = conn
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .expect("no side effects");
            assert_eq!(
                count, 0,
                "{table} must not change during an authorization preflight"
            );
        }
    }

    #[test]
    fn promotion_policy_rejects_wrong_workspace_engagement_workpaper_and_stale_revision() {
        use promotion_policy::{PromotionIntent, PromotionPermission as Permission};
        let fixture = Fixture::new();
        let run =
            execute_comparison(&fixture.path, &fixture.recipe_version_id).expect("comparison run");
        let source =
            normal_data_provenance::inspect_run(&fixture.path, &run.normal_data_comparison_run_id)
                .expect("history");
        let target = create_policy_target(&fixture.path, "DRAFT");
        let other = create_policy_target(&fixture.path, "DRAFT");
        let principal = promotion_policy::VerifiedPrincipal::fixture(&Uuid::new_v4().to_string());
        let mut grants = full_test_grants(&source.normal_data_workspace_id, &target);
        grants.allow(Permission::AttachEvidenceToEngagement, &other.engagement_id);
        grants.allow(Permission::ModifyWorkpaperRevision, &other.revision_id);
        let normal = PromotionIntent {
            run_id: &run.normal_data_comparison_run_id,
            expected_workspace_id: &source.normal_data_workspace_id,
            target_engagement_id: &target.engagement_id,
            target_workpaper_id: &target.workpaper_id,
            target_workpaper_revision_id: &target.revision_id,
        };
        promotion_policy::check_policy(&fixture.path, &normal, Some(&principal), &grants)
            .expect("correct scope");

        let wrong_workspace = Uuid::new_v4().to_string();
        grants.allow(Permission::ReadNormalDataWorkspace, &wrong_workspace);
        assert!(promotion_policy::check_policy(
            &fixture.path,
            &PromotionIntent {
                expected_workspace_id: &wrong_workspace,
                ..normal
            },
            Some(&principal),
            &grants
        )
        .is_err());
        assert!(promotion_policy::check_policy(
            &fixture.path,
            &PromotionIntent {
                target_engagement_id: &other.engagement_id,
                ..normal
            },
            Some(&principal),
            &grants
        )
        .is_err());
        assert!(promotion_policy::check_policy(
            &fixture.path,
            &PromotionIntent {
                target_workpaper_id: &other.workpaper_id,
                ..normal
            },
            Some(&principal),
            &grants
        )
        .is_err());
        let connection = persistence::open_configured_connection(&fixture.path).expect("DB");
        let second_revision = Uuid::new_v4().to_string();
        connection
            .execute(
                "INSERT INTO workpaper_revisions
             (workpaper_revision_id, workpaper_id, revision_number, created_at_ms)
             VALUES (?1, ?2, 2, 2)",
                params![&second_revision, &target.workpaper_id],
            )
            .expect("create subsequent revision");
        assert!(
            promotion_policy::check_policy(&fixture.path, &normal, Some(&principal), &grants)
                .is_err()
        );
    }

    #[test]
    fn promotion_policy_rejects_signed_reviewed_archived_or_invalid_target() {
        use promotion_policy::PromotionIntent;
        let fixture = Fixture::new();
        let run =
            execute_comparison(&fixture.path, &fixture.recipe_version_id).expect("comparison run");
        let source =
            normal_data_provenance::inspect_run(&fixture.path, &run.normal_data_comparison_run_id)
                .expect("history");
        let principal = promotion_policy::VerifiedPrincipal::fixture(&Uuid::new_v4().to_string());
        let target = create_policy_target(&fixture.path, "DRAFT");
        let grants = full_test_grants(&source.normal_data_workspace_id, &target);
        let intent = PromotionIntent {
            run_id: &run.normal_data_comparison_run_id,
            expected_workspace_id: &source.normal_data_workspace_id,
            target_engagement_id: &target.engagement_id,
            target_workpaper_id: &target.workpaper_id,
            target_workpaper_revision_id: &target.revision_id,
        };
        let conn = persistence::open_configured_connection(&fixture.path).expect("DB");
        conn.execute(
            "UPDATE workpapers SET workflow_state = 'SUBMITTED_FOR_REVIEW' WHERE workpaper_id = ?1",
            [&target.workpaper_id],
        )
        .expect("set review state");
        assert!(
            promotion_policy::check_policy(&fixture.path, &intent, Some(&principal), &grants)
                .is_err()
        );
        conn.execute(
            "UPDATE workpapers SET workflow_state = 'DRAFT' WHERE workpaper_id = ?1",
            [&target.workpaper_id],
        )
        .expect("restore draft");
        conn.execute(
            "INSERT INTO workpaper_signoffs
             (signoff_id, workpaper_id, workpaper_revision_id, signoff_type, actor_id, actor_role, signed_at_ms)
             VALUES (?1, ?2, ?3, 'PREPARED', 'fixture-actor', 'PREPARER', 3)",
            params![Uuid::new_v4().to_string(), &target.workpaper_id, &target.revision_id],
        ).expect("record immutable signoff");
        assert!(
            promotion_policy::check_policy(&fixture.path, &intent, Some(&principal), &grants)
                .is_err()
        );
        // A different draft workpaper is not a workaround for an archived engagement.
        let archived = create_policy_target(&fixture.path, "DRAFT");
        let archive_grants = full_test_grants(&source.normal_data_workspace_id, &archived);
        conn.execute(
            "UPDATE engagements SET archived_at_ms = 4 WHERE engagement_id = ?1",
            [&archived.engagement_id],
        )
        .expect("archive target");
        assert!(promotion_policy::check_policy(
            &fixture.path,
            &PromotionIntent {
                target_engagement_id: &archived.engagement_id,
                target_workpaper_id: &archived.workpaper_id,
                target_workpaper_revision_id: &archived.revision_id,
                ..intent
            },
            Some(&principal),
            &archive_grants
        )
        .is_err());
        assert!(promotion_policy::check_policy(
            &fixture.path,
            &PromotionIntent {
                run_id: "not-a-uuid",
                ..intent
            },
            Some(&principal),
            &grants
        )
        .is_err());
        // An unresolved note or archived workpaper is not eligible, even
        // when a test provider grants all three required resource permissions.
        let review_target = create_policy_target(&fixture.path, "DRAFT");
        let review_grants = full_test_grants(&source.normal_data_workspace_id, &review_target);
        conn.execute(
            "INSERT INTO review_notes (
                review_note_id, workpaper_id, workpaper_revision_id,
                title, body, current_state, created_at_ms, latest_event_at_ms
             ) VALUES (?1, ?2, ?3, 'Review note', 'Unresolved', 'OPEN', 2, 2)",
            params![
                Uuid::new_v4().to_string(),
                &review_target.workpaper_id,
                &review_target.revision_id
            ],
        )
        .expect("create open review note");
        assert!(promotion_policy::check_policy(
            &fixture.path,
            &PromotionIntent {
                target_engagement_id: &review_target.engagement_id,
                target_workpaper_id: &review_target.workpaper_id,
                target_workpaper_revision_id: &review_target.revision_id,
                ..intent
            },
            Some(&principal),
            &review_grants,
        )
        .is_err());

        let archived_workpaper = create_policy_target(&fixture.path, "DRAFT");
        let archived_grants =
            full_test_grants(&source.normal_data_workspace_id, &archived_workpaper);
        conn.execute(
            "UPDATE workpapers SET archived_at_ms = 5 WHERE workpaper_id = ?1",
            [&archived_workpaper.workpaper_id],
        )
        .expect("archive workpaper");
        assert!(promotion_policy::check_policy(
            &fixture.path,
            &PromotionIntent {
                target_engagement_id: &archived_workpaper.engagement_id,
                target_workpaper_id: &archived_workpaper.workpaper_id,
                target_workpaper_revision_id: &archived_workpaper.revision_id,
                ..intent
            },
            Some(&principal),
            &archived_grants,
        )
        .is_err());
    }

    #[test]
    fn sqlite_promotion_grants_deny_by_default_and_enforce_exact_resource_scopes() {
        use promotion_policy::{PromotionIntent, SqlitePromotionPermissions, VerifiedPrincipal};
        let fixture = Fixture::new();
        let run =
            execute_comparison(&fixture.path, &fixture.recipe_version_id).expect("comparison run");
        let source =
            normal_data_provenance::inspect_run(&fixture.path, &run.normal_data_comparison_run_id)
                .expect("historical receipt");
        let target = create_policy_target(&fixture.path, "DRAFT");
        let intent = PromotionIntent {
            run_id: &run.normal_data_comparison_run_id,
            expected_workspace_id: &source.normal_data_workspace_id,
            target_engagement_id: &target.engagement_id,
            target_workpaper_id: &target.workpaper_id,
            target_workpaper_revision_id: &target.revision_id,
        };
        let provider = SqlitePromotionPermissions::new(&fixture.path);
        let subject_id = Uuid::new_v4().to_string();
        let subject = VerifiedPrincipal::fixture(&subject_id);
        // A subject ID is not an authenticated principal in production; the
        // fixture constructor is available only when Rust tests are compiled.
        assert!(promotion_policy::check_policy(&fixture.path, &intent, None, &provider).is_err());
        assert!(
            promotion_policy::check_policy(&fixture.path, &intent, Some(&subject), &provider)
                .is_err()
        );
        let conn = persistence::open_configured_connection(&fixture.path).expect("open database");
        conn.execute(
            "INSERT INTO normal_data_permission_subjects
             (subject_id, identity_issuer, registered_at_ms)
             VALUES (?1, 'TRUSTED_NATIVE_IDP', 1)",
            [&subject_id],
        )
        .expect("test-only subject enrollment");
        // Enrollment alone grants zero access.
        assert!(
            promotion_policy::check_policy(&fixture.path, &intent, Some(&subject), &provider)
                .is_err()
        );
        let scopes = [
            (
                "READ_NORMAL_DATA_WORKSPACE",
                source.normal_data_workspace_id.as_str(),
            ),
            (
                "ATTACH_EVIDENCE_TO_ENGAGEMENT",
                target.engagement_id.as_str(),
            ),
            ("MODIFY_WORKPAPER_REVISION", target.revision_id.as_str()),
        ];
        let mut ids = Vec::new();
        for (index, (permission, resource)) in scopes.iter().enumerate() {
            let grant_id = Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO normal_data_permission_grants
                 (grant_id, subject_id, permission, resource_id, granted_at_ms)
                 VALUES (?1, ?2, ?3, ?4, 1)",
                params![&grant_id, &subject_id, permission, resource],
            )
            .expect("test-only exact-scoped grant");
            ids.push(grant_id);
            if index < 2 {
                assert!(
                    promotion_policy::check_policy(
                        &fixture.path,
                        &intent,
                        Some(&subject),
                        &provider,
                    )
                    .is_err(),
                    "partial scope cannot authorize promotion"
                );
            }
        }
        promotion_policy::check_policy(&fixture.path, &intent, Some(&subject), &provider)
            .expect("three exact grants are necessary with a test-only verified principal");
        let different_subject = VerifiedPrincipal::fixture(&Uuid::new_v4().to_string());
        assert!(promotion_policy::check_policy(
            &fixture.path,
            &intent,
            Some(&different_subject),
            &provider,
        )
        .is_err());
        let other_engagement = Uuid::new_v4().to_string();
        assert!(promotion_policy::check_policy(
            &fixture.path,
            &PromotionIntent {
                target_engagement_id: &other_engagement,
                ..intent
            },
            Some(&subject),
            &provider,
        )
        .is_err());

        // Revocation takes effect on the very next independent policy read.
        conn.execute(
            "UPDATE normal_data_permission_grants
             SET revoked_at_ms = 2 WHERE grant_id = ?1",
            [&ids[2]],
        )
        .expect("revoke target revision write grant");
        assert!(
            promotion_policy::check_policy(&fixture.path, &intent, Some(&subject), &provider)
                .is_err()
        );
        assert!(
            conn.execute(
                "UPDATE normal_data_permission_grants
                 SET revoked_at_ms = NULL WHERE grant_id = ?1",
                [&ids[2]],
            )
            .is_err(),
            "a revoked grant cannot be restored in place"
        );
        assert!(
            conn.execute(
                "UPDATE normal_data_permission_grants
                 SET resource_id = ?1 WHERE grant_id = ?2",
                params![Uuid::new_v4().to_string(), &ids[0]],
            )
            .is_err(),
            "resource scope is immutable"
        );
        assert!(
            conn.execute(
                "DELETE FROM normal_data_permission_grants WHERE grant_id = ?1",
                [&ids[0]],
            )
            .is_err(),
            "grant records are append-only except one-way revocation"
        );

        let expired_id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO normal_data_permission_grants
             (grant_id, subject_id, permission, resource_id, granted_at_ms, expires_at_ms)
             VALUES (?1, ?2, 'MODIFY_WORKPAPER_REVISION', ?3, 2, 3)",
            params![&expired_id, &subject_id, &target.revision_id],
        )
        .expect("insert expired test grant");
        assert!(
            promotion_policy::check_policy(&fixture.path, &intent, Some(&subject), &provider)
                .is_err(),
            "expired grants cannot reactivate a revision permission"
        );
        conn.execute(
            "UPDATE normal_data_permission_grants
             SET revoked_at_ms = 3 WHERE grant_id = ?1",
            [&expired_id],
        )
        .expect("revoke expired grant before replacement");
        conn.execute(
            "INSERT INTO normal_data_permission_grants
             (grant_id, subject_id, permission, resource_id, granted_at_ms)
             VALUES (?1, ?2, 'MODIFY_WORKPAPER_REVISION', ?3, 4)",
            params![Uuid::new_v4().to_string(), &subject_id, &target.revision_id],
        )
        .expect("reissue new grant rather than mutating old row");
        promotion_policy::check_policy(&fixture.path, &intent, Some(&subject), &provider)
            .expect("fresh scoped grant resumes test-only eligibility");

        conn.execute(
            "UPDATE normal_data_permission_subjects
             SET disabled_at_ms = 5 WHERE subject_id = ?1",
            [&subject_id],
        )
        .expect("one-way disable test subject");
        assert!(
            promotion_policy::check_policy(&fixture.path, &intent, Some(&subject), &provider)
                .is_err(),
            "disabled subject must lose all permissions"
        );
        assert!(
            conn.execute(
                "UPDATE normal_data_permission_subjects
                 SET disabled_at_ms = NULL WHERE subject_id = ?1",
                [&subject_id],
            )
            .is_err(),
            "disabled subject cannot be silently re-enabled"
        );

        let evidence_links: i64 = conn
            .query_row("SELECT COUNT(*) FROM workpaper_evidence_links", [], |row| {
                row.get(0)
            })
            .expect("no links");
        let preserved: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM controlled_evidence_versions",
                [],
                |row| row.get(0),
            )
            .expect("no evidence captures");
        assert_eq!((evidence_links, preserved), (0, 0));
    }

    #[test]
    fn atomic_promotion_gate_enforces_scopes_target_and_rollback_on_same_connection() {
        use promotion_policy::{PromotionIntent, VerifiedPrincipal};
        let fixture = Fixture::new();
        let run = execute_comparison(&fixture.path, &fixture.recipe_version_id)
            .expect("verified comparison run");
        let source =
            normal_data_provenance::inspect_run(&fixture.path, &run.normal_data_comparison_run_id)
                .expect("frozen historical receipt");
        let target = create_policy_target(&fixture.path, "DRAFT");
        let principal_id = Uuid::new_v4().to_string();
        let principal = VerifiedPrincipal::fixture(&principal_id);
        let intent = PromotionIntent {
            run_id: &run.normal_data_comparison_run_id,
            expected_workspace_id: &source.normal_data_workspace_id,
            target_engagement_id: &target.engagement_id,
            target_workpaper_id: &target.workpaper_id,
            target_workpaper_revision_id: &target.revision_id,
        };
        let conn = persistence::open_configured_connection(&fixture.path).expect("database");
        conn.execute_batch("CREATE TABLE transaction_authorization_probe (value TEXT NOT NULL);")
            .expect("test-only probe");
        conn.execute(
            "INSERT INTO normal_data_permission_subjects (
                subject_id, identity_issuer, registered_at_ms
             ) VALUES (?1, 'TRUSTED_NATIVE_IDP', 1)",
            [&principal_id],
        )
        .expect("test-only enrolled subject");

        let ran = std::cell::Cell::new(false);
        assert!(
            promotion_policy::with_authorized_transaction(&fixture.path, &intent, None, |_| {
                ran.set(true);
                Ok(())
            })
            .is_err(),
            "missing verified principal must deny"
        );
        assert!(!ran.get(), "denied callback must never execute");
        assert!(
            promotion_policy::with_authorized_transaction(
                &fixture.path,
                &intent,
                Some(&principal),
                |_| {
                    ran.set(true);
                    Ok(())
                }
            )
            .is_err(),
            "an enrolled subject has no grants by default"
        );
        assert!(!ran.get());

        for (index, (permission, resource)) in [
            (
                "READ_NORMAL_DATA_WORKSPACE",
                source.normal_data_workspace_id.as_str(),
            ),
            (
                "ATTACH_EVIDENCE_TO_ENGAGEMENT",
                target.engagement_id.as_str(),
            ),
            ("MODIFY_WORKPAPER_REVISION", target.revision_id.as_str()),
        ]
        .iter()
        .enumerate()
        {
            conn.execute(
                "INSERT INTO normal_data_permission_grants (
                    grant_id, subject_id, permission, resource_id, granted_at_ms
                 ) VALUES (?1, ?2, ?3, ?4, 1)",
                params![
                    Uuid::new_v4().to_string(),
                    &principal_id,
                    permission,
                    resource
                ],
            )
            .expect("test scoped grant");
            if index < 2 {
                assert!(
                    promotion_policy::with_authorized_transaction(
                        &fixture.path,
                        &intent,
                        Some(&principal),
                        |_| {
                            ran.set(true);
                            Ok(())
                        }
                    )
                    .is_err(),
                    "all three exact grants required"
                );
                assert!(!ran.get());
            }
        }
        let value = promotion_policy::with_authorized_transaction(
            &fixture.path,
            &intent,
            Some(&principal),
            |tx| {
                tx.execute(
                    "INSERT INTO transaction_authorization_probe (value)
                     VALUES ('committed')",
                    [],
                )?;
                Ok(42)
            },
        )
        .expect("fully eligible test-only callback commits in same transaction");
        assert_eq!(value, 42);
        let error = promotion_policy::with_authorized_transaction(
            &fixture.path,
            &intent,
            Some(&principal),
            |tx| -> Result<(), persistence::PersistenceError> {
                tx.execute(
                    "INSERT INTO transaction_authorization_probe (value)
                     VALUES ('rolled_back')",
                    [],
                )?;
                Err(persistence::PersistenceError::Configuration(
                    "simulate rejected capture".to_string(),
                ))
            },
        );
        assert!(error.is_err(), "operation error aborts same transaction");
        let probe_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM transaction_authorization_probe",
                [],
                |row| row.get(0),
            )
            .expect("probe count");
        assert_eq!(probe_count, 1, "failed callback must roll back");

        // The IMMEDIATE SQLite reservation prevents another writer from
        // revoking permissions between this gate and its callback commit.
        promotion_policy::with_authorized_transaction(
            &fixture.path,
            &intent,
            Some(&principal),
            |_| {
                let outside = persistence::open_configured_connection(&fixture.path)
                    .expect("second connection");
                outside
                    .busy_timeout(std::time::Duration::from_millis(1))
                    .expect("short competing-write timeout");
                assert!(
                    outside
                        .execute(
                            "UPDATE normal_data_permission_grants
                     SET revoked_at_ms = 2
                     WHERE subject_id = ?1
                       AND permission = 'MODIFY_WORKPAPER_REVISION'",
                            [&principal_id],
                        )
                        .is_err(),
                    "competing revocation cannot interleave with transaction"
                );
                Ok(())
            },
        )
        .expect("writer reservation kept through callback");
        conn.execute(
            "UPDATE normal_data_permission_grants
             SET revoked_at_ms = 2
             WHERE subject_id = ?1 AND permission = 'MODIFY_WORKPAPER_REVISION'",
            [&principal_id],
        )
        .expect("revocation after commit");
        assert!(
            promotion_policy::with_authorized_transaction(
                &fixture.path,
                &intent,
                Some(&principal),
                |_| {
                    ran.set(true);
                    Ok(())
                },
            )
            .is_err(),
            "revocation blocks next transaction"
        );
        assert!(!ran.get());

        conn.execute(
            "INSERT INTO normal_data_permission_grants (
                grant_id, subject_id, permission, resource_id, granted_at_ms
             ) VALUES (?1, ?2, 'MODIFY_WORKPAPER_REVISION', ?3, 3)",
            params![
                Uuid::new_v4().to_string(),
                &principal_id,
                &target.revision_id
            ],
        )
        .expect("reissue new target grant");
        conn.execute(
            "UPDATE workpapers SET workflow_state = 'SUBMITTED_FOR_REVIEW'
             WHERE workpaper_id = ?1",
            [&target.workpaper_id],
        )
        .expect("transition to review");
        assert!(
            promotion_policy::with_authorized_transaction(
                &fixture.path,
                &intent,
                Some(&principal),
                |_| {
                    ran.set(true);
                    Ok(())
                }
            )
            .is_err(),
            "review-stage target not writable"
        );
        assert!(!ran.get());
        let evidence_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM workpaper_evidence_links", [], |row| {
                row.get(0)
            })
            .expect("no evidence links");
        assert_eq!(evidence_count, 0);
    }

    #[test]
    fn preservation_material_contains_three_exact_bounded_artifacts_without_writes() {
        let fixture = Fixture::new();
        let run = execute_comparison(&fixture.path, &fixture.recipe_version_id)
            .expect("execute immutable comparison");
        let run_id = &run.normal_data_comparison_run_id;
        let original_a =
            fs::read(fixture.folder.join("data").join("a.csv")).expect("fixture source A");
        let original_b = fs::read(&fixture.source_b).expect("fixture source B");
        let conn = persistence::open_configured_connection(&fixture.path).expect("open database");
        let stored_json: String = conn
            .query_row(
                "SELECT result_json FROM normal_data_comparison_runs
             WHERE normal_data_comparison_run_id = ?1",
                [run_id],
                |row| row.get(0),
            )
            .expect("immutable result JSON");
        let before_audit: i64 = conn
            .query_row("SELECT COUNT(*) FROM audit_events", [], |row| row.get(0))
            .expect("audit baseline");
        let material = crate::normal_data_preservation_material::prepare_preservation_material(
            &fixture.path,
            run_id,
        )
        .expect("three verified transient artifacts");
        let receipt = normal_data_provenance::inspect_run(&fixture.path, run_id)
            .expect("historical provenance");
        assert_eq!(material.run_id, *run_id);
        assert_eq!(material.workspace_id, receipt.normal_data_workspace_id);
        assert_eq!(
            material.recipe_version_id,
            receipt.normal_data_comparison_recipe_version_id
        );
        assert_eq!(
            material.source_a.dataset_version_id,
            receipt.source_a.dataset_version_id
        );
        assert_eq!(
            material.source_a.content_version_id,
            receipt.source_a.content_version_id
        );
        assert_eq!(
            material.source_b.dataset_version_id,
            receipt.source_b.dataset_version_id
        );
        assert_eq!(
            material.source_b.content_version_id,
            receipt.source_b.content_version_id
        );
        assert_ne!(material.source_a.document_id, material.source_b.document_id);
        assert_eq!(material.source_a.bytes, original_a);
        assert_eq!(material.source_b.bytes, original_b);
        assert_eq!(material.source_a.sha256_hex, receipt.source_a.sha256_hex);
        assert_eq!(material.source_b.sha256_hex, receipt.source_b.sha256_hex);
        assert_eq!(material.result.bytes, stored_json.as_bytes());
        assert_eq!(
            material.result.semantic_result_sha256_hex,
            receipt.result_sha256_hex
        );
        let output_sha256 = Sha256::digest(&material.result.bytes);
        let output_hex = output_sha256
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(material.result.artifact_sha256_hex, output_hex);
        assert_ne!(
            material.result.artifact_sha256_hex, material.result.semantic_result_sha256_hex,
            "serialized bytes hash is distinct from canonical calculation digest"
        );
        let after_audit: i64 = conn
            .query_row("SELECT COUNT(*) FROM audit_events", [], |row| row.get(0))
            .expect("audit count unchanged");
        let retained: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM controlled_evidence_versions",
                [],
                |row| row.get(0),
            )
            .expect("retained count");
        let links: i64 = conn
            .query_row("SELECT COUNT(*) FROM workpaper_evidence_links", [], |row| {
                row.get(0)
            })
            .expect("linked count");
        assert_eq!(
            before_audit, after_audit,
            "materialization cannot write audit events"
        );
        assert_eq!(
            (retained, links),
            (0, 0),
            "neither capture nor promotion occurs"
        );
    }

    #[test]
    fn preservation_material_denies_same_size_changed_and_missing_sources() {
        let fixture = Fixture::new();
        let run = execute_comparison(&fixture.path, &fixture.recipe_version_id)
            .expect("execute immutable comparison");
        let original_b = fs::read(&fixture.source_b).expect("original source B");
        let altered = String::from_utf8(original_b.clone())
            .expect("fixture ASCII source")
            .replace("INV-100", "INV-200");
        assert_eq!(altered.len(), original_b.len(), "same-size change");
        fs::write(&fixture.source_b, altered.as_bytes()).expect("change linked bytes");
        assert!(
            crate::normal_data_preservation_material::prepare_preservation_material(
                &fixture.path,
                &run.normal_data_comparison_run_id
            )
            .is_err(),
            "same-size tampering must never produce source material"
        );
        fs::write(&fixture.source_b, original_b).expect("restore source");
        fs::remove_file(&fixture.source_b).expect("remove linked source");
        assert!(
            crate::normal_data_preservation_material::prepare_preservation_material(
                &fixture.path,
                &run.normal_data_comparison_run_id
            )
            .is_err(),
            "missing original cannot be substituted with a historical hash"
        );
        assert!(
            normal_data_provenance::inspect_run(&fixture.path, &run.normal_data_comparison_run_id)
                .is_ok(),
            "historical receipt remains inspectable without current originals"
        );
    }

    #[test]
    fn preservation_material_rejects_tampered_stored_result_and_unknown_run() {
        let fixture = Fixture::new();
        let run = execute_comparison(&fixture.path, &fixture.recipe_version_id)
            .expect("execute immutable comparison");
        assert!(
            crate::normal_data_preservation_material::prepare_preservation_material(
                &fixture.path,
                "forged-run"
            )
            .is_err()
        );
        let conn =
            persistence::open_configured_connection(&fixture.path).expect("fixture database");
        // Fault-inject corruption: normal application writes cannot bypass
        // the immutable run trigger, but a damaged local database can.
        conn.execute_batch("DROP TRIGGER trg_normal_data_comparison_runs_no_update;")
            .expect("fault injection removes immutability guard");
        conn.execute(
            "UPDATE normal_data_comparison_runs
             SET result_json = json_set(result_json, '$.summary.totalBusinessKeys', 999)
             WHERE normal_data_comparison_run_id = ?1",
            [&run.normal_data_comparison_run_id],
        )
        .expect("corrupt frozen result");
        assert!(
            crate::normal_data_preservation_material::prepare_preservation_material(
                &fixture.path,
                &run.normal_data_comparison_run_id
            )
            .is_err(),
            "result digest mismatch must prevent materializing artifacts"
        );
        let retained: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM controlled_evidence_versions",
                [],
                |row| row.get(0),
            )
            .expect("retained count");
        assert_eq!(retained, 0);
    }

    #[test]
    fn staged_preservation_is_recoverable_and_never_promotes_working_data() {
        use crate::normal_data_staging::{self, StageStatus};
        let fixture = Fixture::new();
        let run = execute_comparison(&fixture.path, &fixture.recipe_version_id)
            .expect("frozen verified run");
        let material = crate::normal_data_preservation_material::prepare_preservation_material(
            &fixture.path,
            &run.normal_data_comparison_run_id,
        )
        .expect("exact three artifact bytes");
        let root = fixture.folder.join("private-staging");
        fs::create_dir(&root).expect("private root");
        let conn = persistence::open_configured_connection(&fixture.path).expect("database");
        let audit_before: i64 = conn
            .query_row("SELECT COUNT(*) FROM audit_events", [], |row| row.get(0))
            .expect("audit before");
        let receipt =
            normal_data_staging::stage_material(&root, &material).expect("private staging");
        assert_eq!(receipt.run_id, run.normal_data_comparison_run_id);
        assert_eq!(receipt.workspace_id, material.workspace_id);
        assert_eq!(receipt.source_a_sha256_hex, material.source_a.sha256_hex);
        assert_eq!(receipt.source_b_sha256_hex, material.source_b.sha256_hex);
        assert_eq!(
            receipt.result_semantic_sha256_hex,
            material.result.semantic_result_sha256_hex,
        );
        assert_eq!(
            receipt.result_artifact_sha256_hex,
            material.result.artifact_sha256_hex,
        );
        let package = root.join(format!("{}.ready", receipt.stage_id));
        assert!(package.exists());
        assert!(!root.join(format!("{}.partial", receipt.stage_id)).exists());
        assert_eq!(
            fs::read(package.join("source-a.bin")).expect("staged A"),
            material.source_a.bytes,
        );
        assert_eq!(
            fs::read(package.join("source-b.bin")).expect("staged B"),
            material.source_b.bytes,
        );
        assert_eq!(
            fs::read(package.join("result.json")).expect("staged result"),
            material.result.bytes,
        );
        let manifest =
            fs::read_to_string(package.join("manifest.json")).expect("identity-only manifest");
        assert!(!manifest.contains("INV-100"));
        assert_eq!(
            normal_data_staging::inspect_stage(&root, &receipt.stage_id).expect("reopened receipt"),
            receipt,
        );
        let inventory = normal_data_staging::scan_stages(&root).expect("restart scan");
        assert_eq!(inventory.len(), 1);
        assert_eq!(inventory[0].stage_id, receipt.stage_id);
        assert_eq!(inventory[0].status, StageStatus::ReadyVerified);
        assert!(normal_data_staging::discard_interrupted(&root, &receipt.stage_id).is_err());

        let audit_after: i64 = conn
            .query_row("SELECT COUNT(*) FROM audit_events", [], |row| row.get(0))
            .expect("audit after");
        let evidence_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM controlled_evidence_versions",
                [],
                |row| row.get(0),
            )
            .expect("controlled evidence");
        let links_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM workpaper_evidence_links", [], |row| {
                row.get(0)
            })
            .expect("links");
        assert_eq!(audit_before, audit_after);
        assert_eq!((evidence_count, links_count), (0, 0));

        // The integrity scan must not claim a ready package is still sound
        // after an out-of-band same-size disk edit.
        let mut changed = fs::read(package.join("source-b.bin")).expect("staged source");
        changed[0] ^= 1;
        fs::write(package.join("source-b.bin"), changed).expect("tamper staged bytes");
        assert!(normal_data_staging::inspect_stage(&root, &receipt.stage_id).is_err());
        let scan = normal_data_staging::scan_stages(&root).expect("corruption scan");
        assert_eq!(scan.len(), 1);
        assert_eq!(scan[0].status, StageStatus::Corrupt);
    }

    #[test]
    fn staged_run_binding_denies_forged_metadata_and_rechecks_permissions() {
        use crate::{normal_data_staged_policy, normal_data_staging};
        use promotion_policy::{PromotionIntent, VerifiedPrincipal};
        let fixture = Fixture::new();
        let first = execute_comparison(&fixture.path, &fixture.recipe_version_id)
            .expect("first frozen run");
        let second = execute_comparison(&fixture.path, &fixture.recipe_version_id)
            .expect("second run from same recipe/source versions");
        let material = crate::normal_data_preservation_material::prepare_preservation_material(
            &fixture.path,
            &first.normal_data_comparison_run_id,
        )
        .expect("three exact artifacts");
        let root = fixture.folder.join("private-bound-staging");
        fs::create_dir(&root).expect("private staging root");
        let staged = normal_data_staging::stage_material(&root, &material)
            .expect("verified staging package");
        let target = create_policy_target(&fixture.path, "DRAFT");
        let subject = Uuid::new_v4().to_string();
        let principal = VerifiedPrincipal::fixture(&subject);
        let first_intent = PromotionIntent {
            run_id: &first.normal_data_comparison_run_id,
            expected_workspace_id: &material.workspace_id,
            target_engagement_id: &target.engagement_id,
            target_workpaper_id: &target.workpaper_id,
            target_workpaper_revision_id: &target.revision_id,
        };
        let second_intent = PromotionIntent {
            run_id: &second.normal_data_comparison_run_id,
            expected_workspace_id: &material.workspace_id,
            target_engagement_id: &target.engagement_id,
            target_workpaper_id: &target.workpaper_id,
            target_workpaper_revision_id: &target.revision_id,
        };
        let conn = persistence::open_configured_connection(&fixture.path).expect("connection");
        let baseline: i64 = conn
            .query_row("SELECT COUNT(*) FROM audit_events", [], |row| row.get(0))
            .expect("baseline audit");
        assert!(
            normal_data_staged_policy::inspect_authorized_staged_run(
                &fixture.path,
                &root,
                &staged.stage_id,
                &first_intent,
                None,
            )
            .is_err(),
            "missing identity is denied before staging inspection"
        );
        conn.execute(
            "INSERT INTO normal_data_permission_subjects (
                subject_id, identity_issuer, registered_at_ms
             ) VALUES (?1, 'TRUSTED_NATIVE_IDP', 1)",
            [&subject],
        )
        .expect("test-only subject registration");
        assert!(
            normal_data_staged_policy::inspect_authorized_staged_run(
                &fixture.path,
                &root,
                &staged.stage_id,
                &first_intent,
                Some(&principal),
            )
            .is_err(),
            "subject with no grants is denied"
        );
        for (permission, resource) in [
            ("READ_NORMAL_DATA_WORKSPACE", &material.workspace_id),
            ("ATTACH_EVIDENCE_TO_ENGAGEMENT", &target.engagement_id),
            ("MODIFY_WORKPAPER_REVISION", &target.revision_id),
        ] {
            conn.execute(
                "INSERT INTO normal_data_permission_grants (
                    grant_id, subject_id, permission, resource_id, granted_at_ms
                 ) VALUES (?1, ?2, ?3, ?4, 1)",
                params![Uuid::new_v4().to_string(), &subject, permission, resource],
            )
            .expect("test-only exact grant");
        }
        let actual = normal_data_staged_policy::inspect_authorized_staged_run(
            &fixture.path,
            &root,
            &staged.stage_id,
            &first_intent,
            Some(&principal),
        )
        .expect("exact ready stage bound to eligible frozen run");
        assert_eq!(actual, staged);
        assert!(
            normal_data_staged_policy::inspect_authorized_staged_run(
                &fixture.path,
                &root,
                &staged.stage_id,
                &second_intent,
                Some(&principal),
            )
            .is_err(),
            "same input hashes do not authorize different run UUID"
        );

        // Rewrite only the manifest's source binding: plain on-disk scan
        // still verifies the three bytes, but independent SQLite metadata
        // binding must reject this valid-looking package.
        let manifest_path = root
            .join(format!("{}.ready", staged.stage_id))
            .join("manifest.json");
        let original_manifest = fs::read(&manifest_path).expect("manifest bytes");
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&original_manifest).expect("manifest JSON");
        manifest["source_a"]["content_version_id"] =
            serde_json::Value::String(Uuid::new_v4().to_string());
        fs::write(&manifest_path, serde_json::to_vec(&manifest).expect("JSON"))
            .expect("forge source content ID in manifest");
        assert!(
            normal_data_staging::inspect_stage(&root, &staged.stage_id).is_ok(),
            "plain file scan only guarantees its own declared hashes"
        );
        assert!(
            normal_data_staged_policy::inspect_authorized_staged_run(
                &fixture.path,
                &root,
                &staged.stage_id,
                &first_intent,
                Some(&principal),
            )
            .is_err(),
            "manifest source identity must match frozen run"
        );
        fs::write(&manifest_path, &original_manifest).expect("restore original manifest");

        // A semantically equivalent JSON with one extra trailing space has
        // the same calculation digest but DIFFERENT serialized artifact hash.
        let result_path = root
            .join(format!("{}.ready", staged.stage_id))
            .join("result.json");
        let original_result = fs::read(&result_path).expect("exact result bytes");
        let mut altered_result = original_result.clone();
        altered_result.push(b' ');
        fs::write(&result_path, &altered_result).expect("replace staged JSON bytes");
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&original_manifest).expect("manifest JSON");
        manifest["result"]["size_bytes"] = serde_json::Value::from(altered_result.len() as u64);
        manifest["result"]["sha256_hex"] = serde_json::Value::from(
            Sha256::digest(&altered_result)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
        );
        fs::write(&manifest_path, serde_json::to_vec(&manifest).expect("JSON"))
            .expect("forge consistent artifact metadata");
        assert!(
            normal_data_staging::inspect_stage(&root, &staged.stage_id).is_ok(),
            "semantic-digest-consistent serialization passes isolated scan"
        );
        assert!(
            normal_data_staged_policy::inspect_authorized_staged_run(
                &fixture.path,
                &root,
                &staged.stage_id,
                &first_intent,
                Some(&principal),
            )
            .is_err(),
            "exact result JSON byte digest must come from run database"
        );
        fs::write(&manifest_path, &original_manifest).expect("restore manifest");
        fs::write(&result_path, &original_result).expect("restore original JSON");
        assert!(
            normal_data_staged_policy::inspect_authorized_staged_run(
                &fixture.path,
                &root,
                &staged.stage_id,
                &first_intent,
                Some(&principal),
            )
            .is_ok(),
            "original restored bytes pass again"
        );

        conn.execute(
            "UPDATE normal_data_permission_grants SET revoked_at_ms = 2
             WHERE subject_id = ?1
               AND permission = 'MODIFY_WORKPAPER_REVISION'",
            [&subject],
        )
        .expect("fixture grant revocation");
        assert!(
            normal_data_staged_policy::inspect_authorized_staged_run(
                &fixture.path,
                &root,
                &staged.stage_id,
                &first_intent,
                Some(&principal),
            )
            .is_err(),
            "revocation invalidates next transactional check"
        );
        conn.execute(
            "INSERT INTO normal_data_permission_grants (
                grant_id, subject_id, permission, resource_id, granted_at_ms
             ) VALUES (?1, ?2, 'MODIFY_WORKPAPER_REVISION', ?3, 3)",
            params![Uuid::new_v4().to_string(), &subject, &target.revision_id],
        )
        .expect("new fixture grant");
        conn.execute(
            "UPDATE workpapers SET workflow_state = 'SUBMITTED_FOR_REVIEW'
             WHERE workpaper_id = ?1",
            [&target.workpaper_id],
        )
        .expect("target moves to review");
        assert!(
            normal_data_staged_policy::inspect_authorized_staged_run(
                &fixture.path,
                &root,
                &staged.stage_id,
                &first_intent,
                Some(&principal),
            )
            .is_err(),
            "review state denies after regrant"
        );
        assert!(
            normal_data_staging::inspect_stage(&root, &staged.stage_id).is_ok(),
            "revocation or workflow state never mutates staging bytes"
        );
        let audit_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM audit_events", [], |row| row.get(0))
            .expect("audit count");
        let retained: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM controlled_evidence_versions",
                [],
                |row| row.get(0),
            )
            .expect("evidence count");
        let links: i64 = conn
            .query_row("SELECT COUNT(*) FROM workpaper_evidence_links", [], |row| {
                row.get(0)
            })
            .expect("attachment count");
        assert_eq!(
            baseline, audit_count,
            "inspection cannot record an audit event"
        );
        assert_eq!((retained, links), (0, 0), "inspection never promotes");
    }

    #[test]
    fn recovery_journal_observes_valid_ready_stage_then_corruption_without_evidence() {
        use crate::{
            normal_data_stage_recovery_journal as recovery_journal,
            normal_data_staging::{self, StageStatus},
        };
        let fixture = Fixture::new();
        let run = execute_comparison(&fixture.path, &fixture.recipe_version_id)
            .expect("verified comparison run");
        let material = crate::normal_data_preservation_material::prepare_preservation_material(
            &fixture.path,
            &run.normal_data_comparison_run_id,
        )
        .expect("exact three artifacts");
        let root = fixture.folder.join("recovery-journal-root");
        fs::create_dir(&root).expect("private root");
        let staged = normal_data_staging::stage_material(&root, &material)
            .expect("stage verified exact bytes");
        let first = recovery_journal::record_recovery_snapshot(&fixture.path, &root)
            .expect("record initial recovery observation");
        assert_eq!(first.entries.len(), 1);
        assert_eq!(first.entries[0].stage_id, staged.stage_id);
        assert_eq!(first.entries[0].status, StageStatus::ReadyVerified);
        assert_eq!(
            recovery_journal::read_recovery_snapshot(&fixture.path, &first.recovery_scan_id)
                .expect("reopen snapshot"),
            first,
        );

        let source_path = root
            .join(format!("{}.ready", staged.stage_id))
            .join("source-b.bin");
        let mut bytes = fs::read(&source_path).expect("staged source B");
        bytes[0] ^= 1;
        fs::write(source_path, bytes).expect("simulate same-size corruption");
        let second = recovery_journal::record_recovery_snapshot(&fixture.path, &root)
            .expect("record changed stage observation");
        assert_eq!(second.entries.len(), 1);
        assert_eq!(second.entries[0].status, StageStatus::Corrupt);
        assert_eq!(
            recovery_journal::read_recovery_snapshot(&fixture.path, &first.recovery_scan_id)
                .expect("historical scan immutable"),
            first,
        );
        let conn = persistence::open_configured_connection(&fixture.path).expect("database");
        for table in ["workpaper_evidence_links", "controlled_evidence_versions"] {
            let count: i64 = conn
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .expect("no formal retained evidence created");
            assert_eq!(count, 0);
        }
    }

    #[test]
    fn retention_candidate_preserves_exact_three_artifacts_without_promotion() {
        use crate::normal_data_retention_store::{
            self as retention_store, RetentionCandidateStatus,
        };
        let fixture = Fixture::new();
        let run = execute_comparison(&fixture.path, &fixture.recipe_version_id)
            .expect("verified comparison run");
        let material = crate::normal_data_preservation_material::prepare_preservation_material(
            &fixture.path,
            &run.normal_data_comparison_run_id,
        )
        .expect("exact three artifacts");
        let staging_root = fixture.folder.join("candidate-staging");
        let retention_root = fixture.folder.join("candidate-retention");
        fs::create_dir(&staging_root).expect("staging root");
        fs::create_dir(&retention_root).expect("retention root");
        let staged = crate::normal_data_staging::stage_material(&staging_root, &material)
            .expect("verified stage");

        let retained = retention_store::retain_staged_candidate(
            &fixture.path,
            &staging_root,
            &retention_root,
            &staged.stage_id,
            &run.normal_data_comparison_run_id,
        )
        .expect("retention candidate");
        assert_eq!(retained.stage_id, staged.stage_id);
        assert_eq!(retained.run_id, run.normal_data_comparison_run_id);
        assert_eq!(
            retained.result_artifact_sha256_hex,
            material.result.artifact_sha256_hex
        );
        assert_eq!(
            retained.result_semantic_sha256_hex,
            material.result.semantic_result_sha256_hex
        );

        let folder = retention_root.join(format!("{}.candidate", staged.stage_id));
        assert_eq!(
            fs::read(folder.join("source-a.bin")).expect("retained source A"),
            material.source_a.bytes
        );
        assert_eq!(
            fs::read(folder.join("source-b.bin")).expect("retained source B"),
            material.source_b.bytes
        );
        assert_eq!(
            fs::read(folder.join("result.json")).expect("retained result"),
            material.result.bytes
        );
        let inventory = retention_store::scan_retention_candidates(&fixture.path, &retention_root)
            .expect("candidate inventory");
        assert_eq!(inventory.len(), 1);
        assert_eq!(inventory[0].stage_id, staged.stage_id);
        assert_eq!(inventory[0].status, RetentionCandidateStatus::RecordedValid);
        let initial_snapshot =
            crate::normal_data_retention_recovery_journal::record_retention_recovery_snapshot(
                &fixture.path,
                &retention_root,
            )
            .expect("append registered-valid recovery observation");
        assert_eq!(initial_snapshot.entries, inventory);

        let second = retention_store::retain_staged_candidate(
            &fixture.path,
            &staging_root,
            &retention_root,
            &staged.stage_id,
            &run.normal_data_comparison_run_id,
        )
        .expect("idempotent retry");
        assert_eq!(second, retained);

        let connection = persistence::open_configured_connection(&fixture.path).expect("database");
        let candidate_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM normal_data_retention_candidates",
                [],
                |row| row.get(0),
            )
            .expect("candidate count");
        assert_eq!(candidate_count, 1);
        for table in [
            "controlled_evidence_versions",
            "workpaper_evidence_links",
            "normal_data_permission_subjects",
            "normal_data_permission_grants",
        ] {
            let count: i64 = connection
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .expect("non-promotion count");
            assert_eq!(count, 0, "{table} remains empty");
        }
        assert!(
            connection
                .execute(
                    "UPDATE normal_data_retention_candidates SET retained_at_ms = retained_at_ms + 1
                     WHERE stage_id = ?1",
                    [&staged.stage_id],
                )
                .is_err(),
            "candidate registration is immutable"
        );
        assert!(
            connection
                .execute(
                    "DELETE FROM normal_data_retention_candidates WHERE stage_id = ?1",
                    [&staged.stage_id],
                )
                .is_err(),
            "candidate registration cannot be deleted"
        );

        let source_path = folder.join("source-a.bin");
        let mut tampered = fs::read(&source_path).expect("candidate source");
        tampered[0] ^= 1;
        fs::write(&source_path, tampered).expect("fault-inject retained corruption");
        let inventory = retention_store::scan_retention_candidates(&fixture.path, &retention_root)
            .expect("corruption inventory");
        assert_eq!(inventory[0].status, RetentionCandidateStatus::Corrupt);
        let journal =
            crate::normal_data_retention_recovery_journal::record_retention_recovery_snapshot(
                &fixture.path,
                &retention_root,
            )
            .expect("append corruption observation");
        assert_eq!(journal.entries, inventory);
        assert_eq!(
            crate::normal_data_retention_recovery_journal::read_retention_recovery_snapshot(
                &fixture.path,
                &initial_snapshot.recovery_scan_id,
            )
            .expect("old status never rewritten"),
            initial_snapshot,
        );
        assert!(
            retention_store::recover_retained_candidate(
                &fixture.path,
                &retention_root,
                &staged.stage_id,
            )
            .is_err(),
            "corrupt retained bytes cannot be re-registered"
        );
    }

    #[test]
    fn retention_candidate_bound_to_live_grants_exact_run_and_unreviewed_target() {
        use crate::{
            normal_data_retention_policy, normal_data_retention_store as retention_store,
            normal_data_staging,
        };
        use promotion_policy::{PromotionIntent, VerifiedPrincipal};
        let fixture = Fixture::new();
        let first = execute_comparison(&fixture.path, &fixture.recipe_version_id)
            .expect("first frozen run");
        let second = execute_comparison(&fixture.path, &fixture.recipe_version_id)
            .expect("second distinct run");
        let material = crate::normal_data_preservation_material::prepare_preservation_material(
            &fixture.path,
            &first.normal_data_comparison_run_id,
        )
        .expect("three exact materials");
        let staging_root = fixture.folder.join("bound-candidate-stage");
        let retention_root = fixture.folder.join("bound-candidate-retention");
        fs::create_dir(&staging_root).expect("stage root");
        fs::create_dir(&retention_root).expect("retention root");
        let stage =
            normal_data_staging::stage_material(&staging_root, &material).expect("ready stage");
        let retained = retention_store::retain_staged_candidate(
            &fixture.path,
            &staging_root,
            &retention_root,
            &stage.stage_id,
            &first.normal_data_comparison_run_id,
        )
        .expect("registered candidate");
        let target = create_policy_target(&fixture.path, "DRAFT");
        let subject = Uuid::new_v4().to_string();
        let principal = VerifiedPrincipal::fixture(&subject);
        let intent = PromotionIntent {
            run_id: &first.normal_data_comparison_run_id,
            expected_workspace_id: &material.workspace_id,
            target_engagement_id: &target.engagement_id,
            target_workpaper_id: &target.workpaper_id,
            target_workpaper_revision_id: &target.revision_id,
        };
        let second_intent = PromotionIntent {
            run_id: &second.normal_data_comparison_run_id,
            ..intent
        };
        let inspect = |request: &PromotionIntent<'_>, actor: Option<&VerifiedPrincipal>| {
            normal_data_retention_policy::inspect_authorized_retention_candidate(
                &fixture.path,
                &retention_root,
                &stage.stage_id,
                request,
                actor,
            )
        };
        let conn = persistence::open_configured_connection(&fixture.path).expect("database");
        let audit_before: i64 = conn
            .query_row("SELECT COUNT(*) FROM audit_events", [], |row| row.get(0))
            .expect("audit count");
        assert!(inspect(&intent, None).is_err(), "missing identity denies");
        conn.execute(
            "INSERT INTO normal_data_permission_subjects
             (subject_id, identity_issuer, registered_at_ms)
             VALUES (?1, 'TRUSTED_NATIVE_IDP', 1)",
            [&subject],
        )
        .expect("test enrolled subject");
        assert!(
            inspect(&intent, Some(&principal)).is_err(),
            "no grants deny"
        );
        for (permission, resource) in [
            ("READ_NORMAL_DATA_WORKSPACE", &material.workspace_id),
            ("ATTACH_EVIDENCE_TO_ENGAGEMENT", &target.engagement_id),
            ("MODIFY_WORKPAPER_REVISION", &target.revision_id),
        ] {
            conn.execute(
                "INSERT INTO normal_data_permission_grants
                 (grant_id, subject_id, permission, resource_id, granted_at_ms)
                 VALUES (?1, ?2, ?3, ?4, 1)",
                params![Uuid::new_v4().to_string(), &subject, permission, resource],
            )
            .expect("fixture grant");
        }
        assert_eq!(
            inspect(&intent, Some(&principal)).expect("exact candidate and target"),
            retained,
        );
        let capture_preflight =
            |request: &PromotionIntent<'_>, actor: Option<&VerifiedPrincipal>| {
                crate::normal_data_capture_preflight::inspect_exact_capture_preflight(
                    &fixture.path,
                    &retention_root,
                    &stage.stage_id,
                    request,
                    actor,
                )
            };
        assert!(
            capture_preflight(&intent, None).is_err(),
            "unverified identity never receives capture preflight metadata"
        );
        let preflight = capture_preflight(&intent, Some(&principal))
            .expect("exact source A, source B and result preflight");
        let capture_material =
            |request: &PromotionIntent<'_>, actor: Option<&VerifiedPrincipal>| {
                crate::normal_data_capture_preflight::read_exact_capture_material(
                    &fixture.path,
                    &retention_root,
                    &stage.stage_id,
                    request,
                    actor,
                )
            };
        assert!(
            capture_material(&intent, None).is_err(),
            "working bytes are unavailable without trusted principal"
        );
        let exact_bytes = capture_material(&intent, Some(&principal))
            .expect("one exact transaction-bound three-buffer read");
        assert_eq!(exact_bytes.preflight, preflight);
        assert_eq!(exact_bytes.original_a_bytes, material.source_a.bytes);
        assert_eq!(exact_bytes.original_b_bytes, material.source_b.bytes);
        assert_eq!(exact_bytes.frozen_result_json_bytes, material.result.bytes);
        assert_eq!(preflight.candidate_stage_id, stage.stage_id);
        assert_eq!(preflight.run_id, first.normal_data_comparison_run_id);
        assert_eq!(preflight.workspace_id, material.workspace_id);
        assert_eq!(preflight.recipe_version_id, material.recipe_version_id);
        assert_eq!(preflight.target_engagement_id, target.engagement_id);
        assert_eq!(preflight.target_workpaper_id, target.workpaper_id);
        assert_eq!(preflight.target_workpaper_revision_id, target.revision_id);
        assert_eq!(
            preflight.artifacts.original_a.dataset_version_id,
            material.source_a.dataset_version_id
        );
        assert_eq!(
            preflight.artifacts.original_a.document_id,
            material.source_a.document_id
        );
        assert_eq!(
            preflight.artifacts.original_a.content_version_id,
            material.source_a.content_version_id
        );
        assert_eq!(
            preflight.artifacts.original_a.sha256_hex,
            material.source_a.sha256_hex
        );
        assert_eq!(
            preflight.artifacts.original_a.size_bytes,
            material.source_a.bytes.len() as u64
        );
        assert_eq!(
            preflight.artifacts.original_b.dataset_version_id,
            material.source_b.dataset_version_id
        );
        assert_eq!(
            preflight.artifacts.original_b.document_id,
            material.source_b.document_id
        );
        assert_eq!(
            preflight.artifacts.original_b.content_version_id,
            material.source_b.content_version_id
        );
        assert_eq!(
            preflight.artifacts.original_b.sha256_hex,
            material.source_b.sha256_hex
        );
        assert_eq!(
            preflight.artifacts.original_b.size_bytes,
            material.source_b.bytes.len() as u64
        );
        assert_eq!(
            preflight.artifacts.frozen_result.artifact_sha256_hex,
            material.result.artifact_sha256_hex
        );
        assert_eq!(
            preflight.artifacts.frozen_result.semantic_sha256_hex,
            material.result.semantic_result_sha256_hex
        );
        assert_eq!(
            preflight.artifacts.frozen_result.size_bytes,
            material.result.bytes.len() as u64
        );
        assert!(
            capture_preflight(&second_intent, Some(&principal)).is_err(),
            "same bytes from a different frozen run cannot be rebound"
        );
        assert!(
            capture_material(&second_intent, Some(&principal)).is_err(),
            "raw candidate bytes cannot be rebound to a different frozen run"
        );
        assert!(
            inspect(&second_intent, Some(&principal)).is_err(),
            "same source hashes but different run ID deny"
        );

        let folder = retention_root.join(format!("{}.candidate", stage.stage_id));
        let result_path = folder.join("result.json");
        let manifest_path = folder.join("candidate-manifest.json");
        let original_result = fs::read(&result_path).expect("result bytes");
        let original_manifest = fs::read(&manifest_path).expect("manifest bytes");
        let mut altered = original_result.clone();
        altered.push(b' ');
        fs::write(&result_path, &altered).expect("tamper result serialization");
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&original_manifest).expect("manifest JSON");
        manifest["result"]["size_bytes"] = serde_json::Value::from(altered.len() as u64);
        manifest["result"]["sha256_hex"] = serde_json::Value::from(hex(&Sha256::digest(&altered)));
        fs::write(&manifest_path, serde_json::to_vec(&manifest).expect("JSON"))
            .expect("forge matching self-reported hash");
        assert!(
            capture_material(&intent, Some(&principal)).is_err(),
            "rewritten manifest cannot authorize altered result bytes"
        );
        assert!(
            capture_preflight(&intent, Some(&principal)).is_err(),
            "artifact-byte tampering rejects capture preflight even with rewritten manifest"
        );
        assert!(
            inspect(&intent, Some(&principal)).is_err(),
            "self-consistent but nonhistorical result serialization denies"
        );
        fs::write(&manifest_path, &original_manifest).expect("restore manifest");
        fs::write(&result_path, &original_result).expect("restore exact result");

        let source_a_path = folder.join("source-a.bin");
        let mut source_a = fs::read(&source_a_path).expect("original A bytes");
        source_a[0] ^= 1;
        fs::write(&source_a_path, &source_a).expect("same-size A edit");
        assert!(
            capture_material(&intent, Some(&principal)).is_err(),
            "modified original A fails exact bound byte read"
        );
        source_a[0] ^= 1;
        fs::write(&source_a_path, source_a).expect("restore original A");

        let source_path = folder.join("source-b.bin");
        let mut source = fs::read(&source_path).expect("source bytes");
        source[0] ^= 1;
        fs::write(&source_path, &source).expect("same-size source edit");
        assert!(
            capture_material(&intent, Some(&principal)).is_err(),
            "modified original B fails exact bound byte read"
        );
        assert!(
            capture_preflight(&intent, Some(&principal)).is_err(),
            "same-size source tampering denies the three-artifact preflight"
        );
        assert!(
            inspect(&intent, Some(&principal)).is_err(),
            "byte tamper denies"
        );
        source[0] ^= 1;
        fs::write(&source_path, source).expect("restore source");
        let missing = retention_root.join(format!("{}.missing", stage.stage_id));
        fs::rename(&folder, &missing).expect("hide registered package");
        assert!(
            capture_material(&intent, Some(&principal)).is_err(),
            "missing recorded candidate yields no exact working buffers"
        );
        assert!(
            capture_preflight(&intent, Some(&principal)).is_err(),
            "missing retained bytes deny preflight"
        );
        assert!(
            inspect(&intent, Some(&principal)).is_err(),
            "missing package denies"
        );
        fs::rename(&missing, &folder).expect("restore package");
        assert!(
            inspect(&intent, Some(&principal)).is_ok(),
            "restored bytes revalidate"
        );

        conn.execute(
            "UPDATE normal_data_permission_grants SET revoked_at_ms = 2
             WHERE subject_id = ?1 AND permission = 'MODIFY_WORKPAPER_REVISION'",
            [&subject],
        )
        .expect("revoke grant");
        assert!(
            capture_material(&intent, Some(&principal)).is_err(),
            "revoked permission blocks fresh working-data bytes"
        );
        assert!(
            capture_preflight(&intent, Some(&principal)).is_err(),
            "revoked scoped permission denies fresh preflight"
        );
        assert!(
            inspect(&intent, Some(&principal)).is_err(),
            "revocation denies"
        );
        conn.execute(
            "INSERT INTO normal_data_permission_grants
             (grant_id, subject_id, permission, resource_id, granted_at_ms)
             VALUES (?1, ?2, 'MODIFY_WORKPAPER_REVISION', ?3, 3)",
            params![Uuid::new_v4().to_string(), &subject, &target.revision_id],
        )
        .expect("regrant fixture");
        conn.execute(
            "UPDATE workpapers SET workflow_state = 'SUBMITTED_FOR_REVIEW'
             WHERE workpaper_id = ?1",
            [&target.workpaper_id],
        )
        .expect("advance review");
        assert!(
            capture_material(&intent, Some(&principal)).is_err(),
            "submitted-for-review workpaper blocks fresh working-data bytes"
        );
        assert!(
            capture_preflight(&intent, Some(&principal)).is_err(),
            "submitted-for-review specialist target denies preflight"
        );
        assert!(
            inspect(&intent, Some(&principal)).is_err(),
            "reviewed target denies"
        );
        for table in ["controlled_evidence_versions", "workpaper_evidence_links"] {
            let count: i64 = conn
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .expect("zero formal evidence");
            assert_eq!(count, 0, "{table} remains unchanged");
        }
        let audit_after: i64 = conn
            .query_row("SELECT COUNT(*) FROM audit_events", [], |row| row.get(0))
            .expect("audit");
        assert_eq!(
            audit_before, audit_after,
            "read-only policy never records approval"
        );
    }

    #[test]
    fn retention_candidate_recovers_orphan_and_reports_missing_or_interrupted_state() {
        use crate::normal_data_retention_store::{
            self as retention_store, RetentionCandidateStatus,
        };
        let fixture = Fixture::new();
        let run = execute_comparison(&fixture.path, &fixture.recipe_version_id)
            .expect("verified comparison run");
        let material = crate::normal_data_preservation_material::prepare_preservation_material(
            &fixture.path,
            &run.normal_data_comparison_run_id,
        )
        .expect("exact artifacts");
        let staging_root = fixture.folder.join("orphan-staging");
        let retention_root = fixture.folder.join("orphan-retention");
        fs::create_dir(&staging_root).expect("staging root");
        fs::create_dir(&retention_root).expect("retention root");
        let staged = crate::normal_data_staging::stage_material(&staging_root, &material)
            .expect("verified stage");

        let connection = persistence::open_configured_connection(&fixture.path).expect("database");
        connection
            .execute_batch(
                "CREATE TRIGGER test_block_retention_candidate
                 BEFORE INSERT ON normal_data_retention_candidates
                 BEGIN SELECT RAISE(ABORT, 'simulated ledger failure'); END;",
            )
            .expect("inject registration failure");
        assert!(
            retention_store::retain_staged_candidate(
                &fixture.path,
                &staging_root,
                &retention_root,
                &staged.stage_id,
                &run.normal_data_comparison_run_id,
            )
            .is_err(),
            "ledger failure must surface after candidate publication"
        );
        assert!(
            retention_root
                .join(format!("{}.candidate", staged.stage_id))
                .exists(),
            "published candidate survives database failure for recovery"
        );
        let count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM normal_data_retention_candidates",
                [],
                |row| row.get(0),
            )
            .expect("no record");
        assert_eq!(count, 0);
        let inventory = retention_store::scan_retention_candidates(&fixture.path, &retention_root)
            .expect("orphan inventory");
        assert_eq!(inventory[0].status, RetentionCandidateStatus::OrphanValid);
        let orphan_snapshot =
            crate::normal_data_retention_recovery_journal::record_retention_recovery_snapshot(
                &fixture.path,
                &retention_root,
            )
            .expect("append orphan observation");
        assert_eq!(orphan_snapshot.entries, inventory);

        connection
            .execute_batch("DROP TRIGGER test_block_retention_candidate;")
            .expect("restore ledger writes");
        assert!(
            retention_store::inspect_registered_candidate_on_connection(
                &connection,
                &retention_root,
                &staged.stage_id,
                &run.normal_data_comparison_run_id,
            )
            .is_err(),
            "valid orphan still cannot pass registered-candidate inspection"
        );
        assert!(
            retention_store::read_registered_candidate_bytes_on_connection(
                &connection,
                &retention_root,
                &staged.stage_id,
                &run.normal_data_comparison_run_id,
            )
            .is_err(),
            "an unregistered orphan can never produce trusted capture buffers"
        );
        let recovered = retention_store::recover_retained_candidate(
            &fixture.path,
            &retention_root,
            &staged.stage_id,
        )
        .expect("register exact orphan");
        assert_eq!(
            retention_store::inspect_registered_candidate_on_connection(
                &connection,
                &retention_root,
                &staged.stage_id,
                &run.normal_data_comparison_run_id,
            )
            .expect("registered candidate revalidated"),
            recovered,
        );
        assert_eq!(recovered.stage_id, staged.stage_id);
        let inventory = retention_store::scan_retention_candidates(&fixture.path, &retention_root)
            .expect("recorded inventory");
        assert_eq!(inventory[0].status, RetentionCandidateStatus::RecordedValid);
        let drift =
            crate::normal_data_retention_recovery_journal::compare_retention_recovery_snapshot_to_live(
                &fixture.path,
                &retention_root,
                &orphan_snapshot.recovery_scan_id,
            )
            .expect("registration changes live recovery state");
        assert_eq!(drift.len(), 1);
        assert_eq!(
            drift[0].recorded,
            Some(RetentionCandidateStatus::OrphanValid)
        );
        assert_eq!(
            drift[0].observed_now,
            Some(RetentionCandidateStatus::RecordedValid)
        );

        fs::remove_dir_all(retention_root.join(format!("{}.candidate", staged.stage_id)))
            .expect("simulate filesystem loss after database commit");
        let inventory = retention_store::scan_retention_candidates(&fixture.path, &retention_root)
            .expect("missing inventory");
        assert_eq!(inventory.len(), 1);
        assert_eq!(
            inventory[0].status,
            RetentionCandidateStatus::RecordedMissing
        );
        let missing_snapshot =
            crate::normal_data_retention_recovery_journal::record_retention_recovery_snapshot(
                &fixture.path,
                &retention_root,
            )
            .expect("append recorded-missing observation");
        assert_eq!(missing_snapshot.entries, inventory);
        assert_eq!(
            crate::normal_data_retention_recovery_journal::read_retention_recovery_snapshot(
                &fixture.path,
                &orphan_snapshot.recovery_scan_id,
            )
            .expect("previous orphan state still immutable"),
            orphan_snapshot,
        );

        let interrupted = Uuid::new_v4().to_string();
        fs::create_dir(retention_root.join(format!("{interrupted}.partial")))
            .expect("simulate interrupted publication");
        let inventory = retention_store::scan_retention_candidates(&fixture.path, &retention_root)
            .expect("interrupted inventory");
        assert!(inventory.iter().any(|entry| {
            entry.stage_id == interrupted && entry.status == RetentionCandidateStatus::Interrupted
        }));
        retention_store::discard_interrupted_candidate(&retention_root, &interrupted)
            .expect("discard only exact interrupted candidate");
        assert!(
            retention_store::discard_interrupted_candidate(&retention_root, &staged.stage_id,)
                .is_err(),
            "published or missing recorded candidate is not deletable as partial"
        );
    }

    #[test]
    fn staging_denies_modified_material_before_writing_any_package() {
        let fixture = Fixture::new();
        let run = execute_comparison(&fixture.path, &fixture.recipe_version_id)
            .expect("verified comparison run");
        let mut material = crate::normal_data_preservation_material::prepare_preservation_material(
            &fixture.path,
            &run.normal_data_comparison_run_id,
        )
        .expect("exact material");
        let root = fixture.folder.join("private-staging-reject");
        fs::create_dir(&root).expect("dedicated staging root");
        material.source_a.bytes[0] ^= 1;
        assert!(crate::normal_data_staging::stage_material(&root, &material).is_err());
        assert_eq!(fs::read_dir(&root).expect("staging root").count(), 0);
        material.source_a.bytes[0] ^= 1;
        material.result.bytes.push(b' ');
        assert!(
            crate::normal_data_staging::stage_material(&root, &material).is_err(),
            "result artifact hash prevents silent whitespace change"
        );
        assert_eq!(fs::read_dir(&root).expect("staging root").count(), 0);
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
