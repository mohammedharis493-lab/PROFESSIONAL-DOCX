use crate::filesystem;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    error::Error,
    ffi::OsString,
    fmt, fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;

#[cfg(unix)]
use std::os::unix::ffi::{OsStrExt, OsStringExt};
#[cfg(windows)]
use std::os::windows::ffi::{OsStrExt, OsStringExt};

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);
const LATEST_SCHEMA_VERSION: i64 = 7;

struct Migration {
    version: i64,
    name: &'static str,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "initial",
        sql: include_str!("../migrations/0001_initial.sql"),
    },
    Migration {
        version: 2,
        name: "document_quick_access",
        sql: include_str!("../migrations/0002_document_quick_access.sql"),
    },
    Migration {
        version: 3,
        name: "recent_searches",
        sql: include_str!("../migrations/0003_recent_searches.sql"),
    },
    Migration {
        version: 4,
        name: "controlled_evidence",
        sql: include_str!("../migrations/0004_controlled_evidence.sql"),
    },
    Migration {
        version: 5,
        name: "document_relationships",
        sql: include_str!("../migrations/0005_document_relationships.sql"),
    },
    Migration {
        version: 6,
        name: "engagement_workpapers",
        sql: include_str!("../migrations/0006_engagement_workpapers.sql"),
    },
    Migration {
        version: 7,
        name: "review_workflow",
        sql: include_str!("../migrations/0007_review_workflow.sql"),
    },
];

#[derive(Debug, Clone)]
pub struct DatabaseState {
    path: PathBuf,
}

impl DatabaseState {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[derive(Debug, Clone)]
pub struct StorageRootRecord {
    pub storage_root_id: String,
    pub display_path: String,
    pub availability_state: String,
    pub canonical_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct IndexJobRecord {
    pub index_job_id: String,
    pub storage_root_id: String,
    pub job_type: String,
    pub status: String,
    pub requested_at_ms: i64,
    pub started_at_ms: Option<i64>,
    pub completed_at_ms: Option<i64>,
    pub cancel_requested_at_ms: Option<i64>,
    pub last_heartbeat_at_ms: Option<i64>,
    pub current_phase: Option<String>,
    pub directories_seen: u64,
    pub files_seen: u64,
    pub bytes_seen: u64,
    pub files_persisted: u64,
    pub errors_count: u64,
    pub scan_generation_id: Option<String>,
    pub failure_code: Option<String>,
    pub failure_message: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct IndexProgress {
    pub directories_seen: u64,
    pub files_seen: u64,
    pub bytes_seen: u64,
    pub files_persisted: u64,
    pub errors_count: u64,
}

#[derive(Debug, Clone)]
pub struct FileObservation {
    pub relative_path_native: Vec<u8>,
    pub path_native_encoding: String,
    pub relative_path_display: String,
    pub relative_path_search: String,
    pub display_name: String,
    pub size_bytes: u64,
    pub creation_time_ms: Option<i64>,
    pub last_write_time_ms: Option<i64>,
    pub filesystem_identity: Option<Vec<u8>>,
    pub volume_identity: Option<Vec<u8>>,
    pub file_attributes: Option<i64>,
    pub reparse_tag: Option<i64>,
    pub quick_fingerprint: Option<Vec<u8>>,
    pub source_stable_during_read: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct ScanErrorObservation {
    pub relative_path_native: Option<Vec<u8>>,
    pub path_native_encoding: Option<String>,
    pub relative_path_display: Option<String>,
    pub category: String,
    pub os_error_code: Option<i64>,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct IndexedFilePreviewRecord {
    pub document_id: String,
    pub file_instance_id: String,
    pub name: String,
    pub path: String,
    pub extension: String,
    pub size_bytes: u64,
    pub modified_unix_ms: Option<i64>,
    pub availability_state: String,
}

#[derive(Debug, Clone)]
pub struct RecentDocumentRecord {
    pub file: IndexedFilePreviewRecord,
    pub last_opened_at_ms: i64,
    pub open_count: u64,
}

#[derive(Debug, Clone)]
pub struct PinnedDocumentRecord {
    pub file: IndexedFilePreviewRecord,
    pub pinned_at_ms: i64,
}

#[derive(Debug, Clone)]
pub struct RecentSearchRecord {
    pub query_text: String,
    pub normalized_query: String,
    pub last_used_at_ms: i64,
    pub use_count: u64,
}

#[derive(Debug, Clone)]
pub struct EvidenceCaptureSourceRecord {
    pub document_id: String,
    pub file_instance_id: String,
    pub source: ResolvedFileSource,
    pub size_bytes: u64,
    pub creation_time_ms: Option<i64>,
    pub last_write_time_ms: Option<i64>,
    pub filesystem_identity: Option<Vec<u8>>,
    pub volume_identity: Option<Vec<u8>>,
}

#[derive(Debug, Clone)]
pub struct ControlledEvidenceVersionRecord {
    pub controlled_evidence_version_id: String,
    pub evidence_capture_job_id: String,
    pub document_id: String,
    pub source_content_version_id: String,
    pub version_number: u64,
    pub sha256: Vec<u8>,
    pub size_bytes: u64,
    pub captured_at_ms: i64,
    pub verification_state: String,
}

#[derive(Debug, Clone)]
pub struct DocumentVersionHistoryRecord {
    pub content_version_id: String,
    pub observed_at_ms: i64,
    pub size_bytes: u64,
    pub last_write_time_ms: Option<i64>,
    pub verification_state: String,
    pub source_stable_during_read: Option<bool>,
    pub sha256: Option<Vec<u8>>,
    pub controlled_evidence_version_id: Option<String>,
    pub controlled_version_number: Option<u64>,
    pub captured_at_ms: Option<i64>,
    pub controlled_verification_state: Option<String>,
    pub captured_by: Option<String>,
    pub capture_reason: Option<String>,
    pub capture_policy: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ClientRecord {
    pub client_id: String,
    pub name: String,
    pub created_at_ms: i64,
}

#[derive(Debug, Clone)]
pub struct ServiceTypeRecord {
    pub service_type_id: String,
    pub name: String,
    pub created_at_ms: i64,
}

#[derive(Debug, Clone)]
pub struct EngagementRecord {
    pub engagement_id: String,
    pub client_id: String,
    pub service_type_id: String,
    pub name: String,
    pub period_start: Option<String>,
    pub period_end: Option<String>,
    pub status: String,
    pub created_at_ms: i64,
}

#[derive(Debug, Clone)]
pub struct EngagementAreaRecord {
    pub engagement_area_id: String,
    pub engagement_id: String,
    pub parent_area_id: Option<String>,
    pub name: String,
    pub code: Option<String>,
    pub display_order: i64,
    pub status: String,
    pub created_at_ms: i64,
}

#[derive(Debug, Clone)]
pub struct ProcedureRecord {
    pub procedure_id: String,
    pub engagement_id: String,
    pub engagement_area_id: Option<String>,
    pub reference: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub created_at_ms: i64,
}

#[derive(Debug, Clone)]
pub struct WorkpaperRecord {
    pub workpaper_id: String,
    pub engagement_id: String,
    pub engagement_area_id: Option<String>,
    pub procedure_id: Option<String>,
    pub reference: String,
    pub title: String,
    pub workflow_state: String,
    pub created_at_ms: i64,
    pub latest_revision_number: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct WorkpaperRevisionRecord {
    pub workpaper_revision_id: String,
    pub workpaper_id: String,
    pub revision_number: u64,
    pub created_at_ms: i64,
    pub revision_reason: Option<String>,
    pub supersedes_revision_id: Option<String>,
    pub objective: String,
    pub procedure_performed: String,
    pub population: String,
    pub sample: String,
    pub exceptions: String,
    pub management_explanation: String,
    pub conclusion: String,
    pub content_hash: Option<Vec<u8>>,
}

#[derive(Debug, Clone)]
pub struct WorkpaperEvidenceLinkRecord {
    pub evidence_link_id: String,
    pub workpaper_revision_id: String,
    pub document_id: String,
    pub document_name: String,
    pub content_version_id: Option<String>,
    pub content_observed_at_ms: Option<i64>,
    pub controlled_evidence_version_id: Option<String>,
    pub controlled_version_number: Option<u64>,
    pub controlled_captured_at_ms: Option<i64>,
    pub relationship_type: String,
    pub description: Option<String>,
    pub created_at_ms: i64,
}

#[derive(Debug, Clone)]
pub struct WorkpaperWorkflowEventRecord {
    pub workpaper_workflow_event_id: String,
    pub workpaper_id: String,
    pub workpaper_revision_id: Option<String>,
    pub from_state: String,
    pub to_state: String,
    pub actor_id: Option<String>,
    pub comment: Option<String>,
    pub occurred_at_ms: i64,
}

#[derive(Debug, Clone)]
pub struct ReviewNoteRecord {
    pub review_note_id: String,
    pub workpaper_id: String,
    pub workpaper_revision_id: String,
    pub evidence_link_id: Option<String>,
    pub title: String,
    pub body: String,
    pub owner_id: Option<String>,
    pub due_at_ms: Option<i64>,
    pub location_kind: Option<String>,
    pub location_value: Option<String>,
    pub current_state: String,
    pub raised_by: Option<String>,
    pub created_at_ms: i64,
    pub latest_event_at_ms: i64,
}

#[derive(Debug, Clone)]
pub struct ReviewNoteEventRecord {
    pub review_note_event_id: String,
    pub review_note_id: String,
    pub event_type: String,
    pub actor_id: Option<String>,
    pub response_text: Option<String>,
    pub comment: Option<String>,
    pub occurred_at_ms: i64,
}

pub struct NewReviewNote<'a> {
    pub workpaper_id: &'a str,
    pub workpaper_revision_id: &'a str,
    pub evidence_link_id: Option<&'a str>,
    pub title: &'a str,
    pub body: &'a str,
    pub owner_id: Option<&'a str>,
    pub due_at_ms: Option<i64>,
    pub location_kind: Option<&'a str>,
    pub location_value: Option<&'a str>,
    pub raised_by: Option<&'a str>,
}

pub struct ReviewNoteAction<'a> {
    pub review_note_id: &'a str,
    pub actor_id: Option<&'a str>,
    pub response_text: Option<&'a str>,
    pub comment: Option<&'a str>,
}

pub struct NewWorkpaperRevision<'a> {
    pub revision_reason: Option<&'a str>,
    pub objective: &'a str,
    pub procedure_performed: &'a str,
    pub population: &'a str,
    pub sample: &'a str,
    pub exceptions: &'a str,
    pub management_explanation: &'a str,
    pub conclusion: &'a str,
}

#[derive(Debug, Clone)]
pub struct DocumentRelationshipRecord {
    pub document_relationship_id: String,
    pub relationship_type: String,
    pub direction: String,
    pub created_at_ms: i64,
    pub related_document_id: String,
    pub related_document_name: String,
    pub related_file: Option<IndexedFilePreviewRecord>,
}

pub struct EvidenceCaptureCompletion<'a> {
    pub capture_job_id: &'a str,
    pub controlled_evidence_version_id: &'a str,
    pub document_id: &'a str,
    pub file_instance_id: &'a str,
    pub controlled_storage_locator: &'a str,
    pub sha256: &'a [u8; 32],
    pub size_bytes: u64,
    pub last_write_time_ms: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct SearchProjectionRecord {
    pub document_id: String,
    pub file_instance_id: String,
    pub storage_root_id: String,
    pub display_name: String,
    pub relative_path_display: String,
    pub availability_state: String,
}

#[derive(Debug, Clone)]
pub struct ResolvedFileSource {
    pub storage_root_path: PathBuf,
    pub relative_path: PathBuf,
}

struct ResolvedFileSourceRow {
    root_native: Vec<u8>,
    root_encoding: String,
    relative_native: Vec<u8>,
    relative_encoding: String,
}

#[derive(Debug, Clone)]
pub struct SearchOutboxRecord {
    pub operation_id: String,
    pub entity_id: String,
    pub operation: String,
    pub payload_version: u32,
    pub payload_json: String,
}

#[derive(Debug)]
struct HydratedIndexRow {
    document_id: String,
    file_instance_id: String,
    name: String,
    relative_path_native: Vec<u8>,
    path_native_encoding: String,
    size_bytes: i64,
    modified_unix_ms: Option<i64>,
    availability_state: String,
    root_native: Vec<u8>,
    root_native_encoding: String,
}

#[derive(Debug, Clone)]
pub struct IndexJobCompletion<'a> {
    pub status: &'a str,
    pub failure_code: Option<&'a str>,
    pub failure_message: Option<&'a str>,
}

#[derive(Debug)]
struct ExistingFileInstance {
    file_instance_id: String,
    document_id: String,
    relative_path_native: Vec<u8>,
    path_native_encoding: String,
    relative_path_display: String,
    filesystem_identity: Option<Vec<u8>>,
    volume_identity: Option<Vec<u8>>,
    size_bytes: i64,
    last_write_time_ms: Option<i64>,
    creation_time_ms: Option<i64>,
    availability_state: String,
    latest_quick_fingerprint: Option<Vec<u8>>,
}

struct ObservationContext<'a> {
    storage_root_id: &'a str,
    scan_generation_id: &'a str,
    observation: &'a FileObservation,
    observed_at_ms: i64,
}

struct SearchProjection {
    document_id: String,
    display_name: String,
    storage_root_id: String,
    relative_path_display: String,
    size_bytes: i64,
    last_write_time_ms: Option<i64>,
    availability_state: String,
    content_version_id: Option<String>,
}

#[derive(Debug)]
pub enum PersistenceError {
    Io(std::io::Error),
    Sqlite(rusqlite::Error),
    Configuration(String),
    Migration(String),
}

impl fmt::Display for PersistenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "I/O error: {error}"),
            Self::Sqlite(error) => write!(f, "SQLite error: {error}"),
            Self::Configuration(message) => write!(f, "SQLite configuration error: {message}"),
            Self::Migration(message) => write!(f, "SQLite migration error: {message}"),
        }
    }
}

impl Error for PersistenceError {}

impl From<std::io::Error> for PersistenceError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<rusqlite::Error> for PersistenceError {
    fn from(value: rusqlite::Error) -> Self {
        Self::Sqlite(value)
    }
}

pub fn initialize_database(database_path: &Path) -> Result<(), PersistenceError> {
    let parent = database_path.parent().ok_or_else(|| {
        PersistenceError::Configuration("database path has no parent directory".to_string())
    })?;

    fs::create_dir_all(parent)?;

    let mut connection = open_configured_connection(database_path)?;
    run_migrations(&mut connection)?;
    verify_connection_profile(&connection)?;

    Ok(())
}

pub fn register_storage_root(
    database_path: &Path,
    proposed_storage_root_id: &str,
    selected_path: &Path,
    canonical_path: &Path,
) -> Result<StorageRootRecord, PersistenceError> {
    let mut connection = open_configured_connection(database_path)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

    let (native_locator, native_encoding) = encode_native_path(selected_path);
    let (canonical_native_locator, canonical_encoding) = encode_native_path(canonical_path);

    if native_encoding != canonical_encoding {
        return Err(PersistenceError::Configuration(
            "selected and canonical paths use different native encodings".to_string(),
        ));
    }

    let display_locator = selected_path.to_string_lossy().into_owned();
    let canonical_display_locator = canonical_path.to_string_lossy().into_owned();
    let now = now_unix_ms()?;
    let kind = classify_storage_root(canonical_path);

    let existing_id: Option<String> = transaction
        .query_row(
            "SELECT storage_root_id
             FROM storage_roots
             WHERE native_locator_encoding = ?1
               AND canonical_native_locator = ?2
             LIMIT 1",
            params![native_encoding, &canonical_native_locator],
            |row| row.get(0),
        )
        .optional()?;

    let storage_root_id = if let Some(existing_id) = existing_id {
        transaction.execute(
            "UPDATE storage_roots
             SET kind = ?1,
                 native_locator = ?2,
                 display_locator = ?3,
                 canonical_native_locator = ?4,
                 canonical_display_locator = ?5,
                 availability_state = 'AVAILABLE',
                 updated_at_ms = ?6
             WHERE storage_root_id = ?7",
            params![
                kind,
                &native_locator,
                &display_locator,
                &canonical_native_locator,
                &canonical_display_locator,
                now,
                &existing_id
            ],
        )?;
        existing_id
    } else {
        transaction.execute(
            "INSERT INTO storage_roots (
                storage_root_id,
                kind,
                native_locator,
                native_locator_encoding,
                display_locator,
                canonical_native_locator,
                canonical_display_locator,
                availability_state,
                approved_at_ms,
                approved_by,
                created_at_ms,
                updated_at_ms
             ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, 'AVAILABLE', ?8, NULL, ?8, ?8
             )",
            params![
                proposed_storage_root_id,
                kind,
                &native_locator,
                native_encoding,
                &display_locator,
                &canonical_native_locator,
                &canonical_display_locator,
                now
            ],
        )?;
        proposed_storage_root_id.to_string()
    };

    transaction.commit()?;

    Ok(StorageRootRecord {
        storage_root_id,
        display_path: canonical_display_locator,
        availability_state: "AVAILABLE".to_string(),
        canonical_path: canonical_path.to_path_buf(),
    })
}

pub fn list_storage_roots(
    database_path: &Path,
) -> Result<Vec<StorageRootRecord>, PersistenceError> {
    let connection = open_configured_connection(database_path)?;
    let mut statement = connection.prepare(
        "SELECT
            storage_root_id,
            native_locator_encoding,
            native_locator,
            canonical_native_locator,
            display_locator,
            canonical_display_locator,
            availability_state
         FROM storage_roots
         ORDER BY COALESCE(canonical_display_locator, display_locator) COLLATE NOCASE",
    )?;

    let rows = statement.query_map([], storage_root_from_row)?;
    let mut roots = Vec::new();

    for row in rows {
        roots.push(row?);
    }

    Ok(roots)
}

pub fn get_storage_root(
    database_path: &Path,
    storage_root_id: &str,
) -> Result<Option<StorageRootRecord>, PersistenceError> {
    let connection = open_configured_connection(database_path)?;

    connection
        .query_row(
            "SELECT
                storage_root_id,
                native_locator_encoding,
                native_locator,
                canonical_native_locator,
                display_locator,
                canonical_display_locator,
                availability_state
             FROM storage_roots
             WHERE storage_root_id = ?1",
            [storage_root_id],
            storage_root_from_row,
        )
        .optional()
        .map_err(PersistenceError::from)
}

pub fn recover_interrupted_index_jobs(database_path: &Path) -> Result<u64, PersistenceError> {
    let mut connection = open_configured_connection(database_path)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let now = now_unix_ms()?;

    transaction.execute(
        "UPDATE storage_roots
         SET availability_state = 'NEEDS_RESCAN',
             updated_at_ms = ?1
         WHERE storage_root_id IN (
             SELECT storage_root_id
             FROM index_jobs
             WHERE status IN ('QUEUED', 'RUNNING')
         )",
        [now],
    )?;

    transaction.execute(
        "UPDATE scan_generations
         SET status = 'INTERRUPTED',
             completed_at_ms = ?1,
             is_authoritative = 0
         WHERE scan_generation_id IN (
             SELECT scan_generation_id
             FROM index_jobs
             WHERE status IN ('QUEUED', 'RUNNING')
               AND scan_generation_id IS NOT NULL
         )",
        [now],
    )?;

    let changed = transaction.execute(
        "UPDATE index_jobs
         SET status = 'INTERRUPTED',
             completed_at_ms = ?1,
             last_heartbeat_at_ms = ?1,
             current_phase = 'INTERRUPTED',
             failure_code = 'PROCESS_INTERRUPTED',
             failure_message = 'The application stopped before this indexing job completed.'
         WHERE status IN ('QUEUED', 'RUNNING')",
        [now],
    )?;

    transaction.commit()?;
    Ok(changed as u64)
}

pub fn create_index_job(
    database_path: &Path,
    storage_root_id: &str,
    index_job_id: &str,
    scan_generation_id: &str,
) -> Result<IndexJobRecord, PersistenceError> {
    let mut connection = open_configured_connection(database_path)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

    let root_exists: i64 = transaction.query_row(
        "SELECT EXISTS(
             SELECT 1 FROM storage_roots WHERE storage_root_id = ?1
         )",
        [storage_root_id],
        |row| row.get(0),
    )?;

    if root_exists != 1 {
        return Err(PersistenceError::Configuration(format!(
            "storage root {storage_root_id} does not exist"
        )));
    }

    let active_exists: i64 = transaction.query_row(
        "SELECT EXISTS(
             SELECT 1
             FROM index_jobs
             WHERE storage_root_id = ?1
               AND status IN ('QUEUED', 'RUNNING')
         )",
        [storage_root_id],
        |row| row.get(0),
    )?;

    if active_exists == 1 {
        return Err(PersistenceError::Configuration(
            "an indexing job is already active for this storage root".to_string(),
        ));
    }

    let authoritative_exists: i64 = transaction.query_row(
        "SELECT EXISTS(
             SELECT 1
             FROM scan_generations
             WHERE storage_root_id = ?1
               AND status = 'COMPLETE'
               AND is_authoritative = 1
         )",
        [storage_root_id],
        |row| row.get(0),
    )?;

    let job_type = if authoritative_exists == 1 {
        "FULL_RECONCILIATION"
    } else {
        "INITIAL_SCAN"
    };

    let generation_number: i64 = transaction.query_row(
        "SELECT COALESCE(MAX(generation_number), 0) + 1
         FROM scan_generations
         WHERE storage_root_id = ?1",
        [storage_root_id],
        |row| row.get(0),
    )?;

    let now = now_unix_ms()?;

    transaction.execute(
        "INSERT INTO scan_generations (
            scan_generation_id,
            storage_root_id,
            generation_number,
            started_at_ms,
            completed_at_ms,
            status,
            is_authoritative,
            directories_seen,
            files_seen,
            errors_count
         ) VALUES (?1, ?2, ?3, ?4, NULL, 'QUEUED', 0, 0, 0, 0)",
        params![scan_generation_id, storage_root_id, generation_number, now],
    )?;

    transaction.execute(
        "INSERT INTO index_jobs (
            index_job_id,
            storage_root_id,
            job_type,
            status,
            requested_at_ms,
            started_at_ms,
            completed_at_ms,
            cancel_requested_at_ms,
            last_heartbeat_at_ms,
            current_phase,
            directories_seen,
            files_seen,
            bytes_seen,
            files_persisted,
            errors_count,
            scan_generation_id,
            failure_code,
            failure_message
         ) VALUES (
            ?1, ?2, ?3, 'QUEUED', ?4, NULL, NULL, NULL, NULL,
            'QUEUED', 0, 0, 0, 0, 0, ?5, NULL, NULL
         )",
        params![
            index_job_id,
            storage_root_id,
            job_type,
            now,
            scan_generation_id
        ],
    )?;

    transaction.commit()?;

    get_index_job(database_path, index_job_id)?.ok_or_else(|| {
        PersistenceError::Configuration("newly created indexing job disappeared".to_string())
    })
}

pub fn mark_index_job_running(
    database_path: &Path,
    index_job_id: &str,
    scan_generation_id: &str,
    storage_root_id: &str,
) -> Result<(), PersistenceError> {
    let mut connection = open_configured_connection(database_path)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let now = now_unix_ms()?;

    let changed = transaction.execute(
        "UPDATE index_jobs
         SET status = 'RUNNING',
             started_at_ms = COALESCE(started_at_ms, ?1),
             last_heartbeat_at_ms = ?1,
             current_phase = 'ENUMERATING',
             failure_code = NULL,
             failure_message = NULL
         WHERE index_job_id = ?2
           AND status = 'QUEUED'",
        params![now, index_job_id],
    )?;

    if changed != 1 {
        return Err(PersistenceError::Configuration(format!(
            "indexing job {index_job_id} is not queued"
        )));
    }

    transaction.execute(
        "UPDATE scan_generations
         SET status = 'RUNNING',
             started_at_ms = ?1,
             completed_at_ms = NULL,
             is_authoritative = 0
         WHERE scan_generation_id = ?2",
        params![now, scan_generation_id],
    )?;

    transaction.execute(
        "UPDATE storage_roots
         SET availability_state = 'AVAILABLE',
             updated_at_ms = ?1
         WHERE storage_root_id = ?2",
        params![now, storage_root_id],
    )?;

    transaction.commit()?;
    Ok(())
}

pub fn persist_index_batch(
    database_path: &Path,
    index_job_id: &str,
    scan_generation_id: &str,
    storage_root_id: &str,
    files: &[FileObservation],
    errors: &[ScanErrorObservation],
    progress: &IndexProgress,
) -> Result<(), PersistenceError> {
    let mut connection = open_configured_connection(database_path)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let observed_at = now_unix_ms()?;

    for observation in files {
        persist_file_observation(
            &transaction,
            storage_root_id,
            scan_generation_id,
            observation,
            observed_at,
        )?;
    }

    for error in errors {
        transaction.execute(
            "INSERT INTO scan_errors (
                scan_error_id,
                index_job_id,
                scan_generation_id,
                storage_root_id,
                relative_path_native,
                path_native_encoding,
                relative_path_display,
                category,
                os_error_code,
                message,
                observed_at_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                Uuid::new_v4().to_string(),
                index_job_id,
                scan_generation_id,
                storage_root_id,
                error.relative_path_native.as_deref(),
                error.path_native_encoding.as_deref(),
                error.relative_path_display.as_deref(),
                &error.category,
                error.os_error_code,
                &error.message,
                observed_at
            ],
        )?;
    }

    let files_persisted_target = progress.files_persisted.saturating_add(files.len() as u64);

    transaction.execute(
        "UPDATE index_jobs
         SET last_heartbeat_at_ms = ?1,
             current_phase = 'PERSISTING',
             directories_seen = ?2,
             files_seen = ?3,
             bytes_seen = ?4,
             files_persisted = ?5,
             errors_count = ?6
         WHERE index_job_id = ?7
           AND status = 'RUNNING'",
        params![
            observed_at,
            u64_to_i64(progress.directories_seen)?,
            u64_to_i64(progress.files_seen)?,
            u64_to_i64(progress.bytes_seen)?,
            u64_to_i64(files_persisted_target)?,
            u64_to_i64(progress.errors_count)?,
            index_job_id
        ],
    )?;

    transaction.execute(
        "UPDATE scan_generations
         SET directories_seen = ?1,
             files_seen = ?2,
             errors_count = ?3
         WHERE scan_generation_id = ?4",
        params![
            u64_to_i64(progress.directories_seen)?,
            u64_to_i64(progress.files_seen)?,
            u64_to_i64(progress.errors_count)?,
            scan_generation_id
        ],
    )?;

    transaction.commit()?;
    Ok(())
}

fn persist_file_observation(
    transaction: &rusqlite::Transaction<'_>,
    storage_root_id: &str,
    scan_generation_id: &str,
    observation: &FileObservation,
    observed_at_ms: i64,
) -> Result<(), PersistenceError> {
    let context = ObservationContext {
        storage_root_id,
        scan_generation_id,
        observation,
        observed_at_ms,
    };

    let identity_match = find_existing_by_identity(transaction, &context)?
        .filter(|candidate| !creation_time_conflicts(candidate, observation));

    if let Some(existing) = identity_match {
        return update_existing_file_instance(transaction, &context, existing);
    }

    if let Some(path_candidate) = find_existing_by_path(transaction, &context)? {
        if can_reuse_path_candidate(&path_candidate, observation) {
            return update_existing_file_instance(transaction, &context, path_candidate);
        }

        if path_candidate.availability_state != "CHANGED"
            && path_candidate.availability_state != "MISSING"
        {
            transaction.execute(
                "UPDATE file_instances
                 SET availability_state = 'CHANGED'
                 WHERE file_instance_id = ?1",
                [&path_candidate.file_instance_id],
            )?;
            enqueue_document_projection(
                transaction,
                &path_candidate.file_instance_id,
                observed_at_ms,
            )?;
        }
    }

    insert_new_file_instance(transaction, &context)
}

fn find_existing_by_identity(
    transaction: &rusqlite::Transaction<'_>,
    context: &ObservationContext<'_>,
) -> Result<Option<ExistingFileInstance>, PersistenceError> {
    let (Some(filesystem_identity), Some(volume_identity)) = (
        context.observation.filesystem_identity.as_deref(),
        context.observation.volume_identity.as_deref(),
    ) else {
        return Ok(None);
    };

    transaction
        .query_row(
            "SELECT
                file_instance_id,
                document_id,
                relative_path_native,
                path_native_encoding,
                relative_path_display,
                filesystem_identity,
                volume_identity,
                size_bytes,
                last_write_time_ms,
                creation_time_ms,
                availability_state,
                (
                    SELECT cv.quick_fingerprint
                    FROM content_versions cv
                    WHERE cv.file_instance_id = file_instances.file_instance_id
                    ORDER BY cv.observed_at_ms DESC, cv.rowid DESC
                    LIMIT 1
                )
             FROM file_instances
             WHERE storage_root_id = ?1
               AND filesystem_identity = ?2
               AND volume_identity = ?3
             ORDER BY last_seen_at_ms DESC
             LIMIT 1",
            params![
                context.storage_root_id,
                filesystem_identity,
                volume_identity
            ],
            existing_file_instance_from_row,
        )
        .optional()
        .map_err(PersistenceError::from)
}

fn find_existing_by_path(
    transaction: &rusqlite::Transaction<'_>,
    context: &ObservationContext<'_>,
) -> Result<Option<ExistingFileInstance>, PersistenceError> {
    transaction
        .query_row(
            "SELECT
                file_instance_id,
                document_id,
                relative_path_native,
                path_native_encoding,
                relative_path_display,
                filesystem_identity,
                volume_identity,
                size_bytes,
                last_write_time_ms,
                creation_time_ms,
                availability_state,
                (
                    SELECT cv.quick_fingerprint
                    FROM content_versions cv
                    WHERE cv.file_instance_id = file_instances.file_instance_id
                    ORDER BY cv.observed_at_ms DESC, cv.rowid DESC
                    LIMIT 1
                )
             FROM file_instances
             WHERE storage_root_id = ?1
               AND path_native_encoding = ?2
               AND relative_path_native = ?3
             ORDER BY last_seen_at_ms DESC
             LIMIT 1",
            params![
                context.storage_root_id,
                &context.observation.path_native_encoding,
                &context.observation.relative_path_native
            ],
            existing_file_instance_from_row,
        )
        .optional()
        .map_err(PersistenceError::from)
}

fn existing_file_instance_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<ExistingFileInstance> {
    Ok(ExistingFileInstance {
        file_instance_id: row.get(0)?,
        document_id: row.get(1)?,
        relative_path_native: row.get(2)?,
        path_native_encoding: row.get(3)?,
        relative_path_display: row.get(4)?,
        filesystem_identity: row.get(5)?,
        volume_identity: row.get(6)?,
        size_bytes: row.get(7)?,
        last_write_time_ms: row.get(8)?,
        creation_time_ms: row.get(9)?,
        availability_state: row.get(10)?,
        latest_quick_fingerprint: row.get(11)?,
    })
}

fn creation_time_conflicts(existing: &ExistingFileInstance, observation: &FileObservation) -> bool {
    matches!(
        (existing.creation_time_ms, observation.creation_time_ms),
        (Some(previous), Some(current)) if previous != current
    )
}

fn can_reuse_path_candidate(
    existing: &ExistingFileInstance,
    observation: &FileObservation,
) -> bool {
    if creation_time_conflicts(existing, observation) {
        return false;
    }

    match (
        existing.filesystem_identity.as_deref(),
        existing.volume_identity.as_deref(),
        observation.filesystem_identity.as_deref(),
        observation.volume_identity.as_deref(),
    ) {
        (Some(existing_file), Some(existing_volume), Some(current_file), Some(current_volume)) => {
            existing_file == current_file && existing_volume == current_volume
        }
        _ => true,
    }
}

fn update_existing_file_instance(
    transaction: &rusqlite::Transaction<'_>,
    context: &ObservationContext<'_>,
    existing: ExistingFileInstance,
) -> Result<(), PersistenceError> {
    let observation = context.observation;
    let path_changed = existing.path_native_encoding != observation.path_native_encoding
        || existing.relative_path_native != observation.relative_path_native;
    let fingerprint_changed = matches!(
        (
            existing.latest_quick_fingerprint.as_deref(),
            observation.quick_fingerprint.as_deref(),
        ),
        (Some(previous), Some(current)) if previous != current
    );
    let content_changed = existing.size_bytes != u64_to_i64(observation.size_bytes)?
        || matches!(
            observation.last_write_time_ms,
            Some(current) if existing.last_write_time_ms != Some(current)
        )
        || fingerprint_changed;
    let should_seed_fingerprint =
        existing.latest_quick_fingerprint.is_none() && observation.quick_fingerprint.is_some();
    let next_availability_state =
        if existing.availability_state == "CHANGED" || path_changed || content_changed {
            "CHANGED"
        } else {
            "AVAILABLE"
        };
    let availability_changed = existing.availability_state != next_availability_state;
    let display_name_changed = Path::new(&existing.relative_path_display)
        .file_name()
        .map(|value| value.to_string_lossy().as_ref() != observation.display_name)
        .unwrap_or(true);

    if path_changed {
        transaction.execute(
            "UPDATE file_path_history
             SET observed_until_ms = ?1
             WHERE file_instance_id = ?2
               AND observed_until_ms IS NULL",
            params![context.observed_at_ms, &existing.file_instance_id],
        )?;

        transaction.execute(
            "INSERT INTO file_path_history (
                file_path_history_id,
                file_instance_id,
                storage_root_id,
                relative_path_native,
                path_native_encoding,
                relative_path_display,
                observed_from_ms,
                observed_until_ms,
                change_reason,
                actor_id,
                scan_generation_id
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, ?8, NULL, ?9)",
            params![
                Uuid::new_v4().to_string(),
                &existing.file_instance_id,
                context.storage_root_id,
                &observation.relative_path_native,
                &observation.path_native_encoding,
                &observation.relative_path_display,
                context.observed_at_ms,
                path_change_reason(
                    &existing.relative_path_display,
                    &observation.relative_path_display
                ),
                context.scan_generation_id
            ],
        )?;
    }

    transaction.execute(
        "UPDATE file_instances
         SET relative_path_native = ?1,
             path_native_encoding = ?2,
             relative_path_display = ?3,
             relative_path_search = ?4,
             filesystem_identity = COALESCE(?5, filesystem_identity),
             volume_identity = COALESCE(?6, volume_identity),
             creation_time_ms = COALESCE(?7, creation_time_ms),
             last_write_time_ms = COALESCE(?8, last_write_time_ms),
             size_bytes = ?9,
             file_attributes = COALESCE(?10, file_attributes),
             reparse_tag = COALESCE(?11, reparse_tag),
             last_seen_at_ms = ?12,
             last_seen_generation_id = ?13,
             availability_state = ?14
         WHERE file_instance_id = ?15",
        params![
            &observation.relative_path_native,
            &observation.path_native_encoding,
            &observation.relative_path_display,
            &observation.relative_path_search,
            observation.filesystem_identity.as_deref(),
            observation.volume_identity.as_deref(),
            observation.creation_time_ms,
            observation.last_write_time_ms,
            u64_to_i64(observation.size_bytes)?,
            observation.file_attributes,
            observation.reparse_tag,
            context.observed_at_ms,
            context.scan_generation_id,
            next_availability_state,
            &existing.file_instance_id
        ],
    )?;

    if display_name_changed || path_changed {
        transaction.execute(
            "UPDATE documents
             SET display_name = ?1
             WHERE document_id = ?2",
            params![&observation.display_name, &existing.document_id],
        )?;
    }

    if content_changed || should_seed_fingerprint {
        insert_content_version(
            transaction,
            &existing.document_id,
            &existing.file_instance_id,
            observation,
            context.observed_at_ms,
        )?;
    }

    if path_changed || content_changed || availability_changed || display_name_changed {
        enqueue_document_projection(
            transaction,
            &existing.file_instance_id,
            context.observed_at_ms,
        )?;
    }

    Ok(())
}

fn insert_new_file_instance(
    transaction: &rusqlite::Transaction<'_>,
    context: &ObservationContext<'_>,
) -> Result<(), PersistenceError> {
    let observation = context.observation;
    let document_id = Uuid::new_v4().to_string();
    let file_instance_id = Uuid::new_v4().to_string();

    transaction.execute(
        "INSERT INTO documents (
            document_id,
            storage_state,
            display_name,
            created_at_ms,
            created_by,
            archived_at_ms
         ) VALUES (?1, 'LINKED', ?2, ?3, NULL, NULL)",
        params![
            &document_id,
            &observation.display_name,
            context.observed_at_ms
        ],
    )?;

    transaction.execute(
        "INSERT INTO file_instances (
            file_instance_id,
            document_id,
            storage_root_id,
            relative_path_native,
            path_native_encoding,
            relative_path_display,
            relative_path_search,
            filesystem_identity,
            volume_identity,
            creation_time_ms,
            last_write_time_ms,
            size_bytes,
            file_attributes,
            reparse_tag,
            first_seen_at_ms,
            last_seen_at_ms,
            first_seen_generation_id,
            last_seen_generation_id,
            availability_state
         ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
            ?13, ?14, ?15, ?15, ?16, ?16, 'AVAILABLE'
         )",
        params![
            &file_instance_id,
            &document_id,
            context.storage_root_id,
            &observation.relative_path_native,
            &observation.path_native_encoding,
            &observation.relative_path_display,
            &observation.relative_path_search,
            observation.filesystem_identity.as_deref(),
            observation.volume_identity.as_deref(),
            observation.creation_time_ms,
            observation.last_write_time_ms,
            u64_to_i64(observation.size_bytes)?,
            observation.file_attributes,
            observation.reparse_tag,
            context.observed_at_ms,
            context.scan_generation_id
        ],
    )?;

    transaction.execute(
        "INSERT INTO file_path_history (
            file_path_history_id,
            file_instance_id,
            storage_root_id,
            relative_path_native,
            path_native_encoding,
            relative_path_display,
            observed_from_ms,
            observed_until_ms,
            change_reason,
            actor_id,
            scan_generation_id
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, 'DISCOVERED', NULL, ?8)",
        params![
            Uuid::new_v4().to_string(),
            &file_instance_id,
            context.storage_root_id,
            &observation.relative_path_native,
            &observation.path_native_encoding,
            &observation.relative_path_display,
            context.observed_at_ms,
            context.scan_generation_id
        ],
    )?;

    insert_content_version(
        transaction,
        &document_id,
        &file_instance_id,
        observation,
        context.observed_at_ms,
    )?;
    enqueue_document_projection(transaction, &file_instance_id, context.observed_at_ms)?;

    Ok(())
}

fn insert_content_version(
    transaction: &rusqlite::Transaction<'_>,
    document_id: &str,
    file_instance_id: &str,
    observation: &FileObservation,
    observed_at_ms: i64,
) -> Result<String, PersistenceError> {
    let content_version_id = Uuid::new_v4().to_string();

    transaction.execute(
        "INSERT INTO content_versions (
            content_version_id,
            document_id,
            file_instance_id,
            observed_at_ms,
            size_bytes,
            last_write_time_ms,
            quick_fingerprint,
            sha256,
            verification_state,
            source_stable_during_read
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, ?8, ?9)",
        params![
            &content_version_id,
            document_id,
            file_instance_id,
            observed_at_ms,
            u64_to_i64(observation.size_bytes)?,
            observation.last_write_time_ms,
            observation.quick_fingerprint.as_deref(),
            if observation.quick_fingerprint.is_some() {
                "FINGERPRINTED"
            } else {
                "METADATA_ONLY"
            },
            observation
                .source_stable_during_read
                .map(|stable| if stable { 1_i64 } else { 0_i64 })
        ],
    )?;

    Ok(content_version_id)
}

fn enqueue_document_projection(
    transaction: &rusqlite::Transaction<'_>,
    file_instance_id: &str,
    observed_at_ms: i64,
) -> Result<(), PersistenceError> {
    let projection: SearchProjection = transaction.query_row(
        "SELECT
                d.document_id,
                d.display_name,
                fi.storage_root_id,
                fi.relative_path_display,
                fi.size_bytes,
                fi.last_write_time_ms,
                fi.availability_state,
                (
                    SELECT cv.content_version_id
                    FROM content_versions cv
                    WHERE cv.file_instance_id = fi.file_instance_id
                    ORDER BY cv.observed_at_ms DESC, cv.rowid DESC
                    LIMIT 1
                )
             FROM file_instances fi
             JOIN documents d ON d.document_id = fi.document_id
             WHERE fi.file_instance_id = ?1",
        [file_instance_id],
        |row| {
            Ok(SearchProjection {
                document_id: row.get(0)?,
                display_name: row.get(1)?,
                storage_root_id: row.get(2)?,
                relative_path_display: row.get(3)?,
                size_bytes: row.get(4)?,
                last_write_time_ms: row.get(5)?,
                availability_state: row.get(6)?,
                content_version_id: row.get(7)?,
            })
        },
    )?;

    let payload = json!({
        "documentId": projection.document_id,
        "fileInstanceId": file_instance_id,
        "contentVersionId": projection.content_version_id,
        "storageRootId": projection.storage_root_id,
        "displayName": projection.display_name,
        "relativePath": projection.relative_path_display,
        "sizeBytes": projection.size_bytes.max(0) as u64,
        "modifiedUnixMs": projection.last_write_time_ms,
        "availabilityState": projection.availability_state
    });

    transaction.execute(
        "INSERT INTO search_index_outbox (
            operation_id,
            entity_type,
            entity_id,
            operation,
            payload_version,
            payload_json,
            created_at_ms,
            acknowledged_at_ms,
            attempt_count,
            last_error
         ) VALUES (?1, 'DOCUMENT', ?2, 'UPSERT', 1, ?3, ?4, NULL, 0, NULL)",
        params![
            Uuid::new_v4().to_string(),
            projection.document_id,
            payload.to_string(),
            observed_at_ms
        ],
    )?;

    Ok(())
}

fn reconcile_unseen_as_missing(
    transaction: &rusqlite::Transaction<'_>,
    storage_root_id: &str,
    scan_generation_id: &str,
    observed_at_ms: i64,
) -> Result<(), PersistenceError> {
    let file_instance_ids = {
        let mut statement = transaction.prepare(
            "SELECT file_instance_id
             FROM file_instances
             WHERE storage_root_id = ?1
               AND (
                   last_seen_generation_id IS NULL
                   OR last_seen_generation_id <> ?2
               )
               AND availability_state <> 'MISSING'",
        )?;

        let rows = statement.query_map(params![storage_root_id, scan_generation_id], |row| {
            row.get::<_, String>(0)
        })?;

        let mut ids = Vec::new();
        for row in rows {
            ids.push(row?);
        }
        ids
    };

    for file_instance_id in file_instance_ids {
        transaction.execute(
            "UPDATE file_instances
             SET availability_state = 'MISSING'
             WHERE file_instance_id = ?1",
            [&file_instance_id],
        )?;

        transaction.execute(
            "UPDATE file_path_history
             SET observed_until_ms = COALESCE(observed_until_ms, ?1)
             WHERE file_instance_id = ?2
               AND observed_until_ms IS NULL",
            params![observed_at_ms, &file_instance_id],
        )?;

        enqueue_document_projection(transaction, &file_instance_id, observed_at_ms)?;
    }

    Ok(())
}

fn path_change_reason(previous: &str, current: &str) -> &'static str {
    let previous_parent = Path::new(previous).parent();
    let current_parent = Path::new(current).parent();

    if previous_parent == current_parent {
        "RENAMED"
    } else {
        "MOVED"
    }
}

pub fn finish_index_job(
    database_path: &Path,
    index_job_id: &str,
    scan_generation_id: &str,
    storage_root_id: &str,
    progress: &IndexProgress,
    completion: IndexJobCompletion<'_>,
) -> Result<(), PersistenceError> {
    let (authoritative, root_state, phase) = match completion.status {
        "COMPLETE" => (1_i64, "AVAILABLE", "COMPLETE"),
        "PARTIAL" => (0_i64, "DEGRADED", "PARTIAL"),
        "CANCELLED" => (0_i64, "NEEDS_RESCAN", "CANCELLED"),
        "OFFLINE" => (0_i64, "OFFLINE", "OFFLINE"),
        "FAILED" => (0_i64, "DEGRADED", "FAILED"),
        other => {
            return Err(PersistenceError::Configuration(format!(
                "unsupported terminal indexing status {other}"
            )))
        }
    };

    let mut connection = open_configured_connection(database_path)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let now = now_unix_ms()?;

    if completion.status == "COMPLETE" {
        reconcile_unseen_as_missing(&transaction, storage_root_id, scan_generation_id, now)?;
    }

    transaction.execute(
        "UPDATE index_jobs
         SET status = ?1,
             completed_at_ms = ?2,
             last_heartbeat_at_ms = ?2,
             current_phase = ?3,
             directories_seen = ?4,
             files_seen = ?5,
             bytes_seen = ?6,
             files_persisted = ?7,
             errors_count = ?8,
             failure_code = ?9,
             failure_message = ?10
         WHERE index_job_id = ?11",
        params![
            completion.status,
            now,
            phase,
            u64_to_i64(progress.directories_seen)?,
            u64_to_i64(progress.files_seen)?,
            u64_to_i64(progress.bytes_seen)?,
            u64_to_i64(progress.files_persisted)?,
            u64_to_i64(progress.errors_count)?,
            completion.failure_code,
            completion.failure_message,
            index_job_id
        ],
    )?;

    transaction.execute(
        "UPDATE scan_generations
         SET status = ?1,
             completed_at_ms = ?2,
             is_authoritative = ?3,
             directories_seen = ?4,
             files_seen = ?5,
             errors_count = ?6
         WHERE scan_generation_id = ?7",
        params![
            completion.status,
            now,
            authoritative,
            u64_to_i64(progress.directories_seen)?,
            u64_to_i64(progress.files_seen)?,
            u64_to_i64(progress.errors_count)?,
            scan_generation_id
        ],
    )?;

    transaction.execute(
        "UPDATE storage_roots
         SET availability_state = ?1,
             updated_at_ms = ?2
         WHERE storage_root_id = ?3",
        params![root_state, now, storage_root_id],
    )?;

    transaction.commit()?;
    Ok(())
}

pub fn fail_index_job(
    database_path: &Path,
    index_job_id: &str,
    scan_generation_id: &str,
    storage_root_id: &str,
    failure_code: &str,
    failure_message: &str,
) -> Result<(), PersistenceError> {
    let progress = get_index_job(database_path, index_job_id)?
        .map(|job| IndexProgress {
            directories_seen: job.directories_seen,
            files_seen: job.files_seen,
            bytes_seen: job.bytes_seen,
            files_persisted: job.files_persisted,
            errors_count: job.errors_count,
        })
        .unwrap_or_default();

    finish_index_job(
        database_path,
        index_job_id,
        scan_generation_id,
        storage_root_id,
        &progress,
        IndexJobCompletion {
            status: "FAILED",
            failure_code: Some(failure_code),
            failure_message: Some(failure_message),
        },
    )
}

pub fn request_index_job_cancel(
    database_path: &Path,
    index_job_id: &str,
) -> Result<bool, PersistenceError> {
    let connection = open_configured_connection(database_path)?;
    let changed = connection.execute(
        "UPDATE index_jobs
         SET cancel_requested_at_ms = ?1
         WHERE index_job_id = ?2
           AND status IN ('QUEUED', 'RUNNING')
           AND cancel_requested_at_ms IS NULL",
        params![now_unix_ms()?, index_job_id],
    )?;

    Ok(changed == 1)
}

pub fn get_index_job(
    database_path: &Path,
    index_job_id: &str,
) -> Result<Option<IndexJobRecord>, PersistenceError> {
    let connection = open_configured_connection(database_path)?;

    connection
        .query_row(
            "SELECT
                index_job_id,
                storage_root_id,
                job_type,
                status,
                requested_at_ms,
                started_at_ms,
                completed_at_ms,
                cancel_requested_at_ms,
                last_heartbeat_at_ms,
                current_phase,
                directories_seen,
                files_seen,
                bytes_seen,
                files_persisted,
                errors_count,
                scan_generation_id,
                failure_code,
                failure_message
             FROM index_jobs
             WHERE index_job_id = ?1",
            [index_job_id],
            index_job_from_row,
        )
        .optional()
        .map_err(PersistenceError::from)
}

pub fn get_latest_index_job_for_root(
    database_path: &Path,
    storage_root_id: &str,
) -> Result<Option<IndexJobRecord>, PersistenceError> {
    let connection = open_configured_connection(database_path)?;

    connection
        .query_row(
            "SELECT
                index_job_id,
                storage_root_id,
                job_type,
                status,
                requested_at_ms,
                started_at_ms,
                completed_at_ms,
                cancel_requested_at_ms,
                last_heartbeat_at_ms,
                current_phase,
                directories_seen,
                files_seen,
                bytes_seen,
                files_persisted,
                errors_count,
                scan_generation_id,
                failure_code,
                failure_message
             FROM index_jobs
             WHERE storage_root_id = ?1
             ORDER BY requested_at_ms DESC, rowid DESC
             LIMIT 1",
            [storage_root_id],
            index_job_from_row,
        )
        .optional()
        .map_err(PersistenceError::from)
}

fn index_job_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<IndexJobRecord> {
    Ok(IndexJobRecord {
        index_job_id: row.get(0)?,
        storage_root_id: row.get(1)?,
        job_type: row.get(2)?,
        status: row.get(3)?,
        requested_at_ms: row.get(4)?,
        started_at_ms: row.get(5)?,
        completed_at_ms: row.get(6)?,
        cancel_requested_at_ms: row.get(7)?,
        last_heartbeat_at_ms: row.get(8)?,
        current_phase: row.get(9)?,
        directories_seen: row.get::<_, i64>(10)? as u64,
        files_seen: row.get::<_, i64>(11)? as u64,
        bytes_seen: row.get::<_, i64>(12)? as u64,
        files_persisted: row.get::<_, i64>(13)? as u64,
        errors_count: row.get::<_, i64>(14)? as u64,
        scan_generation_id: row.get(15)?,
        failure_code: row.get(16)?,
        failure_message: row.get(17)?,
    })
}

pub fn list_indexed_file_preview(
    database_path: &Path,
    storage_root_id: &str,
    limit: u32,
) -> Result<Vec<IndexedFilePreviewRecord>, PersistenceError> {
    let root = get_storage_root(database_path, storage_root_id)?.ok_or_else(|| {
        PersistenceError::Configuration(format!("storage root {storage_root_id} does not exist"))
    })?;

    let connection = open_configured_connection(database_path)?;
    let bounded_limit = i64::from(limit.clamp(1, 500));
    let mut statement = connection.prepare(
        "SELECT
            d.document_id,
            fi.file_instance_id,
            d.display_name,
            fi.relative_path_native,
            fi.path_native_encoding,
            fi.size_bytes,
            fi.last_write_time_ms,
            fi.availability_state
         FROM file_instances fi
         JOIN documents d ON d.document_id = fi.document_id
         WHERE fi.storage_root_id = ?1
         ORDER BY
            CASE fi.availability_state
                WHEN 'AVAILABLE' THEN 0
                WHEN 'CHANGED' THEN 1
                WHEN 'UNAVAILABLE' THEN 2
                WHEN 'UNKNOWN' THEN 3
                WHEN 'MISSING' THEN 4
                ELSE 5
            END,
            fi.relative_path_search
         LIMIT ?2",
    )?;

    let rows = statement.query_map(params![storage_root_id, bounded_limit], |row| {
        let document_id: String = row.get(0)?;
        let file_instance_id: String = row.get(1)?;
        let name: String = row.get(2)?;
        let relative_native: Vec<u8> = row.get(3)?;
        let native_encoding: String = row.get(4)?;
        let size_bytes: i64 = row.get(5)?;
        let modified_unix_ms: Option<i64> = row.get(6)?;
        let availability_state: String = row.get(7)?;

        Ok((
            document_id,
            file_instance_id,
            name,
            relative_native,
            native_encoding,
            size_bytes,
            modified_unix_ms,
            availability_state,
        ))
    })?;

    let mut result = Vec::new();
    for row in rows {
        let (
            document_id,
            file_instance_id,
            name,
            relative_native,
            native_encoding,
            size_bytes,
            modified_unix_ms,
            availability_state,
        ) = row?;

        let relative = decode_native_path(&relative_native, &native_encoding)?;
        let absolute = root.canonical_path.join(&relative);
        let extension = relative
            .extension()
            .map(|value| value.to_string_lossy().into_owned())
            .unwrap_or_default();

        result.push(IndexedFilePreviewRecord {
            document_id,
            file_instance_id,
            name,
            path: absolute.to_string_lossy().into_owned(),
            extension,
            size_bytes: size_bytes.max(0) as u64,
            modified_unix_ms,
            availability_state: effective_file_availability_state(
                &availability_state,
                &root.availability_state,
            ),
        });
    }

    Ok(result)
}

fn supports_linked_source_lifecycle(storage_state: &str) -> bool {
    matches!(storage_state, "LINKED" | "CONTROLLED_EVIDENCE")
}

pub fn relink_linked_file_instance(
    database_path: &Path,
    file_instance_id: &str,
    selected_path: &Path,
) -> Result<IndexedFilePreviewRecord, PersistenceError> {
    let canonical_source = fs::canonicalize(selected_path).map_err(|error| {
        PersistenceError::Configuration(format!(
            "selected relink source could not be accessed: {error}"
        ))
    })?;
    let source_file = fs::File::open(&canonical_source).map_err(|error| {
        PersistenceError::Configuration(format!(
            "selected relink source could not be opened: {error}"
        ))
    })?;
    let metadata = source_file.metadata().map_err(|error| {
        PersistenceError::Configuration(format!(
            "selected relink source metadata could not be read: {error}"
        ))
    })?;

    if !metadata.is_file() {
        return Err(PersistenceError::Configuration(
            "selected relink source is not a regular file".to_string(),
        ));
    }

    let mut matching_roots = Vec::new();
    for root in list_storage_roots(database_path)? {
        if root.availability_state != "AVAILABLE" {
            continue;
        }

        let Ok(canonical_root) = fs::canonicalize(&root.canonical_path) else {
            continue;
        };

        if canonical_source.starts_with(&canonical_root) {
            matching_roots.push((root, canonical_root));
        }
    }

    let (target_root, canonical_root) = matching_roots
        .into_iter()
        .max_by_key(|(_, root_path)| root_path.components().count())
        .ok_or_else(|| {
            PersistenceError::Configuration(
                "selected relink source is outside the approved available storage roots"
                    .to_string(),
            )
        })?;

    let relative_path = canonical_source
        .strip_prefix(&canonical_root)
        .map_err(|error| {
            PersistenceError::Configuration(format!(
                "selected relink source could not be made root-relative: {error}"
            ))
        })?
        .to_path_buf();

    if relative_path.as_os_str().is_empty()
        || relative_path.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        })
    {
        return Err(PersistenceError::Configuration(
            "selected relink source is not a valid root-relative file path".to_string(),
        ));
    }

    let relative_path_display = relative_path.to_string_lossy().into_owned();
    let display_name = relative_path
        .file_name()
        .map(|value| value.to_string_lossy().into_owned())
        .ok_or_else(|| {
            PersistenceError::Configuration("selected relink source has no file name".to_string())
        })?;
    let relative_path_search = normalize_search_text(&relative_path_display);
    let (relative_path_native, path_native_encoding) =
        encode_native_path_for_storage(&relative_path);
    let platform = filesystem::platform_file_metadata_from_open_file(&source_file, &metadata);
    let system_time_to_ms = |value: SystemTime| {
        value
            .duration_since(UNIX_EPOCH)
            .ok()
            .and_then(|duration| i64::try_from(duration.as_millis()).ok())
    };
    let creation_time_ms = metadata.created().ok().and_then(system_time_to_ms);
    let last_write_time_ms = metadata.modified().ok().and_then(system_time_to_ms);
    let size_bytes = metadata.len();

    let mut connection = open_configured_connection(database_path)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

    type RelinkRow = (
        String,
        String,
        String,
        String,
        String,
        String,
        i64,
        Option<i64>,
        Option<Vec<u8>>,
        Option<Vec<u8>>,
    );

    let row: Option<RelinkRow> = transaction
        .query_row(
            "SELECT
                fi.document_id,
                d.storage_state,
                fi.availability_state,
                sr.availability_state,
                fi.storage_root_id,
                fi.relative_path_display,
                fi.size_bytes,
                fi.last_write_time_ms,
                fi.filesystem_identity,
                fi.volume_identity
             FROM file_instances fi
             JOIN documents d ON d.document_id = fi.document_id
             JOIN storage_roots sr ON sr.storage_root_id = fi.storage_root_id
             WHERE fi.file_instance_id = ?1
               AND d.archived_at_ms IS NULL",
            [file_instance_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                    row.get(9)?,
                ))
            },
        )
        .optional()?;

    let Some((
        document_id,
        storage_state,
        availability_state,
        source_root_availability_state,
        previous_storage_root_id,
        previous_relative_path_display,
        previous_size_bytes,
        previous_last_write_time_ms,
        previous_filesystem_identity,
        previous_volume_identity,
    )) = row
    else {
        return Err(PersistenceError::Configuration(format!(
            "file instance {file_instance_id} does not exist"
        )));
    };

    if !supports_linked_source_lifecycle(&storage_state) {
        return Err(PersistenceError::Configuration(format!(
            "document storage state {storage_state} does not support linked-source relinking"
        )));
    }

    if availability_state != "MISSING" && source_root_availability_state == "AVAILABLE" {
        return Err(PersistenceError::Configuration(format!(
            "file instance {file_instance_id} is {availability_state}; relinking is limited to missing or unavailable linked sources"
        )));
    }

    let target_root_state: String = transaction.query_row(
        "SELECT availability_state
         FROM storage_roots
         WHERE storage_root_id = ?1",
        [&target_root.storage_root_id],
        |row| row.get(0),
    )?;

    if target_root_state != "AVAILABLE" {
        return Err(PersistenceError::Configuration(format!(
            "selected approved storage root is {target_root_state}; rescan it before relinking"
        )));
    }

    let target_conflict: Option<String> = transaction
        .query_row(
            "SELECT file_instance_id
             FROM file_instances
             WHERE storage_root_id = ?1
               AND path_native_encoding = ?2
               AND relative_path_native = ?3
               AND file_instance_id <> ?4
             ORDER BY last_seen_at_ms DESC
             LIMIT 1",
            params![
                &target_root.storage_root_id,
                &path_native_encoding,
                &relative_path_native,
                file_instance_id
            ],
            |row| row.get(0),
        )
        .optional()?;

    if let Some(conflicting_file_instance_id) = target_conflict {
        return Err(PersistenceError::Configuration(format!(
            "selected relink source is already indexed by file instance {conflicting_file_instance_id}"
        )));
    }

    let content_changed = previous_size_bytes != u64_to_i64(size_bytes)?
        || previous_last_write_time_ms != last_write_time_ms;
    let identity_matches = previous_filesystem_identity == platform.filesystem_identity
        && previous_volume_identity == platform.volume_identity;
    let relinked_at_ms = now_unix_ms()?;

    transaction.execute(
        "UPDATE file_path_history
         SET observed_until_ms = COALESCE(observed_until_ms, ?1)
         WHERE file_instance_id = ?2
           AND observed_until_ms IS NULL",
        params![relinked_at_ms, file_instance_id],
    )?;

    transaction.execute(
        "INSERT INTO file_path_history (
            file_path_history_id,
            file_instance_id,
            storage_root_id,
            relative_path_native,
            path_native_encoding,
            relative_path_display,
            observed_from_ms,
            observed_until_ms,
            change_reason,
            actor_id,
            scan_generation_id
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, 'RELINKED', NULL, NULL)",
        params![
            Uuid::new_v4().to_string(),
            file_instance_id,
            &target_root.storage_root_id,
            &relative_path_native,
            &path_native_encoding,
            &relative_path_display,
            relinked_at_ms
        ],
    )?;

    transaction.execute(
        "UPDATE file_instances
         SET storage_root_id = ?1,
             relative_path_native = ?2,
             path_native_encoding = ?3,
             relative_path_display = ?4,
             relative_path_search = ?5,
             filesystem_identity = ?6,
             volume_identity = ?7,
             creation_time_ms = ?8,
             last_write_time_ms = ?9,
             size_bytes = ?10,
             file_attributes = ?11,
             reparse_tag = ?12,
             last_seen_at_ms = ?13,
             last_seen_generation_id = NULL,
             availability_state = 'CHANGED'
         WHERE file_instance_id = ?14",
        params![
            &target_root.storage_root_id,
            &relative_path_native,
            &path_native_encoding,
            &relative_path_display,
            &relative_path_search,
            platform.filesystem_identity.as_deref(),
            platform.volume_identity.as_deref(),
            creation_time_ms,
            last_write_time_ms,
            u64_to_i64(size_bytes)?,
            platform.file_attributes,
            platform.reparse_tag,
            relinked_at_ms,
            file_instance_id
        ],
    )?;

    transaction.execute(
        "UPDATE documents
         SET display_name = ?1
         WHERE document_id = ?2",
        params![&display_name, &document_id],
    )?;

    if content_changed {
        let observation = FileObservation {
            relative_path_native: relative_path_native.clone(),
            path_native_encoding: path_native_encoding.clone(),
            relative_path_display: relative_path_display.clone(),
            relative_path_search: relative_path_search.clone(),
            display_name: display_name.clone(),
            size_bytes,
            creation_time_ms,
            last_write_time_ms,
            filesystem_identity: platform.filesystem_identity.clone(),
            volume_identity: platform.volume_identity.clone(),
            file_attributes: platform.file_attributes,
            reparse_tag: platform.reparse_tag,
            quick_fingerprint: None,
            source_stable_during_read: None,
        };
        insert_content_version(
            &transaction,
            &document_id,
            file_instance_id,
            &observation,
            relinked_at_ms,
        )?;
    }

    transaction.execute(
        "INSERT INTO audit_events (
            audit_event_id,
            event_type,
            entity_type,
            entity_id,
            related_entity_type,
            related_entity_id,
            occurred_at_ms,
            actor_id,
            details_json
         ) VALUES (?1, 'LINKED_SOURCE_RELINKED', 'FILE_INSTANCE', ?2, 'DOCUMENT', ?3, ?4, NULL, ?5)",
        params![
            Uuid::new_v4().to_string(),
            file_instance_id,
            &document_id,
            relinked_at_ms,
            json!({
                "previousStorageRootId": previous_storage_root_id,
                "newStorageRootId": target_root.storage_root_id,
                "previousRelativePath": previous_relative_path_display,
                "newRelativePath": relative_path_display,
                "previousAvailabilityState": availability_state,
                "newAvailabilityState": "CHANGED",
                "identityMatchesPrevious": identity_matches,
                "contentMetadataChanged": content_changed,
                "reconciliationRequired": true
            })
            .to_string()
        ],
    )?;

    enqueue_document_projection(&transaction, file_instance_id, relinked_at_ms)?;
    transaction.commit()?;

    let extension = relative_path
        .extension()
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_default();

    Ok(IndexedFilePreviewRecord {
        document_id,
        file_instance_id: file_instance_id.to_string(),
        name: display_name,
        path: canonical_source.to_string_lossy().into_owned(),
        extension,
        size_bytes,
        modified_unix_ms: last_write_time_ms,
        availability_state: "CHANGED".to_string(),
    })
}

pub fn reconcile_linked_file_instance(
    database_path: &Path,
    file_instance_id: &str,
) -> Result<(), PersistenceError> {
    type ReconcileRow = (
        String,
        String,
        String,
        String,
        Option<String>,
        Vec<u8>,
        String,
        Vec<u8>,
        String,
        i64,
        Option<i64>,
        Option<i64>,
        Option<Vec<u8>>,
        Option<Vec<u8>>,
    );

    let mut connection = open_configured_connection(database_path)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

    let row: Option<ReconcileRow> = transaction
        .query_row(
            "SELECT
                fi.document_id,
                d.storage_state,
                fi.availability_state,
                sr.availability_state,
                (
                    SELECT cv.content_version_id
                    FROM content_versions cv
                    WHERE cv.file_instance_id = fi.file_instance_id
                    ORDER BY cv.observed_at_ms DESC, cv.rowid DESC
                    LIMIT 1
                ),
                COALESCE(sr.canonical_native_locator, sr.native_locator),
                sr.native_locator_encoding,
                fi.relative_path_native,
                fi.path_native_encoding,
                fi.size_bytes,
                fi.creation_time_ms,
                fi.last_write_time_ms,
                fi.filesystem_identity,
                fi.volume_identity
             FROM file_instances fi
             JOIN documents d ON d.document_id = fi.document_id
             JOIN storage_roots sr ON sr.storage_root_id = fi.storage_root_id
             WHERE fi.file_instance_id = ?1
               AND d.archived_at_ms IS NULL",
            [file_instance_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                    row.get(9)?,
                    row.get(10)?,
                    row.get(11)?,
                    row.get(12)?,
                    row.get(13)?,
                ))
            },
        )
        .optional()?;

    let Some((
        document_id,
        storage_state,
        availability_state,
        root_availability_state,
        content_version_id,
        root_native,
        root_encoding,
        relative_native,
        relative_encoding,
        size_bytes,
        creation_time_ms,
        last_write_time_ms,
        filesystem_identity,
        volume_identity,
    )) = row
    else {
        return Err(PersistenceError::Configuration(format!(
            "file instance {file_instance_id} does not exist"
        )));
    };

    if !supports_linked_source_lifecycle(&storage_state) {
        return Err(PersistenceError::Configuration(format!(
            "document storage state {storage_state} does not support linked-source reconciliation"
        )));
    }

    if root_availability_state != "AVAILABLE" {
        return Err(PersistenceError::Configuration(format!(
            "storage root is {root_availability_state}; complete a successful rescan before reconciliation"
        )));
    }

    if availability_state != "CHANGED" {
        return Err(PersistenceError::Configuration(format!(
            "file instance {file_instance_id} is {availability_state}; only CHANGED linked sources require reconciliation"
        )));
    }

    let storage_root_path = decode_native_path(&root_native, &root_encoding)?;
    let relative_path = decode_native_path(&relative_native, &relative_encoding)?;

    if relative_path.components().any(|component| {
        matches!(
            component,
            std::path::Component::ParentDir
                | std::path::Component::RootDir
                | std::path::Component::Prefix(_)
        )
    }) {
        return Err(PersistenceError::Configuration(
            "stored file-instance path is not root-relative".to_string(),
        ));
    }

    let canonical_root = fs::canonicalize(&storage_root_path).map_err(|error| {
        PersistenceError::Configuration(format!(
            "approved storage root became unavailable before reconciliation: {error}"
        ))
    })?;
    let indexed_path = storage_root_path.join(&relative_path);
    let canonical_source = fs::canonicalize(&indexed_path).map_err(|error| {
        PersistenceError::Configuration(format!(
            "linked source became unavailable before reconciliation: {error}"
        ))
    })?;

    if !canonical_source.starts_with(&canonical_root) {
        return Err(PersistenceError::Configuration(
            "resolved linked source escaped its approved storage root".to_string(),
        ));
    }

    if !canonical_source.is_file() {
        return Err(PersistenceError::Configuration(
            "linked source is no longer a regular file".to_string(),
        ));
    }

    let source_file = fs::File::open(&canonical_source).map_err(|error| {
        PersistenceError::Configuration(format!(
            "linked source could not be opened for reconciliation validation: {error}"
        ))
    })?;
    let metadata = source_file.metadata().map_err(|error| {
        PersistenceError::Configuration(format!(
            "linked source metadata could not be read for reconciliation validation: {error}"
        ))
    })?;
    let platform = filesystem::platform_file_metadata_from_open_file(&source_file, &metadata);

    let system_time_to_ms = |value: SystemTime| {
        value
            .duration_since(UNIX_EPOCH)
            .ok()
            .and_then(|duration| i64::try_from(duration.as_millis()).ok())
    };
    let current_creation_time_ms = metadata.created().ok().and_then(system_time_to_ms);
    let current_last_write_time_ms = metadata.modified().ok().and_then(system_time_to_ms);
    let indexed_size_bytes = u64::try_from(size_bytes).map_err(|_| {
        PersistenceError::Configuration(
            "stored linked-source size cannot be represented as an unsigned value".to_string(),
        )
    })?;

    let metadata_matches_index = metadata.len() == indexed_size_bytes
        && current_creation_time_ms == creation_time_ms
        && current_last_write_time_ms == last_write_time_ms
        && platform.filesystem_identity == filesystem_identity
        && platform.volume_identity == volume_identity;

    if !metadata_matches_index {
        return Err(PersistenceError::Configuration(
            "linked source changed again since the last successful scan; rescan before reconciliation"
                .to_string(),
        ));
    }

    let reconciled_at_ms = now_unix_ms()?;
    transaction.execute(
        "UPDATE file_instances
         SET availability_state = 'AVAILABLE'
         WHERE file_instance_id = ?1",
        [file_instance_id],
    )?;

    let details = json!({
        "previousAvailabilityState": "CHANGED",
        "newAvailabilityState": "AVAILABLE",
        "contentVersionId": content_version_id,
        "reconciliationPolicy": "USER_ACCEPTED_REVALIDATED_INDEXED_SOURCE",
        "validatedSizeBytes": indexed_size_bytes,
        "validatedLastWriteTimeMs": current_last_write_time_ms
    });

    transaction.execute(
        "INSERT INTO audit_events (
            audit_event_id,
            event_type,
            entity_type,
            entity_id,
            related_entity_type,
            related_entity_id,
            occurred_at_ms,
            actor_id,
            details_json
         ) VALUES (?1, 'LINKED_SOURCE_RECONCILED', 'FILE_INSTANCE', ?2, 'DOCUMENT', ?3, ?4, NULL, ?5)",
        params![
            Uuid::new_v4().to_string(),
            file_instance_id,
            document_id,
            reconciled_at_ms,
            details.to_string()
        ],
    )?;

    enqueue_document_projection(&transaction, file_instance_id, reconciled_at_ms)?;
    transaction.commit()?;
    Ok(())
}

#[cfg(test)]
pub(crate) fn count_content_versions_for_test(
    database_path: &Path,
    file_instance_id: &str,
) -> Result<u64, PersistenceError> {
    let connection = open_configured_connection(database_path)?;
    let count: i64 = connection.query_row(
        "SELECT COUNT(*)
         FROM content_versions
         WHERE file_instance_id = ?1",
        [file_instance_id],
        |row| row.get(0),
    )?;
    Ok(count.max(0) as u64)
}

#[cfg(test)]
pub(crate) fn count_audit_events_for_test(
    database_path: &Path,
    event_type: &str,
    entity_id: &str,
) -> Result<u64, PersistenceError> {
    let connection = open_configured_connection(database_path)?;
    let count: i64 = connection.query_row(
        "SELECT COUNT(*)
         FROM audit_events
         WHERE event_type = ?1
           AND entity_id = ?2",
        params![event_type, entity_id],
        |row| row.get(0),
    )?;
    Ok(count.max(0) as u64)
}

#[cfg(test)]
pub(crate) fn path_history_reasons_for_test(
    database_path: &Path,
    file_instance_id: &str,
) -> Result<Vec<String>, PersistenceError> {
    let connection = open_configured_connection(database_path)?;
    let mut statement = connection.prepare(
        "SELECT change_reason
         FROM file_path_history
         WHERE file_instance_id = ?1
         ORDER BY observed_from_ms, rowid",
    )?;

    let rows = statement.query_map([file_instance_id], |row| row.get::<_, String>(0))?;
    let mut reasons = Vec::new();
    for row in rows {
        reasons.push(row?);
    }
    Ok(reasons)
}
pub fn list_search_projection_snapshot(
    database_path: &Path,
) -> Result<Vec<SearchProjectionRecord>, PersistenceError> {
    let connection = open_configured_connection(database_path)?;
    let mut statement = connection.prepare(
        "SELECT
            d.document_id,
            fi.file_instance_id,
            fi.storage_root_id,
            d.display_name,
            fi.relative_path_display,
            fi.availability_state
         FROM file_instances fi
         JOIN documents d ON d.document_id = fi.document_id
         WHERE d.archived_at_ms IS NULL
         ORDER BY fi.file_instance_id",
    )?;

    let rows = statement.query_map([], |row| {
        Ok(SearchProjectionRecord {
            document_id: row.get(0)?,
            file_instance_id: row.get(1)?,
            storage_root_id: row.get(2)?,
            display_name: row.get(3)?,
            relative_path_display: row.get(4)?,
            availability_state: row.get(5)?,
        })
    })?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

pub fn list_pending_search_outbox(
    database_path: &Path,
    limit: u32,
) -> Result<Vec<SearchOutboxRecord>, PersistenceError> {
    let connection = open_configured_connection(database_path)?;
    let bounded_limit = i64::from(limit.clamp(1, 5_000));
    let mut statement = connection.prepare(
        "SELECT
            operation_id,
            entity_id,
            operation,
            payload_version,
            payload_json
         FROM search_index_outbox
         WHERE acknowledged_at_ms IS NULL
         ORDER BY created_at_ms, rowid
         LIMIT ?1",
    )?;

    let rows = statement.query_map([bounded_limit], |row| {
        let payload_version: i64 = row.get(3)?;
        Ok(SearchOutboxRecord {
            operation_id: row.get(0)?,
            entity_id: row.get(1)?,
            operation: row.get(2)?,
            payload_version: payload_version.max(0) as u32,
            payload_json: row.get(4)?,
        })
    })?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

pub fn list_pending_search_outbox_ids(
    database_path: &Path,
) -> Result<Vec<String>, PersistenceError> {
    let connection = open_configured_connection(database_path)?;
    let mut statement = connection.prepare(
        "SELECT operation_id
         FROM search_index_outbox
         WHERE acknowledged_at_ms IS NULL
         ORDER BY created_at_ms, rowid",
    )?;

    let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

pub fn acknowledge_search_outbox(
    database_path: &Path,
    operation_ids: &[String],
) -> Result<(), PersistenceError> {
    if operation_ids.is_empty() {
        return Ok(());
    }

    let mut connection = open_configured_connection(database_path)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let acknowledged_at_ms = now_unix_ms()?;

    for operation_id in operation_ids {
        transaction.execute(
            "UPDATE search_index_outbox
             SET acknowledged_at_ms = ?1,
                 last_error = NULL
             WHERE operation_id = ?2
               AND acknowledged_at_ms IS NULL",
            params![acknowledged_at_ms, operation_id],
        )?;
    }

    transaction.commit()?;
    Ok(())
}

pub fn record_search_outbox_failure(
    database_path: &Path,
    operation_ids: &[String],
    message: &str,
) -> Result<(), PersistenceError> {
    if operation_ids.is_empty() {
        return Ok(());
    }

    let mut connection = open_configured_connection(database_path)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

    for operation_id in operation_ids {
        transaction.execute(
            "UPDATE search_index_outbox
             SET attempt_count = attempt_count + 1,
                 last_error = ?1
             WHERE operation_id = ?2
               AND acknowledged_at_ms IS NULL",
            params![message, operation_id],
        )?;
    }

    transaction.commit()?;
    Ok(())
}

pub fn begin_evidence_capture(
    database_path: &Path,
    capture_job_id: &str,
    file_instance_id: &str,
    capture_reason: &str,
    capture_policy: &str,
) -> Result<EvidenceCaptureSourceRecord, PersistenceError> {
    let mut connection = open_configured_connection(database_path)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

    type CaptureRow = (
        String,
        Vec<u8>,
        String,
        Vec<u8>,
        String,
        i64,
        Option<i64>,
        Option<i64>,
        Option<Vec<u8>>,
        Option<Vec<u8>>,
        String,
        String,
    );

    let row: Option<CaptureRow> = transaction
        .query_row(
            "SELECT
                fi.document_id,
                COALESCE(sr.canonical_native_locator, sr.native_locator),
                sr.native_locator_encoding,
                fi.relative_path_native,
                fi.path_native_encoding,
                fi.size_bytes,
                fi.creation_time_ms,
                fi.last_write_time_ms,
                fi.filesystem_identity,
                fi.volume_identity,
                fi.availability_state,
                sr.availability_state
             FROM file_instances fi
             JOIN storage_roots sr ON sr.storage_root_id = fi.storage_root_id
             JOIN documents d ON d.document_id = fi.document_id
             WHERE fi.file_instance_id = ?1
               AND d.archived_at_ms IS NULL",
            [file_instance_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                    row.get(9)?,
                    row.get(10)?,
                    row.get(11)?,
                ))
            },
        )
        .optional()?;

    let Some((
        document_id,
        root_native,
        root_encoding,
        relative_native,
        relative_encoding,
        size_bytes,
        creation_time_ms,
        last_write_time_ms,
        filesystem_identity,
        volume_identity,
        availability_state,
        root_availability_state,
    )) = row
    else {
        return Err(PersistenceError::Configuration(format!(
            "file instance {file_instance_id} does not exist"
        )));
    };

    if root_availability_state != "AVAILABLE" {
        return Err(PersistenceError::Configuration(format!(
            "storage root is {root_availability_state}; controlled evidence capture requires an available approved source root"
        )));
    }

    if availability_state != "AVAILABLE" {
        return Err(PersistenceError::Configuration(format!(
            "file instance {file_instance_id} is {availability_state}; reconcile it before controlled evidence capture"
        )));
    }

    let storage_root_path = decode_native_path(&root_native, &root_encoding)?;
    let relative_path = decode_native_path(&relative_native, &relative_encoding)?;

    if relative_path.components().any(|component| {
        matches!(
            component,
            std::path::Component::ParentDir
                | std::path::Component::RootDir
                | std::path::Component::Prefix(_)
        )
    }) {
        return Err(PersistenceError::Configuration(
            "stored file-instance path is not root-relative".to_string(),
        ));
    }

    let now = now_unix_ms()?;
    transaction.execute(
        "INSERT INTO evidence_capture_jobs (
            evidence_capture_job_id,
            file_instance_id,
            document_id,
            status,
            requested_at_ms,
            started_at_ms,
            completed_at_ms,
            capture_reason,
            capture_policy,
            failure_code,
            failure_message
         ) VALUES (?1, ?2, ?3, 'CAPTURING', ?4, ?4, NULL, ?5, ?6, NULL, NULL)",
        params![
            capture_job_id,
            file_instance_id,
            &document_id,
            now,
            capture_reason,
            capture_policy
        ],
    )?;

    transaction.execute(
        "INSERT INTO audit_events (
            audit_event_id,
            event_type,
            entity_type,
            entity_id,
            related_entity_type,
            related_entity_id,
            occurred_at_ms,
            actor_id,
            details_json
         ) VALUES (?1, 'EVIDENCE_CAPTURE_STARTED', 'EVIDENCE_CAPTURE_JOB', ?2, 'DOCUMENT', ?3, ?4, NULL, ?5)",
        params![
            Uuid::new_v4().to_string(),
            capture_job_id,
            &document_id,
            now,
            json!({
                "fileInstanceId": file_instance_id,
                "captureReason": capture_reason,
                "capturePolicy": capture_policy
            })
            .to_string()
        ],
    )?;

    transaction.commit()?;

    Ok(EvidenceCaptureSourceRecord {
        document_id,
        file_instance_id: file_instance_id.to_string(),
        source: ResolvedFileSource {
            storage_root_path,
            relative_path,
        },
        size_bytes: size_bytes.max(0) as u64,
        creation_time_ms,
        last_write_time_ms,
        filesystem_identity,
        volume_identity,
    })
}

pub fn list_document_version_history(
    database_path: &Path,
    document_id: &str,
) -> Result<Vec<DocumentVersionHistoryRecord>, PersistenceError> {
    let connection = open_configured_connection(database_path)?;
    let exists: bool = connection.query_row(
        "SELECT EXISTS(
            SELECT 1
            FROM documents
            WHERE document_id = ?1
              AND archived_at_ms IS NULL
        )",
        [document_id],
        |row| row.get(0),
    )?;

    if !exists {
        return Err(PersistenceError::Configuration(format!(
            "document {document_id} does not exist"
        )));
    }

    let mut statement = connection.prepare(
        "SELECT
            cv.content_version_id,
            cv.observed_at_ms,
            cv.size_bytes,
            cv.last_write_time_ms,
            cv.verification_state,
            cv.source_stable_during_read,
            cv.sha256,
            cev.controlled_evidence_version_id,
            cev.version_number,
            cev.captured_at_ms,
            cev.verification_state,
            cev.captured_by,
            cev.capture_reason,
            cev.capture_policy
         FROM content_versions cv
         LEFT JOIN controlled_evidence_versions cev
           ON cev.source_content_version_id = cv.content_version_id
         WHERE cv.document_id = ?1
         ORDER BY cv.observed_at_ms DESC, cv.rowid DESC",
    )?;

    let rows = statement.query_map([document_id], |row| {
        let source_stable: Option<i64> = row.get(5)?;
        let controlled_version_number: Option<i64> = row.get(8)?;

        Ok(DocumentVersionHistoryRecord {
            content_version_id: row.get(0)?,
            observed_at_ms: row.get(1)?,
            size_bytes: row.get::<_, i64>(2)?.max(0) as u64,
            last_write_time_ms: row.get(3)?,
            verification_state: row.get(4)?,
            source_stable_during_read: source_stable.map(|value| value != 0),
            sha256: row.get(6)?,
            controlled_evidence_version_id: row.get(7)?,
            controlled_version_number: controlled_version_number.map(|value| value.max(0) as u64),
            captured_at_ms: row.get(9)?,
            controlled_verification_state: row.get(10)?,
            captured_by: row.get(11)?,
            capture_reason: row.get(12)?,
            capture_policy: row.get(13)?,
        })
    })?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

pub fn complete_evidence_capture(
    database_path: &Path,
    completion: EvidenceCaptureCompletion<'_>,
) -> Result<ControlledEvidenceVersionRecord, PersistenceError> {
    let mut connection = open_configured_connection(database_path)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

    let job: Option<(String, String, String, String, String)> = transaction
        .query_row(
            "SELECT
                document_id,
                file_instance_id,
                status,
                capture_reason,
                capture_policy
             FROM evidence_capture_jobs
             WHERE evidence_capture_job_id = ?1",
            [completion.capture_job_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .optional()?;

    let Some((job_document_id, job_file_instance_id, status, capture_reason, capture_policy)) = job
    else {
        return Err(PersistenceError::Configuration(format!(
            "evidence capture job {} does not exist",
            completion.capture_job_id
        )));
    };

    if status != "CAPTURING"
        || job_document_id != completion.document_id
        || job_file_instance_id != completion.file_instance_id
    {
        return Err(PersistenceError::Configuration(
            "evidence capture job no longer matches the source being finalized".to_string(),
        ));
    }

    let version_number: i64 = transaction.query_row(
        "SELECT COALESCE(MAX(version_number), 0) + 1
         FROM controlled_evidence_versions
         WHERE document_id = ?1",
        [completion.document_id],
        |row| row.get(0),
    )?;

    let now = now_unix_ms()?;
    let source_content_version_id = Uuid::new_v4().to_string();

    transaction.execute(
        "INSERT INTO content_versions (
            content_version_id,
            document_id,
            file_instance_id,
            observed_at_ms,
            size_bytes,
            last_write_time_ms,
            quick_fingerprint,
            sha256,
            verification_state,
            source_stable_during_read
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7, 'HASH_VERIFIED', 1)",
        params![
            &source_content_version_id,
            completion.document_id,
            completion.file_instance_id,
            now,
            u64_to_i64(completion.size_bytes)?,
            completion.last_write_time_ms,
            &completion.sha256[..]
        ],
    )?;

    transaction.execute(
        "INSERT INTO controlled_evidence_versions (
            controlled_evidence_version_id,
            document_id,
            source_file_instance_id,
            source_content_version_id,
            evidence_capture_job_id,
            version_number,
            controlled_storage_locator,
            sha256,
            size_bytes,
            captured_at_ms,
            captured_by,
            capture_reason,
            capture_policy,
            retention_state,
            verification_state,
            source_stable_during_read
         ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, NULL, ?11, ?12,
            'RETAINED', 'HASH_VERIFIED', 1
         )",
        params![
            completion.controlled_evidence_version_id,
            completion.document_id,
            completion.file_instance_id,
            &source_content_version_id,
            completion.capture_job_id,
            version_number,
            completion.controlled_storage_locator,
            &completion.sha256[..],
            u64_to_i64(completion.size_bytes)?,
            now,
            &capture_reason,
            &capture_policy
        ],
    )?;

    transaction.execute(
        "UPDATE documents
         SET storage_state = 'CONTROLLED_EVIDENCE'
         WHERE document_id = ?1",
        [completion.document_id],
    )?;

    let changed = transaction.execute(
        "UPDATE evidence_capture_jobs
         SET status = 'COMPLETE',
             completed_at_ms = ?1,
             failure_code = NULL,
             failure_message = NULL
         WHERE evidence_capture_job_id = ?2
           AND status = 'CAPTURING'",
        params![now, completion.capture_job_id],
    )?;

    if changed != 1 {
        return Err(PersistenceError::Configuration(
            "evidence capture job could not be finalized".to_string(),
        ));
    }

    transaction.execute(
        "INSERT INTO audit_events (
            audit_event_id,
            event_type,
            entity_type,
            entity_id,
            related_entity_type,
            related_entity_id,
            occurred_at_ms,
            actor_id,
            details_json
         ) VALUES (?1, 'EVIDENCE_CAPTURE_COMPLETED', 'CONTROLLED_EVIDENCE_VERSION', ?2, 'DOCUMENT', ?3, ?4, NULL, ?5)",
        params![
            Uuid::new_v4().to_string(),
            completion.controlled_evidence_version_id,
            completion.document_id,
            now,
            json!({
                "captureJobId": completion.capture_job_id,
                "fileInstanceId": completion.file_instance_id,
                "sourceContentVersionId": source_content_version_id,
                "versionNumber": version_number,
                "sizeBytes": completion.size_bytes
            })
            .to_string()
        ],
    )?;

    transaction.commit()?;

    Ok(ControlledEvidenceVersionRecord {
        controlled_evidence_version_id: completion.controlled_evidence_version_id.to_string(),
        evidence_capture_job_id: completion.capture_job_id.to_string(),
        document_id: completion.document_id.to_string(),
        source_content_version_id,
        version_number: version_number.max(0) as u64,
        sha256: completion.sha256.to_vec(),
        size_bytes: completion.size_bytes,
        captured_at_ms: now,
        verification_state: "HASH_VERIFIED".to_string(),
    })
}

pub fn finish_evidence_capture_failure(
    database_path: &Path,
    capture_job_id: &str,
    status: &str,
    failure_code: &str,
    failure_message: &str,
) -> Result<(), PersistenceError> {
    if status != "FAILED" && status != "QUARANTINED" {
        return Err(PersistenceError::Configuration(format!(
            "unsupported evidence capture failure status {status}"
        )));
    }

    let mut connection = open_configured_connection(database_path)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let now = now_unix_ms()?;

    let row: Option<(String, String)> = transaction
        .query_row(
            "SELECT document_id, status
             FROM evidence_capture_jobs
             WHERE evidence_capture_job_id = ?1",
            [capture_job_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;

    let Some((document_id, current_status)) = row else {
        return Ok(());
    };

    if current_status != "CAPTURING" {
        return Ok(());
    }

    transaction.execute(
        "UPDATE evidence_capture_jobs
         SET status = ?1,
             completed_at_ms = ?2,
             failure_code = ?3,
             failure_message = ?4
         WHERE evidence_capture_job_id = ?5
           AND status = 'CAPTURING'",
        params![status, now, failure_code, failure_message, capture_job_id],
    )?;

    let event_type = if status == "QUARANTINED" {
        "EVIDENCE_CAPTURE_QUARANTINED"
    } else {
        "EVIDENCE_CAPTURE_FAILED"
    };

    transaction.execute(
        "INSERT INTO audit_events (
            audit_event_id,
            event_type,
            entity_type,
            entity_id,
            related_entity_type,
            related_entity_id,
            occurred_at_ms,
            actor_id,
            details_json
         ) VALUES (?1, ?2, 'EVIDENCE_CAPTURE_JOB', ?3, 'DOCUMENT', ?4, ?5, NULL, ?6)",
        params![
            Uuid::new_v4().to_string(),
            event_type,
            capture_job_id,
            document_id,
            now,
            json!({
                "failureCode": failure_code,
                "failureMessage": failure_message
            })
            .to_string()
        ],
    )?;

    transaction.commit()?;
    Ok(())
}

fn current_file_preview_for_document(
    connection: &Connection,
    document_id: &str,
) -> Result<Option<IndexedFilePreviewRecord>, PersistenceError> {
    type CurrentFileRow = (
        String,
        String,
        String,
        Vec<u8>,
        String,
        Vec<u8>,
        String,
        i64,
        Option<i64>,
        String,
        String,
    );

    let row: Option<CurrentFileRow> = connection
        .query_row(
            "SELECT
                d.document_id,
                fi.file_instance_id,
                d.display_name,
                COALESCE(sr.canonical_native_locator, sr.native_locator),
                sr.native_locator_encoding,
                fi.relative_path_native,
                fi.path_native_encoding,
                fi.size_bytes,
                fi.last_write_time_ms,
                fi.availability_state,
                sr.availability_state
             FROM documents d
             JOIN file_instances fi ON fi.document_id = d.document_id
             JOIN storage_roots sr ON sr.storage_root_id = fi.storage_root_id
             WHERE d.document_id = ?1
               AND d.archived_at_ms IS NULL
             ORDER BY
                CASE fi.availability_state
                    WHEN 'AVAILABLE' THEN 0
                    WHEN 'CHANGED' THEN 1
                    WHEN 'UNAVAILABLE' THEN 2
                    WHEN 'UNKNOWN' THEN 3
                    WHEN 'MISSING' THEN 4
                    ELSE 5
                END,
                fi.last_seen_at_ms DESC,
                fi.rowid DESC
             LIMIT 1",
            [document_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                    row.get(9)?,
                    row.get(10)?,
                ))
            },
        )
        .optional()?;

    let Some((
        document_id,
        file_instance_id,
        name,
        root_native,
        root_encoding,
        relative_native,
        relative_encoding,
        size_bytes,
        modified_unix_ms,
        file_availability_state,
        root_availability_state,
    )) = row
    else {
        return Ok(None);
    };

    let root_path = decode_native_path(&root_native, &root_encoding)?;
    let relative_path = decode_native_path(&relative_native, &relative_encoding)?;

    if relative_path.components().any(|component| {
        matches!(
            component,
            std::path::Component::ParentDir
                | std::path::Component::RootDir
                | std::path::Component::Prefix(_)
        )
    }) {
        return Err(PersistenceError::Configuration(
            "stored related-file path is not root-relative".to_string(),
        ));
    }

    let extension = relative_path
        .extension()
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_default();

    Ok(Some(IndexedFilePreviewRecord {
        document_id,
        file_instance_id,
        name,
        path: root_path
            .join(&relative_path)
            .to_string_lossy()
            .into_owned(),
        extension,
        size_bytes: size_bytes.max(0) as u64,
        modified_unix_ms,
        availability_state: effective_file_availability_state(
            &file_availability_state,
            &root_availability_state,
        ),
    }))
}

pub fn list_document_relationships(
    database_path: &Path,
    document_id: &str,
) -> Result<Vec<DocumentRelationshipRecord>, PersistenceError> {
    let connection = open_configured_connection(database_path)?;
    let document_exists: bool = connection.query_row(
        "SELECT EXISTS(
            SELECT 1
            FROM documents
            WHERE document_id = ?1
              AND archived_at_ms IS NULL
        )",
        [document_id],
        |row| row.get(0),
    )?;

    if !document_exists {
        return Err(PersistenceError::Configuration(format!(
            "document {document_id} does not exist"
        )));
    }

    let mut statement = connection.prepare(
        "SELECT
            document_relationship_id,
            source_document_id,
            target_document_id,
            relationship_type,
            created_at_ms
         FROM document_relationships
         WHERE removed_at_ms IS NULL
           AND (source_document_id = ?1 OR target_document_id = ?1)
         ORDER BY created_at_ms DESC, rowid DESC",
    )?;

    let rows = statement.query_map([document_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, i64>(4)?,
        ))
    })?;

    let mut relationships = Vec::new();
    for row in rows {
        let (
            document_relationship_id,
            source_document_id,
            target_document_id,
            relationship_type,
            created_at_ms,
        ) = row?;
        let outgoing = source_document_id == document_id;
        let related_document_id = if outgoing {
            target_document_id
        } else {
            source_document_id
        };
        let related_document_name: String = connection.query_row(
            "SELECT display_name
             FROM documents
             WHERE document_id = ?1",
            [&related_document_id],
            |row| row.get(0),
        )?;
        let related_file = current_file_preview_for_document(&connection, &related_document_id)?;

        relationships.push(DocumentRelationshipRecord {
            document_relationship_id,
            relationship_type,
            direction: if outgoing {
                "OUTGOING".to_string()
            } else {
                "INCOMING".to_string()
            },
            created_at_ms,
            related_document_id,
            related_document_name,
            related_file,
        });
    }

    Ok(relationships)
}

pub fn create_document_relationship(
    database_path: &Path,
    source_document_id: &str,
    target_document_id: &str,
    relationship_type: &str,
) -> Result<String, PersistenceError> {
    if source_document_id == target_document_id {
        return Err(PersistenceError::Configuration(
            "a document cannot be related to itself".to_string(),
        ));
    }

    let normalized_type = relationship_type
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if normalized_type.is_empty() || normalized_type.chars().count() > 80 {
        return Err(PersistenceError::Configuration(
            "relationship type must contain 1 to 80 characters".to_string(),
        ));
    }

    let mut connection = open_configured_connection(database_path)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

    for document_id in [source_document_id, target_document_id] {
        let exists: bool = transaction.query_row(
            "SELECT EXISTS(
                SELECT 1
                FROM documents
                WHERE document_id = ?1
                  AND archived_at_ms IS NULL
            )",
            [document_id],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(PersistenceError::Configuration(format!(
                "document {document_id} does not exist"
            )));
        }
    }

    let existing_id: Option<String> = transaction
        .query_row(
            "SELECT document_relationship_id
             FROM document_relationships
             WHERE source_document_id = ?1
               AND target_document_id = ?2
               AND relationship_type = ?3 COLLATE NOCASE
               AND removed_at_ms IS NULL
             LIMIT 1",
            params![source_document_id, target_document_id, &normalized_type],
            |row| row.get(0),
        )
        .optional()?;

    if let Some(existing_id) = existing_id {
        transaction.commit()?;
        return Ok(existing_id);
    }

    let relationship_id = Uuid::new_v4().to_string();
    let now = now_unix_ms()?;
    transaction.execute(
        "INSERT INTO document_relationships (
            document_relationship_id,
            source_document_id,
            target_document_id,
            relationship_type,
            created_at_ms,
            created_by,
            removed_at_ms,
            removed_by
         ) VALUES (?1, ?2, ?3, ?4, ?5, NULL, NULL, NULL)",
        params![
            &relationship_id,
            source_document_id,
            target_document_id,
            &normalized_type,
            now
        ],
    )?;

    transaction.execute(
        "INSERT INTO audit_events (
            audit_event_id,
            event_type,
            entity_type,
            entity_id,
            related_entity_type,
            related_entity_id,
            occurred_at_ms,
            actor_id,
            details_json
         ) VALUES (?1, 'DOCUMENT_RELATIONSHIP_CREATED', 'DOCUMENT_RELATIONSHIP', ?2, 'DOCUMENT', ?3, ?4, NULL, ?5)",
        params![
            Uuid::new_v4().to_string(),
            &relationship_id,
            target_document_id,
            now,
            json!({
                "sourceDocumentId": source_document_id,
                "targetDocumentId": target_document_id,
                "relationshipType": normalized_type
            })
            .to_string()
        ],
    )?;

    transaction.commit()?;
    Ok(relationship_id)
}

pub fn remove_document_relationship(
    database_path: &Path,
    document_relationship_id: &str,
) -> Result<(), PersistenceError> {
    let mut connection = open_configured_connection(database_path)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

    let relationship: Option<(String, String, String, Option<i64>)> = transaction
        .query_row(
            "SELECT
                source_document_id,
                target_document_id,
                relationship_type,
                removed_at_ms
             FROM document_relationships
             WHERE document_relationship_id = ?1",
            [document_relationship_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()?;

    let Some((source_document_id, target_document_id, relationship_type, removed_at_ms)) =
        relationship
    else {
        return Err(PersistenceError::Configuration(format!(
            "document relationship {document_relationship_id} does not exist"
        )));
    };

    if removed_at_ms.is_some() {
        transaction.commit()?;
        return Ok(());
    }

    let now = now_unix_ms()?;
    transaction.execute(
        "UPDATE document_relationships
         SET removed_at_ms = ?1,
             removed_by = NULL
         WHERE document_relationship_id = ?2
           AND removed_at_ms IS NULL",
        params![now, document_relationship_id],
    )?;

    transaction.execute(
        "INSERT INTO audit_events (
            audit_event_id,
            event_type,
            entity_type,
            entity_id,
            related_entity_type,
            related_entity_id,
            occurred_at_ms,
            actor_id,
            details_json
         ) VALUES (?1, 'DOCUMENT_RELATIONSHIP_REMOVED', 'DOCUMENT_RELATIONSHIP', ?2, 'DOCUMENT', ?3, ?4, NULL, ?5)",
        params![
            Uuid::new_v4().to_string(),
            document_relationship_id,
            target_document_id,
            now,
            json!({
                "sourceDocumentId": source_document_id,
                "targetDocumentId": target_document_id,
                "relationshipType": relationship_type
            })
            .to_string()
        ],
    )?;

    transaction.commit()?;
    Ok(())
}

fn normalize_domain_label(
    value: &str,
    field_name: &str,
    max_chars: usize,
) -> Result<String, PersistenceError> {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let count = normalized.chars().count();
    if count == 0 || count > max_chars {
        return Err(PersistenceError::Configuration(format!(
            "{field_name} must contain 1 to {max_chars} characters"
        )));
    }
    Ok(normalized)
}

fn normalize_optional_domain_text(value: Option<&str>, max_chars: usize) -> Option<String> {
    value.and_then(|raw| {
        let normalized = raw.split_whitespace().collect::<Vec<_>>().join(" ");
        if normalized.is_empty() {
            None
        } else {
            Some(normalized.chars().take(max_chars).collect())
        }
    })
}

fn bytes_to_lower_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

struct DomainAuditEvent<'a> {
    event_type: &'a str,
    entity_type: &'a str,
    entity_id: &'a str,
    related_entity_type: Option<&'a str>,
    related_entity_id: Option<&'a str>,
    occurred_at_ms: i64,
    details: serde_json::Value,
}

fn insert_domain_audit_event(
    transaction: &rusqlite::Transaction<'_>,
    event: DomainAuditEvent<'_>,
) -> Result<(), PersistenceError> {
    transaction.execute(
        "INSERT INTO audit_events (
            audit_event_id,
            event_type,
            entity_type,
            entity_id,
            related_entity_type,
            related_entity_id,
            occurred_at_ms,
            actor_id,
            details_json
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, ?8)",
        params![
            Uuid::new_v4().to_string(),
            event.event_type,
            event.entity_type,
            event.entity_id,
            event.related_entity_type,
            event.related_entity_id,
            event.occurred_at_ms,
            event.details.to_string()
        ],
    )?;
    Ok(())
}

pub fn create_client(database_path: &Path, name: &str) -> Result<ClientRecord, PersistenceError> {
    let name = normalize_domain_label(name, "client name", 200)?;
    let client_id = Uuid::new_v4().to_string();
    let now = now_unix_ms()?;
    let connection = open_configured_connection(database_path)?;
    connection.execute(
        "INSERT INTO clients (client_id, name, created_at_ms, archived_at_ms)
         VALUES (?1, ?2, ?3, NULL)",
        params![&client_id, &name, now],
    )?;

    Ok(ClientRecord {
        client_id,
        name,
        created_at_ms: now,
    })
}

pub fn list_clients(database_path: &Path) -> Result<Vec<ClientRecord>, PersistenceError> {
    let connection = open_configured_connection(database_path)?;
    let mut statement = connection.prepare(
        "SELECT client_id, name, created_at_ms
         FROM clients
         WHERE archived_at_ms IS NULL
         ORDER BY name COLLATE NOCASE, created_at_ms",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(ClientRecord {
            client_id: row.get(0)?,
            name: row.get(1)?,
            created_at_ms: row.get(2)?,
        })
    })?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

pub fn create_service_type(
    database_path: &Path,
    name: &str,
) -> Result<ServiceTypeRecord, PersistenceError> {
    let name = normalize_domain_label(name, "service type name", 120)?;
    let normalized_name = normalize_search_text(&name);
    if normalized_name.is_empty() {
        return Err(PersistenceError::Configuration(
            "service type name must contain searchable characters".to_string(),
        ));
    }

    let service_type_id = Uuid::new_v4().to_string();
    let now = now_unix_ms()?;
    let connection = open_configured_connection(database_path)?;
    connection.execute(
        "INSERT INTO service_types (
            service_type_id,
            name,
            normalized_name,
            created_at_ms,
            archived_at_ms
         ) VALUES (?1, ?2, ?3, ?4, NULL)",
        params![&service_type_id, &name, &normalized_name, now],
    )?;

    Ok(ServiceTypeRecord {
        service_type_id,
        name,
        created_at_ms: now,
    })
}

pub fn list_service_types(
    database_path: &Path,
) -> Result<Vec<ServiceTypeRecord>, PersistenceError> {
    let connection = open_configured_connection(database_path)?;
    let mut statement = connection.prepare(
        "SELECT service_type_id, name, created_at_ms
         FROM service_types
         WHERE archived_at_ms IS NULL
         ORDER BY name COLLATE NOCASE, created_at_ms",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(ServiceTypeRecord {
            service_type_id: row.get(0)?,
            name: row.get(1)?,
            created_at_ms: row.get(2)?,
        })
    })?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

pub fn create_engagement(
    database_path: &Path,
    client_id: &str,
    service_type_id: &str,
    name: &str,
    period_start: Option<&str>,
    period_end: Option<&str>,
    status: &str,
) -> Result<EngagementRecord, PersistenceError> {
    let name = normalize_domain_label(name, "engagement name", 240)?;
    let status = normalize_domain_label(status, "engagement status", 80)?;
    let period_start = normalize_optional_domain_text(period_start, 40);
    let period_end = normalize_optional_domain_text(period_end, 40);

    let mut connection = open_configured_connection(database_path)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

    let client_exists: bool = transaction.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM clients
            WHERE client_id = ?1 AND archived_at_ms IS NULL
        )",
        [client_id],
        |row| row.get(0),
    )?;
    if !client_exists {
        return Err(PersistenceError::Configuration(format!(
            "client {client_id} does not exist"
        )));
    }

    let service_exists: bool = transaction.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM service_types
            WHERE service_type_id = ?1 AND archived_at_ms IS NULL
        )",
        [service_type_id],
        |row| row.get(0),
    )?;
    if !service_exists {
        return Err(PersistenceError::Configuration(format!(
            "service type {service_type_id} does not exist"
        )));
    }

    let engagement_id = Uuid::new_v4().to_string();
    let now = now_unix_ms()?;
    transaction.execute(
        "INSERT INTO engagements (
            engagement_id,
            client_id,
            service_type_id,
            name,
            period_start,
            period_end,
            status,
            created_at_ms,
            archived_at_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL)",
        params![
            &engagement_id,
            client_id,
            service_type_id,
            &name,
            period_start.as_deref(),
            period_end.as_deref(),
            &status,
            now
        ],
    )?;

    transaction.commit()?;
    Ok(EngagementRecord {
        engagement_id,
        client_id: client_id.to_string(),
        service_type_id: service_type_id.to_string(),
        name,
        period_start,
        period_end,
        status,
        created_at_ms: now,
    })
}

pub fn list_engagements(
    database_path: &Path,
    client_id: Option<&str>,
) -> Result<Vec<EngagementRecord>, PersistenceError> {
    let connection = open_configured_connection(database_path)?;
    let mut result = Vec::new();

    if let Some(client_id) = client_id {
        let mut statement = connection.prepare(
            "SELECT
                engagement_id,
                client_id,
                service_type_id,
                name,
                period_start,
                period_end,
                status,
                created_at_ms
             FROM engagements
             WHERE archived_at_ms IS NULL
               AND client_id = ?1
             ORDER BY created_at_ms DESC, name COLLATE NOCASE",
        )?;
        let rows = statement.query_map([client_id], engagement_from_row)?;
        for row in rows {
            result.push(row?);
        }
    } else {
        let mut statement = connection.prepare(
            "SELECT
                engagement_id,
                client_id,
                service_type_id,
                name,
                period_start,
                period_end,
                status,
                created_at_ms
             FROM engagements
             WHERE archived_at_ms IS NULL
             ORDER BY created_at_ms DESC, name COLLATE NOCASE",
        )?;
        let rows = statement.query_map([], engagement_from_row)?;
        for row in rows {
            result.push(row?);
        }
    }

    Ok(result)
}

fn engagement_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<EngagementRecord> {
    Ok(EngagementRecord {
        engagement_id: row.get(0)?,
        client_id: row.get(1)?,
        service_type_id: row.get(2)?,
        name: row.get(3)?,
        period_start: row.get(4)?,
        period_end: row.get(5)?,
        status: row.get(6)?,
        created_at_ms: row.get(7)?,
    })
}

pub fn create_engagement_area(
    database_path: &Path,
    engagement_id: &str,
    parent_area_id: Option<&str>,
    name: &str,
    code: Option<&str>,
    display_order: i64,
    status: &str,
) -> Result<EngagementAreaRecord, PersistenceError> {
    let name = normalize_domain_label(name, "area name", 160)?;
    let code = normalize_optional_domain_text(code, 40);
    let status = normalize_domain_label(status, "area status", 80)?;

    let mut connection = open_configured_connection(database_path)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

    let engagement_exists: bool = transaction.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM engagements
            WHERE engagement_id = ?1 AND archived_at_ms IS NULL
        )",
        [engagement_id],
        |row| row.get(0),
    )?;
    if !engagement_exists {
        return Err(PersistenceError::Configuration(format!(
            "engagement {engagement_id} does not exist"
        )));
    }

    if let Some(parent_area_id) = parent_area_id {
        let parent_engagement: Option<String> = transaction
            .query_row(
                "SELECT engagement_id
                 FROM engagement_areas
                 WHERE engagement_area_id = ?1
                   AND archived_at_ms IS NULL",
                [parent_area_id],
                |row| row.get(0),
            )
            .optional()?;
        if parent_engagement.as_deref() != Some(engagement_id) {
            return Err(PersistenceError::Configuration(
                "parent area must belong to the same engagement".to_string(),
            ));
        }
    }

    let engagement_area_id = Uuid::new_v4().to_string();
    let now = now_unix_ms()?;
    transaction.execute(
        "INSERT INTO engagement_areas (
            engagement_area_id,
            engagement_id,
            parent_area_id,
            name,
            code,
            display_order,
            status,
            created_at_ms,
            archived_at_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL)",
        params![
            &engagement_area_id,
            engagement_id,
            parent_area_id,
            &name,
            code.as_deref(),
            display_order,
            &status,
            now
        ],
    )?;
    transaction.commit()?;

    Ok(EngagementAreaRecord {
        engagement_area_id,
        engagement_id: engagement_id.to_string(),
        parent_area_id: parent_area_id.map(str::to_string),
        name,
        code,
        display_order,
        status,
        created_at_ms: now,
    })
}

pub fn list_engagement_areas(
    database_path: &Path,
    engagement_id: &str,
) -> Result<Vec<EngagementAreaRecord>, PersistenceError> {
    let connection = open_configured_connection(database_path)?;
    let mut statement = connection.prepare(
        "SELECT
            engagement_area_id,
            engagement_id,
            parent_area_id,
            name,
            code,
            display_order,
            status,
            created_at_ms
         FROM engagement_areas
         WHERE engagement_id = ?1
           AND archived_at_ms IS NULL
         ORDER BY COALESCE(parent_area_id, ''), display_order, name COLLATE NOCASE",
    )?;
    let rows = statement.query_map([engagement_id], |row| {
        Ok(EngagementAreaRecord {
            engagement_area_id: row.get(0)?,
            engagement_id: row.get(1)?,
            parent_area_id: row.get(2)?,
            name: row.get(3)?,
            code: row.get(4)?,
            display_order: row.get(5)?,
            status: row.get(6)?,
            created_at_ms: row.get(7)?,
        })
    })?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

pub fn create_procedure(
    database_path: &Path,
    engagement_id: &str,
    engagement_area_id: Option<&str>,
    reference: Option<&str>,
    title: &str,
    description: Option<&str>,
    status: &str,
) -> Result<ProcedureRecord, PersistenceError> {
    let title = normalize_domain_label(title, "procedure title", 240)?;
    let reference = normalize_optional_domain_text(reference, 80);
    let description = description
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let status = normalize_domain_label(status, "procedure status", 80)?;

    let mut connection = open_configured_connection(database_path)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

    let engagement_exists: bool = transaction.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM engagements
            WHERE engagement_id = ?1 AND archived_at_ms IS NULL
        )",
        [engagement_id],
        |row| row.get(0),
    )?;
    if !engagement_exists {
        return Err(PersistenceError::Configuration(format!(
            "engagement {engagement_id} does not exist"
        )));
    }

    if let Some(area_id) = engagement_area_id {
        let area_engagement: Option<String> = transaction
            .query_row(
                "SELECT engagement_id
                 FROM engagement_areas
                 WHERE engagement_area_id = ?1
                   AND archived_at_ms IS NULL",
                [area_id],
                |row| row.get(0),
            )
            .optional()?;
        if area_engagement.as_deref() != Some(engagement_id) {
            return Err(PersistenceError::Configuration(
                "procedure area must belong to the same engagement".to_string(),
            ));
        }
    }

    let procedure_id = Uuid::new_v4().to_string();
    let now = now_unix_ms()?;
    transaction.execute(
        "INSERT INTO procedures (
            procedure_id,
            engagement_id,
            engagement_area_id,
            reference,
            title,
            description,
            status,
            created_at_ms,
            archived_at_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL)",
        params![
            &procedure_id,
            engagement_id,
            engagement_area_id,
            reference.as_deref(),
            &title,
            description.as_deref(),
            &status,
            now
        ],
    )?;
    transaction.commit()?;

    Ok(ProcedureRecord {
        procedure_id,
        engagement_id: engagement_id.to_string(),
        engagement_area_id: engagement_area_id.map(str::to_string),
        reference,
        title,
        description,
        status,
        created_at_ms: now,
    })
}

pub fn list_procedures(
    database_path: &Path,
    engagement_id: &str,
) -> Result<Vec<ProcedureRecord>, PersistenceError> {
    let connection = open_configured_connection(database_path)?;
    let mut statement = connection.prepare(
        "SELECT
            procedure_id,
            engagement_id,
            engagement_area_id,
            reference,
            title,
            description,
            status,
            created_at_ms
         FROM procedures
         WHERE engagement_id = ?1
           AND archived_at_ms IS NULL
         ORDER BY created_at_ms, title COLLATE NOCASE",
    )?;
    let rows = statement.query_map([engagement_id], |row| {
        Ok(ProcedureRecord {
            procedure_id: row.get(0)?,
            engagement_id: row.get(1)?,
            engagement_area_id: row.get(2)?,
            reference: row.get(3)?,
            title: row.get(4)?,
            description: row.get(5)?,
            status: row.get(6)?,
            created_at_ms: row.get(7)?,
        })
    })?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

pub fn create_workpaper(
    database_path: &Path,
    engagement_id: &str,
    engagement_area_id: Option<&str>,
    procedure_id: Option<&str>,
    reference: &str,
    title: &str,
    workflow_state: &str,
) -> Result<WorkpaperRecord, PersistenceError> {
    let reference = normalize_domain_label(reference, "workpaper reference", 80)?;
    let title = normalize_domain_label(title, "workpaper title", 240)?;
    let workflow_state = normalize_domain_label(workflow_state, "workflow state", 80)?;

    let mut connection = open_configured_connection(database_path)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

    let engagement_exists: bool = transaction.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM engagements
            WHERE engagement_id = ?1 AND archived_at_ms IS NULL
        )",
        [engagement_id],
        |row| row.get(0),
    )?;
    if !engagement_exists {
        return Err(PersistenceError::Configuration(format!(
            "engagement {engagement_id} does not exist"
        )));
    }

    if let Some(area_id) = engagement_area_id {
        let area_engagement: Option<String> = transaction
            .query_row(
                "SELECT engagement_id
                 FROM engagement_areas
                 WHERE engagement_area_id = ?1
                   AND archived_at_ms IS NULL",
                [area_id],
                |row| row.get(0),
            )
            .optional()?;
        if area_engagement.as_deref() != Some(engagement_id) {
            return Err(PersistenceError::Configuration(
                "workpaper area must belong to the same engagement".to_string(),
            ));
        }
    }

    if let Some(procedure_id) = procedure_id {
        let procedure_context: Option<(String, Option<String>)> = transaction
            .query_row(
                "SELECT engagement_id, engagement_area_id
                 FROM procedures
                 WHERE procedure_id = ?1
                   AND archived_at_ms IS NULL",
                [procedure_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let Some((procedure_engagement, procedure_area)) = procedure_context else {
            return Err(PersistenceError::Configuration(format!(
                "procedure {procedure_id} does not exist"
            )));
        };
        if procedure_engagement != engagement_id {
            return Err(PersistenceError::Configuration(
                "workpaper procedure must belong to the same engagement".to_string(),
            ));
        }
        if let (Some(area_id), Some(procedure_area)) =
            (engagement_area_id, procedure_area.as_deref())
        {
            if area_id != procedure_area {
                return Err(PersistenceError::Configuration(
                    "workpaper area must match the selected procedure area".to_string(),
                ));
            }
        }
    }

    let workpaper_id = Uuid::new_v4().to_string();
    let now = now_unix_ms()?;
    transaction.execute(
        "INSERT INTO workpapers (
            workpaper_id,
            engagement_id,
            engagement_area_id,
            procedure_id,
            reference,
            title,
            workflow_state,
            created_at_ms,
            archived_at_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL)",
        params![
            &workpaper_id,
            engagement_id,
            engagement_area_id,
            procedure_id,
            &reference,
            &title,
            &workflow_state,
            now
        ],
    )?;

    insert_domain_audit_event(
        &transaction,
        DomainAuditEvent {
            event_type: "WORKPAPER_CREATED",
            entity_type: "WORKPAPER",
            entity_id: &workpaper_id,
            related_entity_type: Some("ENGAGEMENT"),
            related_entity_id: Some(engagement_id),
            occurred_at_ms: now,
            details: json!({
                "reference": reference,
                "title": title,
                "workflowState": workflow_state,
                "engagementAreaId": engagement_area_id,
                "procedureId": procedure_id
            }),
        },
    )?;

    transaction.commit()?;
    Ok(WorkpaperRecord {
        workpaper_id,
        engagement_id: engagement_id.to_string(),
        engagement_area_id: engagement_area_id.map(str::to_string),
        procedure_id: procedure_id.map(str::to_string),
        reference,
        title,
        workflow_state,
        created_at_ms: now,
        latest_revision_number: None,
    })
}

pub fn list_workpapers(
    database_path: &Path,
    engagement_id: &str,
) -> Result<Vec<WorkpaperRecord>, PersistenceError> {
    let connection = open_configured_connection(database_path)?;
    let mut statement = connection.prepare(
        "SELECT
            w.workpaper_id,
            w.engagement_id,
            w.engagement_area_id,
            w.procedure_id,
            w.reference,
            w.title,
            w.workflow_state,
            w.created_at_ms,
            (
                SELECT MAX(wr.revision_number)
                FROM workpaper_revisions wr
                WHERE wr.workpaper_id = w.workpaper_id
            )
         FROM workpapers w
         WHERE w.engagement_id = ?1
           AND w.archived_at_ms IS NULL
         ORDER BY w.reference COLLATE NOCASE, w.created_at_ms",
    )?;
    let rows = statement.query_map([engagement_id], |row| {
        let latest_revision_number: Option<i64> = row.get(8)?;
        Ok(WorkpaperRecord {
            workpaper_id: row.get(0)?,
            engagement_id: row.get(1)?,
            engagement_area_id: row.get(2)?,
            procedure_id: row.get(3)?,
            reference: row.get(4)?,
            title: row.get(5)?,
            workflow_state: row.get(6)?,
            created_at_ms: row.get(7)?,
            latest_revision_number: latest_revision_number.map(|value| value.max(0) as u64),
        })
    })?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

pub fn create_workpaper_revision(
    database_path: &Path,
    workpaper_id: &str,
    content: NewWorkpaperRevision<'_>,
) -> Result<WorkpaperRevisionRecord, PersistenceError> {
    let mut connection = open_configured_connection(database_path)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

    let workpaper_exists: bool = transaction.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM workpapers
            WHERE workpaper_id = ?1
              AND archived_at_ms IS NULL
        )",
        [workpaper_id],
        |row| row.get(0),
    )?;
    if !workpaper_exists {
        return Err(PersistenceError::Configuration(format!(
            "workpaper {workpaper_id} does not exist"
        )));
    }

    let previous: Option<(String, i64)> = transaction
        .query_row(
            "SELECT workpaper_revision_id, revision_number
             FROM workpaper_revisions
             WHERE workpaper_id = ?1
             ORDER BY revision_number DESC
             LIMIT 1",
            [workpaper_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let next_revision = previous
        .as_ref()
        .map(|(_, number)| number.saturating_add(1))
        .unwrap_or(1);
    let supersedes_revision_id = previous.map(|(id, _)| id);

    let revision_reason = content
        .revision_reason
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let mut hasher = Sha256::new();
    for value in [
        content.objective,
        content.procedure_performed,
        content.population,
        content.sample,
        content.exceptions,
        content.management_explanation,
        content.conclusion,
    ] {
        hasher.update((value.len() as u64).to_le_bytes());
        hasher.update(value.as_bytes());
    }
    let content_hash = hasher.finalize().to_vec();

    let revision_id = Uuid::new_v4().to_string();
    let now = now_unix_ms()?;
    transaction.execute(
        "INSERT INTO workpaper_revisions (
            workpaper_revision_id,
            workpaper_id,
            revision_number,
            created_at_ms,
            revision_reason,
            supersedes_revision_id,
            objective,
            procedure_performed,
            population,
            sample,
            exceptions,
            management_explanation,
            conclusion,
            content_hash
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        params![
            &revision_id,
            workpaper_id,
            next_revision,
            now,
            revision_reason.as_deref(),
            supersedes_revision_id.as_deref(),
            content.objective,
            content.procedure_performed,
            content.population,
            content.sample,
            content.exceptions,
            content.management_explanation,
            content.conclusion,
            &content_hash
        ],
    )?;

    insert_domain_audit_event(
        &transaction,
        DomainAuditEvent {
            event_type: "WORKPAPER_REVISION_CREATED",
            entity_type: "WORKPAPER_REVISION",
            entity_id: &revision_id,
            related_entity_type: Some("WORKPAPER"),
            related_entity_id: Some(workpaper_id),
            occurred_at_ms: now,
            details: json!({
                "revisionNumber": next_revision,
                "supersedesRevisionId": supersedes_revision_id,
                "revisionReason": revision_reason,
                "contentHash": bytes_to_lower_hex(&content_hash)
            }),
        },
    )?;

    transaction.commit()?;
    Ok(WorkpaperRevisionRecord {
        workpaper_revision_id: revision_id,
        workpaper_id: workpaper_id.to_string(),
        revision_number: next_revision.max(0) as u64,
        created_at_ms: now,
        revision_reason,
        supersedes_revision_id,
        objective: content.objective.to_string(),
        procedure_performed: content.procedure_performed.to_string(),
        population: content.population.to_string(),
        sample: content.sample.to_string(),
        exceptions: content.exceptions.to_string(),
        management_explanation: content.management_explanation.to_string(),
        conclusion: content.conclusion.to_string(),
        content_hash: Some(content_hash),
    })
}

pub fn list_workpaper_revisions(
    database_path: &Path,
    workpaper_id: &str,
) -> Result<Vec<WorkpaperRevisionRecord>, PersistenceError> {
    let connection = open_configured_connection(database_path)?;
    let mut statement = connection.prepare(
        "SELECT
            workpaper_revision_id,
            workpaper_id,
            revision_number,
            created_at_ms,
            revision_reason,
            supersedes_revision_id,
            objective,
            procedure_performed,
            population,
            sample,
            exceptions,
            management_explanation,
            conclusion,
            content_hash
         FROM workpaper_revisions
         WHERE workpaper_id = ?1
         ORDER BY revision_number DESC",
    )?;
    let rows = statement.query_map([workpaper_id], |row| {
        let revision_number: i64 = row.get(2)?;
        Ok(WorkpaperRevisionRecord {
            workpaper_revision_id: row.get(0)?,
            workpaper_id: row.get(1)?,
            revision_number: revision_number.max(0) as u64,
            created_at_ms: row.get(3)?,
            revision_reason: row.get(4)?,
            supersedes_revision_id: row.get(5)?,
            objective: row.get(6)?,
            procedure_performed: row.get(7)?,
            population: row.get(8)?,
            sample: row.get(9)?,
            exceptions: row.get(10)?,
            management_explanation: row.get(11)?,
            conclusion: row.get(12)?,
            content_hash: row.get(13)?,
        })
    })?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

pub fn create_workpaper_evidence_link(
    database_path: &Path,
    workpaper_revision_id: &str,
    document_id: &str,
    content_version_id: Option<&str>,
    controlled_evidence_version_id: Option<&str>,
    relationship_type: &str,
    description: Option<&str>,
) -> Result<WorkpaperEvidenceLinkRecord, PersistenceError> {
    if content_version_id.is_some() == controlled_evidence_version_id.is_some() {
        return Err(PersistenceError::Configuration(
            "evidence link must reference exactly one content version or controlled evidence version"
                .to_string(),
        ));
    }

    let relationship_type =
        normalize_domain_label(relationship_type, "evidence relationship type", 80)?;
    let description = description
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    let mut connection = open_configured_connection(database_path)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

    let revision_context: Option<(String, i64, i64)> = transaction
        .query_row(
            "SELECT
                wr.workpaper_id,
                wr.revision_number,
                (
                    SELECT MAX(latest.revision_number)
                    FROM workpaper_revisions latest
                    WHERE latest.workpaper_id = wr.workpaper_id
                )
             FROM workpaper_revisions wr
             WHERE wr.workpaper_revision_id = ?1",
            [workpaper_revision_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    let Some((_workpaper_id, revision_number, latest_revision_number)) = revision_context else {
        return Err(PersistenceError::Configuration(format!(
            "workpaper revision {workpaper_revision_id} does not exist"
        )));
    };
    if revision_number != latest_revision_number {
        return Err(PersistenceError::Configuration(
            "evidence can only be linked to the latest workpaper revision; create a new revision instead of changing historical evidence"
                .to_string(),
        ));
    }

    let document_name: Option<String> = transaction
        .query_row(
            "SELECT display_name
             FROM documents
             WHERE document_id = ?1
               AND archived_at_ms IS NULL",
            [document_id],
            |row| row.get(0),
        )
        .optional()?;
    let Some(document_name) = document_name else {
        return Err(PersistenceError::Configuration(format!(
            "document {document_id} does not exist"
        )));
    };

    let mut content_observed_at_ms = None;
    if let Some(content_version_id) = content_version_id {
        let version_context: Option<(String, i64)> = transaction
            .query_row(
                "SELECT document_id, observed_at_ms
                 FROM content_versions
                 WHERE content_version_id = ?1",
                [content_version_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let Some((version_document, observed_at_ms)) = version_context else {
            return Err(PersistenceError::Configuration(
                "content version does not exist".to_string(),
            ));
        };
        if version_document != document_id {
            return Err(PersistenceError::Configuration(
                "content version does not belong to the selected document".to_string(),
            ));
        }
        content_observed_at_ms = Some(observed_at_ms);
    }

    let mut controlled_version_number = None;
    let mut controlled_captured_at_ms = None;
    if let Some(controlled_version_id) = controlled_evidence_version_id {
        let version_context: Option<(String, i64, i64)> = transaction
            .query_row(
                "SELECT document_id, version_number, captured_at_ms
                 FROM controlled_evidence_versions
                 WHERE controlled_evidence_version_id = ?1",
                [controlled_version_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        let Some((version_document, version_number, captured_at_ms)) = version_context else {
            return Err(PersistenceError::Configuration(
                "controlled evidence version does not exist".to_string(),
            ));
        };
        if version_document != document_id {
            return Err(PersistenceError::Configuration(
                "controlled evidence version does not belong to the selected document".to_string(),
            ));
        }
        controlled_version_number = Some(version_number.max(0) as u64);
        controlled_captured_at_ms = Some(captured_at_ms);
    }

    let evidence_link_id = Uuid::new_v4().to_string();
    let now = now_unix_ms()?;
    transaction.execute(
        "INSERT INTO workpaper_evidence_links (
            evidence_link_id,
            workpaper_revision_id,
            document_id,
            content_version_id,
            controlled_evidence_version_id,
            relationship_type,
            description,
            created_at_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            &evidence_link_id,
            workpaper_revision_id,
            document_id,
            content_version_id,
            controlled_evidence_version_id,
            &relationship_type,
            description.as_deref(),
            now
        ],
    )?;

    insert_domain_audit_event(
        &transaction,
        DomainAuditEvent {
            event_type: "WORKPAPER_EVIDENCE_LINK_ADDED",
            entity_type: "WORKPAPER_EVIDENCE_LINK",
            entity_id: &evidence_link_id,
            related_entity_type: Some("WORKPAPER_REVISION"),
            related_entity_id: Some(workpaper_revision_id),
            occurred_at_ms: now,
            details: json!({
                "documentId": document_id,
                "contentVersionId": content_version_id,
                "controlledEvidenceVersionId": controlled_evidence_version_id,
                "relationshipType": relationship_type
            }),
        },
    )?;

    transaction.commit()?;
    Ok(WorkpaperEvidenceLinkRecord {
        evidence_link_id,
        workpaper_revision_id: workpaper_revision_id.to_string(),
        document_id: document_id.to_string(),
        document_name,
        content_version_id: content_version_id.map(str::to_string),
        content_observed_at_ms,
        controlled_evidence_version_id: controlled_evidence_version_id.map(str::to_string),
        controlled_version_number,
        controlled_captured_at_ms,
        relationship_type,
        description,
        created_at_ms: now,
    })
}

pub fn list_workpaper_evidence_links(
    database_path: &Path,
    workpaper_revision_id: &str,
) -> Result<Vec<WorkpaperEvidenceLinkRecord>, PersistenceError> {
    let connection = open_configured_connection(database_path)?;
    let mut statement = connection.prepare(
        "SELECT
            wel.evidence_link_id,
            wel.workpaper_revision_id,
            wel.document_id,
            d.display_name,
            wel.content_version_id,
            cv.observed_at_ms,
            wel.controlled_evidence_version_id,
            cev.version_number,
            cev.captured_at_ms,
            wel.relationship_type,
            wel.description,
            wel.created_at_ms
         FROM workpaper_evidence_links wel
         JOIN documents d ON d.document_id = wel.document_id
         LEFT JOIN content_versions cv ON cv.content_version_id = wel.content_version_id
         LEFT JOIN controlled_evidence_versions cev
            ON cev.controlled_evidence_version_id = wel.controlled_evidence_version_id
         WHERE wel.workpaper_revision_id = ?1
         ORDER BY wel.created_at_ms, wel.rowid",
    )?;
    let rows = statement.query_map([workpaper_revision_id], |row| {
        let controlled_version_number: Option<i64> = row.get(7)?;
        Ok(WorkpaperEvidenceLinkRecord {
            evidence_link_id: row.get(0)?,
            workpaper_revision_id: row.get(1)?,
            document_id: row.get(2)?,
            document_name: row.get(3)?,
            content_version_id: row.get(4)?,
            content_observed_at_ms: row.get(5)?,
            controlled_evidence_version_id: row.get(6)?,
            controlled_version_number: controlled_version_number.map(|value| value.max(0) as u64),
            controlled_captured_at_ms: row.get(8)?,
            relationship_type: row.get(9)?,
            description: row.get(10)?,
            created_at_ms: row.get(11)?,
        })
    })?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

fn workflow_state_key(value: &str) -> String {
    value
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .split('_')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

fn state_requires_controlled_evidence(value: &str) -> bool {
    matches!(
        workflow_state_key(value).as_str(),
        "SUBMITTED_FOR_REVIEW" | "REVIEWED" | "FINALISED" | "FINAL"
    )
}

pub fn transition_workpaper_state(
    database_path: &Path,
    workpaper_id: &str,
    to_state: &str,
    actor_id: Option<&str>,
    comment: Option<&str>,
) -> Result<WorkpaperWorkflowEventRecord, PersistenceError> {
    let to_state = normalize_domain_label(to_state, "workflow state", 80)?;
    let actor_id = normalize_optional_domain_text(actor_id, 160);
    let comment = comment
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    let mut connection = open_configured_connection(database_path)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

    let from_state: Option<String> = transaction
        .query_row(
            "SELECT workflow_state
             FROM workpapers
             WHERE workpaper_id = ?1
               AND archived_at_ms IS NULL",
            [workpaper_id],
            |row| row.get(0),
        )
        .optional()?;
    let Some(from_state) = from_state else {
        return Err(PersistenceError::Configuration(format!(
            "workpaper {workpaper_id} does not exist"
        )));
    };

    let latest_revision_id: Option<String> = transaction
        .query_row(
            "SELECT workpaper_revision_id
             FROM workpaper_revisions
             WHERE workpaper_id = ?1
             ORDER BY revision_number DESC
             LIMIT 1",
            [workpaper_id],
            |row| row.get(0),
        )
        .optional()?;

    if state_requires_controlled_evidence(&to_state) {
        let Some(revision_id) = latest_revision_id.as_deref() else {
            return Err(PersistenceError::Configuration(
                "formal review requires a current workpaper revision".to_string(),
            ));
        };

        let unsafe_evidence_count: i64 = transaction.query_row(
            "SELECT COUNT(*)
             FROM workpaper_evidence_links wel
             LEFT JOIN controlled_evidence_versions cev
               ON cev.controlled_evidence_version_id = wel.controlled_evidence_version_id
             WHERE wel.workpaper_revision_id = ?1
               AND (
                   wel.controlled_evidence_version_id IS NULL
                   OR cev.verification_state <> 'HASH_VERIFIED'
               )",
            [revision_id],
            |row| row.get(0),
        )?;
        if unsafe_evidence_count > 0 {
            return Err(PersistenceError::Configuration(
                "formal review requires all linked evidence on the current revision to be immutable hash-verified controlled evidence"
                    .to_string(),
            ));
        }
    }

    let now = now_unix_ms()?;
    let event_id = Uuid::new_v4().to_string();
    transaction.execute(
        "INSERT INTO workpaper_workflow_events (
            workpaper_workflow_event_id,
            workpaper_id,
            workpaper_revision_id,
            from_state,
            to_state,
            actor_id,
            comment,
            occurred_at_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            &event_id,
            workpaper_id,
            latest_revision_id.as_deref(),
            &from_state,
            &to_state,
            actor_id.as_deref(),
            comment.as_deref(),
            now
        ],
    )?;
    transaction.execute(
        "UPDATE workpapers
         SET workflow_state = ?1
         WHERE workpaper_id = ?2",
        params![&to_state, workpaper_id],
    )?;

    insert_domain_audit_event(
        &transaction,
        DomainAuditEvent {
            event_type: "WORKPAPER_STATE_TRANSITIONED",
            entity_type: "WORKPAPER",
            entity_id: workpaper_id,
            related_entity_type: latest_revision_id.as_ref().map(|_| "WORKPAPER_REVISION"),
            related_entity_id: latest_revision_id.as_deref(),
            occurred_at_ms: now,
            details: json!({
                "fromState": from_state,
                "toState": to_state,
                "actorId": actor_id,
                "comment": comment
            }),
        },
    )?;

    transaction.commit()?;
    Ok(WorkpaperWorkflowEventRecord {
        workpaper_workflow_event_id: event_id,
        workpaper_id: workpaper_id.to_string(),
        workpaper_revision_id: latest_revision_id,
        from_state,
        to_state,
        actor_id,
        comment,
        occurred_at_ms: now,
    })
}

pub fn list_workpaper_workflow_events(
    database_path: &Path,
    workpaper_id: &str,
) -> Result<Vec<WorkpaperWorkflowEventRecord>, PersistenceError> {
    let connection = open_configured_connection(database_path)?;
    let mut statement = connection.prepare(
        "SELECT
            workpaper_workflow_event_id,
            workpaper_id,
            workpaper_revision_id,
            from_state,
            to_state,
            actor_id,
            comment,
            occurred_at_ms
         FROM workpaper_workflow_events
         WHERE workpaper_id = ?1
         ORDER BY occurred_at_ms, rowid",
    )?;
    let rows = statement.query_map([workpaper_id], |row| {
        Ok(WorkpaperWorkflowEventRecord {
            workpaper_workflow_event_id: row.get(0)?,
            workpaper_id: row.get(1)?,
            workpaper_revision_id: row.get(2)?,
            from_state: row.get(3)?,
            to_state: row.get(4)?,
            actor_id: row.get(5)?,
            comment: row.get(6)?,
            occurred_at_ms: row.get(7)?,
        })
    })?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

pub fn create_review_note(
    database_path: &Path,
    note: NewReviewNote<'_>,
) -> Result<ReviewNoteRecord, PersistenceError> {
    let title = normalize_domain_label(note.title, "review note title", 240)?;
    let body = note.body.trim().to_string();
    if body.is_empty() {
        return Err(PersistenceError::Configuration(
            "review note body must not be empty".to_string(),
        ));
    }
    let owner_id = normalize_optional_domain_text(note.owner_id, 160);
    let location_kind = normalize_optional_domain_text(note.location_kind, 80);
    let location_value = note
        .location_value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let raised_by = normalize_optional_domain_text(note.raised_by, 160);

    let mut connection = open_configured_connection(database_path)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

    let revision_workpaper: Option<String> = transaction
        .query_row(
            "SELECT workpaper_id
             FROM workpaper_revisions
             WHERE workpaper_revision_id = ?1",
            [note.workpaper_revision_id],
            |row| row.get(0),
        )
        .optional()?;
    if revision_workpaper.as_deref() != Some(note.workpaper_id) {
        return Err(PersistenceError::Configuration(
            "review note revision must belong to the selected workpaper".to_string(),
        ));
    }

    if let Some(evidence_link_id) = note.evidence_link_id {
        let evidence_revision: Option<String> = transaction
            .query_row(
                "SELECT workpaper_revision_id
                 FROM workpaper_evidence_links
                 WHERE evidence_link_id = ?1",
                [evidence_link_id],
                |row| row.get(0),
            )
            .optional()?;
        if evidence_revision.as_deref() != Some(note.workpaper_revision_id) {
            return Err(PersistenceError::Configuration(
                "review note evidence link must belong to the selected workpaper revision"
                    .to_string(),
            ));
        }
    }

    if location_kind.is_some() != location_value.is_some() {
        return Err(PersistenceError::Configuration(
            "review note location kind and value must be supplied together".to_string(),
        ));
    }

    let review_note_id = Uuid::new_v4().to_string();
    let event_id = Uuid::new_v4().to_string();
    let now = now_unix_ms()?;
    transaction.execute(
        "INSERT INTO review_notes (
            review_note_id,
            workpaper_id,
            workpaper_revision_id,
            evidence_link_id,
            title,
            body,
            owner_id,
            due_at_ms,
            location_kind,
            location_value,
            current_state,
            raised_by,
            created_at_ms,
            latest_event_at_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'OPEN', ?11, ?12, ?12)",
        params![
            &review_note_id,
            note.workpaper_id,
            note.workpaper_revision_id,
            note.evidence_link_id,
            &title,
            &body,
            owner_id.as_deref(),
            note.due_at_ms,
            location_kind.as_deref(),
            location_value.as_deref(),
            raised_by.as_deref(),
            now
        ],
    )?;
    transaction.execute(
        "INSERT INTO review_note_events (
            review_note_event_id,
            review_note_id,
            event_type,
            actor_id,
            response_text,
            comment,
            occurred_at_ms
         ) VALUES (?1, ?2, 'RAISED', ?3, NULL, ?4, ?5)",
        params![&event_id, &review_note_id, raised_by.as_deref(), &body, now],
    )?;

    insert_domain_audit_event(
        &transaction,
        DomainAuditEvent {
            event_type: "REVIEW_NOTE_RAISED",
            entity_type: "REVIEW_NOTE",
            entity_id: &review_note_id,
            related_entity_type: Some("WORKPAPER_REVISION"),
            related_entity_id: Some(note.workpaper_revision_id),
            occurred_at_ms: now,
            details: json!({
                "workpaperId": note.workpaper_id,
                "evidenceLinkId": note.evidence_link_id,
                "ownerId": owner_id,
                "dueAtMs": note.due_at_ms,
                "locationKind": location_kind,
                "locationValue": location_value
            }),
        },
    )?;

    transaction.commit()?;
    Ok(ReviewNoteRecord {
        review_note_id,
        workpaper_id: note.workpaper_id.to_string(),
        workpaper_revision_id: note.workpaper_revision_id.to_string(),
        evidence_link_id: note.evidence_link_id.map(str::to_string),
        title,
        body,
        owner_id,
        due_at_ms: note.due_at_ms,
        location_kind,
        location_value,
        current_state: "OPEN".to_string(),
        raised_by,
        created_at_ms: now,
        latest_event_at_ms: now,
    })
}

pub fn list_review_notes(
    database_path: &Path,
    workpaper_id: &str,
) -> Result<Vec<ReviewNoteRecord>, PersistenceError> {
    let connection = open_configured_connection(database_path)?;
    let mut statement = connection.prepare(
        "SELECT
            review_note_id,
            workpaper_id,
            workpaper_revision_id,
            evidence_link_id,
            title,
            body,
            owner_id,
            due_at_ms,
            location_kind,
            location_value,
            current_state,
            raised_by,
            created_at_ms,
            latest_event_at_ms
         FROM review_notes
         WHERE workpaper_id = ?1
         ORDER BY
            CASE current_state WHEN 'CLEARED' THEN 1 ELSE 0 END,
            latest_event_at_ms DESC,
            rowid DESC",
    )?;
    let rows = statement.query_map([workpaper_id], |row| {
        Ok(ReviewNoteRecord {
            review_note_id: row.get(0)?,
            workpaper_id: row.get(1)?,
            workpaper_revision_id: row.get(2)?,
            evidence_link_id: row.get(3)?,
            title: row.get(4)?,
            body: row.get(5)?,
            owner_id: row.get(6)?,
            due_at_ms: row.get(7)?,
            location_kind: row.get(8)?,
            location_value: row.get(9)?,
            current_state: row.get(10)?,
            raised_by: row.get(11)?,
            created_at_ms: row.get(12)?,
            latest_event_at_ms: row.get(13)?,
        })
    })?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

fn append_review_note_event(
    database_path: &Path,
    action: ReviewNoteAction<'_>,
    event_type: &str,
    next_state: &str,
    require_response: bool,
) -> Result<ReviewNoteEventRecord, PersistenceError> {
    let actor_id = normalize_optional_domain_text(action.actor_id, 160);
    let response_text = action
        .response_text
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let comment = action
        .comment
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    if require_response && response_text.is_none() {
        return Err(PersistenceError::Configuration(
            "review note response text is required".to_string(),
        ));
    }

    let mut connection = open_configured_connection(database_path)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let current_state: Option<String> = transaction
        .query_row(
            "SELECT current_state
             FROM review_notes
             WHERE review_note_id = ?1",
            [action.review_note_id],
            |row| row.get(0),
        )
        .optional()?;
    let Some(current_state) = current_state else {
        return Err(PersistenceError::Configuration(format!(
            "review note {} does not exist",
            action.review_note_id
        )));
    };

    match event_type {
        "RESPONSE_SUBMITTED" if current_state == "CLEARED" => {
            return Err(PersistenceError::Configuration(
                "cleared review note must be reopened before another response".to_string(),
            ));
        }
        "CLEARED" if current_state == "CLEARED" => {
            return Err(PersistenceError::Configuration(
                "review note is already cleared".to_string(),
            ));
        }
        "REOPENED" if current_state != "CLEARED" => {
            return Err(PersistenceError::Configuration(
                "only a cleared review note can be reopened".to_string(),
            ));
        }
        _ => {}
    }

    let event_id = Uuid::new_v4().to_string();
    let now = now_unix_ms()?;
    transaction.execute(
        "INSERT INTO review_note_events (
            review_note_event_id,
            review_note_id,
            event_type,
            actor_id,
            response_text,
            comment,
            occurred_at_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            &event_id,
            action.review_note_id,
            event_type,
            actor_id.as_deref(),
            response_text.as_deref(),
            comment.as_deref(),
            now
        ],
    )?;
    transaction.execute(
        "UPDATE review_notes
         SET current_state = ?1,
             latest_event_at_ms = ?2
         WHERE review_note_id = ?3",
        params![next_state, now, action.review_note_id],
    )?;

    let audit_type = match event_type {
        "RESPONSE_SUBMITTED" => "REVIEW_NOTE_RESPONSE_SUBMITTED",
        "CLEARED" => "REVIEW_NOTE_CLEARED",
        "REOPENED" => "REVIEW_NOTE_REOPENED",
        _ => "REVIEW_NOTE_EVENT",
    };
    insert_domain_audit_event(
        &transaction,
        DomainAuditEvent {
            event_type: audit_type,
            entity_type: "REVIEW_NOTE",
            entity_id: action.review_note_id,
            related_entity_type: None,
            related_entity_id: None,
            occurred_at_ms: now,
            details: json!({
                "eventType": event_type,
                "fromState": current_state,
                "toState": next_state,
                "actorId": actor_id,
                "hasResponse": response_text.is_some(),
                "comment": comment
            }),
        },
    )?;

    transaction.commit()?;
    Ok(ReviewNoteEventRecord {
        review_note_event_id: event_id,
        review_note_id: action.review_note_id.to_string(),
        event_type: event_type.to_string(),
        actor_id,
        response_text,
        comment,
        occurred_at_ms: now,
    })
}

pub fn respond_to_review_note(
    database_path: &Path,
    action: ReviewNoteAction<'_>,
) -> Result<ReviewNoteEventRecord, PersistenceError> {
    append_review_note_event(
        database_path,
        action,
        "RESPONSE_SUBMITTED",
        "RESPONSE_SUBMITTED",
        true,
    )
}

pub fn clear_review_note(
    database_path: &Path,
    action: ReviewNoteAction<'_>,
) -> Result<ReviewNoteEventRecord, PersistenceError> {
    append_review_note_event(database_path, action, "CLEARED", "CLEARED", false)
}

pub fn reopen_review_note(
    database_path: &Path,
    action: ReviewNoteAction<'_>,
) -> Result<ReviewNoteEventRecord, PersistenceError> {
    append_review_note_event(database_path, action, "REOPENED", "OPEN", false)
}

pub fn list_review_note_events(
    database_path: &Path,
    review_note_id: &str,
) -> Result<Vec<ReviewNoteEventRecord>, PersistenceError> {
    let connection = open_configured_connection(database_path)?;
    let mut statement = connection.prepare(
        "SELECT
            review_note_event_id,
            review_note_id,
            event_type,
            actor_id,
            response_text,
            comment,
            occurred_at_ms
         FROM review_note_events
         WHERE review_note_id = ?1
         ORDER BY occurred_at_ms, rowid",
    )?;
    let rows = statement.query_map([review_note_id], |row| {
        Ok(ReviewNoteEventRecord {
            review_note_event_id: row.get(0)?,
            review_note_id: row.get(1)?,
            event_type: row.get(2)?,
            actor_id: row.get(3)?,
            response_text: row.get(4)?,
            comment: row.get(5)?,
            occurred_at_ms: row.get(6)?,
        })
    })?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

pub fn resolve_file_instance_source(
    database_path: &Path,
    file_instance_id: &str,
) -> Result<Option<ResolvedFileSource>, PersistenceError> {
    let connection = open_configured_connection(database_path)?;

    let row: Option<ResolvedFileSourceRow> = connection
        .query_row(
            "SELECT
                COALESCE(sr.canonical_native_locator, sr.native_locator),
                sr.native_locator_encoding,
                fi.relative_path_native,
                fi.path_native_encoding
             FROM file_instances fi
             JOIN storage_roots sr ON sr.storage_root_id = fi.storage_root_id
             WHERE fi.file_instance_id = ?1",
            [file_instance_id],
            |row| {
                Ok(ResolvedFileSourceRow {
                    root_native: row.get(0)?,
                    root_encoding: row.get(1)?,
                    relative_native: row.get(2)?,
                    relative_encoding: row.get(3)?,
                })
            },
        )
        .optional()?;

    let Some(row) = row else {
        return Ok(None);
    };

    let storage_root_path = decode_native_path(&row.root_native, &row.root_encoding)?;
    let relative_path = decode_native_path(&row.relative_native, &row.relative_encoding)?;

    if relative_path.components().any(|component| {
        matches!(
            component,
            std::path::Component::ParentDir
                | std::path::Component::RootDir
                | std::path::Component::Prefix(_)
        )
    }) {
        return Err(PersistenceError::Configuration(
            "stored file-instance path is not root-relative".to_string(),
        ));
    }

    Ok(Some(ResolvedFileSource {
        storage_root_path,
        relative_path,
    }))
}

pub fn hydrate_search_files(
    database_path: &Path,
    file_instance_ids: &[String],
) -> Result<Vec<Option<IndexedFilePreviewRecord>>, PersistenceError> {
    if file_instance_ids.is_empty() {
        return Ok(Vec::new());
    }

    let connection = open_configured_connection(database_path)?;
    let mut statement = connection.prepare(
        "SELECT
            d.document_id,
            fi.file_instance_id,
            d.display_name,
            fi.relative_path_native,
            fi.path_native_encoding,
            fi.size_bytes,
            fi.last_write_time_ms,
            CASE
                WHEN sr.availability_state = 'OFFLINE' THEN 'UNAVAILABLE'
                ELSE fi.availability_state
            END,
            COALESCE(sr.canonical_native_locator, sr.native_locator),
            sr.native_locator_encoding
         FROM file_instances fi
         JOIN documents d ON d.document_id = fi.document_id
         JOIN storage_roots sr ON sr.storage_root_id = fi.storage_root_id
         WHERE fi.file_instance_id = ?1
           AND d.archived_at_ms IS NULL",
    )?;

    let mut result = Vec::with_capacity(file_instance_ids.len());

    for file_instance_id in file_instance_ids {
        let row = statement
            .query_row([file_instance_id], |row| {
                Ok(HydratedIndexRow {
                    document_id: row.get(0)?,
                    file_instance_id: row.get(1)?,
                    name: row.get(2)?,
                    relative_path_native: row.get(3)?,
                    path_native_encoding: row.get(4)?,
                    size_bytes: row.get(5)?,
                    modified_unix_ms: row.get(6)?,
                    availability_state: row.get(7)?,
                    root_native: row.get(8)?,
                    root_native_encoding: row.get(9)?,
                })
            })
            .optional()?;

        let Some(row) = row else {
            result.push(None);
            continue;
        };

        let root_path = decode_native_path(&row.root_native, &row.root_native_encoding)?;
        let relative_path =
            decode_native_path(&row.relative_path_native, &row.path_native_encoding)?;
        let absolute_path = root_path.join(&relative_path);
        let extension = relative_path
            .extension()
            .map(|value| value.to_string_lossy().into_owned())
            .unwrap_or_default();

        result.push(Some(IndexedFilePreviewRecord {
            document_id: row.document_id,
            file_instance_id: row.file_instance_id,
            name: row.name,
            path: absolute_path.to_string_lossy().into_owned(),
            extension,
            size_bytes: row.size_bytes.max(0) as u64,
            modified_unix_ms: row.modified_unix_ms,
            availability_state: row.availability_state,
        }));
    }

    Ok(result)
}

pub fn record_document_open(
    database_path: &Path,
    file_instance_id: &str,
) -> Result<(), PersistenceError> {
    let connection = open_configured_connection(database_path)?;
    let opened_at_ms = now_unix_ms()?;

    let changed = connection.execute(
        "INSERT INTO recent_document_access (
            document_id,
            last_file_instance_id,
            last_opened_at_ms,
            open_count
         )
         SELECT
            fi.document_id,
            fi.file_instance_id,
            ?1,
            1
         FROM file_instances fi
         WHERE fi.file_instance_id = ?2
         ON CONFLICT(document_id) DO UPDATE SET
            last_file_instance_id = excluded.last_file_instance_id,
            last_opened_at_ms = excluded.last_opened_at_ms,
            open_count = recent_document_access.open_count + 1",
        params![opened_at_ms, file_instance_id],
    )?;

    if changed != 1 {
        return Err(PersistenceError::Configuration(format!(
            "file instance {file_instance_id} does not exist"
        )));
    }

    Ok(())
}

pub fn set_document_pin(
    database_path: &Path,
    document_id: &str,
    pinned: bool,
) -> Result<(), PersistenceError> {
    let connection = open_configured_connection(database_path)?;

    if pinned {
        let changed = connection.execute(
            "INSERT INTO document_pins (
                document_id,
                pinned_at_ms
             )
             SELECT ?1, ?2
             WHERE EXISTS (
                SELECT 1
                FROM documents
                WHERE document_id = ?1
                  AND archived_at_ms IS NULL
             )
             ON CONFLICT(document_id) DO UPDATE SET
                pinned_at_ms = excluded.pinned_at_ms",
            params![document_id, now_unix_ms()?],
        )?;

        if changed != 1 {
            return Err(PersistenceError::Configuration(format!(
                "document {document_id} does not exist"
            )));
        }
    } else {
        connection.execute(
            "DELETE FROM document_pins WHERE document_id = ?1",
            [document_id],
        )?;
    }

    Ok(())
}

pub fn list_recent_documents(
    database_path: &Path,
    limit: u32,
) -> Result<Vec<RecentDocumentRecord>, PersistenceError> {
    let connection = open_configured_connection(database_path)?;
    let bounded_limit = i64::from(limit.clamp(1, 100));

    let mut statement = connection.prepare(
        "SELECT
            recent.last_file_instance_id,
            recent.last_opened_at_ms,
            recent.open_count
         FROM recent_document_access recent
         JOIN documents d ON d.document_id = recent.document_id
         WHERE d.archived_at_ms IS NULL
           AND recent.last_file_instance_id IS NOT NULL
         ORDER BY recent.last_opened_at_ms DESC
         LIMIT ?1",
    )?;

    let rows = statement.query_map([bounded_limit], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, i64>(2)?,
        ))
    })?;

    let mut metadata = Vec::new();
    for row in rows {
        metadata.push(row?);
    }
    drop(statement);
    drop(connection);

    let file_instance_ids: Vec<String> = metadata.iter().map(|(id, _, _)| id.clone()).collect();
    let files = hydrate_search_files(database_path, &file_instance_ids)?;

    let mut result = Vec::new();
    for (file, (_, last_opened_at_ms, open_count)) in files.into_iter().zip(metadata) {
        let Some(file) = file else {
            continue;
        };

        result.push(RecentDocumentRecord {
            file,
            last_opened_at_ms,
            open_count: open_count.max(0) as u64,
        });
    }

    Ok(result)
}

pub fn record_recent_search(database_path: &Path, query: &str) -> Result<(), PersistenceError> {
    let query_text = query.trim();
    let normalized_query = normalize_search_text(query_text);

    if normalized_query.is_empty() {
        return Ok(());
    }

    let connection = open_configured_connection(database_path)?;
    connection.execute(
        "INSERT INTO recent_searches (
            normalized_query,
            query_text,
            last_used_at_ms,
            use_count
         ) VALUES (?1, ?2, ?3, 1)
         ON CONFLICT(normalized_query) DO UPDATE SET
            query_text = excluded.query_text,
            last_used_at_ms = excluded.last_used_at_ms,
            use_count = recent_searches.use_count + 1",
        params![normalized_query, query_text, now_unix_ms()?],
    )?;

    Ok(())
}

pub fn list_recent_searches(
    database_path: &Path,
    limit: u32,
) -> Result<Vec<RecentSearchRecord>, PersistenceError> {
    let connection = open_configured_connection(database_path)?;
    let bounded_limit = i64::from(limit.clamp(1, 100));
    let mut statement = connection.prepare(
        "SELECT
            query_text,
            normalized_query,
            last_used_at_ms,
            use_count
         FROM recent_searches
         ORDER BY last_used_at_ms DESC, rowid DESC
         LIMIT ?1",
    )?;

    let rows = statement.query_map([bounded_limit], |row| {
        let use_count: i64 = row.get(3)?;
        Ok(RecentSearchRecord {
            query_text: row.get(0)?,
            normalized_query: row.get(1)?,
            last_used_at_ms: row.get(2)?,
            use_count: use_count.max(0) as u64,
        })
    })?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

pub fn list_pinned_documents(
    database_path: &Path,
    limit: u32,
) -> Result<Vec<PinnedDocumentRecord>, PersistenceError> {
    let connection = open_configured_connection(database_path)?;
    let bounded_limit = i64::from(limit.clamp(1, 100));

    let mut statement = connection.prepare(
        "SELECT
            pins.document_id,
            pins.pinned_at_ms,
            (
                SELECT fi.file_instance_id
                FROM file_instances fi
                WHERE fi.document_id = pins.document_id
                ORDER BY
                    CASE fi.availability_state
                        WHEN 'AVAILABLE' THEN 0
                        WHEN 'CHANGED' THEN 1
                        WHEN 'UNAVAILABLE' THEN 2
                        WHEN 'UNKNOWN' THEN 3
                        WHEN 'MISSING' THEN 4
                        ELSE 5
                    END,
                    fi.last_seen_at_ms DESC
                LIMIT 1
            )
         FROM document_pins pins
         JOIN documents d ON d.document_id = pins.document_id
         WHERE d.archived_at_ms IS NULL
         ORDER BY pins.pinned_at_ms DESC
         LIMIT ?1",
    )?;

    let rows = statement.query_map([bounded_limit], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, Option<String>>(2)?,
        ))
    })?;

    let mut metadata = Vec::new();
    for row in rows {
        let (document_id, pinned_at_ms, file_instance_id) = row?;
        if let Some(file_instance_id) = file_instance_id {
            metadata.push((document_id, pinned_at_ms, file_instance_id));
        }
    }
    drop(statement);
    drop(connection);

    let file_instance_ids: Vec<String> = metadata
        .iter()
        .map(|(_, _, file_instance_id)| file_instance_id.clone())
        .collect();
    let files = hydrate_search_files(database_path, &file_instance_ids)?;

    let mut result = Vec::new();
    for (file, (_, pinned_at_ms, _)) in files.into_iter().zip(metadata) {
        let Some(file) = file else {
            continue;
        };

        result.push(PinnedDocumentRecord { file, pinned_at_ms });
    }

    Ok(result)
}

#[cfg(test)]
pub(crate) fn count_controlled_evidence_versions_for_test(
    database_path: &Path,
) -> Result<u64, PersistenceError> {
    let connection = open_configured_connection(database_path)?;
    let count: i64 = connection.query_row(
        "SELECT COUNT(*) FROM controlled_evidence_versions",
        [],
        |row| row.get(0),
    )?;
    Ok(count.max(0) as u64)
}

#[cfg(test)]
pub(crate) fn latest_evidence_capture_status_for_test(
    database_path: &Path,
    file_instance_id: &str,
) -> Result<Option<String>, PersistenceError> {
    let connection = open_configured_connection(database_path)?;
    connection
        .query_row(
            "SELECT status
             FROM evidence_capture_jobs
             WHERE file_instance_id = ?1
             ORDER BY requested_at_ms DESC, rowid DESC
             LIMIT 1",
            [file_instance_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(PersistenceError::from)
}

pub fn encode_native_path_for_storage(path: &Path) -> (Vec<u8>, String) {
    let (bytes, encoding) = encode_native_path(path);
    (bytes, encoding.to_string())
}

pub fn normalize_search_text(value: &str) -> String {
    let mut normalized = String::with_capacity(value.len());

    for character in value.chars().flat_map(char::to_lowercase) {
        if character.is_alphanumeric() {
            normalized.push(character);
        } else {
            normalized.push(' ');
        }
    }

    normalized.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn effective_file_availability_state(
    file_availability_state: &str,
    root_availability_state: &str,
) -> String {
    if root_availability_state == "OFFLINE" {
        "UNAVAILABLE".to_string()
    } else {
        file_availability_state.to_string()
    }
}

fn u64_to_i64(value: u64) -> Result<i64, PersistenceError> {
    i64::try_from(value).map_err(|_| {
        PersistenceError::Configuration(format!("value {value} exceeds SQLite INTEGER range"))
    })
}

fn storage_root_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StorageRootRecord> {
    let storage_root_id: String = row.get(0)?;
    let native_encoding: String = row.get(1)?;
    let native_locator: Vec<u8> = row.get(2)?;
    let canonical_native_locator: Option<Vec<u8>> = row.get(3)?;
    let display_locator: String = row.get(4)?;
    let canonical_display_locator: Option<String> = row.get(5)?;
    let availability_state: String = row.get(6)?;

    let authoritative_native = canonical_native_locator
        .as_deref()
        .unwrap_or(&native_locator);
    let canonical_path =
        decode_native_path(authoritative_native, &native_encoding).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                3,
                rusqlite::types::Type::Blob,
                Box::new(error),
            )
        })?;

    Ok(StorageRootRecord {
        storage_root_id,
        display_path: canonical_display_locator.unwrap_or(display_locator),
        availability_state,
        canonical_path,
    })
}

#[cfg(windows)]
fn encode_native_path(path: &Path) -> (Vec<u8>, &'static str) {
    let mut bytes = Vec::new();

    for unit in path.as_os_str().encode_wide() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }

    (bytes, "windows-utf16le")
}

#[cfg(unix)]
fn encode_native_path(path: &Path) -> (Vec<u8>, &'static str) {
    (path.as_os_str().as_bytes().to_vec(), "unix-bytes")
}

#[cfg(windows)]
fn decode_native_path(bytes: &[u8], encoding: &str) -> Result<PathBuf, PersistenceError> {
    if encoding != "windows-utf16le" {
        return Err(PersistenceError::Configuration(format!(
            "unsupported Windows path encoding {encoding}"
        )));
    }

    if !bytes.len().is_multiple_of(2) {
        return Err(PersistenceError::Configuration(
            "stored UTF-16 path has an odd byte length".to_string(),
        ));
    }

    let wide: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect();

    Ok(PathBuf::from(OsString::from_wide(&wide)))
}

#[cfg(unix)]
fn decode_native_path(bytes: &[u8], encoding: &str) -> Result<PathBuf, PersistenceError> {
    if encoding != "unix-bytes" {
        return Err(PersistenceError::Configuration(format!(
            "unsupported Unix path encoding {encoding}"
        )));
    }

    Ok(PathBuf::from(OsString::from_vec(bytes.to_vec())))
}

#[cfg(windows)]
fn classify_storage_root(path: &Path) -> &'static str {
    use std::path::{Component, Prefix};

    match path.components().next() {
        Some(Component::Prefix(prefix))
            if matches!(prefix.kind(), Prefix::UNC(..) | Prefix::VerbatimUNC(..)) =>
        {
            "NETWORK"
        }
        _ => "LOCAL",
    }
}

#[cfg(unix)]
fn classify_storage_root(_path: &Path) -> &'static str {
    "LOCAL"
}

fn open_configured_connection(database_path: &Path) -> Result<Connection, PersistenceError> {
    let connection = Connection::open(database_path)?;

    connection.busy_timeout(BUSY_TIMEOUT)?;

    let journal_mode: String =
        connection.query_row("PRAGMA journal_mode = WAL;", [], |row| row.get(0))?;

    if !journal_mode.eq_ignore_ascii_case("wal") {
        return Err(PersistenceError::Configuration(format!(
            "expected WAL journal mode, SQLite returned {journal_mode}"
        )));
    }

    connection.execute_batch(
        "PRAGMA synchronous = FULL;
         PRAGMA foreign_keys = ON;",
    )?;

    verify_connection_profile(&connection)?;

    Ok(connection)
}

fn verify_connection_profile(connection: &Connection) -> Result<(), PersistenceError> {
    let journal_mode: String =
        connection.query_row("PRAGMA journal_mode;", [], |row| row.get(0))?;
    if !journal_mode.eq_ignore_ascii_case("wal") {
        return Err(PersistenceError::Configuration(format!(
            "journal mode is {journal_mode}, expected WAL"
        )));
    }

    let synchronous: i64 = connection.query_row("PRAGMA synchronous;", [], |row| row.get(0))?;
    if synchronous != 2 {
        return Err(PersistenceError::Configuration(format!(
            "synchronous mode is {synchronous}, expected FULL (2)"
        )));
    }

    let foreign_keys: i64 = connection.query_row("PRAGMA foreign_keys;", [], |row| row.get(0))?;
    if foreign_keys != 1 {
        return Err(PersistenceError::Configuration(
            "foreign-key enforcement is not enabled".to_string(),
        ));
    }

    let busy_timeout_ms: i64 =
        connection.query_row("PRAGMA busy_timeout;", [], |row| row.get(0))?;
    if busy_timeout_ms < BUSY_TIMEOUT.as_millis() as i64 {
        return Err(PersistenceError::Configuration(format!(
            "busy timeout is {busy_timeout_ms} ms, expected at least {} ms",
            BUSY_TIMEOUT.as_millis()
        )));
    }

    Ok(())
}

fn run_migrations(connection: &mut Connection) -> Result<(), PersistenceError> {
    let user_version: i64 = connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;

    if user_version > LATEST_SCHEMA_VERSION {
        return Err(PersistenceError::Migration(format!(
            "database schema version {user_version} is newer than this application supports ({LATEST_SCHEMA_VERSION})"
        )));
    }

    ensure_migration_history_table(connection)?;

    let recorded_max_version: Option<i64> = connection
        .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
            row.get(0)
        })
        .optional()?
        .flatten();

    if let Some(version) = recorded_max_version {
        if version > LATEST_SCHEMA_VERSION {
            return Err(PersistenceError::Migration(format!(
                "migration history contains unsupported future version {version}"
            )));
        }
    }

    for migration in MIGRATIONS {
        let expected_checksum = migration_checksum(migration.sql);
        let applied_checksum: Option<String> = connection
            .query_row(
                "SELECT checksum FROM schema_migrations WHERE version = ?1",
                [migration.version],
                |row| row.get(0),
            )
            .optional()?;

        match applied_checksum {
            Some(checksum) => {
                if checksum != expected_checksum {
                    return Err(PersistenceError::Migration(format!(
                        "checksum mismatch for migration {:04}_{}",
                        migration.version, migration.name
                    )));
                }
            }
            None => apply_migration(connection, migration, &expected_checksum)?,
        }
    }

    let final_user_version: i64 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;

    if final_user_version != LATEST_SCHEMA_VERSION {
        return Err(PersistenceError::Migration(format!(
            "database user_version is {final_user_version}, expected {LATEST_SCHEMA_VERSION}"
        )));
    }

    Ok(())
}

fn ensure_migration_history_table(connection: &Connection) -> Result<(), PersistenceError> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY NOT NULL,
            name TEXT NOT NULL,
            checksum TEXT NOT NULL,
            applied_at_ms INTEGER NOT NULL,
            application_version TEXT NOT NULL
        );",
    )?;

    Ok(())
}

fn apply_migration(
    connection: &mut Connection,
    migration: &Migration,
    checksum: &str,
) -> Result<(), PersistenceError> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    transaction.execute_batch(migration.sql)?;

    transaction.execute(
        "INSERT INTO schema_migrations (
            version,
            name,
            checksum,
            applied_at_ms,
            application_version
        ) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            migration.version,
            migration.name,
            checksum,
            now_unix_ms()?,
            env!("CARGO_PKG_VERSION")
        ],
    )?;

    transaction.execute_batch(&format!("PRAGMA user_version = {};", migration.version))?;
    transaction.commit()?;

    Ok(())
}

fn migration_checksum(sql: &str) -> String {
    let digest = Sha256::digest(sql.as_bytes());
    let mut output = String::with_capacity(digest.len() * 2);

    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }

    output
}

fn now_unix_ms() -> Result<i64, PersistenceError> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| {
            PersistenceError::Configuration(format!(
                "system clock is before the Unix epoch: {error}"
            ))
        })?;

    i64::try_from(duration.as_millis()).map_err(|_| {
        PersistenceError::Configuration(
            "current timestamp exceeds SQLite INTEGER range".to_string(),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    struct TestDatabase {
        directory: PathBuf,
        path: PathBuf,
    }

    impl TestDatabase {
        fn new() -> Self {
            let directory =
                std::env::temp_dir().join(format!("professional-docx-db-test-{}", Uuid::new_v4()));
            let path = directory.join("metadata.sqlite");

            Self { directory, path }
        }

        fn source_root(&self, name: &str) -> PathBuf {
            self.directory.join(name)
        }
    }

    impl Drop for TestDatabase {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.directory);
        }
    }

    #[test]
    fn fingerprint_baseline_does_not_create_false_change_but_mismatch_does() {
        let database = TestDatabase::new();
        initialize_database(&database.path).expect("database initialization should succeed");

        let source_root = database.source_root("fingerprint-source");
        fs::create_dir_all(&source_root).expect("source root should exist");
        let canonical_root =
            fs::canonicalize(&source_root).expect("source root should canonicalize");
        let root = register_storage_root(
            &database.path,
            "fingerprint-root",
            &source_root,
            &canonical_root,
        )
        .expect("source root should register");

        let index_job_id = Uuid::new_v4().to_string();
        let scan_generation_id = Uuid::new_v4().to_string();
        create_index_job(
            &database.path,
            &root.storage_root_id,
            &index_job_id,
            &scan_generation_id,
        )
        .expect("index job should be created");
        mark_index_job_running(
            &database.path,
            &index_job_id,
            &scan_generation_id,
            &root.storage_root_id,
        )
        .expect("index job should start");

        let relative = Path::new("Ledger.xlsx");
        let (relative_path_native, path_native_encoding) = encode_native_path_for_storage(relative);
        let base_observation = FileObservation {
            relative_path_native,
            path_native_encoding,
            relative_path_display: "Ledger.xlsx".to_string(),
            relative_path_search: normalize_search_text("Ledger.xlsx"),
            display_name: "Ledger.xlsx".to_string(),
            size_bytes: 128,
            creation_time_ms: Some(1_000),
            last_write_time_ms: Some(2_000),
            filesystem_identity: None,
            volume_identity: None,
            file_attributes: None,
            reparse_tag: None,
            quick_fingerprint: None,
            source_stable_during_read: None,
        };

        persist_index_batch(
            &database.path,
            &index_job_id,
            &scan_generation_id,
            &root.storage_root_id,
            std::slice::from_ref(&base_observation),
            &[],
            &IndexProgress::default(),
        )
        .expect("metadata-only observation should persist");

        let initial = list_indexed_file_preview(&database.path, &root.storage_root_id, 10)
            .expect("initial preview should load");
        assert_eq!(initial.len(), 1);
        assert_eq!(initial[0].availability_state, "AVAILABLE");
        assert_eq!(
            count_content_versions_for_test(&database.path, &initial[0].file_instance_id)
                .expect("initial version count should load"),
            1
        );

        let mut fingerprinted = base_observation.clone();
        fingerprinted.quick_fingerprint = Some(vec![0x11; 32]);
        fingerprinted.source_stable_during_read = Some(true);
        persist_index_batch(
            &database.path,
            &index_job_id,
            &scan_generation_id,
            &root.storage_root_id,
            std::slice::from_ref(&fingerprinted),
            &[],
            &IndexProgress::default(),
        )
        .expect("fingerprint baseline should persist");

        let seeded = list_indexed_file_preview(&database.path, &root.storage_root_id, 10)
            .expect("seeded preview should load");
        assert_eq!(seeded[0].availability_state, "AVAILABLE");
        assert_eq!(
            count_content_versions_for_test(&database.path, &seeded[0].file_instance_id)
                .expect("seeded version count should load"),
            2
        );

        persist_index_batch(
            &database.path,
            &index_job_id,
            &scan_generation_id,
            &root.storage_root_id,
            std::slice::from_ref(&fingerprinted),
            &[],
            &IndexProgress::default(),
        )
        .expect("unchanged fingerprint should persist without another version");
        assert_eq!(
            count_content_versions_for_test(&database.path, &seeded[0].file_instance_id)
                .expect("unchanged fingerprint version count should load"),
            2
        );

        let mut changed = fingerprinted;
        changed.quick_fingerprint = Some(vec![0x22; 32]);
        persist_index_batch(
            &database.path,
            &index_job_id,
            &scan_generation_id,
            &root.storage_root_id,
            std::slice::from_ref(&changed),
            &[],
            &IndexProgress::default(),
        )
        .expect("fingerprint change should persist");

        let changed_preview = list_indexed_file_preview(&database.path, &root.storage_root_id, 10)
            .expect("changed preview should load");
        assert_eq!(changed_preview[0].availability_state, "CHANGED");
        assert_eq!(
            count_content_versions_for_test(&database.path, &changed_preview[0].file_instance_id,)
                .expect("changed fingerprint version count should load"),
            3
        );

        let connection =
            open_configured_connection(&database.path).expect("configured connection should open");
        let (verification_state, quick_fingerprint): (String, Option<Vec<u8>>) = connection
            .query_row(
                "SELECT verification_state, quick_fingerprint
                 FROM content_versions
                 WHERE file_instance_id = ?1
                 ORDER BY observed_at_ms DESC, rowid DESC
                 LIMIT 1",
                [&changed_preview[0].file_instance_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("latest content version should load");
        assert_eq!(verification_state, "FINGERPRINTED");
        assert_eq!(quick_fingerprint, Some(vec![0x22; 32]));
    }

    #[test]
    fn fresh_database_applies_initial_migration() {
        let database = TestDatabase::new();

        initialize_database(&database.path).expect("database initialization should succeed");

        let connection =
            open_configured_connection(&database.path).expect("configured connection should open");

        let user_version: i64 = connection
            .query_row("PRAGMA user_version;", [], |row| row.get(0))
            .expect("user_version should be readable");

        assert_eq!(user_version, LATEST_SCHEMA_VERSION);

        let migration_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .expect("migration history should be readable");

        assert_eq!(migration_count, 7);

        let table_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'table'
                   AND name IN (
                       'storage_roots',
                       'documents',
                       'scan_generations',
                       'index_jobs',
                       'file_instances',
                       'file_path_history',
                       'content_versions',
                       'scan_errors',
                       'search_index_outbox',
                       'document_pins',
                       'recent_document_access',
                       'recent_searches',
                       'evidence_capture_jobs',
                       'controlled_evidence_versions',
                       'audit_events',
                       'document_relationships',
                       'clients',
                       'service_types',
                       'engagements',
                       'engagement_areas',
                       'procedures',
                       'workpapers',
                       'workpaper_revisions',
                       'workpaper_evidence_links',
                       'workpaper_workflow_events',
                       'review_notes',
                       'review_note_events'
                   )",
                [],
                |row| row.get(0),
            )
            .expect("schema tables should be queryable");

        assert_eq!(table_count, 27);
    }

    #[test]
    fn second_migration_upgrades_existing_v1_database() {
        let database = TestDatabase::new();
        let parent = database
            .path
            .parent()
            .expect("test database should have a parent");
        fs::create_dir_all(parent).expect("test database directory should be created");

        {
            let mut connection =
                open_configured_connection(&database.path).expect("database should open");
            ensure_migration_history_table(&connection)
                .expect("migration history table should initialize");
            let first = &MIGRATIONS[0];
            let checksum = migration_checksum(first.sql);
            apply_migration(&mut connection, first, &checksum)
                .expect("initial migration should apply");

            let user_version: i64 = connection
                .query_row("PRAGMA user_version;", [], |row| row.get(0))
                .expect("version should be readable");
            assert_eq!(user_version, 1);
        }

        initialize_database(&database.path)
            .expect("database should upgrade through later migrations");

        let connection =
            open_configured_connection(&database.path).expect("upgraded database should open");
        let user_version: i64 = connection
            .query_row("PRAGMA user_version;", [], |row| row.get(0))
            .expect("version should be readable");
        assert_eq!(user_version, 7);

        let table_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'table'
                   AND name IN ('document_pins', 'recent_document_access', 'recent_searches')",
                [],
                |row| row.get(0),
            )
            .expect("quick-access tables should exist");
        assert_eq!(table_count, 3);
    }

    #[test]
    fn third_migration_upgrades_existing_v2_database() {
        let database = TestDatabase::new();
        let parent = database
            .path
            .parent()
            .expect("test database should have a parent");
        fs::create_dir_all(parent).expect("test database directory should be created");

        {
            let mut connection =
                open_configured_connection(&database.path).expect("database should open");
            ensure_migration_history_table(&connection)
                .expect("migration history table should initialize");

            for migration in &MIGRATIONS[..2] {
                let checksum = migration_checksum(migration.sql);
                apply_migration(&mut connection, migration, &checksum)
                    .expect("prior migration should apply");
            }

            let user_version: i64 = connection
                .query_row("PRAGMA user_version;", [], |row| row.get(0))
                .expect("version should be readable");
            assert_eq!(user_version, 2);
        }

        initialize_database(&database.path).expect("database should upgrade through version 7");

        let connection =
            open_configured_connection(&database.path).expect("upgraded database should open");
        let user_version: i64 = connection
            .query_row("PRAGMA user_version;", [], |row| row.get(0))
            .expect("version should be readable");
        assert_eq!(user_version, 7);

        let table_exists: i64 = connection
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM sqlite_master
                    WHERE type = 'table' AND name = 'recent_searches'
                 )",
                [],
                |row| row.get(0),
            )
            .expect("recent-search table should exist");
        assert_eq!(table_exists, 1);
    }

    #[test]
    fn fourth_migration_upgrades_existing_v3_database() {
        let database = TestDatabase::new();
        let parent = database
            .path
            .parent()
            .expect("test database should have a parent");
        fs::create_dir_all(parent).expect("test database directory should be created");

        {
            let mut connection =
                open_configured_connection(&database.path).expect("database should open");
            ensure_migration_history_table(&connection)
                .expect("migration history table should initialize");

            for migration in &MIGRATIONS[..3] {
                let checksum = migration_checksum(migration.sql);
                apply_migration(&mut connection, migration, &checksum)
                    .expect("prior migration should apply");
            }

            let user_version: i64 = connection
                .query_row("PRAGMA user_version;", [], |row| row.get(0))
                .expect("version should be readable");
            assert_eq!(user_version, 3);
        }

        initialize_database(&database.path).expect("database should upgrade through version 7");

        let connection =
            open_configured_connection(&database.path).expect("upgraded database should open");
        let user_version: i64 = connection
            .query_row("PRAGMA user_version;", [], |row| row.get(0))
            .expect("version should be readable");
        assert_eq!(user_version, 7);

        let table_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'table'
                   AND name IN (
                       'evidence_capture_jobs',
                       'controlled_evidence_versions',
                       'audit_events'
                   )",
                [],
                |row| row.get(0),
            )
            .expect("controlled-evidence tables should exist");
        assert_eq!(table_count, 3);
    }

    #[test]
    fn fifth_migration_upgrades_existing_v4_database() {
        let database = TestDatabase::new();
        let parent = database
            .path
            .parent()
            .expect("test database should have a parent");
        fs::create_dir_all(parent).expect("test database directory should be created");

        {
            let mut connection =
                open_configured_connection(&database.path).expect("database should open");
            ensure_migration_history_table(&connection)
                .expect("migration history table should initialize");

            for migration in &MIGRATIONS[..4] {
                let checksum = migration_checksum(migration.sql);
                apply_migration(&mut connection, migration, &checksum)
                    .expect("prior migration should apply");
            }

            let user_version: i64 = connection
                .query_row("PRAGMA user_version;", [], |row| row.get(0))
                .expect("version should be readable");
            assert_eq!(user_version, 4);
        }

        initialize_database(&database.path).expect("database should upgrade to version 7");

        let connection =
            open_configured_connection(&database.path).expect("upgraded database should open");
        let user_version: i64 = connection
            .query_row("PRAGMA user_version;", [], |row| row.get(0))
            .expect("version should be readable");
        assert_eq!(user_version, 7);

        let table_exists: bool = connection
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM sqlite_master
                    WHERE type = 'table' AND name = 'document_relationships'
                 )",
                [],
                |row| row.get(0),
            )
            .expect("document relationship table should exist");
        assert!(table_exists);
    }

    #[test]
    fn sixth_migration_upgrades_existing_v5_database() {
        let database = TestDatabase::new();
        let parent = database
            .path
            .parent()
            .expect("test database should have a parent");
        fs::create_dir_all(parent).expect("test database directory should be created");

        {
            let mut connection =
                open_configured_connection(&database.path).expect("database should open");
            ensure_migration_history_table(&connection)
                .expect("migration history table should initialize");

            for migration in &MIGRATIONS[..5] {
                let checksum = migration_checksum(migration.sql);
                apply_migration(&mut connection, migration, &checksum)
                    .expect("prior migration should apply");
            }

            let user_version: i64 = connection
                .query_row("PRAGMA user_version;", [], |row| row.get(0))
                .expect("version should be readable");
            assert_eq!(user_version, 5);
        }

        initialize_database(&database.path).expect("database should upgrade to version 7");

        let connection =
            open_configured_connection(&database.path).expect("upgraded database should open");
        let user_version: i64 = connection
            .query_row("PRAGMA user_version;", [], |row| row.get(0))
            .expect("version should be readable");
        assert_eq!(user_version, 7);

        let table_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'table'
                   AND name IN (
                       'clients',
                       'service_types',
                       'engagements',
                       'engagement_areas',
                       'procedures',
                       'workpapers',
                       'workpaper_revisions',
                       'workpaper_evidence_links'
                   )",
                [],
                |row| row.get(0),
            )
            .expect("engagement and workpaper tables should exist");
        assert_eq!(table_count, 8);
    }

    #[test]
    fn seventh_migration_upgrades_existing_v6_database() {
        let database = TestDatabase::new();
        let parent = database
            .path
            .parent()
            .expect("test database should have a parent");
        fs::create_dir_all(parent).expect("test database directory should be created");

        {
            let mut connection =
                open_configured_connection(&database.path).expect("database should open");
            ensure_migration_history_table(&connection)
                .expect("migration history table should initialize");

            for migration in &MIGRATIONS[..6] {
                let checksum = migration_checksum(migration.sql);
                apply_migration(&mut connection, migration, &checksum)
                    .expect("prior migration should apply");
            }

            let user_version: i64 = connection
                .query_row("PRAGMA user_version;", [], |row| row.get(0))
                .expect("version should be readable");
            assert_eq!(user_version, 6);
        }

        initialize_database(&database.path).expect("database should upgrade to version 7");

        let connection =
            open_configured_connection(&database.path).expect("upgraded database should open");
        let user_version: i64 = connection
            .query_row("PRAGMA user_version;", [], |row| row.get(0))
            .expect("version should be readable");
        assert_eq!(user_version, 7);

        let table_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'table'
                   AND name IN (
                       'workpaper_workflow_events',
                       'review_notes',
                       'review_note_events'
                   )",
                [],
                |row| row.get(0),
            )
            .expect("review workflow tables should exist");
        assert_eq!(table_count, 3);
    }

    #[test]
    fn engagement_workpapers_use_immutable_revisions_and_exact_evidence_versions() {
        let database = TestDatabase::new();
        initialize_database(&database.path).expect("database initialization should succeed");

        let client = create_client(&database.path, "Northwind Advisory Client")
            .expect("client should be created");
        let service = create_service_type(&database.path, "Custom Assurance Review")
            .expect("custom service type should be created");
        let second_service = create_service_type(&database.path, "Operational Review")
            .expect("second custom service type should be created");

        let engagement = create_engagement(
            &database.path,
            &client.client_id,
            &service.service_type_id,
            "Northwind Custom Review 2026",
            Some("2026-04-01"),
            Some("2027-03-31"),
            "ACTIVE",
        )
        .expect("engagement should be created");
        let second_engagement = create_engagement(
            &database.path,
            &client.client_id,
            &second_service.service_type_id,
            "Northwind Operations 2026",
            None,
            None,
            "ACTIVE",
        )
        .expect("second engagement should be created");

        let root_area = create_engagement_area(
            &database.path,
            &engagement.engagement_id,
            None,
            "Process Assurance",
            Some("PA"),
            10,
            "ACTIVE",
        )
        .expect("root area should be created");
        let child_area = create_engagement_area(
            &database.path,
            &engagement.engagement_id,
            Some(&root_area.engagement_area_id),
            "Vendor Onboarding",
            Some("VO"),
            20,
            "ACTIVE",
        )
        .expect("nested custom area should be created");

        let cross_engagement_error = create_engagement_area(
            &database.path,
            &second_engagement.engagement_id,
            Some(&root_area.engagement_area_id),
            "Invalid Child",
            None,
            0,
            "ACTIVE",
        )
        .expect_err("parent area from another engagement must be rejected");
        assert!(cross_engagement_error
            .to_string()
            .contains("parent area must belong to the same engagement"));

        let procedure = create_procedure(
            &database.path,
            &engagement.engagement_id,
            Some(&child_area.engagement_area_id),
            Some("PROC-01"),
            "Inspect vendor onboarding evidence",
            Some("Review the configured onboarding control and supporting records."),
            "ACTIVE",
        )
        .expect("procedure should be created");

        let workpaper = create_workpaper(
            &database.path,
            &engagement.engagement_id,
            Some(&child_area.engagement_area_id),
            Some(&procedure.procedure_id),
            "WP-01",
            "Vendor onboarding workpaper",
            "IN_PROGRESS",
        )
        .expect("workpaper should be created");

        let revision_one = create_workpaper_revision(
            &database.path,
            &workpaper.workpaper_id,
            NewWorkpaperRevision {
                revision_reason: Some("Initial documentation"),
                objective: "Confirm onboarding approvals operate as designed.",
                procedure_performed: "Inspected selected onboarding records.",
                population: "All vendors added during the period.",
                sample: "Five judgmentally selected vendors.",
                exceptions: "",
                management_explanation: "",
                conclusion: "No exception identified in the initial sample.",
            },
        )
        .expect("first revision should be created");
        assert_eq!(revision_one.revision_number, 1);
        assert_eq!(revision_one.supersedes_revision_id, None);
        assert_eq!(revision_one.content_hash.as_ref().map(Vec::len), Some(32));

        let revision_two = create_workpaper_revision(
            &database.path,
            &workpaper.workpaper_id,
            NewWorkpaperRevision {
                revision_reason: Some("Expanded sample after preparer review"),
                objective: "Confirm onboarding approvals operate as designed.",
                procedure_performed: "Inspected expanded onboarding records.",
                population: "All vendors added during the period.",
                sample: "Eight judgmentally selected vendors.",
                exceptions: "One delayed approval noted.",
                management_explanation: "Approval was completed the following business day.",
                conclusion: "Control operated with one timing exception.",
            },
        )
        .expect("second revision should be created");
        assert_eq!(revision_two.revision_number, 2);
        assert_eq!(
            revision_two.supersedes_revision_id.as_deref(),
            Some(revision_one.workpaper_revision_id.as_str())
        );

        let revisions = list_workpaper_revisions(&database.path, &workpaper.workpaper_id)
            .expect("revision history should load");
        assert_eq!(revisions.len(), 2);
        assert_eq!(revisions[0].revision_number, 2);
        assert_eq!(revisions[1].revision_number, 1);

        let connection =
            open_configured_connection(&database.path).expect("database should reopen");
        let mutation_error = connection
            .execute(
                "UPDATE workpaper_revisions
                 SET conclusion = 'tampered'
                 WHERE workpaper_revision_id = ?1",
                [&revision_one.workpaper_revision_id],
            )
            .expect_err("immutable revision update must be rejected");
        assert!(mutation_error
            .to_string()
            .contains("workpaper revisions are immutable"));

        let source_root = database.source_root("workpaper-evidence-source");
        fs::create_dir_all(&source_root).expect("evidence source root should exist");
        let canonical_root =
            fs::canonicalize(&source_root).expect("evidence source root should canonicalize");
        let storage_root = register_storage_root(
            &database.path,
            "workpaper-evidence-root",
            &source_root,
            &canonical_root,
        )
        .expect("evidence root should register");
        let index_job_id = Uuid::new_v4().to_string();
        let scan_generation_id = Uuid::new_v4().to_string();
        create_index_job(
            &database.path,
            &storage_root.storage_root_id,
            &index_job_id,
            &scan_generation_id,
        )
        .expect("index job should be created");
        mark_index_job_running(
            &database.path,
            &index_job_id,
            &scan_generation_id,
            &storage_root.storage_root_id,
        )
        .expect("index job should start");

        let observation = |name: &str, fingerprint: u8| {
            let relative = Path::new(name);
            let (relative_path_native, path_native_encoding) =
                encode_native_path_for_storage(relative);
            FileObservation {
                relative_path_native,
                path_native_encoding,
                relative_path_display: name.to_string(),
                relative_path_search: normalize_search_text(name),
                display_name: name.to_string(),
                size_bytes: 64,
                creation_time_ms: Some(1_000),
                last_write_time_ms: Some(2_000),
                filesystem_identity: None,
                volume_identity: None,
                file_attributes: None,
                reparse_tag: None,
                quick_fingerprint: Some(vec![fingerprint; 32]),
                source_stable_during_read: Some(true),
            }
        };
        let observations = [
            observation("Support A.pdf", 0x31),
            observation("Support B.pdf", 0x42),
        ];
        persist_index_batch(
            &database.path,
            &index_job_id,
            &scan_generation_id,
            &storage_root.storage_root_id,
            &observations,
            &[],
            &IndexProgress::default(),
        )
        .expect("evidence documents should persist");

        let files = list_indexed_file_preview(&database.path, &storage_root.storage_root_id, 10)
            .expect("evidence files should list");
        let support_a = files
            .iter()
            .find(|file| file.name == "Support A.pdf")
            .expect("support A should exist");
        let support_b = files
            .iter()
            .find(|file| file.name == "Support B.pdf")
            .expect("support B should exist");

        let connection =
            open_configured_connection(&database.path).expect("database should reopen");
        let version_a: String = connection
            .query_row(
                "SELECT content_version_id
                 FROM content_versions
                 WHERE document_id = ?1
                 ORDER BY observed_at_ms DESC, rowid DESC
                 LIMIT 1",
                [&support_a.document_id],
                |row| row.get(0),
            )
            .expect("support A content version should load");

        let historical_link_error = create_workpaper_evidence_link(
            &database.path,
            &revision_one.workpaper_revision_id,
            &support_a.document_id,
            Some(&version_a),
            None,
            "SUPPORTS",
            None,
        )
        .expect_err("superseded workpaper revision must reject new evidence links");
        assert!(historical_link_error
            .to_string()
            .contains("evidence can only be linked to the latest workpaper revision"));

        let evidence_link = create_workpaper_evidence_link(
            &database.path,
            &revision_two.workpaper_revision_id,
            &support_a.document_id,
            Some(&version_a),
            None,
            "SUPPORTS",
            Some("Primary onboarding support"),
        )
        .expect("exact-version evidence link should be created");
        assert_eq!(
            evidence_link.content_version_id.as_deref(),
            Some(version_a.as_str())
        );
        assert_eq!(evidence_link.controlled_evidence_version_id, None);
        assert_eq!(evidence_link.document_name, "Support A.pdf");
        assert!(evidence_link.content_observed_at_ms.is_some());
        assert_eq!(evidence_link.controlled_version_number, None);

        let mismatch = create_workpaper_evidence_link(
            &database.path,
            &revision_two.workpaper_revision_id,
            &support_b.document_id,
            Some(&version_a),
            None,
            "SUPPORTS",
            None,
        )
        .expect_err("version from another document must be rejected");
        assert!(mismatch
            .to_string()
            .contains("content version does not belong to the selected document"));

        let links =
            list_workpaper_evidence_links(&database.path, &revision_two.workpaper_revision_id)
                .expect("evidence links should load");
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].evidence_link_id, evidence_link.evidence_link_id);
        assert_eq!(links[0].document_name, "Support A.pdf");
        assert_eq!(
            links[0].content_version_id.as_deref(),
            Some(version_a.as_str())
        );
        assert!(links[0].content_observed_at_ms.is_some());

        let prepared_event = transition_workpaper_state(
            &database.path,
            &workpaper.workpaper_id,
            "PREPARED",
            Some("preparer@example.test"),
            Some("Preparation complete"),
        )
        .expect("prepared transition should succeed");
        assert_eq!(prepared_event.from_state, "IN_PROGRESS");
        assert_eq!(prepared_event.to_state, "PREPARED");
        assert_eq!(
            prepared_event.workpaper_revision_id.as_deref(),
            Some(revision_two.workpaper_revision_id.as_str())
        );

        let formal_review_error = transition_workpaper_state(
            &database.path,
            &workpaper.workpaper_id,
            "SUBMITTED_FOR_REVIEW",
            Some("preparer@example.test"),
            None,
        )
        .expect_err("raw observed evidence must block formal review");
        assert!(formal_review_error
            .to_string()
            .contains("immutable hash-verified controlled evidence"));

        let workflow_events =
            list_workpaper_workflow_events(&database.path, &workpaper.workpaper_id)
                .expect("workflow history should load");
        assert_eq!(workflow_events.len(), 1);
        assert_eq!(workflow_events[0].to_state, "PREPARED");

        let review_note = create_review_note(
            &database.path,
            NewReviewNote {
                workpaper_id: &workpaper.workpaper_id,
                workpaper_revision_id: &revision_two.workpaper_revision_id,
                evidence_link_id: Some(&evidence_link.evidence_link_id),
                title: "Explain timing exception",
                body: "Please document why the delayed approval does not change the conclusion.",
                owner_id: Some("preparer@example.test"),
                due_at_ms: Some(9_999_999),
                location_kind: Some("PAGE"),
                location_value: Some("2"),
                raised_by: Some("reviewer@example.test"),
            },
        )
        .expect("review note should be raised");
        assert_eq!(review_note.current_state, "OPEN");
        assert_eq!(
            review_note.evidence_link_id.as_deref(),
            Some(evidence_link.evidence_link_id.as_str())
        );

        respond_to_review_note(
            &database.path,
            ReviewNoteAction {
                review_note_id: &review_note.review_note_id,
                actor_id: Some("preparer@example.test"),
                response_text: Some("The approval was completed the next business day."),
                comment: None,
            },
        )
        .expect("review note response should be recorded");
        clear_review_note(
            &database.path,
            ReviewNoteAction {
                review_note_id: &review_note.review_note_id,
                actor_id: Some("reviewer@example.test"),
                response_text: None,
                comment: Some("Response accepted."),
            },
        )
        .expect("review note should clear");
        reopen_review_note(
            &database.path,
            ReviewNoteAction {
                review_note_id: &review_note.review_note_id,
                actor_id: Some("reviewer@example.test"),
                response_text: None,
                comment: Some("Reopened after additional evidence arrived."),
            },
        )
        .expect("cleared review note should reopen");

        let notes = list_review_notes(&database.path, &workpaper.workpaper_id)
            .expect("review notes should load");
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].current_state, "OPEN");

        let note_events = list_review_note_events(&database.path, &review_note.review_note_id)
            .expect("review note history should load");
        assert_eq!(note_events.len(), 4);
        assert_eq!(note_events[0].event_type, "RAISED");
        assert_eq!(note_events[1].event_type, "RESPONSE_SUBMITTED");
        assert_eq!(note_events[2].event_type, "CLEARED");
        assert_eq!(note_events[3].event_type, "REOPENED");

        let connection =
            open_configured_connection(&database.path).expect("database should reopen");
        let event_mutation_error = connection
            .execute(
                "UPDATE review_note_events
                 SET comment = 'tampered'
                 WHERE review_note_event_id = ?1",
                [&note_events[1].review_note_event_id],
            )
            .expect_err("review note event history must be immutable");
        assert!(event_mutation_error
            .to_string()
            .contains("review note events are immutable"));

        let connection =
            open_configured_connection(&database.path).expect("database should reopen");
        let delete_error = connection
            .execute(
                "DELETE FROM workpaper_evidence_links
                 WHERE evidence_link_id = ?1",
                [&evidence_link.evidence_link_id],
            )
            .expect_err("immutable evidence link deletion must be rejected");
        assert!(delete_error
            .to_string()
            .contains("workpaper evidence links are immutable"));

        assert_eq!(
            count_audit_events_for_test(
                &database.path,
                "WORKPAPER_CREATED",
                &workpaper.workpaper_id,
            )
            .expect("workpaper audit count should load"),
            1
        );
        assert_eq!(
            count_audit_events_for_test(
                &database.path,
                "WORKPAPER_REVISION_CREATED",
                &revision_one.workpaper_revision_id,
            )
            .expect("revision audit count should load"),
            1
        );
        assert_eq!(
            count_audit_events_for_test(
                &database.path,
                "WORKPAPER_EVIDENCE_LINK_ADDED",
                &evidence_link.evidence_link_id,
            )
            .expect("evidence-link audit count should load"),
            1
        );

        let workpapers = list_workpapers(&database.path, &engagement.engagement_id)
            .expect("workpapers should list");
        assert_eq!(workpapers.len(), 1);
        assert_eq!(workpapers[0].latest_revision_number, Some(2));

        let areas = list_engagement_areas(&database.path, &engagement.engagement_id)
            .expect("engagement areas should list");
        assert_eq!(areas.len(), 2);
        assert!(areas.iter().any(|area| area.name == "Vendor Onboarding"));
    }

    #[test]
    fn document_relationships_are_bidirectional_in_context_and_audited() {
        let database = TestDatabase::new();
        initialize_database(&database.path).expect("database initialization should succeed");

        let source_root = database.source_root("relationship-source");
        fs::create_dir_all(&source_root).expect("source root should exist");
        let canonical_root =
            fs::canonicalize(&source_root).expect("source root should canonicalize");
        let root = register_storage_root(
            &database.path,
            "relationship-root",
            &source_root,
            &canonical_root,
        )
        .expect("source root should register");

        let index_job_id = Uuid::new_v4().to_string();
        let scan_generation_id = Uuid::new_v4().to_string();
        create_index_job(
            &database.path,
            &root.storage_root_id,
            &index_job_id,
            &scan_generation_id,
        )
        .expect("index job should be created");
        mark_index_job_running(
            &database.path,
            &index_job_id,
            &scan_generation_id,
            &root.storage_root_id,
        )
        .expect("index job should start");

        let make_observation = |name: &str, fingerprint_byte: u8| {
            let relative = Path::new(name);
            let (relative_path_native, path_native_encoding) =
                encode_native_path_for_storage(relative);
            FileObservation {
                relative_path_native,
                path_native_encoding,
                relative_path_display: name.to_string(),
                relative_path_search: normalize_search_text(name),
                display_name: name.to_string(),
                size_bytes: 128,
                creation_time_ms: Some(1_000),
                last_write_time_ms: Some(2_000),
                filesystem_identity: None,
                volume_identity: None,
                file_attributes: None,
                reparse_tag: None,
                quick_fingerprint: Some(vec![fingerprint_byte; 32]),
                source_stable_during_read: Some(true),
            }
        };

        let observations = vec![
            make_observation("Workpaper.xlsx", 0x11),
            make_observation("Invoice.pdf", 0x22),
        ];
        persist_index_batch(
            &database.path,
            &index_job_id,
            &scan_generation_id,
            &root.storage_root_id,
            &observations,
            &[],
            &IndexProgress::default(),
        )
        .expect("relationship fixture files should persist");

        let files = list_indexed_file_preview(&database.path, &root.storage_root_id, 10)
            .expect("relationship fixture preview should load");
        let workpaper = files
            .iter()
            .find(|file| file.name == "Workpaper.xlsx")
            .expect("workpaper should exist");
        let invoice = files
            .iter()
            .find(|file| file.name == "Invoice.pdf")
            .expect("invoice should exist");

        let relationship_id = create_document_relationship(
            &database.path,
            &workpaper.document_id,
            &invoice.document_id,
            "Supports",
        )
        .expect("relationship should be created");
        let duplicate_id = create_document_relationship(
            &database.path,
            &workpaper.document_id,
            &invoice.document_id,
            "supports",
        )
        .expect("case-insensitive duplicate should be idempotent");
        assert_eq!(duplicate_id, relationship_id);

        let outgoing = list_document_relationships(&database.path, &workpaper.document_id)
            .expect("outgoing relationships should load");
        assert_eq!(outgoing.len(), 1);
        assert_eq!(outgoing[0].direction, "OUTGOING");
        assert_eq!(outgoing[0].relationship_type, "Supports");
        assert_eq!(outgoing[0].related_document_id, invoice.document_id);
        assert_eq!(
            outgoing[0]
                .related_file
                .as_ref()
                .expect("related invoice should have a current file")
                .file_instance_id,
            invoice.file_instance_id
        );

        let incoming = list_document_relationships(&database.path, &invoice.document_id)
            .expect("incoming relationships should load");
        assert_eq!(incoming.len(), 1);
        assert_eq!(incoming[0].direction, "INCOMING");
        assert_eq!(incoming[0].related_document_id, workpaper.document_id);
        assert_eq!(
            count_audit_events_for_test(
                &database.path,
                "DOCUMENT_RELATIONSHIP_CREATED",
                &relationship_id,
            )
            .expect("relationship creation audit count should load"),
            1
        );

        remove_document_relationship(&database.path, &relationship_id)
            .expect("relationship should be removed");
        assert!(
            list_document_relationships(&database.path, &workpaper.document_id)
                .expect("removed source context should load")
                .is_empty()
        );
        assert!(
            list_document_relationships(&database.path, &invoice.document_id)
                .expect("removed target context should load")
                .is_empty()
        );
        assert_eq!(
            count_audit_events_for_test(
                &database.path,
                "DOCUMENT_RELATIONSHIP_REMOVED",
                &relationship_id,
            )
            .expect("relationship removal audit count should load"),
            1
        );
    }

    #[test]
    fn recent_searches_deduplicate_normalized_query_and_track_usage() {
        let database = TestDatabase::new();
        initialize_database(&database.path).expect("database initialization should succeed");

        record_recent_search(&database.path, "Salamudd ITR")
            .expect("first recent search should record");
        record_recent_search(&database.path, "salamudd   itr")
            .expect("normalized duplicate should update");
        record_recent_search(&database.path, "GST RCM March")
            .expect("second recent search should record");

        let recent = list_recent_searches(&database.path, 20).expect("recent searches should list");
        assert_eq!(recent.len(), 2);

        let salamudd = recent
            .iter()
            .find(|item| item.normalized_query == "salamudd itr")
            .expect("normalized Salamudd search should exist");
        assert_eq!(salamudd.use_count, 2);
        assert_eq!(salamudd.query_text, "salamudd   itr");
    }

    #[test]
    fn configured_connection_enforces_durability_profile() {
        let database = TestDatabase::new();

        initialize_database(&database.path).expect("database initialization should succeed");

        let connection =
            open_configured_connection(&database.path).expect("configured connection should open");

        verify_connection_profile(&connection).expect("connection profile should remain valid");
    }

    #[test]
    fn foreign_keys_are_actively_enforced() {
        let database = TestDatabase::new();

        initialize_database(&database.path).expect("database initialization should succeed");

        let connection =
            open_configured_connection(&database.path).expect("configured connection should open");

        let result = connection.execute(
            "INSERT INTO documents (
                document_id,
                storage_state,
                display_name,
                created_at_ms
             ) VALUES ('doc-1', 'LINKED', 'Example', 1)",
            [],
        );
        assert!(result.is_ok());

        let invalid_child = connection.execute(
            "INSERT INTO file_instances (
                file_instance_id,
                document_id,
                storage_root_id,
                relative_path_native,
                path_native_encoding,
                relative_path_display,
                relative_path_search,
                size_bytes,
                first_seen_at_ms,
                last_seen_at_ms,
                availability_state
             ) VALUES (
                'instance-1',
                'doc-1',
                'missing-root',
                X'01',
                'test',
                'file.txt',
                'file.txt',
                1,
                1,
                1,
                'AVAILABLE'
             )",
            [],
        );

        assert!(invalid_child.is_err());
    }

    #[test]
    fn applied_migration_checksum_is_verified() {
        let database = TestDatabase::new();

        initialize_database(&database.path).expect("database initialization should succeed");

        {
            let connection = open_configured_connection(&database.path)
                .expect("configured connection should open");
            connection
                .execute(
                    "UPDATE schema_migrations SET checksum = 'tampered' WHERE version = 1",
                    [],
                )
                .expect("test should be able to tamper with migration checksum");
        }

        let error = initialize_database(&database.path)
            .expect_err("tampered migration history must be rejected");

        assert!(error.to_string().contains("checksum mismatch"));
    }

    #[test]
    fn newer_schema_version_is_rejected() {
        let database = TestDatabase::new();

        initialize_database(&database.path).expect("database initialization should succeed");

        {
            let connection = open_configured_connection(&database.path)
                .expect("configured connection should open");
            connection
                .execute_batch("PRAGMA user_version = 999;")
                .expect("test should be able to set a future user_version");
        }

        let error = initialize_database(&database.path)
            .expect_err("future schema versions must be rejected");

        assert!(error
            .to_string()
            .contains("newer than this application supports"));
    }

    #[test]
    fn storage_root_survives_database_reopen() {
        let database = TestDatabase::new();
        let source_root = database.source_root("client-files");
        fs::create_dir_all(&source_root).expect("test source root should be created");

        initialize_database(&database.path).expect("database initialization should succeed");

        let canonical =
            fs::canonicalize(&source_root).expect("test source root should canonicalize");

        let registered =
            register_storage_root(&database.path, "root-persistent", &source_root, &canonical)
                .expect("storage root should register");

        assert_eq!(registered.storage_root_id, "root-persistent");

        initialize_database(&database.path).expect("database reopen should succeed");

        let loaded = get_storage_root(&database.path, "root-persistent")
            .expect("storage root lookup should succeed")
            .expect("storage root should still exist");

        assert_eq!(loaded.storage_root_id, "root-persistent");
        assert_eq!(loaded.canonical_path, canonical);

        let roots = list_storage_roots(&database.path).expect("storage roots should list");
        assert_eq!(roots.len(), 1);
    }

    #[test]
    fn registering_same_canonical_root_reuses_identity() {
        let database = TestDatabase::new();
        let source_root = database.source_root("same-root");
        fs::create_dir_all(&source_root).expect("test source root should be created");

        initialize_database(&database.path).expect("database initialization should succeed");

        let canonical =
            fs::canonicalize(&source_root).expect("test source root should canonicalize");

        let first = register_storage_root(&database.path, "root-one", &source_root, &canonical)
            .expect("first registration should succeed");
        let second = register_storage_root(&database.path, "root-two", &source_root, &canonical)
            .expect("second registration should succeed");

        assert_eq!(first.storage_root_id, "root-one");
        assert_eq!(second.storage_root_id, "root-one");

        let roots = list_storage_roots(&database.path).expect("storage roots should list");
        assert_eq!(roots.len(), 1);
    }
}
