//! Immutable Normal Data dataset source bindings and user-declared column semantics.
//! This module never accepts a filesystem path, reads source bytes, or claims that
//! a metadata fingerprint is equivalent to a verified content hash.
use crate::persistence::{self, PersistenceError};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::Serialize;
use serde_json::json;
use std::fmt::Write;
use std::path::Path;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatasetRecord {
    pub normal_data_dataset_id: String,
    pub normal_data_workspace_id: String,
    pub normal_data_dataset_version_id: String,
    pub version_number: i64,
    pub name: String,
    pub document_id: String,
    pub file_instance_id: String,
    pub content_version_id: String,
    pub source_observed_at_ms: i64,
    pub source_size_bytes: i64,
    pub source_verification_state: String,
    pub source_stable_during_read: Option<bool>,
    pub source_fingerprint_present: bool,
    pub source_sha256_hex: Option<String>,
    pub created_at_ms: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnSemanticRecord {
    pub normal_data_column_semantic_id: String,
    pub normal_data_dataset_version_id: String,
    pub column_name: String,
    pub semantic_role: String,
    pub data_type: String,
    pub created_at_ms: i64,
}

fn invalid(message: impl Into<String>) -> PersistenceError {
    PersistenceError::Configuration(message.into())
}

fn label(value: &str, field_name: &str) -> Result<String, PersistenceError> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.chars().count() > 240
        || trimmed.chars().any(char::is_control)
    {
        return Err(invalid(format!("{field_name} must contain 1 to 240 printable characters")));
    }
    Ok(trimmed.to_string())
}

fn as_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut output, "{byte:02x}").expect("writing to a String cannot fail");
    }
    output
}

fn dataset_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<DatasetRecord> {
    let stable: Option<i64> = row.get(11)?;
    let fingerprint: Option<Vec<u8>> = row.get(12)?;
    let sha: Option<Vec<u8>> = row.get(13)?;
    Ok(DatasetRecord {
        normal_data_dataset_id: row.get(0)?,
        normal_data_workspace_id: row.get(1)?,
        normal_data_dataset_version_id: row.get(2)?,
        version_number: row.get(3)?,
        name: row.get(4)?,
        document_id: row.get(5)?,
        file_instance_id: row.get(6)?,
        content_version_id: row.get(7)?,
        source_observed_at_ms: row.get(8)?,
        source_size_bytes: row.get(9)?,
        source_verification_state: row.get(10)?,
        source_stable_during_read: stable.map(|x| x != 0),
        source_fingerprint_present: fingerprint.is_some(),
        source_sha256_hex: sha.as_deref().map(as_hex),
        created_at_ms: row.get(14)?,
    })
}

pub fn create_dataset(
    database_path: &Path, workspace_id: &str, file_instance_id: &str, name: &str,
) -> Result<DatasetRecord, PersistenceError> {
    let name = label(name, "dataset name")?;
    let mut connection = persistence::open_configured_connection(database_path)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

    let workspace_exists: bool = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM normal_data_workspaces WHERE normal_data_workspace_id = ?1)",
        [workspace_id], |row| row.get(0),
    )?;
    if !workspace_exists {
        return Err(invalid("normal data workspace does not exist"));
    }

    // The backend, not the frontend, selects the newest indexed content version.
    // The record freezes the observation and its verification status without
    // pretending that a linked file is retained or re-readable historically.
    let source: Option<(String, String, i64, i64, String, Option<i64>, Option<Vec<u8>>, Option<Vec<u8>>)> =
        transaction.query_row(
            "SELECT fi.document_id, cv.content_version_id, cv.observed_at_ms,
                    cv.size_bytes, cv.verification_state, cv.source_stable_during_read,
                    cv.quick_fingerprint, cv.sha256
             FROM file_instances fi
             JOIN documents d ON d.document_id = fi.document_id
             JOIN content_versions cv
               ON cv.file_instance_id = fi.file_instance_id
              AND cv.document_id = fi.document_id
             WHERE fi.file_instance_id = ?1
               AND fi.availability_state = 'AVAILABLE'
               AND d.archived_at_ms IS NULL
             ORDER BY cv.observed_at_ms DESC, cv.rowid DESC
             LIMIT 1",
            [file_instance_id],
            |row| Ok((
                row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?,
                row.get(4)?, row.get(5)?, row.get(6)?, row.get(7)?
            )),
        ).optional()?;
    let Some((document_id, content_version_id, observed_at, size_bytes,
        verification, stable, fingerprint, sha)) = source else {
        return Err(invalid("file instance has no available indexed content version"));
    };
    if stable == Some(0) || verification == "FAILED"
        || (verification == "HASH_VERIFIED" && sha.as_ref().is_none_or(|hash| hash.len() != 32))
    {
        return Err(invalid("indexed content version cannot be bound as a working source"));
    }

    let dataset_id = Uuid::new_v4().to_string();
    let version_id = Uuid::new_v4().to_string();
    let now = persistence::now_unix_ms()?;
    transaction.execute(
        "INSERT INTO normal_data_datasets (
            normal_data_dataset_id, normal_data_workspace_id, name, created_at_ms
         ) VALUES (?1, ?2, ?3, ?4)",
        params![&dataset_id, workspace_id, &name, now],
    )?;
    transaction.execute(
        "INSERT INTO normal_data_dataset_versions (
            normal_data_dataset_version_id, normal_data_dataset_id, version_number,
            document_id, file_instance_id, content_version_id, source_observed_at_ms,
            source_size_bytes, source_verification_state, source_stable_during_read,
            source_quick_fingerprint, source_sha256, created_at_ms
        ) VALUES (?1, ?2, 1, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            &version_id, &dataset_id, &document_id, file_instance_id,
            &content_version_id, observed_at, size_bytes, &verification,
            stable, &fingerprint, &sha, now
        ],
    )?;
    transaction.execute(
        "INSERT INTO audit_events (
            audit_event_id, event_type, entity_type, entity_id,
            related_entity_type, related_entity_id, occurred_at_ms,
            actor_id, details_json
         ) VALUES (?1, 'NORMAL_DATA_DATASET_CREATED', 'NORMAL_DATA_DATASET',
                   ?2, 'NORMAL_DATA_WORKSPACE', ?3, ?4, NULL, ?5)",
        params![
            Uuid::new_v4().to_string(), &dataset_id, workspace_id, now,
            json!({"dataset_version_id": version_id, "content_version_id": content_version_id,
                "source_verification_state": verification}).to_string()
        ],
    )?;
    transaction.commit()?;
    Ok(DatasetRecord {
        normal_data_dataset_id: dataset_id,
        normal_data_workspace_id: workspace_id.to_string(),
        normal_data_dataset_version_id: version_id,
        version_number: 1,
        name,
        document_id,
        file_instance_id: file_instance_id.to_string(),
        content_version_id,
        source_observed_at_ms: observed_at,
        source_size_bytes: size_bytes,
        source_verification_state: verification,
        source_stable_during_read: stable.map(|value| value != 0),
        source_fingerprint_present: fingerprint.is_some(),
        source_sha256_hex: sha.as_deref().map(as_hex),
        created_at_ms: now,
    })
}

pub fn list_datasets(database_path: &Path, workspace_id: &str)
    -> Result<Vec<DatasetRecord>, PersistenceError>
{
    let connection = persistence::open_configured_connection(database_path)?;
    let workspace_exists: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM normal_data_workspaces WHERE normal_data_workspace_id = ?1)",
        [workspace_id], |row| row.get(0),
    )?;
    if !workspace_exists {
        return Err(invalid("normal data workspace does not exist"));
    }
    let mut statement = connection.prepare(
        "SELECT d.normal_data_dataset_id, d.normal_data_workspace_id,
                v.normal_data_dataset_version_id, v.version_number, d.name,
                v.document_id, v.file_instance_id, v.content_version_id,
                v.source_observed_at_ms, v.source_size_bytes, v.source_verification_state,
                v.source_stable_during_read, v.source_quick_fingerprint, v.source_sha256,
                d.created_at_ms
         FROM normal_data_datasets d
         JOIN normal_data_dataset_versions v
           ON v.normal_data_dataset_id = d.normal_data_dataset_id
          AND v.version_number = (
              SELECT MAX(v2.version_number) FROM normal_data_dataset_versions v2
              WHERE v2.normal_data_dataset_id = d.normal_data_dataset_id
          )
         WHERE d.normal_data_workspace_id = ?1
         ORDER BY d.created_at_ms DESC, d.normal_data_dataset_id"
    )?;
    let rows = statement.query_map([workspace_id], dataset_from_row)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn declare_column(
    database_path: &Path, dataset_version_id: &str, column_name: &str,
    semantic_role: &str, data_type: &str,
) -> Result<ColumnSemanticRecord, PersistenceError> {
    let column_name = label(column_name, "column name")?;
    const ROLES: &[&str] = &[
        "BUSINESS_KEY", "FILING_PERIOD", "INVOICE_DATE", "POSTING_DATE",
        "ACCOUNTING_PERIOD", "TRANSACTION_DATE", "NUMERIC_VALUE", "OTHER",
    ];
    if !ROLES.contains(&semantic_role) {
        return Err(invalid("unsupported column semantic role"));
    }
    if !["TEXT", "DATE", "PERIOD", "DECIMAL"].contains(&data_type) {
        return Err(invalid("unsupported declared column data type"));
    }
    let expected_type = match semantic_role {
        "FILING_PERIOD" | "ACCOUNTING_PERIOD" => Some("PERIOD"),
        "INVOICE_DATE" | "POSTING_DATE" | "TRANSACTION_DATE" => Some("DATE"),
        "NUMERIC_VALUE" => Some("DECIMAL"),
        _ => None,
    };
    if expected_type.is_some_and(|expected| data_type != expected) {
        return Err(invalid("declared data type is incompatible with semantic role"));
    }
    let mut connection = persistence::open_configured_connection(database_path)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let exists: bool = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM normal_data_dataset_versions
                       WHERE normal_data_dataset_version_id = ?1)",
        [dataset_version_id], |row| row.get(0),
    )?;
    if !exists {
        return Err(invalid("dataset version does not exist"));
    }
    let id = Uuid::new_v4().to_string();
    let now = persistence::now_unix_ms()?;
    transaction.execute(
        "INSERT INTO normal_data_column_semantics (
            normal_data_column_semantic_id, normal_data_dataset_version_id,
            column_name, semantic_role, data_type, created_at_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![&id, dataset_version_id, &column_name, semantic_role, data_type, now],
    )?;
    transaction.execute(
        "INSERT INTO audit_events (
            audit_event_id, event_type, entity_type, entity_id,
            related_entity_type, related_entity_id, occurred_at_ms,
            actor_id, details_json
         ) VALUES (?1, 'NORMAL_DATA_COLUMN_DECLARED', 'NORMAL_DATA_COLUMN_SEMANTIC',
                   ?2, 'NORMAL_DATA_DATASET_VERSION', ?3, ?4, NULL, ?5)",
        params![
            Uuid::new_v4().to_string(), &id, dataset_version_id, now,
            json!({"column_name": column_name, "semantic_role": semantic_role,
                "data_type": data_type}).to_string()
        ],
    )?;
    transaction.commit()?;
    Ok(ColumnSemanticRecord {
        normal_data_column_semantic_id: id,
        normal_data_dataset_version_id: dataset_version_id.to_string(),
        column_name,
        semantic_role: semantic_role.to_string(),
        data_type: data_type.to_string(),
        created_at_ms: now,
    })
}

pub fn list_columns(database_path: &Path, dataset_version_id: &str)
    -> Result<Vec<ColumnSemanticRecord>, PersistenceError>
{
    let connection = persistence::open_configured_connection(database_path)?;
    let exists: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM normal_data_dataset_versions
                       WHERE normal_data_dataset_version_id = ?1)",
        [dataset_version_id], |row| row.get(0),
    )?;
    if !exists {
        return Err(invalid("dataset version does not exist"));
    }
    let mut statement = connection.prepare(
        "SELECT normal_data_column_semantic_id, normal_data_dataset_version_id,
                column_name, semantic_role, data_type, created_at_ms
         FROM normal_data_column_semantics
         WHERE normal_data_dataset_version_id = ?1
         ORDER BY created_at_ms, normal_data_column_semantic_id"
    )?;
    let rows = statement.query_map([dataset_version_id], |row| {
        Ok(ColumnSemanticRecord {
            normal_data_column_semantic_id: row.get(0)?,
            normal_data_dataset_version_id: row.get(1)?,
            column_name: row.get(2)?,
            semantic_role: row.get(3)?,
            data_type: row.get(4)?,
            created_at_ms: row.get(5)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::normal_data;
    use std::fs;
    use std::path::PathBuf;

    struct Fixture { folder: PathBuf, path: PathBuf, file_instance_id: String }
    impl Fixture {
        fn new() -> Self {
            let folder = std::env::temp_dir()
                .join(format!("pdox-data-source-{}", Uuid::new_v4()));
            fs::create_dir_all(&folder).expect("create test directory");
            let path = folder.join("metadata.sqlite");
            persistence::initialize_database(&path).expect("initialize DB");
            let source_dir = folder.join("source");
            fs::create_dir_all(&source_dir).expect("create source directory");
            let canonical = fs::canonicalize(&source_dir).expect("canonical source");
            let root = persistence::register_storage_root(
                &path, "test-root", &source_dir, &canonical,
            ).expect("approve source root");
            let document_id = Uuid::new_v4().to_string();
            let file_instance_id = Uuid::new_v4().to_string();
            let version_id = Uuid::new_v4().to_string();
            let connection = persistence::open_configured_connection(&path)
                .expect("open connection");
            connection.execute(
                "INSERT INTO documents (
                    document_id, storage_state, display_name, created_at_ms
                 ) VALUES (?1, 'LINKED', 'Invoices.csv', 1)",
                [&document_id],
            ).expect("insert linked document");
            connection.execute(
                "INSERT INTO file_instances (
                    file_instance_id, document_id, storage_root_id, relative_path_native,
                    path_native_encoding, relative_path_display, relative_path_search,
                    size_bytes, first_seen_at_ms, last_seen_at_ms, availability_state
                 ) VALUES (?1, ?2, ?3, X'696E766F696365732E637376', 'test',
                           'Invoices.csv', 'invoices.csv', 64, 1, 1, 'AVAILABLE')",
                params![&file_instance_id, &document_id, &root.storage_root_id],
            ).expect("insert available linked identity");
            connection.execute(
                "INSERT INTO content_versions (
                    content_version_id, document_id, file_instance_id, observed_at_ms,
                    size_bytes, quick_fingerprint, sha256, verification_state,
                    source_stable_during_read
                 ) VALUES (?1, ?2, ?3, 50, 64, ?4, ?5, 'HASH_VERIFIED', 1)",
                params![
                    &version_id, &document_id, &file_instance_id,
                    vec![6u8; 32], vec![9u8; 32]
                ],
            ).expect("insert verified content version");
            Self { folder, path, file_instance_id }
        }
        fn workspace(&self) -> normal_data::WorkspaceRecord {
            normal_data::create_workspace(
                &self.path,
                normal_data::WorkspaceDefinition {
                    name: "Normal analysis", description: None, client_id: None,
                    period_start: None, period_end: None,
                },
            ).expect("create workspace")
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) { let _ = fs::remove_dir_all(&self.folder); }
    }

    #[test]
    fn source_binding_is_immutable_and_does_not_require_an_engagement() {
        let fixture = Fixture::new();
        let workspace = fixture.workspace();
        let dataset = create_dataset(
            &fixture.path, &workspace.normal_data_workspace_id,
            &fixture.file_instance_id, " Filing source ",
        ).expect("bind dataset");
        assert_eq!(dataset.name, "Filing source");
        assert_eq!(dataset.source_verification_state, "HASH_VERIFIED");
        assert_eq!(dataset.source_sha256_hex.as_deref(), Some("09".repeat(32).as_str()));
        assert!(dataset.source_fingerprint_present);
        let listed = list_datasets(&fixture.path, &workspace.normal_data_workspace_id)
            .expect("list datasets");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].content_version_id, dataset.content_version_id);
        let connection = persistence::open_configured_connection(&fixture.path)
            .expect("open DB");
        assert!(connection.execute(
            "UPDATE normal_data_dataset_versions SET source_size_bytes = 0
             WHERE normal_data_dataset_version_id = ?1",
            [&dataset.normal_data_dataset_version_id],
        ).is_err());
        assert!(connection.execute(
            "DELETE FROM normal_data_dataset_versions
             WHERE normal_data_dataset_version_id = ?1",
            [&dataset.normal_data_dataset_version_id],
        ).is_err());
        let count = persistence::count_audit_events_for_test(
            &fixture.path, "NORMAL_DATA_DATASET_CREATED", &dataset.normal_data_dataset_id,
        ).expect("dataset audit event");
        assert_eq!(count, 1);
    }

    #[test]
    fn explicit_period_roles_do_not_confuse_invoice_date_and_filing_month() {
        let fixture = Fixture::new();
        let workspace = fixture.workspace();
        let dataset = create_dataset(
            &fixture.path, &workspace.normal_data_workspace_id,
            &fixture.file_instance_id, "GST working dataset",
        ).expect("create dataset");
        let version = &dataset.normal_data_dataset_version_id;
        declare_column(&fixture.path, version, "Invoice Date", "INVOICE_DATE", "DATE")
            .expect("invoice date role");
        declare_column(&fixture.path, version, "Filing Period", "FILING_PERIOD", "PERIOD")
            .expect("filing period role");
        assert!(declare_column(&fixture.path, version, "Filing Period", "INVOICE_DATE", "DATE").is_err());
        assert!(declare_column(&fixture.path, version, "Posting Date", "POSTING_DATE", "PERIOD").is_err());
        let columns = list_columns(&fixture.path, version).expect("list semantic roles");
        assert_eq!(columns.len(), 2);
        assert!(columns.iter().any(|column| column.semantic_role == "INVOICE_DATE"));
        assert!(columns.iter().any(|column| column.semantic_role == "FILING_PERIOD"));
        let connection = persistence::open_configured_connection(&fixture.path)
            .expect("open DB");
        assert!(connection.execute(
            "DELETE FROM normal_data_column_semantics
             WHERE normal_data_dataset_version_id = ?1",
            [version],
        ).is_err());
    }

    #[test]
    fn invalid_source_rejected_without_mutating_workspace() {
        let fixture = Fixture::new();
        let workspace = fixture.workspace();
        let missing = Uuid::new_v4().to_string();
        assert!(create_dataset(
            &fixture.path, &workspace.normal_data_workspace_id, &missing, "Missing",
        ).is_err());
        assert!(list_datasets(&fixture.path, &workspace.normal_data_workspace_id)
            .expect("list after failure").is_empty());
    }
}
