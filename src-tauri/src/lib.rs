mod evidence;
mod filesystem;
mod indexer;
mod launcher;
mod ledger;
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
struct EngagementTemplateDto {
    engagement_template_id: String,
    name: String,
    description: Option<String>,
    latest_version_id: String,
    latest_version_number: u64,
    service_type_id: String,
    source_engagement_id: Option<String>,
    created_at_ms: i64,
}

impl From<persistence::EngagementTemplateRecord> for EngagementTemplateDto {
    fn from(value: persistence::EngagementTemplateRecord) -> Self {
        Self {
            engagement_template_id: value.engagement_template_id,
            name: value.name,
            description: value.description,
            latest_version_id: value.latest_version_id,
            latest_version_number: value.latest_version_number,
            service_type_id: value.service_type_id,
            source_engagement_id: value.source_engagement_id,
            created_at_ms: value.created_at_ms,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FirmLibraryItemDto {
    firm_library_item_id: String,
    category: String,
    name: String,
    description: Option<String>,
    service_type_id: Option<String>,
    latest_version_id: String,
    latest_version_number: u64,
    latest_definition_hash_hex: String,
    created_at_ms: i64,
}

impl From<persistence::FirmLibraryItemRecord> for FirmLibraryItemDto {
    fn from(value: persistence::FirmLibraryItemRecord) -> Self {
        Self {
            firm_library_item_id: value.firm_library_item_id,
            category: value.category,
            name: value.name,
            description: value.description,
            service_type_id: value.service_type_id,
            latest_version_id: value.latest_version_id,
            latest_version_number: value.latest_version_number,
            latest_definition_hash_hex: hex_bytes(&value.latest_definition_hash),
            created_at_ms: value.created_at_ms,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FirmLibraryVersionDto {
    firm_library_version_id: String,
    firm_library_item_id: String,
    version_number: u64,
    definition_json: String,
    definition_hash_hex: String,
    created_at_ms: i64,
}

impl From<persistence::FirmLibraryVersionRecord> for FirmLibraryVersionDto {
    fn from(value: persistence::FirmLibraryVersionRecord) -> Self {
        Self {
            firm_library_version_id: value.firm_library_version_id,
            firm_library_item_id: value.firm_library_item_id,
            version_number: value.version_number,
            definition_json: value.definition_json,
            definition_hash_hex: hex_bytes(&value.definition_hash),
            created_at_ms: value.created_at_ms,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LedgerColumnMappingInputDto {
    amount_column: u32,
    date_column: Option<u32>,
    account_column: Option<u32>,
    voucher_column: Option<u32>,
    narration_column: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LedgerImportInputDto {
    engagement_id: String,
    controlled_evidence_version_id: String,
    sheet_name: String,
    header_row_number: u32,
    amount_scale: u32,
    mapping: LedgerColumnMappingInputDto,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LedgerImportDto {
    ledger_import_id: String,
    engagement_id: String,
    controlled_evidence_version_id: String,
    document_id: String,
    source_content_version_id: String,
    source_sha256_hex: String,
    sheet_name: String,
    header_row_number: u64,
    amount_column: u32,
    date_column: Option<u32>,
    account_column: Option<u32>,
    voucher_column: Option<u32>,
    narration_column: Option<u32>,
    amount_scale: u32,
    transaction_count: u64,
    imported_at_ms: i64,
}

impl From<persistence::LedgerImportRecord> for LedgerImportDto {
    fn from(value: persistence::LedgerImportRecord) -> Self {
        Self {
            ledger_import_id: value.ledger_import_id,
            engagement_id: value.engagement_id,
            controlled_evidence_version_id: value.controlled_evidence_version_id,
            document_id: value.document_id,
            source_content_version_id: value.source_content_version_id,
            source_sha256_hex: hex_bytes(&value.source_sha256),
            sheet_name: value.sheet_name,
            header_row_number: value.header_row_number,
            amount_column: value.amount_column,
            date_column: value.date_column,
            account_column: value.account_column,
            voucher_column: value.voucher_column,
            narration_column: value.narration_column,
            amount_scale: value.amount_scale,
            transaction_count: value.transaction_count,
            imported_at_ms: value.imported_at_ms,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LedgerTestRunDto {
    ledger_test_run_id: String,
    ledger_import_id: String,
    test_type: String,
    threshold_minor: i64,
    exception_count: u64,
    ran_at_ms: i64,
}

impl From<persistence::LedgerTestRunRecord> for LedgerTestRunDto {
    fn from(value: persistence::LedgerTestRunRecord) -> Self {
        Self {
            ledger_test_run_id: value.ledger_test_run_id,
            ledger_import_id: value.ledger_import_id,
            test_type: value.test_type,
            threshold_minor: value.threshold_minor,
            exception_count: value.exception_count,
            ran_at_ms: value.ran_at_ms,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LedgerExceptionDto {
    ledger_exception_id: String,
    ledger_test_run_id: String,
    ledger_transaction_id: String,
    exception_code: String,
    amount_minor: i64,
    transaction_date_text: Option<String>,
    account_text: Option<String>,
    voucher_text: Option<String>,
    narration_text: Option<String>,
    controlled_evidence_version_id: String,
    document_id: String,
    source_content_version_id: String,
    source_sha256_hex: String,
    sheet_name: String,
    source_row_number: u64,
    source_row_hash_hex: String,
    created_at_ms: i64,
}

impl From<persistence::LedgerExceptionRecord> for LedgerExceptionDto {
    fn from(value: persistence::LedgerExceptionRecord) -> Self {
        Self {
            ledger_exception_id: value.ledger_exception_id,
            ledger_test_run_id: value.ledger_test_run_id,
            ledger_transaction_id: value.ledger_transaction_id,
            exception_code: value.exception_code,
            amount_minor: value.amount_minor,
            transaction_date_text: value.transaction_date_text,
            account_text: value.account_text,
            voucher_text: value.voucher_text,
            narration_text: value.narration_text,
            controlled_evidence_version_id: value.controlled_evidence_version_id,
            document_id: value.document_id,
            source_content_version_id: value.source_content_version_id,
            source_sha256_hex: hex_bytes(&value.source_sha256),
            sheet_name: value.sheet_name,
            source_row_number: value.source_row_number,
            source_row_hash_hex: hex_bytes(&value.source_row_hash),
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
    document_name: String,
    content_version_id: Option<String>,
    content_observed_at_ms: Option<i64>,
    controlled_evidence_version_id: Option<String>,
    controlled_version_number: Option<u64>,
    controlled_captured_at_ms: Option<i64>,
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
            document_name: value.document_name,
            content_version_id: value.content_version_id,
            content_observed_at_ms: value.content_observed_at_ms,
            controlled_evidence_version_id: value.controlled_evidence_version_id,
            controlled_version_number: value.controlled_version_number,
            controlled_captured_at_ms: value.controlled_captured_at_ms,
            relationship_type: value.relationship_type,
            description: value.description,
            created_at_ms: value.created_at_ms,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkpaperSignoffInputDto {
    workpaper_revision_id: String,
    signoff_type: String,
    actor_id: String,
    actor_role: String,
    comment: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkpaperSignoffDto {
    signoff_id: String,
    workpaper_id: String,
    workpaper_revision_id: String,
    revision_number: u64,
    signoff_type: String,
    actor_id: String,
    actor_role: String,
    signed_at_ms: i64,
    comment: Option<String>,
    evidence_link_ids: Vec<String>,
    superseded_at_ms: Option<i64>,
    superseded_reason: Option<String>,
    superseded_by_revision_id: Option<String>,
}

impl From<persistence::WorkpaperSignoffRecord> for WorkpaperSignoffDto {
    fn from(value: persistence::WorkpaperSignoffRecord) -> Self {
        Self {
            signoff_id: value.signoff_id,
            workpaper_id: value.workpaper_id,
            workpaper_revision_id: value.workpaper_revision_id,
            revision_number: value.revision_number,
            signoff_type: value.signoff_type,
            actor_id: value.actor_id,
            actor_role: value.actor_role,
            signed_at_ms: value.signed_at_ms,
            comment: value.comment,
            evidence_link_ids: value.evidence_link_ids,
            superseded_at_ms: value.superseded_at_ms,
            superseded_reason: value.superseded_reason,
            superseded_by_revision_id: value.superseded_by_revision_id,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PbcRequestDto {
    pbc_request_id: String,
    engagement_id: String,
    engagement_area_id: Option<String>,
    request_number: String,
    description: String,
    requested_from_party: String,
    due_at_ms: Option<i64>,
    status: String,
    client_visible_content: Option<String>,
    internal_notes: Option<String>,
    latest_assessment: Option<String>,
    created_at_ms: i64,
    updated_at_ms: i64,
}

impl From<persistence::PbcRequestRecord> for PbcRequestDto {
    fn from(value: persistence::PbcRequestRecord) -> Self {
        Self {
            pbc_request_id: value.pbc_request_id,
            engagement_id: value.engagement_id,
            engagement_area_id: value.engagement_area_id,
            request_number: value.request_number,
            description: value.description,
            requested_from_party: value.requested_from_party,
            due_at_ms: value.due_at_ms,
            status: value.status,
            client_visible_content: value.client_visible_content,
            internal_notes: value.internal_notes,
            latest_assessment: value.latest_assessment,
            created_at_ms: value.created_at_ms,
            updated_at_ms: value.updated_at_ms,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PbcRequestEventDto {
    pbc_request_event_id: String,
    pbc_request_id: String,
    event_type: String,
    actor_id: Option<String>,
    from_status: Option<String>,
    to_status: Option<String>,
    assessment_text: Option<String>,
    comment: Option<String>,
    occurred_at_ms: i64,
}

impl From<persistence::PbcRequestEventRecord> for PbcRequestEventDto {
    fn from(value: persistence::PbcRequestEventRecord) -> Self {
        Self {
            pbc_request_event_id: value.pbc_request_event_id,
            pbc_request_id: value.pbc_request_id,
            event_type: value.event_type,
            actor_id: value.actor_id,
            from_status: value.from_status,
            to_status: value.to_status,
            assessment_text: value.assessment_text,
            comment: value.comment,
            occurred_at_ms: value.occurred_at_ms,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PbcRequestEvidenceLinkDto {
    pbc_request_evidence_link_id: String,
    pbc_request_id: String,
    document_id: String,
    document_name: String,
    content_version_id: Option<String>,
    content_observed_at_ms: Option<i64>,
    controlled_evidence_version_id: Option<String>,
    controlled_version_number: Option<u64>,
    controlled_captured_at_ms: Option<i64>,
    description: Option<String>,
    created_at_ms: i64,
}

impl From<persistence::PbcRequestEvidenceLinkRecord> for PbcRequestEvidenceLinkDto {
    fn from(value: persistence::PbcRequestEvidenceLinkRecord) -> Self {
        Self {
            pbc_request_evidence_link_id: value.pbc_request_evidence_link_id,
            pbc_request_id: value.pbc_request_id,
            document_id: value.document_id,
            document_name: value.document_name,
            content_version_id: value.content_version_id,
            content_observed_at_ms: value.content_observed_at_ms,
            controlled_evidence_version_id: value.controlled_evidence_version_id,
            controlled_version_number: value.controlled_version_number,
            controlled_captured_at_ms: value.controlled_captured_at_ms,
            description: value.description,
            created_at_ms: value.created_at_ms,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PbcRequestInputDto {
    engagement_area_id: Option<String>,
    request_number: String,
    description: String,
    requested_from_party: String,
    due_at_ms: Option<i64>,
    status: Option<String>,
    client_visible_content: Option<String>,
    internal_notes: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkpaperWorkflowEventDto {
    workpaper_workflow_event_id: String,
    workpaper_id: String,
    workpaper_revision_id: Option<String>,
    from_state: String,
    to_state: String,
    actor_id: Option<String>,
    comment: Option<String>,
    occurred_at_ms: i64,
}

impl From<persistence::WorkpaperWorkflowEventRecord> for WorkpaperWorkflowEventDto {
    fn from(value: persistence::WorkpaperWorkflowEventRecord) -> Self {
        Self {
            workpaper_workflow_event_id: value.workpaper_workflow_event_id,
            workpaper_id: value.workpaper_id,
            workpaper_revision_id: value.workpaper_revision_id,
            from_state: value.from_state,
            to_state: value.to_state,
            actor_id: value.actor_id,
            comment: value.comment,
            occurred_at_ms: value.occurred_at_ms,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReviewNoteDto {
    review_note_id: String,
    workpaper_id: String,
    workpaper_revision_id: String,
    evidence_link_id: Option<String>,
    title: String,
    body: String,
    owner_id: Option<String>,
    due_at_ms: Option<i64>,
    location_kind: Option<String>,
    location_value: Option<String>,
    current_state: String,
    raised_by: Option<String>,
    created_at_ms: i64,
    latest_event_at_ms: i64,
}

impl From<persistence::ReviewNoteRecord> for ReviewNoteDto {
    fn from(value: persistence::ReviewNoteRecord) -> Self {
        Self {
            review_note_id: value.review_note_id,
            workpaper_id: value.workpaper_id,
            workpaper_revision_id: value.workpaper_revision_id,
            evidence_link_id: value.evidence_link_id,
            title: value.title,
            body: value.body,
            owner_id: value.owner_id,
            due_at_ms: value.due_at_ms,
            location_kind: value.location_kind,
            location_value: value.location_value,
            current_state: value.current_state,
            raised_by: value.raised_by,
            created_at_ms: value.created_at_ms,
            latest_event_at_ms: value.latest_event_at_ms,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReviewNoteEventDto {
    review_note_event_id: String,
    review_note_id: String,
    event_type: String,
    actor_id: Option<String>,
    response_text: Option<String>,
    comment: Option<String>,
    occurred_at_ms: i64,
}

impl From<persistence::ReviewNoteEventRecord> for ReviewNoteEventDto {
    fn from(value: persistence::ReviewNoteEventRecord) -> Self {
        Self {
            review_note_event_id: value.review_note_event_id,
            review_note_id: value.review_note_id,
            event_type: value.event_type,
            actor_id: value.actor_id,
            response_text: value.response_text,
            comment: value.comment,
            occurred_at_ms: value.occurred_at_ms,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewNoteInputDto {
    workpaper_revision_id: String,
    evidence_link_id: Option<String>,
    title: String,
    body: String,
    owner_id: Option<String>,
    due_at_ms: Option<i64>,
    location_kind: Option<String>,
    location_value: Option<String>,
    raised_by: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewNoteActionInputDto {
    actor_id: Option<String>,
    response_text: Option<String>,
    comment: Option<String>,
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
fn create_engagement_template_from_engagement(
    source_engagement_id: String,
    name: String,
    description: Option<String>,
    database: State<'_, persistence::DatabaseState>,
) -> Result<EngagementTemplateDto, String> {
    validate_uuid(&source_engagement_id, "source-engagement")?;
    persistence::create_engagement_template_from_engagement(
        database.path(),
        &source_engagement_id,
        &name,
        description.as_deref(),
    )
    .map(Into::into)
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_engagement_templates(
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<EngagementTemplateDto>, String> {
    persistence::list_engagement_templates(database.path())
        .map(|records| records.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn create_engagement_template_version_from_engagement(
    engagement_template_id: String,
    source_engagement_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<EngagementTemplateDto, String> {
    validate_uuid(&engagement_template_id, "engagement-template")?;
    validate_uuid(&source_engagement_id, "source-engagement")?;
    persistence::create_engagement_template_version_from_engagement(
        database.path(),
        &engagement_template_id,
        &source_engagement_id,
    )
    .map(Into::into)
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn create_firm_library_item(
    category: String,
    name: String,
    description: Option<String>,
    service_type_id: Option<String>,
    definition_json: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<FirmLibraryItemDto, String> {
    validate_optional_uuid(service_type_id.as_deref(), "service-type")?;
    persistence::create_firm_library_item(
        database.path(),
        &category,
        &name,
        description.as_deref(),
        service_type_id.as_deref(),
        &definition_json,
    )
    .map(Into::into)
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn publish_firm_library_version(
    firm_library_item_id: String,
    definition_json: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<FirmLibraryItemDto, String> {
    validate_uuid(&firm_library_item_id, "firm-library-item")?;
    persistence::publish_firm_library_version(
        database.path(),
        &firm_library_item_id,
        &definition_json,
    )
    .map(Into::into)
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_firm_library_items(
    category: Option<String>,
    service_type_id: Option<String>,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<FirmLibraryItemDto>, String> {
    validate_optional_uuid(service_type_id.as_deref(), "service-type")?;
    persistence::list_firm_library_items(
        database.path(),
        category.as_deref(),
        service_type_id.as_deref(),
    )
    .map(|records| records.into_iter().map(Into::into).collect())
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_firm_library_versions(
    firm_library_item_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<FirmLibraryVersionDto>, String> {
    validate_uuid(&firm_library_item_id, "firm-library-item")?;
    persistence::list_firm_library_versions(database.path(), &firm_library_item_id)
        .map(|records| records.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn import_ledger_from_controlled_evidence(
    input: LedgerImportInputDto,
    database: State<'_, persistence::DatabaseState>,
    evidence_state: State<'_, evidence::EvidenceState>,
) -> Result<LedgerImportDto, String> {
    let LedgerImportInputDto {
        engagement_id,
        controlled_evidence_version_id,
        sheet_name,
        header_row_number,
        amount_scale,
        mapping,
    } = input;

    validate_uuid(&engagement_id, "engagement")?;
    validate_uuid(
        &controlled_evidence_version_id,
        "controlled-evidence-version",
    )?;

    let database_path = database.path().to_path_buf();
    let evidence_handle = evidence_state.inner().clone();

    tauri::async_runtime::spawn_blocking(move || {
        let controlled = evidence::read_controlled_evidence_bytes(
            &database_path,
            &evidence_handle,
            &controlled_evidence_version_id,
            ledger::MAX_LEDGER_IMPORT_BYTES,
        )
        .map_err(|error| error.to_string())?;

        let ledger_mapping = ledger::LedgerColumnMapping {
            amount_column: mapping.amount_column,
            date_column: mapping.date_column,
            account_column: mapping.account_column,
            voucher_column: mapping.voucher_column,
            narration_column: mapping.narration_column,
        };
        let parsed = ledger::parse_ledger_workbook(
            &controlled.bytes,
            &sheet_name,
            header_row_number,
            amount_scale,
            &ledger_mapping,
        )?;
        let transactions = parsed
            .into_iter()
            .map(|transaction| persistence::LedgerTransactionInput {
                source_row_number: transaction.source_row_number,
                source_row_hash: transaction.source_row_hash,
                transaction_date_text: transaction.transaction_date_text,
                account_text: transaction.account_text,
                voucher_text: transaction.voucher_text,
                narration_text: transaction.narration_text,
                amount_minor: transaction.amount_minor,
            })
            .collect::<Vec<_>>();

        persistence::create_ledger_import(
            &database_path,
            persistence::LedgerImportDefinition {
                engagement_id: &engagement_id,
                controlled_evidence_version_id: &controlled.record.controlled_evidence_version_id,
                sheet_name: &sheet_name,
                header_row_number: u64::from(header_row_number),
                amount_column: mapping.amount_column,
                date_column: mapping.date_column,
                account_column: mapping.account_column,
                voucher_column: mapping.voucher_column,
                narration_column: mapping.narration_column,
                amount_scale,
                transactions: &transactions,
            },
        )
        .map(Into::into)
        .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("Ledger import task failed to join: {error}"))?
}

#[tauri::command]
fn list_ledger_imports(
    engagement_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<LedgerImportDto>, String> {
    validate_uuid(&engagement_id, "engagement")?;
    persistence::list_ledger_imports(database.path(), &engagement_id)
        .map(|records| records.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn run_high_value_ledger_test(
    ledger_import_id: String,
    threshold_minor: i64,
    database: State<'_, persistence::DatabaseState>,
) -> Result<LedgerTestRunDto, String> {
    validate_uuid(&ledger_import_id, "ledger-import")?;
    persistence::run_high_value_ledger_test(database.path(), &ledger_import_id, threshold_minor)
        .map(Into::into)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_ledger_exceptions(
    ledger_test_run_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<LedgerExceptionDto>, String> {
    validate_uuid(&ledger_test_run_id, "ledger-test-run")?;
    persistence::list_ledger_exceptions(database.path(), &ledger_test_run_id)
        .map(|records| records.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn create_engagement_from_template(
    engagement_template_version_id: String,
    client_id: String,
    name: String,
    period_start: Option<String>,
    period_end: Option<String>,
    status: Option<String>,
    database: State<'_, persistence::DatabaseState>,
) -> Result<EngagementDto, String> {
    validate_uuid(
        &engagement_template_version_id,
        "engagement-template-version",
    )?;
    validate_uuid(&client_id, "client")?;
    persistence::create_engagement_from_template(
        database.path(),
        &engagement_template_version_id,
        &client_id,
        &name,
        period_start.as_deref(),
        period_end.as_deref(),
        status.as_deref().unwrap_or("ACTIVE"),
    )
    .map(Into::into)
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
fn create_workpaper_signoff(
    workpaper_id: String,
    signoff: WorkpaperSignoffInputDto,
    database: State<'_, persistence::DatabaseState>,
) -> Result<WorkpaperSignoffDto, String> {
    validate_uuid(&workpaper_id, "workpaper")?;
    validate_uuid(&signoff.workpaper_revision_id, "workpaper-revision")?;

    persistence::create_workpaper_signoff(
        database.path(),
        persistence::NewWorkpaperSignoff {
            workpaper_id: &workpaper_id,
            workpaper_revision_id: &signoff.workpaper_revision_id,
            signoff_type: &signoff.signoff_type,
            actor_id: &signoff.actor_id,
            actor_role: &signoff.actor_role,
            comment: signoff.comment.as_deref(),
        },
    )
    .map(Into::into)
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_workpaper_signoffs(
    workpaper_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<WorkpaperSignoffDto>, String> {
    validate_uuid(&workpaper_id, "workpaper")?;
    persistence::list_workpaper_signoffs(database.path(), &workpaper_id)
        .map(|records| records.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn create_pbc_request(
    engagement_id: String,
    request: PbcRequestInputDto,
    database: State<'_, persistence::DatabaseState>,
) -> Result<PbcRequestDto, String> {
    validate_uuid(&engagement_id, "engagement")?;
    validate_optional_uuid(request.engagement_area_id.as_deref(), "engagement-area")?;

    persistence::create_pbc_request(
        database.path(),
        persistence::NewPbcRequest {
            engagement_id: &engagement_id,
            engagement_area_id: request.engagement_area_id.as_deref(),
            request_number: &request.request_number,
            description: &request.description,
            requested_from_party: &request.requested_from_party,
            due_at_ms: request.due_at_ms,
            status: request.status.as_deref().unwrap_or("REQUESTED"),
            client_visible_content: request.client_visible_content.as_deref(),
            internal_notes: request.internal_notes.as_deref(),
        },
    )
    .map(Into::into)
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_pbc_requests(
    engagement_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<PbcRequestDto>, String> {
    validate_uuid(&engagement_id, "engagement")?;
    persistence::list_pbc_requests(database.path(), &engagement_id)
        .map(|records| records.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn transition_pbc_request_status(
    pbc_request_id: String,
    to_status: String,
    actor_id: Option<String>,
    comment: Option<String>,
    database: State<'_, persistence::DatabaseState>,
) -> Result<PbcRequestEventDto, String> {
    validate_uuid(&pbc_request_id, "PBC request")?;
    persistence::transition_pbc_request_status(
        database.path(),
        &pbc_request_id,
        &to_status,
        actor_id.as_deref(),
        comment.as_deref(),
    )
    .map(Into::into)
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn add_pbc_request_assessment(
    pbc_request_id: String,
    assessment_text: String,
    actor_id: Option<String>,
    database: State<'_, persistence::DatabaseState>,
) -> Result<PbcRequestEventDto, String> {
    validate_uuid(&pbc_request_id, "PBC request")?;
    persistence::add_pbc_request_assessment(
        database.path(),
        &pbc_request_id,
        &assessment_text,
        actor_id.as_deref(),
    )
    .map(Into::into)
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_pbc_request_events(
    pbc_request_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<PbcRequestEventDto>, String> {
    validate_uuid(&pbc_request_id, "PBC request")?;
    persistence::list_pbc_request_events(database.path(), &pbc_request_id)
        .map(|records| records.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn create_pbc_request_evidence_link(
    pbc_request_id: String,
    document_id: String,
    content_version_id: Option<String>,
    controlled_evidence_version_id: Option<String>,
    description: Option<String>,
    database: State<'_, persistence::DatabaseState>,
) -> Result<PbcRequestEvidenceLinkDto, String> {
    validate_uuid(&pbc_request_id, "PBC request")?;
    validate_uuid(&document_id, "document")?;
    validate_optional_uuid(content_version_id.as_deref(), "content-version")?;
    validate_optional_uuid(
        controlled_evidence_version_id.as_deref(),
        "controlled-evidence-version",
    )?;

    persistence::create_pbc_request_evidence_link(
        database.path(),
        &pbc_request_id,
        &document_id,
        content_version_id.as_deref(),
        controlled_evidence_version_id.as_deref(),
        description.as_deref(),
    )
    .map(Into::into)
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_pbc_request_evidence_links(
    pbc_request_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<PbcRequestEvidenceLinkDto>, String> {
    validate_uuid(&pbc_request_id, "PBC request")?;
    persistence::list_pbc_request_evidence_links(database.path(), &pbc_request_id)
        .map(|records| records.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn transition_workpaper_state(
    workpaper_id: String,
    to_state: String,
    actor_id: Option<String>,
    comment: Option<String>,
    database: State<'_, persistence::DatabaseState>,
) -> Result<WorkpaperWorkflowEventDto, String> {
    validate_uuid(&workpaper_id, "workpaper")?;
    persistence::transition_workpaper_state(
        database.path(),
        &workpaper_id,
        &to_state,
        actor_id.as_deref(),
        comment.as_deref(),
    )
    .map(Into::into)
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_workpaper_workflow_events(
    workpaper_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<WorkpaperWorkflowEventDto>, String> {
    validate_uuid(&workpaper_id, "workpaper")?;
    persistence::list_workpaper_workflow_events(database.path(), &workpaper_id)
        .map(|records| records.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn create_review_note(
    workpaper_id: String,
    note: ReviewNoteInputDto,
    database: State<'_, persistence::DatabaseState>,
) -> Result<ReviewNoteDto, String> {
    validate_uuid(&workpaper_id, "workpaper")?;
    validate_uuid(&note.workpaper_revision_id, "workpaper-revision")?;
    validate_optional_uuid(note.evidence_link_id.as_deref(), "evidence-link")?;

    persistence::create_review_note(
        database.path(),
        persistence::NewReviewNote {
            workpaper_id: &workpaper_id,
            workpaper_revision_id: &note.workpaper_revision_id,
            evidence_link_id: note.evidence_link_id.as_deref(),
            title: &note.title,
            body: &note.body,
            owner_id: note.owner_id.as_deref(),
            due_at_ms: note.due_at_ms,
            location_kind: note.location_kind.as_deref(),
            location_value: note.location_value.as_deref(),
            raised_by: note.raised_by.as_deref(),
        },
    )
    .map(Into::into)
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_review_notes(
    workpaper_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<ReviewNoteDto>, String> {
    validate_uuid(&workpaper_id, "workpaper")?;
    persistence::list_review_notes(database.path(), &workpaper_id)
        .map(|records| records.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
}

fn review_note_action<'a>(
    review_note_id: &'a str,
    action: &'a ReviewNoteActionInputDto,
) -> persistence::ReviewNoteAction<'a> {
    persistence::ReviewNoteAction {
        review_note_id,
        actor_id: action.actor_id.as_deref(),
        response_text: action.response_text.as_deref(),
        comment: action.comment.as_deref(),
    }
}

#[tauri::command]
fn respond_to_review_note(
    review_note_id: String,
    action: ReviewNoteActionInputDto,
    database: State<'_, persistence::DatabaseState>,
) -> Result<ReviewNoteEventDto, String> {
    validate_uuid(&review_note_id, "review-note")?;
    persistence::respond_to_review_note(
        database.path(),
        review_note_action(&review_note_id, &action),
    )
    .map(Into::into)
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn clear_review_note(
    review_note_id: String,
    action: ReviewNoteActionInputDto,
    database: State<'_, persistence::DatabaseState>,
) -> Result<ReviewNoteEventDto, String> {
    validate_uuid(&review_note_id, "review-note")?;
    persistence::clear_review_note(
        database.path(),
        review_note_action(&review_note_id, &action),
    )
    .map(Into::into)
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn reopen_review_note(
    review_note_id: String,
    action: ReviewNoteActionInputDto,
    database: State<'_, persistence::DatabaseState>,
) -> Result<ReviewNoteEventDto, String> {
    validate_uuid(&review_note_id, "review-note")?;
    persistence::reopen_review_note(
        database.path(),
        review_note_action(&review_note_id, &action),
    )
    .map(Into::into)
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_review_note_events(
    review_note_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<ReviewNoteEventDto>, String> {
    validate_uuid(&review_note_id, "review-note")?;
    persistence::list_review_note_events(database.path(), &review_note_id)
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
            create_engagement_template_from_engagement,
            list_engagement_templates,
            create_engagement_template_version_from_engagement,
            create_firm_library_item,
            publish_firm_library_version,
            list_firm_library_items,
            list_firm_library_versions,
            import_ledger_from_controlled_evidence,
            list_ledger_imports,
            run_high_value_ledger_test,
            list_ledger_exceptions,
            create_engagement_from_template,
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
            create_workpaper_signoff,
            list_workpaper_signoffs,
            create_pbc_request,
            list_pbc_requests,
            transition_pbc_request_status,
            add_pbc_request_assessment,
            list_pbc_request_events,
            create_pbc_request_evidence_link,
            list_pbc_request_evidence_links,
            transition_workpaper_state,
            list_workpaper_workflow_events,
            create_review_note,
            list_review_notes,
            respond_to_review_note,
            clear_review_note,
            reopen_review_note,
            list_review_note_events,
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
