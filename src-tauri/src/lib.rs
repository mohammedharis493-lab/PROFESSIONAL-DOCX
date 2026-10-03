mod evidence;
mod filesystem;
mod indexer;
mod launcher;
mod persistence;
mod search;

use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;
use uuid::Uuid;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApprovedStorageRootDto {
    storage_root_id: String,
    display_path: String,
    availability_state: String,
}

impl From<persistence::StorageRootRecord> for ApprovedStorageRootDto {
    fn from(value: persistence::StorageRootRecord) -> Self {
        Self {
            storage_root_id: value.storage_root_id,
            display_path: value.display_path,
            availability_state: value.availability_state,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct IndexJobDto {
    index_job_id: String,
    storage_root_id: String,
    job_type: String,
    status: String,
    requested_at_ms: i64,
    started_at_ms: Option<i64>,
    completed_at_ms: Option<i64>,
    cancel_requested_at_ms: Option<i64>,
    last_heartbeat_at_ms: Option<i64>,
    current_phase: Option<String>,
    directories_seen: u64,
    files_seen: u64,
    bytes_seen: u64,
    files_persisted: u64,
    errors_count: u64,
    scan_generation_id: Option<String>,
    failure_code: Option<String>,
    failure_message: Option<String>,
}

impl From<persistence::IndexJobRecord> for IndexJobDto {
    fn from(value: persistence::IndexJobRecord) -> Self {
        Self {
            index_job_id: value.index_job_id,
            storage_root_id: value.storage_root_id,
            job_type: value.job_type,
            status: value.status,
            requested_at_ms: value.requested_at_ms,
            started_at_ms: value.started_at_ms,
            completed_at_ms: value.completed_at_ms,
            cancel_requested_at_ms: value.cancel_requested_at_ms,
            last_heartbeat_at_ms: value.last_heartbeat_at_ms,
            current_phase: value.current_phase,
            directories_seen: value.directories_seen,
            files_seen: value.files_seen,
            bytes_seen: value.bytes_seen,
            files_persisted: value.files_persisted,
            errors_count: value.errors_count,
            scan_generation_id: value.scan_generation_id,
            failure_code: value.failure_code,
            failure_message: value.failure_message,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct IndexedFilePreviewDto {
    document_id: String,
    file_instance_id: String,
    name: String,
    path: String,
    extension: String,
    size_bytes: u64,
    modified_unix_ms: Option<i64>,
    availability_state: String,
}

impl From<persistence::IndexedFilePreviewRecord> for IndexedFilePreviewDto {
    fn from(value: persistence::IndexedFilePreviewRecord) -> Self {
        Self {
            document_id: value.document_id,
            file_instance_id: value.file_instance_id,
            name: value.name,
            path: value.path,
            extension: value.extension,
            size_bytes: value.size_bytes,
            modified_unix_ms: value.modified_unix_ms,
            availability_state: value.availability_state,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RecentDocumentDto {
    document_id: String,
    file_instance_id: String,
    name: String,
    path: String,
    extension: String,
    size_bytes: u64,
    modified_unix_ms: Option<i64>,
    availability_state: String,
    last_opened_at_ms: i64,
    open_count: u64,
}

impl From<persistence::RecentDocumentRecord> for RecentDocumentDto {
    fn from(value: persistence::RecentDocumentRecord) -> Self {
        Self {
            document_id: value.file.document_id,
            file_instance_id: value.file.file_instance_id,
            name: value.file.name,
            path: value.file.path,
            extension: value.file.extension,
            size_bytes: value.file.size_bytes,
            modified_unix_ms: value.file.modified_unix_ms,
            availability_state: value.file.availability_state,
            last_opened_at_ms: value.last_opened_at_ms,
            open_count: value.open_count,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PinnedDocumentDto {
    document_id: String,
    file_instance_id: String,
    name: String,
    path: String,
    extension: String,
    size_bytes: u64,
    modified_unix_ms: Option<i64>,
    availability_state: String,
    pinned_at_ms: i64,
}

impl From<persistence::PinnedDocumentRecord> for PinnedDocumentDto {
    fn from(value: persistence::PinnedDocumentRecord) -> Self {
        Self {
            document_id: value.file.document_id,
            file_instance_id: value.file.file_instance_id,
            name: value.file.name,
            path: value.file.path,
            extension: value.file.extension,
            size_bytes: value.file.size_bytes,
            modified_unix_ms: value.file.modified_unix_ms,
            availability_state: value.file.availability_state,
            pinned_at_ms: value.pinned_at_ms,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RecentSearchDto {
    query_text: String,
    normalized_query: String,
    last_used_at_ms: i64,
    use_count: u64,
}

impl From<persistence::RecentSearchRecord> for RecentSearchDto {
    fn from(value: persistence::RecentSearchRecord) -> Self {
        Self {
            query_text: value.query_text,
            normalized_query: value.normalized_query,
            last_used_at_ms: value.last_used_at_ms,
            use_count: value.use_count,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ControlledEvidenceVersionDto {
    controlled_evidence_version_id: String,
    evidence_capture_job_id: String,
    document_id: String,
    source_content_version_id: String,
    version_number: u64,
    sha256_hex: String,
    size_bytes: u64,
    captured_at_ms: i64,
    verification_state: String,
}

impl From<persistence::ControlledEvidenceVersionRecord> for ControlledEvidenceVersionDto {
    fn from(value: persistence::ControlledEvidenceVersionRecord) -> Self {
        Self {
            controlled_evidence_version_id: value.controlled_evidence_version_id,
            evidence_capture_job_id: value.evidence_capture_job_id,
            document_id: value.document_id,
            source_content_version_id: value.source_content_version_id,
            version_number: value.version_number,
            sha256_hex: hex_bytes(&value.sha256),
            size_bytes: value.size_bytes,
            captured_at_ms: value.captured_at_ms,
            verification_state: value.verification_state,
        }
    }
}

fn hex_bytes(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SearchResultDto {
    document_id: String,
    file_instance_id: String,
    name: String,
    path: String,
    extension: String,
    size_bytes: u64,
    modified_unix_ms: Option<i64>,
    availability_state: String,
    matched_field: String,
    score: f32,
}

impl From<search::SearchResultRecord> for SearchResultDto {
    fn from(value: search::SearchResultRecord) -> Self {
        Self {
            document_id: value.document_id,
            file_instance_id: value.file_instance_id,
            name: value.name,
            path: value.path,
            extension: value.extension,
            size_bytes: value.size_bytes,
            modified_unix_ms: value.modified_unix_ms,
            availability_state: value.availability_state,
            matched_field: value.matched_field,
            score: value.score,
        }
    }
}

#[tauri::command]
async fn choose_and_register_storage_root(
    app: AppHandle,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Option<ApprovedStorageRootDto>, String> {
    let selected = app.dialog().file().blocking_pick_folder();

    let Some(selected) = selected else {
        return Ok(None);
    };

    let selected_path = selected
        .into_path()
        .map_err(|_| "Selected folder could not be resolved to a native path.".to_string())?;

    let canonical_path = std::fs::canonicalize(&selected_path)
        .map_err(|error| format!("Unable to access selected folder: {error}"))?;

    if !canonical_path.is_dir() {
        return Err("Selected path is not a folder.".to_string());
    }

    let registered = persistence::register_storage_root(
        database.path(),
        &Uuid::new_v4().to_string(),
        &selected_path,
        &canonical_path,
    )
    .map_err(|error| error.to_string())?;

    Ok(Some(registered.into()))
}

#[tauri::command]
fn list_storage_roots(
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<ApprovedStorageRootDto>, String> {
    persistence::list_storage_roots(database.path())
        .map(|roots| roots.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn start_index_job(
    storage_root_id: String,
    database: State<'_, persistence::DatabaseState>,
    runtime: State<'_, indexer::IndexRuntime>,
) -> Result<IndexJobDto, String> {
    Uuid::parse_str(&storage_root_id)
        .map_err(|_| "Invalid storage-root identifier.".to_string())?;

    let root = persistence::get_storage_root(database.path(), &storage_root_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "Storage root is not approved.".to_string())?;

    let index_job_id = Uuid::new_v4().to_string();
    let scan_generation_id = Uuid::new_v4().to_string();
    let runtime_handle = runtime.inner().clone();
    let cancellation = runtime_handle.reserve(&index_job_id)?;

    let job = match persistence::create_index_job(
        database.path(),
        &storage_root_id,
        &index_job_id,
        &scan_generation_id,
    ) {
        Ok(job) => job,
        Err(error) => {
            runtime_handle.finish(&index_job_id);
            return Err(error.to_string());
        }
    };

    let database_path = database.path().to_path_buf();
    let worker_database_path = database_path.clone();
    let worker_job_id = index_job_id.clone();
    let worker_generation_id = scan_generation_id.clone();
    let worker_root_id = storage_root_id.clone();

    tauri::async_runtime::spawn(async move {
        let worker_result = tauri::async_runtime::spawn_blocking(move || {
            indexer::run_index_job(
                worker_database_path,
                root,
                worker_job_id,
                worker_generation_id,
                cancellation,
            )
            .map_err(|error| error.to_string())
        })
        .await;

        match worker_result {
            Ok(Ok(())) => {}
            Ok(Err(message)) => {
                let _ = persistence::fail_index_job(
                    &database_path,
                    &index_job_id,
                    &scan_generation_id,
                    &worker_root_id,
                    "WORKER_FAILED",
                    &message,
                );
            }
            Err(error) => {
                let _ = persistence::fail_index_job(
                    &database_path,
                    &index_job_id,
                    &scan_generation_id,
                    &worker_root_id,
                    "WORKER_JOIN_FAILED",
                    &error.to_string(),
                );
            }
        }

        runtime_handle.finish(&index_job_id);
    });

    Ok(job.into())
}

#[tauri::command]
fn get_index_job(
    index_job_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Option<IndexJobDto>, String> {
    Uuid::parse_str(&index_job_id).map_err(|_| "Invalid indexing-job identifier.".to_string())?;

    persistence::get_index_job(database.path(), &index_job_id)
        .map(|job| job.map(Into::into))
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn get_latest_index_job_for_root(
    storage_root_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Option<IndexJobDto>, String> {
    Uuid::parse_str(&storage_root_id)
        .map_err(|_| "Invalid storage-root identifier.".to_string())?;

    persistence::get_latest_index_job_for_root(database.path(), &storage_root_id)
        .map(|job| job.map(Into::into))
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn cancel_index_job(
    index_job_id: String,
    database: State<'_, persistence::DatabaseState>,
    runtime: State<'_, indexer::IndexRuntime>,
) -> Result<Option<IndexJobDto>, String> {
    Uuid::parse_str(&index_job_id).map_err(|_| "Invalid indexing-job identifier.".to_string())?;

    let persisted = persistence::request_index_job_cancel(database.path(), &index_job_id)
        .map_err(|error| error.to_string())?;

    if persisted {
        runtime.cancel(&index_job_id)?;
    }

    persistence::get_index_job(database.path(), &index_job_id)
        .map(|job| job.map(Into::into))
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_indexed_file_preview(
    storage_root_id: String,
    limit: Option<u32>,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<IndexedFilePreviewDto>, String> {
    Uuid::parse_str(&storage_root_id)
        .map_err(|_| "Invalid storage-root identifier.".to_string())?;

    persistence::list_indexed_file_preview(database.path(), &storage_root_id, limit.unwrap_or(200))
        .map(|files| files.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_recent_documents(
    limit: Option<u32>,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<RecentDocumentDto>, String> {
    persistence::list_recent_documents(database.path(), limit.unwrap_or(20))
        .map(|records| records.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_recent_searches(
    limit: Option<u32>,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<RecentSearchDto>, String> {
    persistence::list_recent_searches(database.path(), limit.unwrap_or(20))
        .map(|records| records.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn record_recent_search(
    query: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<(), String> {
    persistence::record_recent_search(database.path(), &query).map_err(|error| error.to_string())
}

#[tauri::command]
fn list_pinned_documents(
    limit: Option<u32>,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<PinnedDocumentDto>, String> {
    persistence::list_pinned_documents(database.path(), limit.unwrap_or(50))
        .map(|records| records.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn set_document_pin(
    document_id: String,
    pinned: bool,
    database: State<'_, persistence::DatabaseState>,
) -> Result<(), String> {
    Uuid::parse_str(&document_id).map_err(|_| "Invalid document identifier.".to_string())?;

    persistence::set_document_pin(database.path(), &document_id, pinned)
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn search_documents(
    query: String,
    limit: Option<u32>,
    database: State<'_, persistence::DatabaseState>,
    search_state: State<'_, search::SearchState>,
) -> Result<Vec<SearchResultDto>, String> {
    let database_path = database.path().to_path_buf();
    let search_handle = search_state.inner().clone();
    let bounded_limit = limit.unwrap_or(50);

    tauri::async_runtime::spawn_blocking(move || {
        search::search_documents(&database_path, &search_handle, &query, bounded_limit)
            .map(|results| results.into_iter().map(Into::into).collect())
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("Search task failed to join: {error}"))?
}

#[tauri::command]
async fn capture_controlled_evidence(
    file_instance_id: String,
    database: State<'_, persistence::DatabaseState>,
    evidence_state: State<'_, evidence::EvidenceState>,
) -> Result<ControlledEvidenceVersionDto, String> {
    Uuid::parse_str(&file_instance_id)
        .map_err(|_| "Invalid file-instance identifier.".to_string())?;

    let database_path = database.path().to_path_buf();
    let evidence_handle = evidence_state.inner().clone();

    tauri::async_runtime::spawn_blocking(move || {
        evidence::capture_controlled_evidence(
            &database_path,
            &evidence_handle,
            &file_instance_id,
        )
        .map(Into::into)
        .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("Evidence capture task failed to join: {error}"))?
}

#[tauri::command]
async fn open_file_instance(
    file_instance_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<(), String> {
    Uuid::parse_str(&file_instance_id)
        .map_err(|_| "Invalid file-instance identifier.".to_string())?;

    let database_path = database.path().to_path_buf();

    tauri::async_runtime::spawn_blocking(move || {
        let source = persistence::resolve_file_instance_source(&database_path, &file_instance_id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "Indexed file instance no longer exists.".to_string())?;

        launcher::open_source(&source).map_err(|error| error.to_string())?;
        persistence::record_document_open(&database_path, &file_instance_id)
            .map_err(|error| error.to_string())?;
        Ok(())
    })
    .await
    .map_err(|error| format!("Open-source task failed to join: {error}"))?
}

#[tauri::command]
async fn reveal_file_instance(
    file_instance_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<(), String> {
    Uuid::parse_str(&file_instance_id)
        .map_err(|_| "Invalid file-instance identifier.".to_string())?;

    let database_path = database.path().to_path_buf();

    tauri::async_runtime::spawn_blocking(move || {
        let source = persistence::resolve_file_instance_source(&database_path, &file_instance_id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "Indexed file instance no longer exists.".to_string())?;

        launcher::reveal_source(&source).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("Reveal-source task failed to join: {error}"))?
}

#[tauri::command]
async fn rebuild_search_index(
    database: State<'_, persistence::DatabaseState>,
    search_state: State<'_, search::SearchState>,
) -> Result<(), String> {
    let database_path = database.path().to_path_buf();
    let search_handle = search_state.inner().clone();

    tauri::async_runtime::spawn_blocking(move || {
        search::rebuild_search_index(&database_path, &search_handle)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("Search rebuild task failed to join: {error}"))?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?.join("data");
            let database_path = data_dir.join("metadata.sqlite");
            let search_path = data_dir.join("search-index");
            let evidence_path = data_dir.join("controlled-evidence");

            persistence::initialize_database(&database_path)?;
            persistence::recover_interrupted_index_jobs(&database_path)?;

            let search_state = search::SearchState::new(search_path);
            let search_sync_worker =
                search::SearchSyncWorker::start(database_path.clone(), search_state.clone());

            app.manage(persistence::DatabaseState::new(database_path));
            app.manage(indexer::IndexRuntime::default());
            app.manage(search_state);
            app.manage(search_sync_worker);
            app.manage(evidence::EvidenceState::new(evidence_path));

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            choose_and_register_storage_root,
            list_storage_roots,
            start_index_job,
            get_index_job,
            get_latest_index_job_for_root,
            cancel_index_job,
            list_indexed_file_preview,
            list_recent_documents,
            list_recent_searches,
            record_recent_search,
            list_pinned_documents,
            set_document_pin,
            search_documents,
            capture_controlled_evidence,
            open_file_instance,
            reveal_file_instance,
            rebuild_search_index
        ])
        .run(tauri::generate_context!())
        .expect("error while running Professional DocX");
}
