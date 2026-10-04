use crate::filesystem;
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

pub fn run_index_job(
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
        let platform = filesystem::platform_file_metadata(entry.path(), &metadata);

        file_batch.push(FileObservation {
            relative_path_native: relative_native,
            path_native_encoding: native_encoding,
            relative_path_display: relative_display.clone(),
            relative_path_search: persistence::normalize_search_text(&relative_display),
            display_name: entry.file_name().to_string_lossy().into_owned(),
            size_bytes: metadata.len(),
            creation_time_ms: metadata.created().ok().and_then(system_time_to_ms),
            last_write_time_ms: metadata.modified().ok().and_then(system_time_to_ms),
            filesystem_identity: platform.filesystem_identity,
            volume_identity: platform.volume_identity,
            file_attributes: platform.file_attributes,
            reparse_tag: platform.reparse_tag,
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

        persistence::create_index_job(
            &test.database_path,
            &root.storage_root_id,
            &job_id,
            &generation_id,
        )
        .expect("initial job should be created");

        run_index_job(
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

        let second_job_id = Uuid::new_v4().to_string();
        let second_generation_id = Uuid::new_v4().to_string();
        let second = persistence::create_index_job(
            &test.database_path,
            &root.storage_root_id,
            &second_job_id,
            &second_generation_id,
        )
        .expect("completed initial index should create a reconciliation job");

        assert_eq!(second.job_type, "FULL_RECONCILIATION");

        run_index_job(
            test.database_path.clone(),
            root.clone(),
            second_job_id,
            second_generation_id,
            Arc::new(AtomicBool::new(false)),
        )
        .expect("unchanged reconciliation should complete");

        let after =
            persistence::list_indexed_file_preview(&test.database_path, &root.storage_root_id, 20)
                .expect("reconciled preview should load");

        assert_eq!(after.len(), 2);
        assert!(after
            .iter()
            .all(|file| file.availability_state == "AVAILABLE"));
    }

    #[test]
    fn pre_cancelled_index_job_finishes_cancelled_without_authoritative_completion() {
        let test = TestIndex::new();
        fs::write(test.source_root.join("file.txt"), b"data").expect("test file should be written");

        let root = test.register_root("root-cancel");
        let job_id = Uuid::new_v4().to_string();
        let generation_id = Uuid::new_v4().to_string();

        persistence::create_index_job(
            &test.database_path,
            &root.storage_root_id,
            &job_id,
            &generation_id,
        )
        .expect("initial job should be created");

        let token = Arc::new(AtomicBool::new(true));

        run_index_job(
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
    fn rename_preserves_document_and_file_instance_identity() {
        let test = TestIndex::new();
        let original = test.source_root.join("Revenue March.xlsx");
        let renamed = test.source_root.join("Revenue Apr.xlsx");
        fs::write(&original, b"same bytes").expect("test file should be written");

        let root = test.register_root("root-rename");
        let first_job = Uuid::new_v4().to_string();
        let first_generation = Uuid::new_v4().to_string();
        persistence::create_index_job(
            &test.database_path,
            &root.storage_root_id,
            &first_job,
            &first_generation,
        )
        .expect("initial job should be created");
        run_index_job(
            test.database_path.clone(),
            root.clone(),
            first_job,
            first_generation,
            Arc::new(AtomicBool::new(false)),
        )
        .expect("initial scan should complete");

        let before =
            persistence::list_indexed_file_preview(&test.database_path, &root.storage_root_id, 20)
                .expect("preview should load");
        assert_eq!(before.len(), 1);

        fs::rename(&original, &renamed).expect("test file should rename");

        let second_job = Uuid::new_v4().to_string();
        let second_generation = Uuid::new_v4().to_string();
        let job = persistence::create_index_job(
            &test.database_path,
            &root.storage_root_id,
            &second_job,
            &second_generation,
        )
        .expect("reconciliation job should be created");
        assert_eq!(job.job_type, "FULL_RECONCILIATION");

        run_index_job(
            test.database_path.clone(),
            root.clone(),
            second_job,
            second_generation,
            Arc::new(AtomicBool::new(false)),
        )
        .expect("rename reconciliation should complete");

        let after =
            persistence::list_indexed_file_preview(&test.database_path, &root.storage_root_id, 20)
                .expect("preview should load");
        assert_eq!(after.len(), 1);
        assert_eq!(after[0].document_id, before[0].document_id);
        assert_eq!(after[0].file_instance_id, before[0].file_instance_id);
        assert_eq!(after[0].name, "Revenue Apr.xlsx");
        assert_eq!(after[0].availability_state, "CHANGED");

        let reasons = persistence::path_history_reasons_for_test(
            &test.database_path,
            &after[0].file_instance_id,
        )
        .expect("path history should load");
        assert_eq!(reasons, vec!["DISCOVERED", "RENAMED"]);

        persistence::reconcile_linked_file_instance(
            &test.database_path,
            &after[0].file_instance_id,
        )
        .expect("renamed linked source should reconcile explicitly");

        let reconciled =
            persistence::list_indexed_file_preview(&test.database_path, &root.storage_root_id, 20)
                .expect("reconciled preview should load");
        assert_eq!(reconciled[0].availability_state, "AVAILABLE");
        assert_eq!(
            persistence::count_audit_events_for_test(
                &test.database_path,
                "LINKED_SOURCE_RECONCILED",
                &after[0].file_instance_id,
            )
            .expect("reconciliation audit event count should load"),
            1
        );
    }

    #[test]
    fn in_place_modification_preserves_identity_and_adds_content_version() {
        let test = TestIndex::new();
        let path = test.source_root.join("Ledger.xlsx");
        fs::write(&path, b"old").expect("test file should be written");

        let root = test.register_root("root-modify");
        let first_job = Uuid::new_v4().to_string();
        let first_generation = Uuid::new_v4().to_string();
        persistence::create_index_job(
            &test.database_path,
            &root.storage_root_id,
            &first_job,
            &first_generation,
        )
        .expect("initial job should be created");
        run_index_job(
            test.database_path.clone(),
            root.clone(),
            first_job,
            first_generation,
            Arc::new(AtomicBool::new(false)),
        )
        .expect("initial scan should complete");

        let before =
            persistence::list_indexed_file_preview(&test.database_path, &root.storage_root_id, 20)
                .expect("preview should load");
        assert_eq!(before.len(), 1);
        assert_eq!(
            persistence::count_content_versions_for_test(
                &test.database_path,
                &before[0].file_instance_id
            )
            .expect("version count should load"),
            1
        );

        fs::write(&path, b"new content with different size").expect("file should be modified");

        let second_job = Uuid::new_v4().to_string();
        let second_generation = Uuid::new_v4().to_string();
        persistence::create_index_job(
            &test.database_path,
            &root.storage_root_id,
            &second_job,
            &second_generation,
        )
        .expect("reconciliation job should be created");
        run_index_job(
            test.database_path.clone(),
            root.clone(),
            second_job,
            second_generation,
            Arc::new(AtomicBool::new(false)),
        )
        .expect("modified-file reconciliation should complete");

        let after =
            persistence::list_indexed_file_preview(&test.database_path, &root.storage_root_id, 20)
                .expect("preview should load");
        assert_eq!(after.len(), 1);
        assert_eq!(after[0].document_id, before[0].document_id);
        assert_eq!(after[0].file_instance_id, before[0].file_instance_id);
        assert_eq!(
            persistence::count_content_versions_for_test(
                &test.database_path,
                &after[0].file_instance_id
            )
            .expect("version count should load"),
            2
        );
        assert_eq!(after[0].availability_state, "CHANGED");

        let third_job = Uuid::new_v4().to_string();
        let third_generation = Uuid::new_v4().to_string();
        persistence::create_index_job(
            &test.database_path,
            &root.storage_root_id,
            &third_job,
            &third_generation,
        )
        .expect("follow-up reconciliation job should be created");
        run_index_job(
            test.database_path.clone(),
            root.clone(),
            third_job,
            third_generation,
            Arc::new(AtomicBool::new(false)),
        )
        .expect("unchanged follow-up scan should complete");

        let still_changed =
            persistence::list_indexed_file_preview(&test.database_path, &root.storage_root_id, 20)
                .expect("follow-up preview should load");
        assert_eq!(still_changed[0].availability_state, "CHANGED");
        assert_eq!(
            persistence::count_content_versions_for_test(
                &test.database_path,
                &still_changed[0].file_instance_id
            )
            .expect("version count should remain stable"),
            2
        );

        fs::write(
            &path,
            b"unindexed third state that must invalidate stale reconciliation",
        )
        .expect("source should change again after the successful scan");

        let reconcile_error = persistence::reconcile_linked_file_instance(
            &test.database_path,
            &still_changed[0].file_instance_id,
        )
        .expect_err("reconciliation must reject a source that changed again after scanning");
        assert!(reconcile_error
            .to_string()
            .contains("changed again since the last successful scan"));

        let rejected =
            persistence::list_indexed_file_preview(&test.database_path, &root.storage_root_id, 20)
                .expect("preview should remain readable after rejected reconciliation");
        assert_eq!(rejected[0].availability_state, "CHANGED");
        assert_eq!(
            persistence::count_content_versions_for_test(
                &test.database_path,
                &rejected[0].file_instance_id
            )
            .expect("rejected reconciliation must not invent a content version"),
            2
        );
        assert_eq!(
            persistence::count_audit_events_for_test(
                &test.database_path,
                "LINKED_SOURCE_RECONCILED",
                &rejected[0].file_instance_id,
            )
            .expect("rejected reconciliation must not record a successful audit event"),
            0
        );
    }

    #[test]
    fn copied_file_becomes_a_distinct_document_and_file_instance() {
        let test = TestIndex::new();
        let source = test.source_root.join("Invoice.pdf");
        let copy = test.source_root.join("Invoice Copy.pdf");
        fs::write(&source, b"identical bytes").expect("source should be written");

        let root = test.register_root("root-copy");
        let first_job = Uuid::new_v4().to_string();
        let first_generation = Uuid::new_v4().to_string();
        persistence::create_index_job(
            &test.database_path,
            &root.storage_root_id,
            &first_job,
            &first_generation,
        )
        .expect("initial job should be created");
        run_index_job(
            test.database_path.clone(),
            root.clone(),
            first_job,
            first_generation,
            Arc::new(AtomicBool::new(false)),
        )
        .expect("initial scan should complete");

        fs::copy(&source, &copy).expect("copy should be created");

        let second_job = Uuid::new_v4().to_string();
        let second_generation = Uuid::new_v4().to_string();
        persistence::create_index_job(
            &test.database_path,
            &root.storage_root_id,
            &second_job,
            &second_generation,
        )
        .expect("reconciliation job should be created");
        run_index_job(
            test.database_path.clone(),
            root.clone(),
            second_job,
            second_generation,
            Arc::new(AtomicBool::new(false)),
        )
        .expect("copy reconciliation should complete");

        let files =
            persistence::list_indexed_file_preview(&test.database_path, &root.storage_root_id, 20)
                .expect("preview should load");
        let available: Vec<_> = files
            .iter()
            .filter(|file| file.availability_state == "AVAILABLE")
            .collect();

        assert_eq!(available.len(), 2);
        assert_ne!(available[0].file_instance_id, available[1].file_instance_id);
        assert_ne!(available[0].document_id, available[1].document_id);
    }

    #[test]
    fn complete_reconciliation_marks_unseen_file_missing() {
        let test = TestIndex::new();
        let path = test.source_root.join("Old Support.pdf");
        fs::write(&path, b"evidence").expect("test file should be written");

        let root = test.register_root("root-missing");
        let first_job = Uuid::new_v4().to_string();
        let first_generation = Uuid::new_v4().to_string();
        persistence::create_index_job(
            &test.database_path,
            &root.storage_root_id,
            &first_job,
            &first_generation,
        )
        .expect("initial job should be created");
        run_index_job(
            test.database_path.clone(),
            root.clone(),
            first_job,
            first_generation,
            Arc::new(AtomicBool::new(false)),
        )
        .expect("initial scan should complete");

        fs::remove_file(&path).expect("test file should be removed");

        let second_job = Uuid::new_v4().to_string();
        let second_generation = Uuid::new_v4().to_string();
        persistence::create_index_job(
            &test.database_path,
            &root.storage_root_id,
            &second_job,
            &second_generation,
        )
        .expect("reconciliation job should be created");
        run_index_job(
            test.database_path.clone(),
            root.clone(),
            second_job,
            second_generation,
            Arc::new(AtomicBool::new(false)),
        )
        .expect("reconciliation should complete");

        let files =
            persistence::list_indexed_file_preview(&test.database_path, &root.storage_root_id, 20)
                .expect("preview should load");
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].availability_state, "MISSING");
    }

    #[test]
    fn missing_linked_file_can_be_relinked_to_an_approved_root_without_changing_identity() {
        let test = TestIndex::new();
        let source_path = test.source_root.join("Moved Support.pdf");
        fs::write(&source_path, b"same evidence").expect("test file should be written");

        let source_root = test.register_root("root-relink-source");
        let target_directory = test.directory.join("target");
        fs::create_dir_all(&target_directory).expect("target root should be created");
        let target_canonical =
            fs::canonicalize(&target_directory).expect("target root should canonicalize");
        let target_root = persistence::register_storage_root(
            &test.database_path,
            "root-relink-target",
            &target_directory,
            &target_canonical,
        )
        .expect("target root should register");

        let first_job = Uuid::new_v4().to_string();
        let first_generation = Uuid::new_v4().to_string();
        persistence::create_index_job(
            &test.database_path,
            &source_root.storage_root_id,
            &first_job,
            &first_generation,
        )
        .expect("initial job should be created");
        run_index_job(
            test.database_path.clone(),
            source_root.clone(),
            first_job,
            first_generation,
            Arc::new(AtomicBool::new(false)),
        )
        .expect("initial scan should complete");

        let before = persistence::list_indexed_file_preview(
            &test.database_path,
            &source_root.storage_root_id,
            20,
        )
        .expect("source preview should load");
        assert_eq!(before.len(), 1);

        let target_path = target_directory.join("Moved Support.pdf");
        fs::rename(&source_path, &target_path).expect("test file should move to approved target");

        let second_job = Uuid::new_v4().to_string();
        let second_generation = Uuid::new_v4().to_string();
        persistence::create_index_job(
            &test.database_path,
            &source_root.storage_root_id,
            &second_job,
            &second_generation,
        )
        .expect("missing-file reconciliation job should be created");
        run_index_job(
            test.database_path.clone(),
            source_root.clone(),
            second_job,
            second_generation,
            Arc::new(AtomicBool::new(false)),
        )
        .expect("missing-file reconciliation should complete");

        let missing = persistence::list_indexed_file_preview(
            &test.database_path,
            &source_root.storage_root_id,
            20,
        )
        .expect("missing preview should load");
        assert_eq!(missing[0].availability_state, "MISSING");

        let relinked = persistence::relink_linked_file_instance(
            &test.database_path,
            &missing[0].file_instance_id,
            &target_path,
        )
        .expect("approved target should relink");
        assert_eq!(relinked.document_id, before[0].document_id);
        assert_eq!(relinked.file_instance_id, before[0].file_instance_id);
        assert_eq!(relinked.availability_state, "CHANGED");
        assert_eq!(relinked.path, target_canonical.join("Moved Support.pdf").to_string_lossy());

        let target_preview = persistence::list_indexed_file_preview(
            &test.database_path,
            &target_root.storage_root_id,
            20,
        )
        .expect("target preview should load");
        assert_eq!(target_preview.len(), 1);
        assert_eq!(target_preview[0].file_instance_id, before[0].file_instance_id);
        assert_eq!(target_preview[0].availability_state, "CHANGED");
        assert_eq!(
            persistence::path_history_reasons_for_test(
                &test.database_path,
                &before[0].file_instance_id,
            )
            .expect("path history should load"),
            vec!["DISCOVERED".to_string(), "RELINKED".to_string()]
        );
        assert_eq!(
            persistence::count_audit_events_for_test(
                &test.database_path,
                "LINKED_SOURCE_RELINKED",
                &before[0].file_instance_id,
            )
            .expect("relink audit event count should load"),
            1
        );

        persistence::reconcile_linked_file_instance(
            &test.database_path,
            &before[0].file_instance_id,
        )
        .expect("relinked source should still require explicit reconciliation");

        let reconciled = persistence::list_indexed_file_preview(
            &test.database_path,
            &target_root.storage_root_id,
            20,
        )
        .expect("reconciled target preview should load");
        assert_eq!(reconciled[0].availability_state, "AVAILABLE");
    }

    #[test]
    fn relink_rejects_a_selected_file_outside_approved_roots() {
        let test = TestIndex::new();
        let source_path = test.source_root.join("Missing Source.pdf");
        fs::write(&source_path, b"original evidence").expect("test file should be written");

        let root = test.register_root("root-relink-boundary");
        let first_job = Uuid::new_v4().to_string();
        let first_generation = Uuid::new_v4().to_string();
        persistence::create_index_job(
            &test.database_path,
            &root.storage_root_id,
            &first_job,
            &first_generation,
        )
        .expect("initial job should be created");
        run_index_job(
            test.database_path.clone(),
            root.clone(),
            first_job,
            first_generation,
            Arc::new(AtomicBool::new(false)),
        )
        .expect("initial scan should complete");

        let indexed =
            persistence::list_indexed_file_preview(&test.database_path, &root.storage_root_id, 20)
                .expect("indexed preview should load");
        fs::remove_file(&source_path).expect("indexed source should be removed");

        let second_job = Uuid::new_v4().to_string();
        let second_generation = Uuid::new_v4().to_string();
        persistence::create_index_job(
            &test.database_path,
            &root.storage_root_id,
            &second_job,
            &second_generation,
        )
        .expect("reconciliation job should be created");
        run_index_job(
            test.database_path.clone(),
            root.clone(),
            second_job,
            second_generation,
            Arc::new(AtomicBool::new(false)),
        )
        .expect("missing-file reconciliation should complete");

        let outside_directory = test.directory.join("outside");
        fs::create_dir_all(&outside_directory).expect("outside directory should be created");
        let outside_path = outside_directory.join("Unapproved.pdf");
        fs::write(&outside_path, b"not approved").expect("outside file should be written");

        let error = persistence::relink_linked_file_instance(
            &test.database_path,
            &indexed[0].file_instance_id,
            &outside_path,
        )
        .expect_err("unapproved relink target must be rejected");
        assert!(error
            .to_string()
            .contains("outside the approved available storage roots"));

        let still_missing =
            persistence::list_indexed_file_preview(&test.database_path, &root.storage_root_id, 20)
                .expect("source preview should still load");
        assert_eq!(still_missing[0].availability_state, "MISSING");
        assert_eq!(
            persistence::path_history_reasons_for_test(
                &test.database_path,
                &indexed[0].file_instance_id,
            )
            .expect("path history should remain unchanged"),
            vec!["DISCOVERED".to_string()]
        );
        assert_eq!(
            persistence::count_audit_events_for_test(
                &test.database_path,
                "LINKED_SOURCE_RELINKED",
                &indexed[0].file_instance_id,
            )
            .expect("rejected relink must not emit audit event"),
            0
        );
    }

    #[test]
    fn offline_root_surfaces_indexed_files_as_unavailable_without_marking_them_missing() {
        let test = TestIndex::new();
        let path = test.source_root.join("Network Support.pdf");
        fs::write(&path, b"evidence").expect("test file should be written");

        let root = test.register_root("root-offline");
        let first_job = Uuid::new_v4().to_string();
        let first_generation = Uuid::new_v4().to_string();
        persistence::create_index_job(
            &test.database_path,
            &root.storage_root_id,
            &first_job,
            &first_generation,
        )
        .expect("initial job should be created");
        run_index_job(
            test.database_path.clone(),
            root.clone(),
            first_job,
            first_generation,
            Arc::new(AtomicBool::new(false)),
        )
        .expect("initial scan should complete");

        fs::remove_dir_all(&test.source_root).expect("source root should become unavailable");

        let second_job = Uuid::new_v4().to_string();
        let second_generation = Uuid::new_v4().to_string();
        persistence::create_index_job(
            &test.database_path,
            &root.storage_root_id,
            &second_job,
            &second_generation,
        )
        .expect("offline check should create a reconciliation job");
        run_index_job(
            test.database_path.clone(),
            root.clone(),
            second_job.clone(),
            second_generation,
            Arc::new(AtomicBool::new(false)),
        )
        .expect("offline reconciliation should close cleanly");

        let job = persistence::get_index_job(&test.database_path, &second_job)
            .expect("job lookup should succeed")
            .expect("offline job should exist");
        assert_eq!(job.status, "OFFLINE");

        let files =
            persistence::list_indexed_file_preview(&test.database_path, &root.storage_root_id, 20)
                .expect("offline preview should remain available from the index");
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].availability_state, "UNAVAILABLE");

        let capture_error = persistence::begin_evidence_capture(
            &test.database_path,
            &Uuid::new_v4().to_string(),
            &files[0].file_instance_id,
            "USER_PROMOTED",
            "IMMUTABLE_SNAPSHOT",
        )
        .expect_err("offline approved roots must reject controlled evidence capture");
        assert!(capture_error
            .to_string()
            .contains("storage root is OFFLINE"));
    }

    #[test]
    fn cancelled_reconciliation_never_marks_unseen_file_missing() {
        let test = TestIndex::new();
        let path = test.source_root.join("Keep Me.pdf");
        fs::write(&path, b"evidence").expect("test file should be written");

        let root = test.register_root("root-cancel-reconcile");
        let first_job = Uuid::new_v4().to_string();
        let first_generation = Uuid::new_v4().to_string();
        persistence::create_index_job(
            &test.database_path,
            &root.storage_root_id,
            &first_job,
            &first_generation,
        )
        .expect("initial job should be created");
        run_index_job(
            test.database_path.clone(),
            root.clone(),
            first_job,
            first_generation,
            Arc::new(AtomicBool::new(false)),
        )
        .expect("initial scan should complete");

        fs::remove_file(&path).expect("test file should be removed");

        let second_job = Uuid::new_v4().to_string();
        let second_generation = Uuid::new_v4().to_string();
        persistence::create_index_job(
            &test.database_path,
            &root.storage_root_id,
            &second_job,
            &second_generation,
        )
        .expect("reconciliation job should be created");
        run_index_job(
            test.database_path.clone(),
            root.clone(),
            second_job,
            second_generation,
            Arc::new(AtomicBool::new(true)),
        )
        .expect("cancelled reconciliation should close cleanly");

        let files =
            persistence::list_indexed_file_preview(&test.database_path, &root.storage_root_id, 20)
                .expect("preview should load");
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].availability_state, "AVAILABLE");
    }

    #[test]
    fn recents_and_pins_follow_document_identity_across_rename() {
        let test = TestIndex::new();
        let original = test.source_root.join("Pinned Workpaper.xlsx");
        let renamed = test.source_root.join("Pinned Workpaper Final.xlsx");
        fs::write(&original, b"working paper").expect("test file should be written");

        let root = test.register_root("root-quick-access");
        let first_job = Uuid::new_v4().to_string();
        let first_generation = Uuid::new_v4().to_string();

        persistence::create_index_job(
            &test.database_path,
            &root.storage_root_id,
            &first_job,
            &first_generation,
        )
        .expect("initial job should be created");
        run_index_job(
            test.database_path.clone(),
            root.clone(),
            first_job,
            first_generation,
            Arc::new(AtomicBool::new(false)),
        )
        .expect("initial scan should complete");

        let before =
            persistence::list_indexed_file_preview(&test.database_path, &root.storage_root_id, 20)
                .expect("preview should load");
        assert_eq!(before.len(), 1);

        persistence::set_document_pin(&test.database_path, &before[0].document_id, true)
            .expect("document should pin");
        persistence::record_document_open(&test.database_path, &before[0].file_instance_id)
            .expect("first open should record");
        persistence::record_document_open(&test.database_path, &before[0].file_instance_id)
            .expect("second open should record");

        let recents = persistence::list_recent_documents(&test.database_path, 10)
            .expect("recents should load");
        assert_eq!(recents.len(), 1);
        assert_eq!(recents[0].file.document_id, before[0].document_id);
        assert_eq!(recents[0].open_count, 2);

        let pins =
            persistence::list_pinned_documents(&test.database_path, 10).expect("pins should load");
        assert_eq!(pins.len(), 1);
        assert_eq!(pins[0].file.document_id, before[0].document_id);

        fs::rename(&original, &renamed).expect("test file should rename");

        let second_job = Uuid::new_v4().to_string();
        let second_generation = Uuid::new_v4().to_string();
        persistence::create_index_job(
            &test.database_path,
            &root.storage_root_id,
            &second_job,
            &second_generation,
        )
        .expect("reconciliation job should be created");
        run_index_job(
            test.database_path.clone(),
            root,
            second_job,
            second_generation,
            Arc::new(AtomicBool::new(false)),
        )
        .expect("rename reconciliation should complete");

        let recents_after = persistence::list_recent_documents(&test.database_path, 10)
            .expect("recents should survive rename");
        let pins_after = persistence::list_pinned_documents(&test.database_path, 10)
            .expect("pins should survive rename");

        assert_eq!(recents_after.len(), 1);
        assert_eq!(pins_after.len(), 1);
        assert_eq!(recents_after[0].file.name, "Pinned Workpaper Final.xlsx");
        assert_eq!(pins_after[0].file.name, "Pinned Workpaper Final.xlsx");
        assert_eq!(pins_after[0].file.document_id, before[0].document_id);

        persistence::set_document_pin(&test.database_path, &before[0].document_id, false)
            .expect("document should unpin");
        assert!(persistence::list_pinned_documents(&test.database_path, 10)
            .expect("pins should reload")
            .is_empty());
    }

    #[test]
    fn startup_recovery_marks_running_job_interrupted() {
        let test = TestIndex::new();
        let root = test.register_root("root-recovery");
        let job_id = Uuid::new_v4().to_string();
        let generation_id = Uuid::new_v4().to_string();

        persistence::create_index_job(
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
