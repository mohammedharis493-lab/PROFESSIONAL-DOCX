mod evidence;
mod filesystem;
mod indexer;
mod launcher;
mod persistence;
mod preview;
mod search;
mod spreadsheet;
mod word;

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
struct DocumentRelationshipDto {
    document_relationship_id: String,
    relationship_type: String,
    direction: String,
    created_at_ms: i64,
    related_document_id: String,
    related_document_name: String,
    related_file: Option<IndexedFilePreviewDto>,
}

impl From<persistence::DocumentRelationshipRecord> for DocumentRelationshipDto {
    fn from(value: persistence::DocumentRelationshipRecord) -> Self {
        Self {
            document_relationship_id: value.document_relationship_id,
            relationship_type: value.relationship_type,
            direction: value.direction,
            created_at_ms: value.created_at_ms,
            related_document_id: value.related_document_id,
            related_document_name: value.related_document_name,
            related_file: value.related_file.map(Into::into),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FirmDto {
    firm_id: String,
    name: String,
    created_at_ms: i64,
}

impl From<persistence::FirmRecord> for FirmDto {
    fn from(value: persistence::FirmRecord) -> Self {
        Self {
            firm_id: value.firm_id,
            name: value.name,
            created_at_ms: value.created_at_ms,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClientDto {
    client_id: String,
    firm_id: String,
    name: String,
    reference: Option<String>,
    created_at_ms: i64,
}

impl From<persistence::ClientRecord> for ClientDto {
    fn from(value: persistence::ClientRecord) -> Self {
        Self {
            client_id: value.client_id,
            firm_id: value.firm_id,
            name: value.name,
            reference: value.reference,
            created_at_ms: value.created_at_ms,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ServiceTypeDto {
    service_type_id: String,
    firm_id: String,
    name: String,
    description: Option<String>,
    created_at_ms: i64,
}

impl From<persistence::ServiceTypeRecord> for ServiceTypeDto {
    fn from(value: persistence::ServiceTypeRecord) -> Self {
        Self {
            service_type_id: value.service_type_id,
            firm_id: value.firm_id,
            name: value.name,
            description: value.description,
            created_at_ms: value.created_at_ms,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EngagementDto {
    engagement_id: String,
    firm_id: String,
    client_id: String,
    service_type_id: String,
    name: String,
    period_start: Option<String>,
    period_end: Option<String>,
    status: String,
    created_at_ms: i64,
}

impl From<persistence::EngagementRecord> for EngagementDto {
    fn from(value: persistence::EngagementRecord) -> Self {
        Self {
            engagement_id: value.engagement_id,
            firm_id: value.firm_id,
            client_id: value.client_id,
            service_type_id: value.service_type_id,
            name: value.name,
            period_start: value.period_start,
            period_end: value.period_end,
            status: value.status,
            created_at_ms: value.created_at_ms,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EngagementAreaDto {
    engagement_area_id: String,
    engagement_id: String,
    parent_engagement_area_id: Option<String>,
    name: String,
    code: Option<String>,
    display_order: u32,
    status: String,
    created_at_ms: i64,
}

impl From<persistence::EngagementAreaRecord> for EngagementAreaDto {
    fn from(value: persistence::EngagementAreaRecord) -> Self {
        Self {
            engagement_area_id: value.engagement_area_id,
            engagement_id: value.engagement_id,
            parent_engagement_area_id: value.parent_engagement_area_id,
            name: value.name,
            code: value.code,
            display_order: value.display_order,
            status: value.status,
            created_at_ms: value.created_at_ms,
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DocumentVersionHistoryDto {
    content_version_id: String,
    observed_at_ms: i64,
    size_bytes: u64,
    last_write_time_ms: Option<i64>,
    verification_state: String,
    source_stable_during_read: Option<bool>,
    sha256_hex: Option<String>,
    controlled_evidence_version_id: Option<String>,
    controlled_version_number: Option<u64>,
    captured_at_ms: Option<i64>,
    controlled_verification_state: Option<String>,
    captured_by: Option<String>,
    capture_reason: Option<String>,
    capture_policy: Option<String>,
}

impl From<persistence::DocumentVersionHistoryRecord> for DocumentVersionHistoryDto {
    fn from(value: persistence::DocumentVersionHistoryRecord) -> Self {
        Self {
            content_version_id: value.content_version_id,
            observed_at_ms: value.observed_at_ms,
            size_bytes: value.size_bytes,
            last_write_time_ms: value.last_write_time_ms,
            verification_state: value.verification_state,
            source_stable_during_read: value.source_stable_during_read,
            sha256_hex: value.sha256.map(|bytes| hex_bytes(&bytes)),
            controlled_evidence_version_id: value.controlled_evidence_version_id,
            controlled_version_number: value.controlled_version_number,
            captured_at_ms: value.captured_at_ms,
            controlled_verification_state: value.controlled_verification_state,
            captured_by: value.captured_by,
            capture_reason: value.capture_reason,
            capture_policy: value.capture_policy,
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
fn create_firm(
    name: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<FirmDto, String> {
    persistence::create_firm(database.path(), &name)
        .map(Into::into)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_firms(
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<FirmDto>, String> {
    persistence::list_firms(database.path())
        .map(|records| records.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn create_client(
    firm_id: String,
    name: String,
    reference: Option<String>,
    database: State<'_, persistence::DatabaseState>,
) -> Result<ClientDto, String> {
    Uuid::parse_str(&firm_id).map_err(|_| "Invalid firm identifier.".to_string())?;

    persistence::create_client(database.path(), &firm_id, &name, reference.as_deref())
        .map(Into::into)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_clients(
    firm_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<ClientDto>, String> {
    Uuid::parse_str(&firm_id).map_err(|_| "Invalid firm identifier.".to_string())?;

    persistence::list_clients(database.path(), &firm_id)
        .map(|records| records.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn create_service_type(
    firm_id: String,
    name: String,
    description: Option<String>,
    database: State<'_, persistence::DatabaseState>,
) -> Result<ServiceTypeDto, String> {
    Uuid::parse_str(&firm_id).map_err(|_| "Invalid firm identifier.".to_string())?;

    persistence::create_service_type(database.path(), &firm_id, &name, description.as_deref())
        .map(Into::into)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_service_types(
    firm_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<ServiceTypeDto>, String> {
    Uuid::parse_str(&firm_id).map_err(|_| "Invalid firm identifier.".to_string())?;

    persistence::list_service_types(database.path(), &firm_id)
        .map(|records| records.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn create_engagement(
    firm_id: String,
    client_id: String,
    service_type_id: String,
    name: String,
    period_start: Option<String>,
    period_end: Option<String>,
    status: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<EngagementDto, String> {
    Uuid::parse_str(&firm_id).map_err(|_| "Invalid firm identifier.".to_string())?;
    Uuid::parse_str(&client_id).map_err(|_| "Invalid client identifier.".to_string())?;
    Uuid::parse_str(&service_type_id)
        .map_err(|_| "Invalid service-type identifier.".to_string())?;

    persistence::create_engagement(
        database.path(),
        &firm_id,
        &client_id,
        &service_type_id,
        &name,
        period_start.as_deref(),
        period_end.as_deref(),
        &status,
    )
    .map(Into::into)
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_engagements_for_client(
    client_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<EngagementDto>, String> {
    Uuid::parse_str(&client_id).map_err(|_| "Invalid client identifier.".to_string())?;

    persistence::list_engagements_for_client(database.path(), &client_id)
        .map(|records| records.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn create_engagement_area(
    engagement_id: String,
    parent_engagement_area_id: Option<String>,
    name: String,
    code: Option<String>,
    display_order: u32,
    status: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<EngagementAreaDto, String> {
    Uuid::parse_str(&engagement_id).map_err(|_| "Invalid engagement identifier.".to_string())?;
    if let Some(parent_id) = parent_engagement_area_id.as_deref() {
        Uuid::parse_str(parent_id)
            .map_err(|_| "Invalid parent engagement-area identifier.".to_string())?;
    }

    persistence::create_engagement_area(
        database.path(),
        &engagement_id,
        parent_engagement_area_id.as_deref(),
        &name,
        code.as_deref(),
        display_order,
        &status,
    )
    .map(Into::into)
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_engagement_areas(
    engagement_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<EngagementAreaDto>, String> {
    Uuid::parse_str(&engagement_id).map_err(|_| "Invalid engagement identifier.".to_string())?;

    persistence::list_engagement_areas(database.path(), &engagement_id)
        .map(|records| records.into_iter().map(Into::into).collect())
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
async fn choose_and_relink_file_instance(
    app: AppHandle,
    file_instance_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Option<IndexedFilePreviewDto>, String> {
    Uuid::parse_str(&file_instance_id)
        .map_err(|_| "Invalid file-instance identifier.".to_string())?;

    let selected = app.dialog().file().blocking_pick_file();
    let Some(selected) = selected else {
        return Ok(None);
    };

    let selected_path = selected
        .into_path()
        .map_err(|_| "Selected file could not be resolved to a native path.".to_string())?;

    persistence::relink_linked_file_instance(database.path(), &file_instance_id, &selected_path)
        .map(Into::into)
        .map(Some)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn reconcile_linked_file_instance(
    file_instance_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<(), String> {
    Uuid::parse_str(&file_instance_id)
        .map_err(|_| "Invalid file-instance identifier.".to_string())?;

    persistence::reconcile_linked_file_instance(database.path(), &file_instance_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_document_version_history(
    document_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<DocumentVersionHistoryDto>, String> {
    Uuid::parse_str(&document_id).map_err(|_| "Invalid document identifier.".to_string())?;

    persistence::list_document_version_history(database.path(), &document_id)
        .map(|records| records.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_document_relationships(
    document_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<DocumentRelationshipDto>, String> {
    Uuid::parse_str(&document_id).map_err(|_| "Invalid document identifier.".to_string())?;

    persistence::list_document_relationships(database.path(), &document_id)
        .map(|records| records.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn create_document_relationship(
    source_document_id: String,
    target_document_id: String,
    relationship_type: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<String, String> {
    Uuid::parse_str(&source_document_id)
        .map_err(|_| "Invalid source document identifier.".to_string())?;
    Uuid::parse_str(&target_document_id)
        .map_err(|_| "Invalid target document identifier.".to_string())?;

    persistence::create_document_relationship(
        database.path(),
        &source_document_id,
        &target_document_id,
        &relationship_type,
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn remove_document_relationship(
    document_relationship_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<(), String> {
    Uuid::parse_str(&document_relationship_id)
        .map_err(|_| "Invalid document relationship identifier.".to_string())?;

    persistence::remove_document_relationship(database.path(), &document_relationship_id)
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
        evidence::capture_controlled_evidence(&database_path, &evidence_handle, &file_instance_id)
            .map(Into::into)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("Evidence capture task failed to join: {error}"))?
}

#[tauri::command]
async fn preview_text_file_instance(
    file_instance_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<preview::TextPreview, String> {
    Uuid::parse_str(&file_instance_id)
        .map_err(|_| "Invalid file-instance identifier.".to_string())?;

    let database_path = database.path().to_path_buf();

    tauri::async_runtime::spawn_blocking(move || {
        let source = persistence::resolve_file_instance_source(&database_path, &file_instance_id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "Indexed file instance no longer exists.".to_string())?;

        let preview = preview::preview_text_source(&source)?;
        persistence::record_document_open(&database_path, &file_instance_id)
            .map_err(|error| error.to_string())?;
        Ok(preview)
    })
    .await
    .map_err(|error| format!("Preview task failed to join: {error}"))?
}

#[tauri::command]
async fn search_workbook_file_instance(
    file_instance_id: String,
    query: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<spreadsheet::WorkbookSearchResult, String> {
    Uuid::parse_str(&file_instance_id)
        .map_err(|_| "Invalid file-instance identifier.".to_string())?;

    let database_path = database.path().to_path_buf();

    tauri::async_runtime::spawn_blocking(move || {
        let source = persistence::resolve_file_instance_source(&database_path, &file_instance_id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "Indexed file instance no longer exists.".to_string())?;

        spreadsheet::search_workbook_source(&source, &query)
    })
    .await
    .map_err(|error| format!("Workbook search task failed to join: {error}"))?
}

#[tauri::command]
async fn preview_workbook_file_instance(
    file_instance_id: String,
    sheet_name: Option<String>,
    row_offset: Option<u32>,
    column_offset: Option<u32>,
    row_limit: Option<u32>,
    column_limit: Option<u32>,
    database: State<'_, persistence::DatabaseState>,
) -> Result<spreadsheet::WorkbookPreview, String> {
    Uuid::parse_str(&file_instance_id)
        .map_err(|_| "Invalid file-instance identifier.".to_string())?;

    let database_path = database.path().to_path_buf();
    let record_open = sheet_name.is_none() && row_offset.is_none() && column_offset.is_none();

    tauri::async_runtime::spawn_blocking(move || {
        let source = persistence::resolve_file_instance_source(&database_path, &file_instance_id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "Indexed file instance no longer exists.".to_string())?;

        let preview = spreadsheet::preview_workbook_source(
            &source,
            sheet_name.as_deref(),
            row_offset,
            column_offset,
            row_limit,
            column_limit,
        )?;

        if record_open {
            persistence::record_document_open(&database_path, &file_instance_id)
                .map_err(|error| error.to_string())?;
        }

        Ok(preview)
    })
    .await
    .map_err(|error| format!("Workbook preview task failed to join: {error}"))?
}

#[tauri::command]
async fn preview_word_file_instance(
    file_instance_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<word::WordPreview, String> {
    Uuid::parse_str(&file_instance_id)
        .map_err(|_| "Invalid file-instance identifier.".to_string())?;

    let database_path = database.path().to_path_buf();

    tauri::async_runtime::spawn_blocking(move || {
        let source = persistence::resolve_file_instance_source(&database_path, &file_instance_id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "Indexed file instance no longer exists.".to_string())?;

        let preview = word::preview_word_source(&source)?;
        persistence::record_document_open(&database_path, &file_instance_id)
            .map_err(|error| error.to_string())?;
        Ok(preview)
    })
    .await
    .map_err(|error| format!("Word preview task failed to join: {error}"))?
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
        .register_asynchronous_uri_scheme_protocol("pdx-preview", |context, request, responder| {
            let database_path = context
                .app_handle()
                .state::<persistence::DatabaseState>()
                .path()
                .to_path_buf();

            std::thread::spawn(move || {
                let response = if request.uri().path().starts_with("/image/") {
                    preview::image_preview_response(&database_path, request)
                } else {
                    preview::pdf_preview_response(&database_path, request)
                };
                responder.respond(response);
            });
        })
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
            create_firm,
            list_firms,
            create_client,
            list_clients,
            create_service_type,
            list_service_types,
            create_engagement,
            list_engagements_for_client,
            create_engagement_area,
            list_engagement_areas,
            list_recent_documents,
            list_recent_searches,
            record_recent_search,
            list_pinned_documents,
            set_document_pin,
            choose_and_relink_file_instance,
            reconcile_linked_file_instance,
            list_document_version_history,
            list_document_relationships,
            create_document_relationship,
            remove_document_relationship,
            search_documents,
            capture_controlled_evidence,
            preview_text_file_instance,
            search_workbook_file_instance,
            preview_workbook_file_instance,
            preview_word_file_instance,
            open_file_instance,
            reveal_file_instance,
            rebuild_search_index
        ])
        .run(tauri::generate_context!())
        .expect("error while running Professional DocX");
}
