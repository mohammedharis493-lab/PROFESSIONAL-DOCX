mod evidence;
mod filesystem;
mod indexer;
mod launcher;
mod persistence;
mod preview;
mod search;
mod spreadsheet;
mod word;

use serde::{Deserialize, Serialize};
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
struct ClientDto {
    client_id: String,
    name: String,
    created_at_ms: i64,
}

impl From<persistence::ClientRecord> for ClientDto {
    fn from(value: persistence::ClientRecord) -> Self {
        Self {
            client_id: value.client_id,
            name: value.name,
            created_at_ms: value.created_at_ms,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ServiceTypeDto {
    service_type_id: String,
    name: String,
    created_at_ms: i64,
}

impl From<persistence::ServiceTypeRecord> for ServiceTypeDto {
    fn from(value: persistence::ServiceTypeRecord) -> Self {
        Self {
            service_type_id: value.service_type_id,
            name: value.name,
            created_at_ms: value.created_at_ms,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EngagementDto {
    engagement_id: String,
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
    parent_area_id: Option<String>,
    name: String,
    code: Option<String>,
    display_order: i64,
    status: String,
    created_at_ms: i64,
}

impl From<persistence::EngagementAreaRecord> for EngagementAreaDto {
    fn from(value: persistence::EngagementAreaRecord) -> Self {
        Self {
            engagement_area_id: value.engagement_area_id,
            engagement_id: value.engagement_id,
            parent_area_id: value.parent_area_id,
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
struct ProcedureDto {
    procedure_id: String,
    engagement_id: String,
    engagement_area_id: Option<String>,
    reference: Option<String>,
    title: String,
    description: Option<String>,
    status: String,
    created_at_ms: i64,
}

impl From<persistence::ProcedureRecord> for ProcedureDto {
    fn from(value: persistence::ProcedureRecord) -> Self {
        Self {
            procedure_id: value.procedure_id,
            engagement_id: value.engagement_id,
            engagement_area_id: value.engagement_area_id,
            reference: value.reference,
            title: value.title,
            description: value.description,
            status: value.status,
            created_at_ms: value.created_at_ms,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkpaperDto {
    workpaper_id: String,
    engagement_id: String,
    engagement_area_id: Option<String>,
    procedure_id: Option<String>,
    reference: String,
    title: String,
    workflow_state: String,
    created_at_ms: i64,
    latest_revision_number: Option<u64>,
}

impl From<persistence::WorkpaperRecord> for WorkpaperDto {
    fn from(value: persistence::WorkpaperRecord) -> Self {
        Self {
            workpaper_id: value.workpaper_id,
            engagement_id: value.engagement_id,
            engagement_area_id: value.engagement_area_id,
            procedure_id: value.procedure_id,
            reference: value.reference,
            title: value.title,
            workflow_state: value.workflow_state,
            created_at_ms: value.created_at_ms,
            latest_revision_number: value.latest_revision_number,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkpaperRevisionInputDto {
    revision_reason: Option<String>,
    objective: String,
    procedure_performed: String,
    population: String,
    sample: String,
    exceptions: String,
    management_explanation: String,
    conclusion: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkpaperRevisionDto {
    workpaper_revision_id: String,
    workpaper_id: String,
    revision_number: u64,
    created_at_ms: i64,
    revision_reason: Option<String>,
    supersedes_revision_id: Option<String>,
    objective: String,
    procedure_performed: String,
    population: String,
    sample: String,
    exceptions: String,
    management_explanation: String,
    conclusion: String,
    content_hash_hex: Option<String>,
}

impl From<persistence::WorkpaperRevisionRecord> for WorkpaperRevisionDto {
    fn from(value: persistence::WorkpaperRevisionRecord) -> Self {
        Self {
            workpaper_revision_id: value.workpaper_revision_id,
            workpaper_id: value.workpaper_id,
            revision_number: value.revision_number,
            created_at_ms: value.created_at_ms,
            revision_reason: value.revision_reason,
            supersedes_revision_id: value.supersedes_revision_id,
            objective: value.objective,
            procedure_performed: value.procedure_performed,
            population: value.population,
            sample: value.sample,
            exceptions: value.exceptions,
            management_explanation: value.management_explanation,
            conclusion: value.conclusion,
            content_hash_hex: value.content_hash.map(|bytes| hex_bytes(&bytes)),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkpaperEvidenceLinkDto {
    evidence_link_id: String,
    workpaper_revision_id: String,
    document_id: String,
    content_version_id: Option<String>,
    controlled_evidence_version_id: Option<String>,
    relationship_type: String,
    description: Option<String>,
    created_at_ms: i64,
}

impl From<persistence::WorkpaperEvidenceLinkRecord> for WorkpaperEvidenceLinkDto {
    fn from(value: persistence::WorkpaperEvidenceLinkRecord) -> Self {
        Self {
            evidence_link_id: value.evidence_link_id,
            workpaper_revision_id: value.workpaper_revision_id,
            document_id: value.document_id,
            content_version_id: value.content_version_id,
            controlled_evidence_version_id: value.controlled_evidence_version_id,
            relationship_type: value.relationship_type,
            description: value.description,
            created_at_ms: value.created_at_ms,
        }
    }
}

fn validate_uuid(value: &str, label: &str) -> Result<(), String> {
    Uuid::parse_str(value).map_err(|_| format!("Invalid {label} identifier."))?;
    Ok(())
}

fn validate_optional_uuid(value: Option<&str>, label: &str) -> Result<(), String> {
    if let Some(value) = value {
        validate_uuid(value, label)?;
    }
    Ok(())
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
fn create_client(
    name: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<ClientDto, String> {
    persistence::create_client(database.path(), &name)
        .map(Into::into)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_clients(database: State<'_, persistence::DatabaseState>) -> Result<Vec<ClientDto>, String> {
    persistence::list_clients(database.path())
        .map(|records| records.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn create_service_type(
    name: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<ServiceTypeDto, String> {
    persistence::create_service_type(database.path(), &name)
        .map(Into::into)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_service_types(
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<ServiceTypeDto>, String> {
    persistence::list_service_types(database.path())
        .map(|records| records.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn create_engagement(
    client_id: String,
    service_type_id: String,
    name: String,
    period_start: Option<String>,
    period_end: Option<String>,
    status: Option<String>,
    database: State<'_, persistence::DatabaseState>,
) -> Result<EngagementDto, String> {
    validate_uuid(&client_id, "client")?;
    validate_uuid(&service_type_id, "service-type")?;
    persistence::create_engagement(
        database.path(),
        &client_id,
        &service_type_id,
        &name,
        period_start.as_deref(),
        period_end.as_deref(),
        status.as_deref().unwrap_or("ACTIVE"),
    )
    .map(Into::into)
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_engagements(
    client_id: Option<String>,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<EngagementDto>, String> {
    validate_optional_uuid(client_id.as_deref(), "client")?;
    persistence::list_engagements(database.path(), client_id.as_deref())
        .map(|records| records.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn create_engagement_area(
    engagement_id: String,
    parent_area_id: Option<String>,
    name: String,
    code: Option<String>,
    display_order: Option<i64>,
    status: Option<String>,
    database: State<'_, persistence::DatabaseState>,
) -> Result<EngagementAreaDto, String> {
    validate_uuid(&engagement_id, "engagement")?;
    validate_optional_uuid(parent_area_id.as_deref(), "parent-area")?;
    persistence::create_engagement_area(
        database.path(),
        &engagement_id,
        parent_area_id.as_deref(),
        &name,
        code.as_deref(),
        display_order.unwrap_or(0),
        status.as_deref().unwrap_or("ACTIVE"),
    )
    .map(Into::into)
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_engagement_areas(
    engagement_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<EngagementAreaDto>, String> {
    validate_uuid(&engagement_id, "engagement")?;
    persistence::list_engagement_areas(database.path(), &engagement_id)
        .map(|records| records.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn create_procedure(
    engagement_id: String,
    engagement_area_id: Option<String>,
    reference: Option<String>,
    title: String,
    description: Option<String>,
    status: Option<String>,
    database: State<'_, persistence::DatabaseState>,
) -> Result<ProcedureDto, String> {
    validate_uuid(&engagement_id, "engagement")?;
    validate_optional_uuid(engagement_area_id.as_deref(), "engagement-area")?;
    persistence::create_procedure(
        database.path(),
        &engagement_id,
        engagement_area_id.as_deref(),
        reference.as_deref(),
        &title,
        description.as_deref(),
        status.as_deref().unwrap_or("ACTIVE"),
    )
    .map(Into::into)
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_procedures(
    engagement_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<ProcedureDto>, String> {
    validate_uuid(&engagement_id, "engagement")?;
    persistence::list_procedures(database.path(), &engagement_id)
        .map(|records| records.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn create_workpaper(
    engagement_id: String,
    engagement_area_id: Option<String>,
    procedure_id: Option<String>,
    reference: String,
    title: String,
    workflow_state: Option<String>,
    database: State<'_, persistence::DatabaseState>,
) -> Result<WorkpaperDto, String> {
    validate_uuid(&engagement_id, "engagement")?;
    validate_optional_uuid(engagement_area_id.as_deref(), "engagement-area")?;
    validate_optional_uuid(procedure_id.as_deref(), "procedure")?;
    persistence::create_workpaper(
        database.path(),
        &engagement_id,
        engagement_area_id.as_deref(),
        procedure_id.as_deref(),
        &reference,
        &title,
        workflow_state.as_deref().unwrap_or("NOT_STARTED"),
    )
    .map(Into::into)
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_workpapers(
    engagement_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<WorkpaperDto>, String> {
    validate_uuid(&engagement_id, "engagement")?;
    persistence::list_workpapers(database.path(), &engagement_id)
        .map(|records| records.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn create_workpaper_revision(
    workpaper_id: String,
    revision: WorkpaperRevisionInputDto,
    database: State<'_, persistence::DatabaseState>,
) -> Result<WorkpaperRevisionDto, String> {
    validate_uuid(&workpaper_id, "workpaper")?;
    persistence::create_workpaper_revision(
        database.path(),
        &workpaper_id,
        persistence::NewWorkpaperRevision {
            revision_reason: revision.revision_reason.as_deref(),
            objective: &revision.objective,
            procedure_performed: &revision.procedure_performed,
            population: &revision.population,
            sample: &revision.sample,
            exceptions: &revision.exceptions,
            management_explanation: &revision.management_explanation,
            conclusion: &revision.conclusion,
        },
    )
    .map(Into::into)
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_workpaper_revisions(
    workpaper_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<WorkpaperRevisionDto>, String> {
    validate_uuid(&workpaper_id, "workpaper")?;
    persistence::list_workpaper_revisions(database.path(), &workpaper_id)
        .map(|records| records.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn create_workpaper_evidence_link(
    workpaper_revision_id: String,
    document_id: String,
    content_version_id: Option<String>,
    controlled_evidence_version_id: Option<String>,
    relationship_type: String,
    description: Option<String>,
    database: State<'_, persistence::DatabaseState>,
) -> Result<WorkpaperEvidenceLinkDto, String> {
    validate_uuid(&workpaper_revision_id, "workpaper-revision")?;
    validate_uuid(&document_id, "document")?;
    validate_optional_uuid(content_version_id.as_deref(), "content-version")?;
    validate_optional_uuid(
        controlled_evidence_version_id.as_deref(),
        "controlled-evidence-version",
    )?;

    persistence::create_workpaper_evidence_link(
        database.path(),
        &workpaper_revision_id,
        &document_id,
        content_version_id.as_deref(),
        controlled_evidence_version_id.as_deref(),
        &relationship_type,
        description.as_deref(),
    )
    .map(Into::into)
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_workpaper_evidence_links(
    workpaper_revision_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<WorkpaperEvidenceLinkDto>, String> {
    validate_uuid(&workpaper_revision_id, "workpaper-revision")?;
    persistence::list_workpaper_evidence_links(database.path(), &workpaper_revision_id)
        .map(|records| records.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
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
            create_client,
            list_clients,
            create_service_type,
            list_service_types,
            create_engagement,
            list_engagements,
            create_engagement_area,
            list_engagement_areas,
            create_procedure,
            list_procedures,
            create_workpaper,
            list_workpapers,
            create_workpaper_revision,
            list_workpaper_revisions,
            create_workpaper_evidence_link,
            list_workpaper_evidence_links,
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
