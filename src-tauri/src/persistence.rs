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
const LATEST_SCHEMA_VERSION: i64 = 1;

struct Migration {
    version: i64,
    name: &'static str,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    name: "initial",
    sql: include_str!("../migrations/0001_initial.sql"),
}];

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
                availability_state
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
                availability_state
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
    let content_changed = existing.size_bytes != u64_to_i64(observation.size_bytes)?
        || matches!(
            observation.last_write_time_ms,
            Some(current) if existing.last_write_time_ms != Some(current)
        );
    let availability_changed = existing.availability_state != "AVAILABLE";
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
             availability_state = 'AVAILABLE'
         WHERE file_instance_id = ?14",
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

    if content_changed {
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
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 'METADATA_ONLY', NULL)",
        params![
            &content_version_id,
            document_id,
            file_instance_id,
            observed_at_ms,
            u64_to_i64(observation.size_bytes)?,
            observation.last_write_time_ms
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
            availability_state,
        });
    }

    Ok(result)
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

        assert_eq!(migration_count, 1);

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
                       'search_index_outbox'
                   )",
                [],
                |row| row.get(0),
            )
            .expect("schema tables should be queryable");

        assert_eq!(table_count, 9);
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
