use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use sha2::{Digest, Sha256};
use std::{
    error::Error,
    ffi::OsString,
    fmt, fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

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

pub fn set_storage_root_availability(
    database_path: &Path,
    storage_root_id: &str,
    availability_state: &str,
) -> Result<(), PersistenceError> {
    let connection = open_configured_connection(database_path)?;
    let updated = connection.execute(
        "UPDATE storage_roots
         SET availability_state = ?1,
             updated_at_ms = ?2
         WHERE storage_root_id = ?3",
        params![availability_state, now_unix_ms()?, storage_root_id],
    )?;

    if updated != 1 {
        return Err(PersistenceError::Configuration(format!(
            "storage root {storage_root_id} does not exist"
        )));
    }

    Ok(())
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
