use crate::persistence::{
    self, FileObservation, IndexProgress, PersistenceError, ScanErrorObservation, StorageRootRecord,
};
use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{SystemTime, UNIX_EPOCH},
};
use walkdir::WalkDir;

const INDEX_BATCH_SIZE: usize = 500;

struct BatchContext<'a> {
    database_path: &'a Path,
    root: &'a StorageRootRecord,
    index_job_id: &'a str,
    scan_generation_id: &'a str,
}

#[derive(Clone, Default)]
pub struct IndexRuntime {
    running: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
}

impl IndexRuntime {
    pub fn reserve(&self, index_job_id: &str) -> Result<Arc<AtomicBool>, String> {
        let mut running = self
            .running
            .lock()
            .map_err(|_| "Index worker registry is unavailable.".to_string())?;

        if !running.is_empty() {
            return Err(
                "Another indexing job is already running. This foundation build allows one active filesystem index job at a time."
                    .to_string(),
            );
        }

        let token = Arc::new(AtomicBool::new(false));
        running.insert(index_job_id.to_string(), token.clone());
        Ok(token)
    }

    pub fn cancel(&self, index_job_id: &str) -> Result<bool, String> {
        let running = self
            .running
            .lock()
            .map_err(|_| "Index worker registry is unavailable.".to_string())?;

        if let Some(token) = running.get(index_job_id) {
            token.store(true, Ordering::Release);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn finish(&self, index_job_id: &str) {
        if let Ok(mut running) = self.running.lock() {
            running.remove(index_job_id);
        }
    }
}

pub fn run_initial_index_job(
    database_path: PathBuf,
    root: StorageRootRecord,
    index_job_id: String,
    scan_generation_id: String,
    cancellation: Arc<AtomicBool>,
) -> Result<(), PersistenceError> {
    persistence::mark_index_job_running(
        &database_path,
        &index_job_id,
        &scan_generation_id,
        &root.storage_root_id,
    )?;

    let batch_context = BatchContext {
        database_path: &database_path,
        root: &root,
        index_job_id: &index_job_id,
        scan_generation_id: &scan_generation_id,
    };

    let mut progress = IndexProgress::default();
    let mut file_batch = Vec::with_capacity(INDEX_BATCH_SIZE);
    let mut error_batch = Vec::new();
    let mut entries_since_flush = 0_usize;
    let mut authority_breaking_error = false;

    match fs::metadata(&root.canonical_path) {
        Ok(metadata) if metadata.is_dir() => {}
        Ok(_) => {
            progress.errors_count = 1;
            error_batch.push(scan_error(
                &root.canonical_path,
                &root.canonical_path,
                "SOURCE_OFFLINE",
                None,
                "Approved storage root is no longer a directory.".to_string(),
            ));
            flush_batch(
                &batch_context,
                &mut file_batch,
                &mut error_batch,
                &mut progress,
            )?;
            return persistence::finish_index_job(
                &database_path,
                &index_job_id,
                &scan_generation_id,
                &root.storage_root_id,
                &progress,
                persistence::IndexJobCompletion {
                    status: "OFFLINE",
                    failure_code: Some("ROOT_NOT_DIRECTORY"),
                    failure_message: Some("Approved storage root is no longer a directory."),
                },
            );
        }
        Err(error) => {
            progress.errors_count = 1;
            error_batch.push(scan_error(
                &root.canonical_path,
                &root.canonical_path,
                "SOURCE_OFFLINE",
                Some(&error),
                format!("Approved storage root is unavailable: {error}"),
            ));
            flush_batch(
                &batch_context,
                &mut file_batch,
                &mut error_batch,
                &mut progress,
            )?;
            return persistence::finish_index_job(
                &database_path,
                &index_job_id,
                &scan_generation_id,
                &root.storage_root_id,
                &progress,
                persistence::IndexJobCompletion {
                    status: "OFFLINE",
                    failure_code: Some("ROOT_OFFLINE"),
                    failure_message: Some("Approved storage root could not be accessed."),
                },
            );
        }
    }

    for entry_result in WalkDir::new(&root.canonical_path).follow_links(false) {
        if cancellation.load(Ordering::Acquire) {
            flush_batch(
                &batch_context,
                &mut file_batch,
                &mut error_batch,
                &mut progress,
            )?;

            return persistence::finish_index_job(
                &database_path,
                &index_job_id,
                &scan_generation_id,
                &root.storage_root_id,
                &progress,
                persistence::IndexJobCompletion {
                    status: "CANCELLED",
                    failure_code: Some("USER_CANCELLED"),
                    failure_message: Some("Indexing was cancelled by the user."),
                },
            );
        }

        entries_since_flush += 1;

        let entry = match entry_result {
            Ok(entry) => entry,
            Err(error) => {
                progress.errors_count = progress.errors_count.saturating_add(1);
                authority_breaking_error = true;
                let path = error.path().unwrap_or(&root.canonical_path);
                error_batch.push(scan_error(
                    &root.canonical_path,
                    path,
                    error
                        .io_error()
                        .map(io_error_category)
                        .unwrap_or("OTHER_IO"),
                    error.io_error(),
                    error.to_string(),
                ));
                maybe_flush(
                    &batch_context,
                    &mut file_batch,
                    &mut error_batch,
                    &mut progress,
                    &mut entries_since_flush,
                )?;
                continue;
            }
        };

        if entry.file_type().is_dir() {
            progress.directories_seen = progress.directories_seen.saturating_add(1);
            maybe_flush(
                &batch_context,
                &mut file_batch,
                &mut error_batch,
                &mut progress,
                &mut entries_since_flush,
            )?;
            continue;
        }

        if entry.file_type().is_symlink() {
            progress.errors_count = progress.errors_count.saturating_add(1);
            error_batch.push(scan_error(
                &root.canonical_path,
                entry.path(),
                "REPARSE_SKIPPED",
                None,
                "Symbolic link or reparse-point entry was intentionally not traversed.".to_string(),
            ));
            maybe_flush(
                &batch_context,
                &mut file_batch,
                &mut error_batch,
                &mut progress,
                &mut entries_since_flush,
            )?;
            continue;
        }

        if !entry.file_type().is_file() {
            continue;
        }

        progress.files_seen = progress.files_seen.saturating_add(1);

        let metadata = match fs::metadata(entry.path()) {
            Ok(metadata) => metadata,
            Err(error) => {
                progress.errors_count = progress.errors_count.saturating_add(1);
                authority_breaking_error = true;
                error_batch.push(scan_error(
                    &root.canonical_path,
                    entry.path(),
                    io_error_category(&error),
                    Some(&error),
                    format!("Unable to read file metadata: {error}"),
                ));
                maybe_flush(
                    &batch_context,
                    &mut file_batch,
                    &mut error_batch,
                    &mut progress,
                    &mut entries_since_flush,
                )?;
                continue;
            }
        };

        let relative_path = match entry.path().strip_prefix(&root.canonical_path) {
            Ok(relative) => relative,
            Err(error) => {
                progress.errors_count = progress.errors_count.saturating_add(1);
                authority_breaking_error = true;
                error_batch.push(scan_error(
                    &root.canonical_path,
                    entry.path(),
                    "OTHER_IO",
                    None,
                    format!("Unable to derive root-relative path: {error}"),
                ));
                maybe_flush(
                    &batch_context,
                    &mut file_batch,
                    &mut error_batch,
                    &mut progress,
                    &mut entries_since_flush,
                )?;
                continue;
            }
        };

        let relative_display = relative_path.to_string_lossy().into_owned();
        let (relative_native, native_encoding) =
            persistence::encode_native_path_for_storage(relative_path);

        progress.bytes_seen = progress.bytes_seen.saturating_add(metadata.len());

        file_batch.push(FileObservation {
            relative_path_native: relative_native,
            path_native_encoding: native_encoding,
            relative_path_display: relative_display.clone(),
            relative_path_search: persistence::normalize_search_text(&relative_display),
            display_name: entry.file_name().to_string_lossy().into_owned(),
            size_bytes: metadata.len(),
            creation_time_ms: metadata.created().ok().and_then(system_time_to_ms),
            last_write_time_ms: metadata.modified().ok().and_then(system_time_to_ms),
        });

        maybe_flush(
            &batch_context,
            &mut file_batch,
            &mut error_batch,
            &mut progress,
            &mut entries_since_flush,
        )?;
    }

    flush_batch(
        &batch_context,
        &mut file_batch,
        &mut error_batch,
        &mut progress,
    )?;

    let final_status = if authority_breaking_error {
        "PARTIAL"
    } else {
        "COMPLETE"
    };

    persistence::finish_index_job(
        &database_path,
        &index_job_id,
        &scan_generation_id,
        &root.storage_root_id,
        &progress,
        persistence::IndexJobCompletion {
            status: final_status,
            failure_code: None,
            failure_message: None,
        },
    )
}

fn maybe_flush(
    context: &BatchContext<'_>,
    files: &mut Vec<FileObservation>,
    errors: &mut Vec<ScanErrorObservation>,
    progress: &mut IndexProgress,
    entries_since_flush: &mut usize,
) -> Result<(), PersistenceError> {
    if *entries_since_flush < INDEX_BATCH_SIZE {
        return Ok(());
    }

    flush_batch(context, files, errors, progress)?;
    *entries_since_flush = 0;
    Ok(())
}

fn flush_batch(
    context: &BatchContext<'_>,
    files: &mut Vec<FileObservation>,
    errors: &mut Vec<ScanErrorObservation>,
    progress: &mut IndexProgress,
) -> Result<(), PersistenceError> {
    if files.is_empty() && errors.is_empty() {
        persistence::persist_index_batch(
            context.database_path,
            context.index_job_id,
            context.scan_generation_id,
            &context.root.storage_root_id,
            files,
            errors,
            progress,
        )?;
        return Ok(());
    }

    let persisted_now = files.len() as u64;

    persistence::persist_index_batch(
        context.database_path,
        context.index_job_id,
        context.scan_generation_id,
        &context.root.storage_root_id,
        files,
        errors,
        progress,
    )?;

    progress.files_persisted = progress.files_persisted.saturating_add(persisted_now);
    files.clear();
    errors.clear();
    Ok(())
}

fn scan_error(
    root: &Path,
    path: &Path,
    category: &str,
    io_error: Option<&io::Error>,
    message: String,
) -> ScanErrorObservation {
    let relative = path.strip_prefix(root).ok();

    let (relative_path_native, path_native_encoding, relative_path_display) =
        if let Some(relative) = relative {
            let (native, encoding) = persistence::encode_native_path_for_storage(relative);
            (
                Some(native),
                Some(encoding),
                Some(relative.to_string_lossy().into_owned()),
            )
        } else {
            (None, None, Some(path.to_string_lossy().into_owned()))
        };

    ScanErrorObservation {
        relative_path_native,
        path_native_encoding,
        relative_path_display,
        category: category.to_string(),
        os_error_code: io_error
            .and_then(|error| error.raw_os_error())
            .map(i64::from),
        message,
    }
}

fn io_error_category(error: &io::Error) -> &'static str {
    match error.kind() {
        io::ErrorKind::PermissionDenied => "PERMISSION_DENIED",
        io::ErrorKind::NotFound => "PATH_NOT_FOUND",
        io::ErrorKind::WouldBlock => "LOCKED",
        _ => "OTHER_IO",
    }
}

fn system_time_to_ms(value: SystemTime) -> Option<i64> {
    value
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_millis()).ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;
    use uuid::Uuid;

    struct TestIndex {
        directory: PathBuf,
        database_path: PathBuf,
        source_root: PathBuf,
    }

    impl TestIndex {
        fn new() -> Self {
            let directory = std::env::temp_dir()
                .join(format!("professional-docx-index-test-{}", Uuid::new_v4()));
            let database_path = directory.join("app-data").join("metadata.sqlite");
            let source_root = directory.join("source");
            fs::create_dir_all(&source_root).expect("test source root should be created");
            persistence::initialize_database(&database_path)
                .expect("test database should initialize");

            Self {
                directory,
                database_path,
                source_root,
            }
        }

        fn register_root(&self, root_id: &str) -> StorageRootRecord {
            let canonical =
                fs::canonicalize(&self.source_root).expect("source root should canonicalize");
            persistence::register_storage_root(
                &self.database_path,
                root_id,
                &self.source_root,
                &canonical,
            )
            .expect("source root should register")
        }
    }

    impl Drop for TestIndex {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.directory);
        }
    }

    #[test]
    fn initial_index_streams_files_into_sqlite_and_completes() {
        let test = TestIndex::new();
        fs::write(test.source_root.join("2024-25 Salamudd ITR.pdf"), b"pdf")
            .expect("test file should be written");
        fs::create_dir_all(test.source_root.join("GST")).expect("nested folder should be created");
        fs::write(test.source_root.join("GST").join("RCM March.xlsx"), b"xlsx")
            .expect("nested test file should be written");

        let root = test.register_root("root-1");
        let job_id = Uuid::new_v4().to_string();
        let generation_id = Uuid::new_v4().to_string();

        persistence::create_initial_index_job(
            &test.database_path,
            &root.storage_root_id,
            &job_id,
            &generation_id,
        )
        .expect("initial job should be created");

        run_initial_index_job(
            test.database_path.clone(),
            root.clone(),
            job_id.clone(),
            generation_id,
            Arc::new(AtomicBool::new(false)),
        )
        .expect("initial index should complete");

        let job = persistence::get_index_job(&test.database_path, &job_id)
            .expect("job lookup should succeed")
            .expect("job should exist");

        assert_eq!(job.status, "COMPLETE");
        assert_eq!(job.files_seen, 2);
        assert_eq!(job.files_persisted, 2);

        let preview =
            persistence::list_indexed_file_preview(&test.database_path, &root.storage_root_id, 20)
                .expect("indexed preview should load");

        assert_eq!(preview.len(), 2);
        assert!(preview.iter().any(|file| file.name.contains("Salamudd")));
        assert!(preview.iter().any(|file| file.name.contains("RCM March")));

        let second = persistence::create_initial_index_job(
            &test.database_path,
            &root.storage_root_id,
            &Uuid::new_v4().to_string(),
            &Uuid::new_v4().to_string(),
        )
        .expect_err("completed initial index must not silently become a path-based rescan");

        assert!(second.to_string().contains("reconciliation"));
    }

    #[test]
    fn pre_cancelled_index_job_finishes_cancelled_without_authoritative_completion() {
        let test = TestIndex::new();
        fs::write(test.source_root.join("file.txt"), b"data").expect("test file should be written");

        let root = test.register_root("root-cancel");
        let job_id = Uuid::new_v4().to_string();
        let generation_id = Uuid::new_v4().to_string();

        persistence::create_initial_index_job(
            &test.database_path,
            &root.storage_root_id,
            &job_id,
            &generation_id,
        )
        .expect("initial job should be created");

        let token = Arc::new(AtomicBool::new(true));

        run_initial_index_job(
            test.database_path.clone(),
            root,
            job_id.clone(),
            generation_id,
            token,
        )
        .expect("cancelled index should close cleanly");

        let job = persistence::get_index_job(&test.database_path, &job_id)
            .expect("job lookup should succeed")
            .expect("job should exist");

        assert_eq!(job.status, "CANCELLED");
        assert_eq!(job.files_persisted, 0);
    }

    #[test]
    fn startup_recovery_marks_running_job_interrupted() {
        let test = TestIndex::new();
        let root = test.register_root("root-recovery");
        let job_id = Uuid::new_v4().to_string();
        let generation_id = Uuid::new_v4().to_string();

        persistence::create_initial_index_job(
            &test.database_path,
            &root.storage_root_id,
            &job_id,
            &generation_id,
        )
        .expect("job should be created");

        persistence::mark_index_job_running(
            &test.database_path,
            &job_id,
            &generation_id,
            &root.storage_root_id,
        )
        .expect("job should start");

        let changed = persistence::recover_interrupted_index_jobs(&test.database_path)
            .expect("startup recovery should succeed");
        assert_eq!(changed, 1);

        let job = persistence::get_index_job(&test.database_path, &job_id)
            .expect("job lookup should succeed")
            .expect("job should exist");
        assert_eq!(job.status, "INTERRUPTED");
    }
}
