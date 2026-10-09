mod evidence;
mod filesystem;
mod indexer;
mod launcher;
mod ledger;
mod normal_data;
pub mod normal_data_comparison;
mod normal_data_datasets;
pub mod normal_data_source_reader;
mod persistence;
mod preview;
mod reconciliation;
mod search;
mod spreadsheet;
mod trial_balance;
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StatutoryComplianceRequirementDto {
    statutory_compliance_requirement_id: String,
    engagement_id: String,
    firm_library_item_id: String,
    firm_library_version_id: String,
    requirement_name: String,
    requirement_description: Option<String>,
    version_number: u64,
    definition_json: String,
    definition_hash_hex: String,
    created_at_ms: i64,
}

impl From<persistence::StatutoryComplianceRequirementRecord> for StatutoryComplianceRequirementDto {
    fn from(value: persistence::StatutoryComplianceRequirementRecord) -> Self {
        Self {
            statutory_compliance_requirement_id: value.statutory_compliance_requirement_id,
            engagement_id: value.engagement_id,
            firm_library_item_id: value.firm_library_item_id,
            firm_library_version_id: value.firm_library_version_id,
            requirement_name: value.requirement_name,
            requirement_description: value.requirement_description,
            version_number: value.version_number,
            definition_json: value.definition_json,
            definition_hash_hex: hex_bytes(&value.definition_hash),
            created_at_ms: value.created_at_ms,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StatutoryComplianceAssessmentDto {
    statutory_compliance_assessment_id: String,
    statutory_compliance_requirement_id: String,
    version_number: u64,
    supersedes_assessment_id: Option<String>,
    applicability: String,
    due_date: Option<String>,
    actual_compliance_date: Option<String>,
    status: String,
    exception_text: Option<String>,
    conclusion: Option<String>,
    evidence_count: u64,
    assessed_at_ms: i64,
}

impl From<persistence::StatutoryComplianceAssessmentRecord> for StatutoryComplianceAssessmentDto {
    fn from(value: persistence::StatutoryComplianceAssessmentRecord) -> Self {
        Self {
            statutory_compliance_assessment_id: value.statutory_compliance_assessment_id,
            statutory_compliance_requirement_id: value.statutory_compliance_requirement_id,
            version_number: value.version_number,
            supersedes_assessment_id: value.supersedes_assessment_id,
            applicability: value.applicability,
            due_date: value.due_date,
            actual_compliance_date: value.actual_compliance_date,
            status: value.status,
            exception_text: value.exception_text,
            conclusion: value.conclusion,
            evidence_count: value.evidence_count,
            assessed_at_ms: value.assessed_at_ms,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StatutoryComplianceEvidenceDto {
    statutory_compliance_evidence_link_id: String,
    statutory_compliance_assessment_id: String,
    controlled_evidence_version_id: String,
    document_id: String,
    source_content_version_id: String,
    source_sha256_hex: String,
    linked_at_ms: i64,
}

impl From<persistence::StatutoryComplianceEvidenceRecord> for StatutoryComplianceEvidenceDto {
    fn from(value: persistence::StatutoryComplianceEvidenceRecord) -> Self {
        Self {
            statutory_compliance_evidence_link_id: value.statutory_compliance_evidence_link_id,
            statutory_compliance_assessment_id: value.statutory_compliance_assessment_id,
            controlled_evidence_version_id: value.controlled_evidence_version_id,
            document_id: value.document_id,
            source_content_version_id: value.source_content_version_id,
            source_sha256_hex: hex_bytes(&value.source_sha256),
            linked_at_ms: value.linked_at_ms,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct InternalAuditProcessDto {
    internal_audit_process_id: String,
    engagement_id: String,
    parent_process_id: Option<String>,
    code: Option<String>,
    name: String,
    description: Option<String>,
    display_order: i64,
    status: String,
    created_at_ms: i64,
}

impl From<persistence::InternalAuditProcessRecord> for InternalAuditProcessDto {
    fn from(value: persistence::InternalAuditProcessRecord) -> Self {
        Self {
            internal_audit_process_id: value.internal_audit_process_id,
            engagement_id: value.engagement_id,
            parent_process_id: value.parent_process_id,
            code: value.code,
            name: value.name,
            description: value.description,
            display_order: value.display_order,
            status: value.status,
            created_at_ms: value.created_at_ms,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct InternalAuditObjectiveDto {
    internal_audit_objective_id: String,
    internal_audit_process_id: String,
    reference: Option<String>,
    title: String,
    description: Option<String>,
    status: String,
    created_at_ms: i64,
}

impl From<persistence::InternalAuditObjectiveRecord> for InternalAuditObjectiveDto {
    fn from(value: persistence::InternalAuditObjectiveRecord) -> Self {
        Self {
            internal_audit_objective_id: value.internal_audit_objective_id,
            internal_audit_process_id: value.internal_audit_process_id,
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
struct InternalAuditRiskDto {
    internal_audit_risk_id: String,
    internal_audit_objective_id: String,
    reference: Option<String>,
    title: String,
    description: Option<String>,
    risk_classification: Option<String>,
    inherent_rating: Option<String>,
    status: String,
    created_at_ms: i64,
}

impl From<persistence::InternalAuditRiskRecord> for InternalAuditRiskDto {
    fn from(value: persistence::InternalAuditRiskRecord) -> Self {
        Self {
            internal_audit_risk_id: value.internal_audit_risk_id,
            internal_audit_objective_id: value.internal_audit_objective_id,
            reference: value.reference,
            title: value.title,
            description: value.description,
            risk_classification: value.risk_classification,
            inherent_rating: value.inherent_rating,
            status: value.status,
            created_at_ms: value.created_at_ms,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct InternalAuditControlDto {
    internal_audit_control_id: String,
    internal_audit_risk_id: String,
    reference: Option<String>,
    title: String,
    description: Option<String>,
    control_type: Option<String>,
    frequency: Option<String>,
    owner_text: Option<String>,
    status: String,
    created_at_ms: i64,
}

impl From<persistence::InternalAuditControlRecord> for InternalAuditControlDto {
    fn from(value: persistence::InternalAuditControlRecord) -> Self {
        Self {
            internal_audit_control_id: value.internal_audit_control_id,
            internal_audit_risk_id: value.internal_audit_risk_id,
            reference: value.reference,
            title: value.title,
            description: value.description,
            control_type: value.control_type,
            frequency: value.frequency,
            owner_text: value.owner_text,
            status: value.status,
            created_at_ms: value.created_at_ms,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct InternalAuditTestDto {
    internal_audit_test_id: String,
    internal_audit_control_id: String,
    reference: Option<String>,
    title: String,
    procedure_text: String,
    sample_strategy: Option<String>,
    expected_result: Option<String>,
    status: String,
    created_at_ms: i64,
}

impl From<persistence::InternalAuditTestRecord> for InternalAuditTestDto {
    fn from(value: persistence::InternalAuditTestRecord) -> Self {
        Self {
            internal_audit_test_id: value.internal_audit_test_id,
            internal_audit_control_id: value.internal_audit_control_id,
            reference: value.reference,
            title: value.title,
            procedure_text: value.procedure_text,
            sample_strategy: value.sample_strategy,
            expected_result: value.expected_result,
            status: value.status,
            created_at_ms: value.created_at_ms,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct InternalAuditTestEvidenceDto {
    internal_audit_test_evidence_link_id: String,
    internal_audit_test_id: String,
    controlled_evidence_version_id: String,
    document_id: String,
    source_content_version_id: String,
    source_sha256_hex: String,
    description: Option<String>,
    linked_at_ms: i64,
}

impl From<persistence::InternalAuditTestEvidenceRecord> for InternalAuditTestEvidenceDto {
    fn from(value: persistence::InternalAuditTestEvidenceRecord) -> Self {
        Self {
            internal_audit_test_evidence_link_id: value.internal_audit_test_evidence_link_id,
            internal_audit_test_id: value.internal_audit_test_id,
            controlled_evidence_version_id: value.controlled_evidence_version_id,
            document_id: value.document_id,
            source_content_version_id: value.source_content_version_id,
            source_sha256_hex: hex_bytes(&value.source_sha256),
            description: value.description,
            linked_at_ms: value.linked_at_ms,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct InternalAuditFindingDto {
    internal_audit_finding_id: String,
    origin_engagement_id: String,
    origin_engagement_name: String,
    internal_audit_process_id: String,
    process_name: String,
    internal_audit_risk_id: Option<String>,
    internal_audit_control_id: Option<String>,
    internal_audit_test_id: Option<String>,
    workpaper_id: Option<String>,
    repeated_from_finding_id: Option<String>,
    reference: Option<String>,
    title: String,
    condition_text: String,
    criteria_text: Option<String>,
    cause_text: Option<String>,
    risk_effect_text: Option<String>,
    recommendation_text: Option<String>,
    risk_classification: Option<String>,
    created_at_ms: i64,
    latest_followup_id: String,
    latest_tracking_engagement_id: String,
    latest_tracking_engagement_name: String,
    latest_sequence_number: u64,
    latest_status: String,
    latest_management_response: Option<String>,
    latest_action_owner: Option<String>,
    latest_target_date: Option<String>,
    latest_follow_up_text: Option<String>,
    latest_verification_conclusion: Option<String>,
    latest_actor_id: Option<String>,
    latest_occurred_at_ms: i64,
}

impl From<persistence::InternalAuditFindingRecord> for InternalAuditFindingDto {
    fn from(value: persistence::InternalAuditFindingRecord) -> Self {
        Self {
            internal_audit_finding_id: value.internal_audit_finding_id,
            origin_engagement_id: value.origin_engagement_id,
            origin_engagement_name: value.origin_engagement_name,
            internal_audit_process_id: value.internal_audit_process_id,
            process_name: value.process_name,
            internal_audit_risk_id: value.internal_audit_risk_id,
            internal_audit_control_id: value.internal_audit_control_id,
            internal_audit_test_id: value.internal_audit_test_id,
            workpaper_id: value.workpaper_id,
            repeated_from_finding_id: value.repeated_from_finding_id,
            reference: value.reference,
            title: value.title,
            condition_text: value.condition_text,
            criteria_text: value.criteria_text,
            cause_text: value.cause_text,
            risk_effect_text: value.risk_effect_text,
            recommendation_text: value.recommendation_text,
            risk_classification: value.risk_classification,
            created_at_ms: value.created_at_ms,
            latest_followup_id: value.latest_followup_id,
            latest_tracking_engagement_id: value.latest_tracking_engagement_id,
            latest_tracking_engagement_name: value.latest_tracking_engagement_name,
            latest_sequence_number: value.latest_sequence_number,
            latest_status: value.latest_status,
            latest_management_response: value.latest_management_response,
            latest_action_owner: value.latest_action_owner,
            latest_target_date: value.latest_target_date,
            latest_follow_up_text: value.latest_follow_up_text,
            latest_verification_conclusion: value.latest_verification_conclusion,
            latest_actor_id: value.latest_actor_id,
            latest_occurred_at_ms: value.latest_occurred_at_ms,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct InternalAuditFindingFollowupDto {
    internal_audit_finding_followup_id: String,
    internal_audit_finding_id: String,
    tracking_engagement_id: String,
    tracking_engagement_name: String,
    sequence_number: u64,
    status: String,
    management_response: Option<String>,
    action_owner: Option<String>,
    target_date: Option<String>,
    follow_up_text: Option<String>,
    verification_conclusion: Option<String>,
    actor_id: Option<String>,
    occurred_at_ms: i64,
}

impl From<persistence::InternalAuditFindingFollowupRecord> for InternalAuditFindingFollowupDto {
    fn from(value: persistence::InternalAuditFindingFollowupRecord) -> Self {
        Self {
            internal_audit_finding_followup_id: value.internal_audit_finding_followup_id,
            internal_audit_finding_id: value.internal_audit_finding_id,
            tracking_engagement_id: value.tracking_engagement_id,
            tracking_engagement_name: value.tracking_engagement_name,
            sequence_number: value.sequence_number,
            status: value.status,
            management_response: value.management_response,
            action_owner: value.action_owner,
            target_date: value.target_date,
            follow_up_text: value.follow_up_text,
            verification_conclusion: value.verification_conclusion,
            actor_id: value.actor_id,
            occurred_at_ms: value.occurred_at_ms,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct InternalAuditFindingEvidenceDto {
    internal_audit_finding_evidence_link_id: String,
    internal_audit_finding_id: String,
    internal_audit_finding_followup_id: Option<String>,
    controlled_evidence_version_id: String,
    document_id: String,
    source_content_version_id: String,
    source_sha256_hex: String,
    description: Option<String>,
    linked_at_ms: i64,
}

impl From<persistence::InternalAuditFindingEvidenceRecord> for InternalAuditFindingEvidenceDto {
    fn from(value: persistence::InternalAuditFindingEvidenceRecord) -> Self {
        Self {
            internal_audit_finding_evidence_link_id: value.internal_audit_finding_evidence_link_id,
            internal_audit_finding_id: value.internal_audit_finding_id,
            internal_audit_finding_followup_id: value.internal_audit_finding_followup_id,
            controlled_evidence_version_id: value.controlled_evidence_version_id,
            document_id: value.document_id,
            source_content_version_id: value.source_content_version_id,
            source_sha256_hex: hex_bytes(&value.source_sha256),
            description: value.description,
            linked_at_ms: value.linked_at_ms,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DueDiligenceWorkspaceDto {
    due_diligence_workspace_id: String,
    engagement_id: String,
    engagement_name: String,
    name: String,
    created_at_ms: i64,
}

impl From<persistence::DueDiligenceWorkspaceRecord> for DueDiligenceWorkspaceDto {
    fn from(value: persistence::DueDiligenceWorkspaceRecord) -> Self {
        Self {
            due_diligence_workspace_id: value.due_diligence_workspace_id,
            engagement_id: value.engagement_id,
            engagement_name: value.engagement_name,
            name: value.name,
            created_at_ms: value.created_at_ms,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DueDiligenceSectionDto {
    due_diligence_section_id: String,
    due_diligence_workspace_id: String,
    parent_section_id: Option<String>,
    code: Option<String>,
    name: String,
    description: Option<String>,
    display_order: i64,
    created_at_ms: i64,
}

impl From<persistence::DueDiligenceSectionRecord> for DueDiligenceSectionDto {
    fn from(value: persistence::DueDiligenceSectionRecord) -> Self {
        Self {
            due_diligence_section_id: value.due_diligence_section_id,
            due_diligence_workspace_id: value.due_diligence_workspace_id,
            parent_section_id: value.parent_section_id,
            code: value.code,
            name: value.name,
            description: value.description,
            display_order: value.display_order,
            created_at_ms: value.created_at_ms,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DueDiligenceSectionInputDto {
    due_diligence_workspace_id: String,
    parent_section_id: Option<String>,
    code: Option<String>,
    name: String,
    description: Option<String>,
    display_order: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DueDiligenceRequestDto {
    due_diligence_request_id: String,
    due_diligence_workspace_id: String,
    due_diligence_section_id: Option<String>,
    section_name: Option<String>,
    reference: Option<String>,
    title: String,
    description: Option<String>,
    requested_from_party: Option<String>,
    due_date: Option<String>,
    internal_notes: Option<String>,
    created_at_ms: i64,
    latest_event_id: String,
    latest_sequence_number: u64,
    latest_status: String,
    latest_response_text: Option<String>,
    latest_internal_assessment: Option<String>,
    latest_actor_id: Option<String>,
    latest_occurred_at_ms: i64,
}

impl From<persistence::DueDiligenceRequestRecord> for DueDiligenceRequestDto {
    fn from(value: persistence::DueDiligenceRequestRecord) -> Self {
        Self {
            due_diligence_request_id: value.due_diligence_request_id,
            due_diligence_workspace_id: value.due_diligence_workspace_id,
            due_diligence_section_id: value.due_diligence_section_id,
            section_name: value.section_name,
            reference: value.reference,
            title: value.title,
            description: value.description,
            requested_from_party: value.requested_from_party,
            due_date: value.due_date,
            internal_notes: value.internal_notes,
            created_at_ms: value.created_at_ms,
            latest_event_id: value.latest_event_id,
            latest_sequence_number: value.latest_sequence_number,
            latest_status: value.latest_status,
            latest_response_text: value.latest_response_text,
            latest_internal_assessment: value.latest_internal_assessment,
            latest_actor_id: value.latest_actor_id,
            latest_occurred_at_ms: value.latest_occurred_at_ms,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DueDiligenceRequestInputDto {
    due_diligence_workspace_id: String,
    due_diligence_section_id: Option<String>,
    reference: Option<String>,
    title: String,
    description: Option<String>,
    requested_from_party: Option<String>,
    due_date: Option<String>,
    internal_notes: Option<String>,
    actor_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DueDiligenceRequestEventDto {
    due_diligence_request_event_id: String,
    due_diligence_request_id: String,
    sequence_number: u64,
    status: String,
    response_text: Option<String>,
    internal_assessment: Option<String>,
    actor_id: Option<String>,
    occurred_at_ms: i64,
}

impl From<persistence::DueDiligenceRequestEventRecord> for DueDiligenceRequestEventDto {
    fn from(value: persistence::DueDiligenceRequestEventRecord) -> Self {
        Self {
            due_diligence_request_event_id: value.due_diligence_request_event_id,
            due_diligence_request_id: value.due_diligence_request_id,
            sequence_number: value.sequence_number,
            status: value.status,
            response_text: value.response_text,
            internal_assessment: value.internal_assessment,
            actor_id: value.actor_id,
            occurred_at_ms: value.occurred_at_ms,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DueDiligenceRequestEventInputDto {
    due_diligence_request_id: String,
    status: String,
    response_text: Option<String>,
    internal_assessment: Option<String>,
    actor_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DueDiligenceRequestEvidenceDto {
    due_diligence_request_evidence_link_id: String,
    due_diligence_request_id: String,
    due_diligence_request_event_id: Option<String>,
    controlled_evidence_version_id: String,
    document_id: String,
    source_content_version_id: String,
    source_sha256_hex: String,
    description: Option<String>,
    linked_at_ms: i64,
}

impl From<persistence::DueDiligenceRequestEvidenceRecord> for DueDiligenceRequestEvidenceDto {
    fn from(value: persistence::DueDiligenceRequestEvidenceRecord) -> Self {
        Self {
            due_diligence_request_evidence_link_id: value.due_diligence_request_evidence_link_id,
            due_diligence_request_id: value.due_diligence_request_id,
            due_diligence_request_event_id: value.due_diligence_request_event_id,
            controlled_evidence_version_id: value.controlled_evidence_version_id,
            document_id: value.document_id,
            source_content_version_id: value.source_content_version_id,
            source_sha256_hex: hex_bytes(&value.source_sha256),
            description: value.description,
            linked_at_ms: value.linked_at_ms,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DueDiligenceIssueDto {
    due_diligence_issue_id: String,
    due_diligence_workspace_id: String,
    due_diligence_section_id: Option<String>,
    section_name: Option<String>,
    due_diligence_request_id: Option<String>,
    request_title: Option<String>,
    issue_type: String,
    reference: Option<String>,
    title: String,
    description: Option<String>,
    category: Option<String>,
    severity: Option<String>,
    created_at_ms: i64,
    latest_event_id: String,
    latest_sequence_number: u64,
    latest_status: String,
    latest_internal_conclusion: Option<String>,
    latest_deal_impact: Option<String>,
    latest_recommendation: Option<String>,
    latest_actor_id: Option<String>,
    latest_occurred_at_ms: i64,
}

impl From<persistence::DueDiligenceIssueRecord> for DueDiligenceIssueDto {
    fn from(value: persistence::DueDiligenceIssueRecord) -> Self {
        Self {
            due_diligence_issue_id: value.due_diligence_issue_id,
            due_diligence_workspace_id: value.due_diligence_workspace_id,
            due_diligence_section_id: value.due_diligence_section_id,
            section_name: value.section_name,
            due_diligence_request_id: value.due_diligence_request_id,
            request_title: value.request_title,
            issue_type: value.issue_type,
            reference: value.reference,
            title: value.title,
            description: value.description,
            category: value.category,
            severity: value.severity,
            created_at_ms: value.created_at_ms,
            latest_event_id: value.latest_event_id,
            latest_sequence_number: value.latest_sequence_number,
            latest_status: value.latest_status,
            latest_internal_conclusion: value.latest_internal_conclusion,
            latest_deal_impact: value.latest_deal_impact,
            latest_recommendation: value.latest_recommendation,
            latest_actor_id: value.latest_actor_id,
            latest_occurred_at_ms: value.latest_occurred_at_ms,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DueDiligenceIssueInputDto {
    due_diligence_workspace_id: String,
    due_diligence_section_id: Option<String>,
    due_diligence_request_id: Option<String>,
    issue_type: String,
    reference: Option<String>,
    title: String,
    description: Option<String>,
    category: Option<String>,
    severity: Option<String>,
    actor_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DueDiligenceIssueEventDto {
    due_diligence_issue_event_id: String,
    due_diligence_issue_id: String,
    sequence_number: u64,
    status: String,
    internal_conclusion: Option<String>,
    deal_impact: Option<String>,
    recommendation: Option<String>,
    actor_id: Option<String>,
    occurred_at_ms: i64,
}

impl From<persistence::DueDiligenceIssueEventRecord> for DueDiligenceIssueEventDto {
    fn from(value: persistence::DueDiligenceIssueEventRecord) -> Self {
        Self {
            due_diligence_issue_event_id: value.due_diligence_issue_event_id,
            due_diligence_issue_id: value.due_diligence_issue_id,
            sequence_number: value.sequence_number,
            status: value.status,
            internal_conclusion: value.internal_conclusion,
            deal_impact: value.deal_impact,
            recommendation: value.recommendation,
            actor_id: value.actor_id,
            occurred_at_ms: value.occurred_at_ms,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DueDiligenceIssueEventInputDto {
    due_diligence_issue_id: String,
    status: String,
    internal_conclusion: Option<String>,
    deal_impact: Option<String>,
    recommendation: Option<String>,
    actor_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DueDiligenceIssueEvidenceDto {
    due_diligence_issue_evidence_link_id: String,
    due_diligence_issue_id: String,
    due_diligence_issue_event_id: Option<String>,
    controlled_evidence_version_id: String,
    document_id: String,
    source_content_version_id: String,
    source_sha256_hex: String,
    description: Option<String>,
    linked_at_ms: i64,
}

impl From<persistence::DueDiligenceIssueEvidenceRecord> for DueDiligenceIssueEvidenceDto {
    fn from(value: persistence::DueDiligenceIssueEvidenceRecord) -> Self {
        Self {
            due_diligence_issue_evidence_link_id: value.due_diligence_issue_evidence_link_id,
            due_diligence_issue_id: value.due_diligence_issue_id,
            due_diligence_issue_event_id: value.due_diligence_issue_event_id,
            controlled_evidence_version_id: value.controlled_evidence_version_id,
            document_id: value.document_id,
            source_content_version_id: value.source_content_version_id,
            source_sha256_hex: hex_bytes(&value.source_sha256),
            description: value.description,
            linked_at_ms: value.linked_at_ms,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DueDiligenceReportVersionInputDto {
    title: String,
    executive_summary: Option<String>,
    scope_summary: Option<String>,
    overall_conclusion: Option<String>,
    issue_ids: Vec<String>,
    created_by: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DueDiligenceReportDto {
    due_diligence_report_id: String,
    due_diligence_workspace_id: String,
    latest_version_id: String,
    latest_version_number: u64,
    latest_title: String,
    latest_executive_summary: Option<String>,
    latest_scope_summary: Option<String>,
    latest_overall_conclusion: Option<String>,
    latest_issue_count: u64,
    latest_issue_snapshot_hash_hex: String,
    latest_created_by: Option<String>,
    latest_created_at_ms: i64,
    created_at_ms: i64,
}

impl From<persistence::DueDiligenceReportRecord> for DueDiligenceReportDto {
    fn from(value: persistence::DueDiligenceReportRecord) -> Self {
        Self {
            due_diligence_report_id: value.due_diligence_report_id,
            due_diligence_workspace_id: value.due_diligence_workspace_id,
            latest_version_id: value.latest_version_id,
            latest_version_number: value.latest_version_number,
            latest_title: value.latest_title,
            latest_executive_summary: value.latest_executive_summary,
            latest_scope_summary: value.latest_scope_summary,
            latest_overall_conclusion: value.latest_overall_conclusion,
            latest_issue_count: value.latest_issue_count,
            latest_issue_snapshot_hash_hex: hex_bytes(&value.latest_issue_snapshot_hash),
            latest_created_by: value.latest_created_by,
            latest_created_at_ms: value.latest_created_at_ms,
            created_at_ms: value.created_at_ms,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DueDiligenceReportVersionDto {
    due_diligence_report_version_id: String,
    due_diligence_report_id: String,
    version_number: u64,
    title: String,
    executive_summary: Option<String>,
    scope_summary: Option<String>,
    overall_conclusion: Option<String>,
    issue_count: u64,
    issue_snapshot_hash_hex: String,
    created_by: Option<String>,
    created_at_ms: i64,
}

impl From<persistence::DueDiligenceReportVersionRecord> for DueDiligenceReportVersionDto {
    fn from(value: persistence::DueDiligenceReportVersionRecord) -> Self {
        Self {
            due_diligence_report_version_id: value.due_diligence_report_version_id,
            due_diligence_report_id: value.due_diligence_report_id,
            version_number: value.version_number,
            title: value.title,
            executive_summary: value.executive_summary,
            scope_summary: value.scope_summary,
            overall_conclusion: value.overall_conclusion,
            issue_count: value.issue_count,
            issue_snapshot_hash_hex: hex_bytes(&value.issue_snapshot_hash),
            created_by: value.created_by,
            created_at_ms: value.created_at_ms,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DueDiligenceReportIssueDto {
    due_diligence_report_issue_link_id: String,
    due_diligence_report_version_id: String,
    due_diligence_issue_id: String,
    due_diligence_issue_event_id: String,
    issue_type: String,
    reference: Option<String>,
    title: String,
    category: Option<String>,
    severity: Option<String>,
    sequence_number: u64,
    status: String,
    internal_conclusion: Option<String>,
    deal_impact: Option<String>,
    recommendation: Option<String>,
    linked_at_ms: i64,
}

impl From<persistence::DueDiligenceReportIssueRecord> for DueDiligenceReportIssueDto {
    fn from(value: persistence::DueDiligenceReportIssueRecord) -> Self {
        Self {
            due_diligence_report_issue_link_id: value.due_diligence_report_issue_link_id,
            due_diligence_report_version_id: value.due_diligence_report_version_id,
            due_diligence_issue_id: value.due_diligence_issue_id,
            due_diligence_issue_event_id: value.due_diligence_issue_event_id,
            issue_type: value.issue_type,
            reference: value.reference,
            title: value.title,
            category: value.category,
            severity: value.severity,
            sequence_number: value.sequence_number,
            status: value.status,
            internal_conclusion: value.internal_conclusion,
            deal_impact: value.deal_impact,
            recommendation: value.recommendation,
            linked_at_ms: value.linked_at_ms,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InternalAuditFindingInputDto {
    origin_engagement_id: String,
    internal_audit_process_id: String,
    internal_audit_risk_id: Option<String>,
    internal_audit_control_id: Option<String>,
    internal_audit_test_id: Option<String>,
    workpaper_id: Option<String>,
    repeated_from_finding_id: Option<String>,
    reference: Option<String>,
    title: String,
    condition_text: String,
    criteria_text: Option<String>,
    cause_text: Option<String>,
    risk_effect_text: Option<String>,
    recommendation_text: Option<String>,
    risk_classification: Option<String>,
    actor_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InternalAuditFindingFollowupInputDto {
    internal_audit_finding_id: String,
    tracking_engagement_id: String,
    status: String,
    management_response: Option<String>,
    action_owner: Option<String>,
    target_date: Option<String>,
    follow_up_text: Option<String>,
    verification_conclusion: Option<String>,
    actor_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InternalAuditProcessInputDto {
    engagement_id: String,
    parent_process_id: Option<String>,
    code: Option<String>,
    name: String,
    description: Option<String>,
    display_order: i64,
    status: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InternalAuditRiskInputDto {
    internal_audit_objective_id: String,
    reference: Option<String>,
    title: String,
    description: Option<String>,
    risk_classification: Option<String>,
    inherent_rating: Option<String>,
    status: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InternalAuditControlInputDto {
    internal_audit_risk_id: String,
    reference: Option<String>,
    title: String,
    description: Option<String>,
    control_type: Option<String>,
    frequency: Option<String>,
    owner_text: Option<String>,
    status: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InternalAuditTestInputDto {
    internal_audit_control_id: String,
    reference: Option<String>,
    title: String,
    procedure_text: String,
    sample_strategy: Option<String>,
    expected_result: Option<String>,
    status: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StatutoryComplianceAssessmentInputDto {
    statutory_compliance_requirement_id: String,
    applicability: String,
    due_date: Option<String>,
    actual_compliance_date: Option<String>,
    status: String,
    exception_text: Option<String>,
    conclusion: Option<String>,
    controlled_evidence_version_ids: Vec<String>,
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
struct LedgerAccountSummaryDto {
    ledger_import_id: String,
    account_key: String,
    account_text: String,
    transaction_count: u64,
    total_minor: i64,
    first_source_row_number: u64,
    last_source_row_number: u64,
}

impl From<persistence::LedgerAccountSummaryRecord> for LedgerAccountSummaryDto {
    fn from(value: persistence::LedgerAccountSummaryRecord) -> Self {
        Self {
            ledger_import_id: value.ledger_import_id,
            account_key: value.account_key,
            account_text: value.account_text,
            transaction_count: value.transaction_count,
            total_minor: value.total_minor,
            first_source_row_number: value.first_source_row_number,
            last_source_row_number: value.last_source_row_number,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LedgerTbMappingDto {
    ledger_tb_mapping_id: String,
    ledger_import_id: String,
    trial_balance_import_id: String,
    ledger_account_key: String,
    ledger_account_text: String,
    trial_balance_account_id: String,
    trial_balance_account_code_text: Option<String>,
    trial_balance_account_name_text: String,
    trial_balance_source_row_number: u64,
    trial_balance_source_row_hash_hex: String,
    version_number: u64,
    supersedes_mapping_id: Option<String>,
    mapped_at_ms: i64,
}

impl From<persistence::LedgerTbMappingRecord> for LedgerTbMappingDto {
    fn from(value: persistence::LedgerTbMappingRecord) -> Self {
        Self {
            ledger_tb_mapping_id: value.ledger_tb_mapping_id,
            ledger_import_id: value.ledger_import_id,
            trial_balance_import_id: value.trial_balance_import_id,
            ledger_account_key: value.ledger_account_key,
            ledger_account_text: value.ledger_account_text,
            trial_balance_account_id: value.trial_balance_account_id,
            trial_balance_account_code_text: value.trial_balance_account_code_text,
            trial_balance_account_name_text: value.trial_balance_account_name_text,
            trial_balance_source_row_number: value.trial_balance_source_row_number,
            trial_balance_source_row_hash_hex: hex_bytes(&value.trial_balance_source_row_hash),
            version_number: value.version_number,
            supersedes_mapping_id: value.supersedes_mapping_id,
            mapped_at_ms: value.mapped_at_ms,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FinancialStatementScheduleDto {
    financial_statement_schedule_id: String,
    engagement_id: String,
    reference: String,
    name: String,
    created_at_ms: i64,
}

impl From<persistence::FinancialStatementScheduleRecord> for FinancialStatementScheduleDto {
    fn from(value: persistence::FinancialStatementScheduleRecord) -> Self {
        Self {
            financial_statement_schedule_id: value.financial_statement_schedule_id,
            engagement_id: value.engagement_id,
            reference: value.reference,
            name: value.name,
            created_at_ms: value.created_at_ms,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TrialBalanceScheduleMappingDto {
    trial_balance_schedule_mapping_id: String,
    trial_balance_import_id: String,
    trial_balance_account_id: String,
    trial_balance_account_code_text: Option<String>,
    trial_balance_account_name_text: String,
    trial_balance_source_row_number: u64,
    trial_balance_source_row_hash_hex: String,
    financial_statement_schedule_id: String,
    schedule_reference: String,
    schedule_name: String,
    version_number: u64,
    supersedes_mapping_id: Option<String>,
    mapped_at_ms: i64,
}

impl From<persistence::TrialBalanceScheduleMappingRecord> for TrialBalanceScheduleMappingDto {
    fn from(value: persistence::TrialBalanceScheduleMappingRecord) -> Self {
        Self {
            trial_balance_schedule_mapping_id: value.trial_balance_schedule_mapping_id,
            trial_balance_import_id: value.trial_balance_import_id,
            trial_balance_account_id: value.trial_balance_account_id,
            trial_balance_account_code_text: value.trial_balance_account_code_text,
            trial_balance_account_name_text: value.trial_balance_account_name_text,
            trial_balance_source_row_number: value.trial_balance_source_row_number,
            trial_balance_source_row_hash_hex: hex_bytes(&value.trial_balance_source_row_hash),
            financial_statement_schedule_id: value.financial_statement_schedule_id,
            schedule_reference: value.schedule_reference,
            schedule_name: value.schedule_name,
            version_number: value.version_number,
            supersedes_mapping_id: value.supersedes_mapping_id,
            mapped_at_ms: value.mapped_at_ms,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FinancialStatementScheduleLinkDto {
    financial_statement_schedule_link_id: String,
    financial_statement_schedule_id: String,
    schedule_reference: String,
    schedule_name: String,
    controlled_evidence_version_id: String,
    document_id: String,
    source_content_version_id: String,
    source_sha256_hex: String,
    location_kind: String,
    location_value: String,
    version_number: u64,
    supersedes_link_id: Option<String>,
    linked_at_ms: i64,
}

impl From<persistence::FinancialStatementScheduleLinkRecord> for FinancialStatementScheduleLinkDto {
    fn from(value: persistence::FinancialStatementScheduleLinkRecord) -> Self {
        Self {
            financial_statement_schedule_link_id: value.financial_statement_schedule_link_id,
            financial_statement_schedule_id: value.financial_statement_schedule_id,
            schedule_reference: value.schedule_reference,
            schedule_name: value.schedule_name,
            controlled_evidence_version_id: value.controlled_evidence_version_id,
            document_id: value.document_id,
            source_content_version_id: value.source_content_version_id,
            source_sha256_hex: hex_bytes(&value.source_sha256),
            location_kind: value.location_kind,
            location_value: value.location_value,
            version_number: value.version_number,
            supersedes_link_id: value.supersedes_link_id,
            linked_at_ms: value.linked_at_ms,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrialBalanceColumnMappingInputDto {
    account_name_column: u32,
    account_code_column: Option<u32>,
    opening_balance_column: Option<u32>,
    closing_balance_column: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrialBalanceImportInputDto {
    engagement_id: String,
    controlled_evidence_version_id: String,
    sheet_name: String,
    header_row_number: u32,
    amount_scale: u32,
    mapping: TrialBalanceColumnMappingInputDto,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TrialBalanceImportDto {
    trial_balance_import_id: String,
    engagement_id: String,
    controlled_evidence_version_id: String,
    document_id: String,
    source_content_version_id: String,
    source_sha256_hex: String,
    sheet_name: String,
    header_row_number: u64,
    account_name_column: u32,
    account_code_column: Option<u32>,
    opening_balance_column: Option<u32>,
    closing_balance_column: u32,
    amount_scale: u32,
    account_count: u64,
    opening_total_minor: i64,
    closing_total_minor: i64,
    imported_at_ms: i64,
}

impl From<persistence::TrialBalanceImportRecord> for TrialBalanceImportDto {
    fn from(value: persistence::TrialBalanceImportRecord) -> Self {
        Self {
            trial_balance_import_id: value.trial_balance_import_id,
            engagement_id: value.engagement_id,
            controlled_evidence_version_id: value.controlled_evidence_version_id,
            document_id: value.document_id,
            source_content_version_id: value.source_content_version_id,
            source_sha256_hex: hex_bytes(&value.source_sha256),
            sheet_name: value.sheet_name,
            header_row_number: value.header_row_number,
            account_name_column: value.account_name_column,
            account_code_column: value.account_code_column,
            opening_balance_column: value.opening_balance_column,
            closing_balance_column: value.closing_balance_column,
            amount_scale: value.amount_scale,
            account_count: value.account_count,
            opening_total_minor: value.opening_total_minor,
            closing_total_minor: value.closing_total_minor,
            imported_at_ms: value.imported_at_ms,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TrialBalanceAccountDto {
    trial_balance_account_id: String,
    trial_balance_import_id: String,
    source_row_number: u64,
    source_row_hash_hex: String,
    account_code_text: Option<String>,
    account_name_text: String,
    opening_minor: i64,
    closing_minor: i64,
    created_at_ms: i64,
}

impl From<persistence::TrialBalanceAccountRecord> for TrialBalanceAccountDto {
    fn from(value: persistence::TrialBalanceAccountRecord) -> Self {
        Self {
            trial_balance_account_id: value.trial_balance_account_id,
            trial_balance_import_id: value.trial_balance_import_id,
            source_row_number: value.source_row_number,
            source_row_hash_hex: hex_bytes(&value.source_row_hash),
            account_code_text: value.account_code_text,
            account_name_text: value.account_name_text,
            opening_minor: value.opening_minor,
            closing_minor: value.closing_minor,
            created_at_ms: value.created_at_ms,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TrialBalanceMovementDto {
    trial_balance_account_id: String,
    source_row_number: u64,
    source_row_hash_hex: String,
    account_code_text: Option<String>,
    account_name_text: String,
    opening_minor: i64,
    closing_minor: i64,
    movement_minor: i64,
}

impl From<persistence::TrialBalanceMovementRecord> for TrialBalanceMovementDto {
    fn from(value: persistence::TrialBalanceMovementRecord) -> Self {
        Self {
            trial_balance_account_id: value.trial_balance_account_id,
            source_row_number: value.source_row_number,
            source_row_hash_hex: hex_bytes(&value.source_row_hash),
            account_code_text: value.account_code_text,
            account_name_text: value.account_name_text,
            opening_minor: value.opening_minor,
            closing_minor: value.closing_minor,
            movement_minor: value.movement_minor,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TrialBalanceComparisonDto {
    trial_balance_import_id: String,
    opening_total_minor: i64,
    closing_total_minor: i64,
    net_movement_minor: i64,
    account_count: u64,
    movements: Vec<TrialBalanceMovementDto>,
}

impl From<persistence::TrialBalanceComparisonRecord> for TrialBalanceComparisonDto {
    fn from(value: persistence::TrialBalanceComparisonRecord) -> Self {
        Self {
            trial_balance_import_id: value.trial_balance_import_id,
            opening_total_minor: value.opening_total_minor,
            closing_total_minor: value.closing_total_minor,
            net_movement_minor: value.net_movement_minor,
            account_count: value.account_count,
            movements: value.movements.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReconciliationRunDto {
    reconciliation_run_id: String,
    engagement_id: String,
    reconciliation_type: String,
    title: String,
    rule_code: String,
    parameters_json: String,
    left_item_count: u64,
    right_item_count: u64,
    matched_pair_count: u64,
    exception_count: u64,
    ran_at_ms: i64,
}

impl From<persistence::ReconciliationRunRecord> for ReconciliationRunDto {
    fn from(value: persistence::ReconciliationRunRecord) -> Self {
        Self {
            reconciliation_run_id: value.reconciliation_run_id,
            engagement_id: value.engagement_id,
            reconciliation_type: value.reconciliation_type,
            title: value.title,
            rule_code: value.rule_code,
            parameters_json: value.parameters_json,
            left_item_count: value.left_item_count,
            right_item_count: value.right_item_count,
            matched_pair_count: value.matched_pair_count,
            exception_count: value.exception_count,
            ran_at_ms: value.ran_at_ms,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReconciliationExceptionDto {
    reconciliation_exception_id: String,
    reconciliation_run_id: String,
    reconciliation_item_id: String,
    exception_code: String,
    side: String,
    match_key: String,
    amount_minor: i64,
    event_date_text: Option<String>,
    description_text: Option<String>,
    source_kind: String,
    source_entity_id: String,
    controlled_evidence_version_id: String,
    document_id: String,
    source_content_version_id: String,
    source_sha256_hex: String,
    sheet_name: Option<String>,
    source_row_number: Option<u64>,
    source_row_hash_hex: Option<String>,
    created_at_ms: i64,
}

impl From<persistence::ReconciliationExceptionRecord> for ReconciliationExceptionDto {
    fn from(value: persistence::ReconciliationExceptionRecord) -> Self {
        Self {
            reconciliation_exception_id: value.reconciliation_exception_id,
            reconciliation_run_id: value.reconciliation_run_id,
            reconciliation_item_id: value.reconciliation_item_id,
            exception_code: value.exception_code,
            side: value.side,
            match_key: value.match_key,
            amount_minor: value.amount_minor,
            event_date_text: value.event_date_text,
            description_text: value.description_text,
            source_kind: value.source_kind,
            source_entity_id: value.source_entity_id,
            controlled_evidence_version_id: value.controlled_evidence_version_id,
            document_id: value.document_id,
            source_content_version_id: value.source_content_version_id,
            source_sha256_hex: hex_bytes(&value.source_sha256),
            sheet_name: value.sheet_name,
            source_row_number: value.source_row_number,
            source_row_hash_hex: value.source_row_hash.map(|hash| hex_bytes(&hash)),
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
fn create_normal_data_dataset(
    normal_data_workspace_id: String,
    file_instance_id: String,
    name: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<normal_data_datasets::DatasetRecord, String> {
    validate_uuid(&normal_data_workspace_id, "normal-data-workspace")?;
    validate_uuid(&file_instance_id, "indexed-file-instance")?;
    normal_data_datasets::create_dataset(
        database.path(),
        &normal_data_workspace_id,
        &file_instance_id,
        &name,
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_normal_data_datasets(
    normal_data_workspace_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<normal_data_datasets::DatasetRecord>, String> {
    validate_uuid(&normal_data_workspace_id, "normal-data-workspace")?;
    normal_data_datasets::list_datasets(database.path(), &normal_data_workspace_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn declare_normal_data_column_semantic(
    normal_data_dataset_version_id: String,
    column_name: String,
    semantic_role: String,
    data_type: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<normal_data_datasets::ColumnSemanticRecord, String> {
    validate_uuid(
        &normal_data_dataset_version_id,
        "normal-data-dataset-version",
    )?;
    normal_data_datasets::declare_column(
        database.path(),
        &normal_data_dataset_version_id,
        &column_name,
        &semantic_role,
        &data_type,
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_normal_data_column_semantics(
    normal_data_dataset_version_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<normal_data_datasets::ColumnSemanticRecord>, String> {
    validate_uuid(
        &normal_data_dataset_version_id,
        "normal-data-dataset-version",
    )?;
    normal_data_datasets::list_columns(database.path(), &normal_data_dataset_version_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn create_normal_data_workspace(
    name: String,
    description: Option<String>,
    client_id: Option<String>,
    period_start: Option<String>,
    period_end: Option<String>,
    database: State<'_, persistence::DatabaseState>,
) -> Result<normal_data::WorkspaceRecord, String> {
    validate_optional_uuid(client_id.as_deref(), "normal-data-client")?;
    normal_data::create_workspace(
        database.path(),
        normal_data::WorkspaceDefinition {
            name: &name,
            description: description.as_deref(),
            client_id: client_id.as_deref(),
            period_start: period_start.as_deref(),
            period_end: period_end.as_deref(),
        },
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_normal_data_workspaces(
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<normal_data::WorkspaceRecord>, String> {
    normal_data::list_workspaces(database.path()).map_err(|error| error.to_string())
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
fn create_statutory_compliance_requirement(
    engagement_id: String,
    firm_library_version_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<StatutoryComplianceRequirementDto, String> {
    validate_uuid(&engagement_id, "engagement")?;
    validate_uuid(&firm_library_version_id, "firm-library-version")?;
    persistence::create_statutory_compliance_requirement(
        database.path(),
        &engagement_id,
        &firm_library_version_id,
    )
    .map(Into::into)
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_statutory_compliance_requirements(
    engagement_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<StatutoryComplianceRequirementDto>, String> {
    validate_uuid(&engagement_id, "engagement")?;
    persistence::list_statutory_compliance_requirements(database.path(), &engagement_id)
        .map(|records| records.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn create_statutory_compliance_assessment(
    input: StatutoryComplianceAssessmentInputDto,
    database: State<'_, persistence::DatabaseState>,
) -> Result<StatutoryComplianceAssessmentDto, String> {
    validate_uuid(
        &input.statutory_compliance_requirement_id,
        "statutory-compliance-requirement",
    )?;
    for controlled_evidence_version_id in &input.controlled_evidence_version_ids {
        validate_uuid(
            controlled_evidence_version_id,
            "controlled-evidence-version",
        )?;
    }

    persistence::create_statutory_compliance_assessment(
        database.path(),
        persistence::StatutoryComplianceAssessmentDefinition {
            statutory_compliance_requirement_id: &input.statutory_compliance_requirement_id,
            applicability: &input.applicability,
            due_date: input.due_date.as_deref(),
            actual_compliance_date: input.actual_compliance_date.as_deref(),
            status: &input.status,
            exception_text: input.exception_text.as_deref(),
            conclusion: input.conclusion.as_deref(),
            controlled_evidence_version_ids: &input.controlled_evidence_version_ids,
        },
    )
    .map(Into::into)
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_statutory_compliance_assessments(
    statutory_compliance_requirement_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<StatutoryComplianceAssessmentDto>, String> {
    validate_uuid(
        &statutory_compliance_requirement_id,
        "statutory-compliance-requirement",
    )?;
    persistence::list_statutory_compliance_assessments(
        database.path(),
        &statutory_compliance_requirement_id,
    )
    .map(|records| records.into_iter().map(Into::into).collect())
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_statutory_compliance_evidence(
    statutory_compliance_assessment_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<StatutoryComplianceEvidenceDto>, String> {
    validate_uuid(
        &statutory_compliance_assessment_id,
        "statutory-compliance-assessment",
    )?;
    persistence::list_statutory_compliance_evidence(
        database.path(),
        &statutory_compliance_assessment_id,
    )
    .map(|records| records.into_iter().map(Into::into).collect())
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn create_internal_audit_process(
    input: InternalAuditProcessInputDto,
    database: State<'_, persistence::DatabaseState>,
) -> Result<InternalAuditProcessDto, String> {
    validate_uuid(&input.engagement_id, "engagement")?;
    validate_optional_uuid(
        input.parent_process_id.as_deref(),
        "internal-audit-parent-process",
    )?;
    persistence::create_internal_audit_process(
        database.path(),
        persistence::InternalAuditProcessDefinition {
            engagement_id: &input.engagement_id,
            parent_process_id: input.parent_process_id.as_deref(),
            code: input.code.as_deref(),
            name: &input.name,
            description: input.description.as_deref(),
            display_order: input.display_order,
            status: input.status.as_deref().unwrap_or("ACTIVE"),
        },
    )
    .map(Into::into)
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_internal_audit_processes(
    engagement_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<InternalAuditProcessDto>, String> {
    validate_uuid(&engagement_id, "engagement")?;
    persistence::list_internal_audit_processes(database.path(), &engagement_id)
        .map(|records| records.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn create_internal_audit_objective(
    internal_audit_process_id: String,
    reference: Option<String>,
    title: String,
    description: Option<String>,
    status: Option<String>,
    database: State<'_, persistence::DatabaseState>,
) -> Result<InternalAuditObjectiveDto, String> {
    validate_uuid(&internal_audit_process_id, "internal-audit-process")?;
    persistence::create_internal_audit_objective(
        database.path(),
        &internal_audit_process_id,
        reference.as_deref(),
        &title,
        description.as_deref(),
        status.as_deref().unwrap_or("ACTIVE"),
    )
    .map(Into::into)
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_internal_audit_objectives(
    internal_audit_process_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<InternalAuditObjectiveDto>, String> {
    validate_uuid(&internal_audit_process_id, "internal-audit-process")?;
    persistence::list_internal_audit_objectives(database.path(), &internal_audit_process_id)
        .map(|records| records.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn create_internal_audit_risk(
    input: InternalAuditRiskInputDto,
    database: State<'_, persistence::DatabaseState>,
) -> Result<InternalAuditRiskDto, String> {
    validate_uuid(
        &input.internal_audit_objective_id,
        "internal-audit-objective",
    )?;
    persistence::create_internal_audit_risk(
        database.path(),
        persistence::InternalAuditRiskDefinition {
            internal_audit_objective_id: &input.internal_audit_objective_id,
            reference: input.reference.as_deref(),
            title: &input.title,
            description: input.description.as_deref(),
            risk_classification: input.risk_classification.as_deref(),
            inherent_rating: input.inherent_rating.as_deref(),
            status: input.status.as_deref().unwrap_or("ACTIVE"),
        },
    )
    .map(Into::into)
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_internal_audit_risks(
    internal_audit_objective_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<InternalAuditRiskDto>, String> {
    validate_uuid(&internal_audit_objective_id, "internal-audit-objective")?;
    persistence::list_internal_audit_risks(database.path(), &internal_audit_objective_id)
        .map(|records| records.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn create_internal_audit_control(
    input: InternalAuditControlInputDto,
    database: State<'_, persistence::DatabaseState>,
) -> Result<InternalAuditControlDto, String> {
    validate_uuid(&input.internal_audit_risk_id, "internal-audit-risk")?;
    persistence::create_internal_audit_control(
        database.path(),
        persistence::InternalAuditControlDefinition {
            internal_audit_risk_id: &input.internal_audit_risk_id,
            reference: input.reference.as_deref(),
            title: &input.title,
            description: input.description.as_deref(),
            control_type: input.control_type.as_deref(),
            frequency: input.frequency.as_deref(),
            owner_text: input.owner_text.as_deref(),
            status: input.status.as_deref().unwrap_or("ACTIVE"),
        },
    )
    .map(Into::into)
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_internal_audit_controls(
    internal_audit_risk_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<InternalAuditControlDto>, String> {
    validate_uuid(&internal_audit_risk_id, "internal-audit-risk")?;
    persistence::list_internal_audit_controls(database.path(), &internal_audit_risk_id)
        .map(|records| records.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn create_internal_audit_test(
    input: InternalAuditTestInputDto,
    database: State<'_, persistence::DatabaseState>,
) -> Result<InternalAuditTestDto, String> {
    validate_uuid(&input.internal_audit_control_id, "internal-audit-control")?;
    persistence::create_internal_audit_test(
        database.path(),
        persistence::InternalAuditTestDefinition {
            internal_audit_control_id: &input.internal_audit_control_id,
            reference: input.reference.as_deref(),
            title: &input.title,
            procedure_text: &input.procedure_text,
            sample_strategy: input.sample_strategy.as_deref(),
            expected_result: input.expected_result.as_deref(),
            status: input.status.as_deref().unwrap_or("ACTIVE"),
        },
    )
    .map(Into::into)
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_internal_audit_tests(
    internal_audit_control_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<InternalAuditTestDto>, String> {
    validate_uuid(&internal_audit_control_id, "internal-audit-control")?;
    persistence::list_internal_audit_tests(database.path(), &internal_audit_control_id)
        .map(|records| records.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn create_internal_audit_test_evidence_link(
    internal_audit_test_id: String,
    controlled_evidence_version_id: String,
    description: Option<String>,
    database: State<'_, persistence::DatabaseState>,
) -> Result<InternalAuditTestEvidenceDto, String> {
    validate_uuid(&internal_audit_test_id, "internal-audit-test")?;
    validate_uuid(
        &controlled_evidence_version_id,
        "controlled-evidence-version",
    )?;
    persistence::create_internal_audit_test_evidence_link(
        database.path(),
        &internal_audit_test_id,
        &controlled_evidence_version_id,
        description.as_deref(),
    )
    .map(Into::into)
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_internal_audit_test_evidence(
    internal_audit_test_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<InternalAuditTestEvidenceDto>, String> {
    validate_uuid(&internal_audit_test_id, "internal-audit-test")?;
    persistence::list_internal_audit_test_evidence(database.path(), &internal_audit_test_id)
        .map(|records| records.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn create_internal_audit_finding(
    input: InternalAuditFindingInputDto,
    database: State<'_, persistence::DatabaseState>,
) -> Result<InternalAuditFindingDto, String> {
    validate_uuid(&input.origin_engagement_id, "origin-engagement")?;
    validate_uuid(&input.internal_audit_process_id, "internal-audit-process")?;
    validate_optional_uuid(
        input.internal_audit_risk_id.as_deref(),
        "internal-audit-risk",
    )?;
    validate_optional_uuid(
        input.internal_audit_control_id.as_deref(),
        "internal-audit-control",
    )?;
    validate_optional_uuid(
        input.internal_audit_test_id.as_deref(),
        "internal-audit-test",
    )?;
    validate_optional_uuid(input.workpaper_id.as_deref(), "workpaper")?;
    validate_optional_uuid(
        input.repeated_from_finding_id.as_deref(),
        "repeated-internal-audit-finding",
    )?;

    persistence::create_internal_audit_finding(
        database.path(),
        persistence::InternalAuditFindingDefinition {
            origin_engagement_id: &input.origin_engagement_id,
            internal_audit_process_id: &input.internal_audit_process_id,
            internal_audit_risk_id: input.internal_audit_risk_id.as_deref(),
            internal_audit_control_id: input.internal_audit_control_id.as_deref(),
            internal_audit_test_id: input.internal_audit_test_id.as_deref(),
            workpaper_id: input.workpaper_id.as_deref(),
            repeated_from_finding_id: input.repeated_from_finding_id.as_deref(),
            reference: input.reference.as_deref(),
            title: &input.title,
            condition_text: &input.condition_text,
            criteria_text: input.criteria_text.as_deref(),
            cause_text: input.cause_text.as_deref(),
            risk_effect_text: input.risk_effect_text.as_deref(),
            recommendation_text: input.recommendation_text.as_deref(),
            risk_classification: input.risk_classification.as_deref(),
            actor_id: input.actor_id.as_deref(),
        },
    )
    .map(Into::into)
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_internal_audit_findings_for_engagement(
    engagement_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<InternalAuditFindingDto>, String> {
    validate_uuid(&engagement_id, "engagement")?;
    persistence::list_internal_audit_findings_for_engagement(database.path(), &engagement_id)
        .map(|records| records.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_internal_audit_findings_for_client(
    client_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<InternalAuditFindingDto>, String> {
    validate_uuid(&client_id, "client")?;
    persistence::list_internal_audit_findings_for_client(database.path(), &client_id)
        .map(|records| records.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn create_internal_audit_finding_followup(
    input: InternalAuditFindingFollowupInputDto,
    database: State<'_, persistence::DatabaseState>,
) -> Result<InternalAuditFindingFollowupDto, String> {
    validate_uuid(&input.internal_audit_finding_id, "internal-audit-finding")?;
    validate_uuid(&input.tracking_engagement_id, "tracking-engagement")?;
    persistence::create_internal_audit_finding_followup(
        database.path(),
        persistence::InternalAuditFindingFollowupDefinition {
            internal_audit_finding_id: &input.internal_audit_finding_id,
            tracking_engagement_id: &input.tracking_engagement_id,
            status: &input.status,
            management_response: input.management_response.as_deref(),
            action_owner: input.action_owner.as_deref(),
            target_date: input.target_date.as_deref(),
            follow_up_text: input.follow_up_text.as_deref(),
            verification_conclusion: input.verification_conclusion.as_deref(),
            actor_id: input.actor_id.as_deref(),
        },
    )
    .map(Into::into)
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_internal_audit_finding_followups(
    internal_audit_finding_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<InternalAuditFindingFollowupDto>, String> {
    validate_uuid(&internal_audit_finding_id, "internal-audit-finding")?;
    persistence::list_internal_audit_finding_followups(database.path(), &internal_audit_finding_id)
        .map(|records| records.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn create_internal_audit_finding_evidence_link(
    internal_audit_finding_id: String,
    internal_audit_finding_followup_id: Option<String>,
    controlled_evidence_version_id: String,
    description: Option<String>,
    database: State<'_, persistence::DatabaseState>,
) -> Result<InternalAuditFindingEvidenceDto, String> {
    validate_uuid(&internal_audit_finding_id, "internal-audit-finding")?;
    validate_optional_uuid(
        internal_audit_finding_followup_id.as_deref(),
        "internal-audit-finding-followup",
    )?;
    validate_uuid(
        &controlled_evidence_version_id,
        "controlled-evidence-version",
    )?;

    persistence::create_internal_audit_finding_evidence_link(
        database.path(),
        &internal_audit_finding_id,
        internal_audit_finding_followup_id.as_deref(),
        &controlled_evidence_version_id,
        description.as_deref(),
    )
    .map(Into::into)
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_internal_audit_finding_evidence(
    internal_audit_finding_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<InternalAuditFindingEvidenceDto>, String> {
    validate_uuid(&internal_audit_finding_id, "internal-audit-finding")?;
    persistence::list_internal_audit_finding_evidence(database.path(), &internal_audit_finding_id)
        .map(|records| records.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn create_due_diligence_workspace(
    engagement_id: String,
    name: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<DueDiligenceWorkspaceDto, String> {
    validate_uuid(&engagement_id, "engagement")?;
    persistence::create_due_diligence_workspace(database.path(), &engagement_id, &name)
        .map(Into::into)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn get_due_diligence_workspace_for_engagement(
    engagement_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Option<DueDiligenceWorkspaceDto>, String> {
    validate_uuid(&engagement_id, "engagement")?;
    persistence::get_due_diligence_workspace_for_engagement(database.path(), &engagement_id)
        .map(|record| record.map(Into::into))
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn create_due_diligence_section(
    input: DueDiligenceSectionInputDto,
    database: State<'_, persistence::DatabaseState>,
) -> Result<DueDiligenceSectionDto, String> {
    validate_uuid(&input.due_diligence_workspace_id, "due-diligence-workspace")?;
    validate_optional_uuid(
        input.parent_section_id.as_deref(),
        "due-diligence-parent-section",
    )?;
    persistence::create_due_diligence_section(
        database.path(),
        persistence::DueDiligenceSectionDefinition {
            due_diligence_workspace_id: &input.due_diligence_workspace_id,
            parent_section_id: input.parent_section_id.as_deref(),
            code: input.code.as_deref(),
            name: &input.name,
            description: input.description.as_deref(),
            display_order: input.display_order,
        },
    )
    .map(Into::into)
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_due_diligence_sections(
    due_diligence_workspace_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<DueDiligenceSectionDto>, String> {
    validate_uuid(&due_diligence_workspace_id, "due-diligence-workspace")?;
    persistence::list_due_diligence_sections(database.path(), &due_diligence_workspace_id)
        .map(|records| records.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn create_due_diligence_request(
    input: DueDiligenceRequestInputDto,
    database: State<'_, persistence::DatabaseState>,
) -> Result<DueDiligenceRequestDto, String> {
    validate_uuid(&input.due_diligence_workspace_id, "due-diligence-workspace")?;
    validate_optional_uuid(
        input.due_diligence_section_id.as_deref(),
        "due-diligence-section",
    )?;
    persistence::create_due_diligence_request(
        database.path(),
        persistence::DueDiligenceRequestDefinition {
            due_diligence_workspace_id: &input.due_diligence_workspace_id,
            due_diligence_section_id: input.due_diligence_section_id.as_deref(),
            reference: input.reference.as_deref(),
            title: &input.title,
            description: input.description.as_deref(),
            requested_from_party: input.requested_from_party.as_deref(),
            due_date: input.due_date.as_deref(),
            internal_notes: input.internal_notes.as_deref(),
            actor_id: input.actor_id.as_deref(),
        },
    )
    .map(Into::into)
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_due_diligence_requests(
    due_diligence_workspace_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<DueDiligenceRequestDto>, String> {
    validate_uuid(&due_diligence_workspace_id, "due-diligence-workspace")?;
    persistence::list_due_diligence_requests(database.path(), &due_diligence_workspace_id)
        .map(|records| records.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn create_due_diligence_request_event(
    input: DueDiligenceRequestEventInputDto,
    database: State<'_, persistence::DatabaseState>,
) -> Result<DueDiligenceRequestEventDto, String> {
    validate_uuid(&input.due_diligence_request_id, "due-diligence-request")?;
    persistence::create_due_diligence_request_event(
        database.path(),
        persistence::DueDiligenceRequestEventDefinition {
            due_diligence_request_id: &input.due_diligence_request_id,
            status: &input.status,
            response_text: input.response_text.as_deref(),
            internal_assessment: input.internal_assessment.as_deref(),
            actor_id: input.actor_id.as_deref(),
        },
    )
    .map(Into::into)
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_due_diligence_request_events(
    due_diligence_request_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<DueDiligenceRequestEventDto>, String> {
    validate_uuid(&due_diligence_request_id, "due-diligence-request")?;
    persistence::list_due_diligence_request_events(database.path(), &due_diligence_request_id)
        .map(|records| records.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn create_due_diligence_request_evidence_link(
    due_diligence_request_id: String,
    due_diligence_request_event_id: Option<String>,
    controlled_evidence_version_id: String,
    description: Option<String>,
    database: State<'_, persistence::DatabaseState>,
) -> Result<DueDiligenceRequestEvidenceDto, String> {
    validate_uuid(&due_diligence_request_id, "due-diligence-request")?;
    validate_optional_uuid(
        due_diligence_request_event_id.as_deref(),
        "due-diligence-request-event",
    )?;
    validate_uuid(
        &controlled_evidence_version_id,
        "controlled-evidence-version",
    )?;
    persistence::create_due_diligence_request_evidence_link(
        database.path(),
        &due_diligence_request_id,
        due_diligence_request_event_id.as_deref(),
        &controlled_evidence_version_id,
        description.as_deref(),
    )
    .map(Into::into)
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_due_diligence_request_evidence(
    due_diligence_request_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<DueDiligenceRequestEvidenceDto>, String> {
    validate_uuid(&due_diligence_request_id, "due-diligence-request")?;
    persistence::list_due_diligence_request_evidence(database.path(), &due_diligence_request_id)
        .map(|records| records.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn create_due_diligence_issue(
    input: DueDiligenceIssueInputDto,
    database: State<'_, persistence::DatabaseState>,
) -> Result<DueDiligenceIssueDto, String> {
    validate_uuid(&input.due_diligence_workspace_id, "due-diligence-workspace")?;
    validate_optional_uuid(
        input.due_diligence_section_id.as_deref(),
        "due-diligence-section",
    )?;
    validate_optional_uuid(
        input.due_diligence_request_id.as_deref(),
        "due-diligence-request",
    )?;

    persistence::create_due_diligence_issue(
        database.path(),
        persistence::DueDiligenceIssueDefinition {
            due_diligence_workspace_id: &input.due_diligence_workspace_id,
            due_diligence_section_id: input.due_diligence_section_id.as_deref(),
            due_diligence_request_id: input.due_diligence_request_id.as_deref(),
            issue_type: &input.issue_type,
            reference: input.reference.as_deref(),
            title: &input.title,
            description: input.description.as_deref(),
            category: input.category.as_deref(),
            severity: input.severity.as_deref(),
            actor_id: input.actor_id.as_deref(),
        },
    )
    .map(Into::into)
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_due_diligence_issues(
    due_diligence_workspace_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<DueDiligenceIssueDto>, String> {
    validate_uuid(&due_diligence_workspace_id, "due-diligence-workspace")?;
    persistence::list_due_diligence_issues(database.path(), &due_diligence_workspace_id)
        .map(|records| records.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn create_due_diligence_issue_event(
    input: DueDiligenceIssueEventInputDto,
    database: State<'_, persistence::DatabaseState>,
) -> Result<DueDiligenceIssueEventDto, String> {
    validate_uuid(&input.due_diligence_issue_id, "due-diligence-issue")?;
    persistence::create_due_diligence_issue_event(
        database.path(),
        persistence::DueDiligenceIssueEventDefinition {
            due_diligence_issue_id: &input.due_diligence_issue_id,
            status: &input.status,
            internal_conclusion: input.internal_conclusion.as_deref(),
            deal_impact: input.deal_impact.as_deref(),
            recommendation: input.recommendation.as_deref(),
            actor_id: input.actor_id.as_deref(),
        },
    )
    .map(Into::into)
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_due_diligence_issue_events(
    due_diligence_issue_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<DueDiligenceIssueEventDto>, String> {
    validate_uuid(&due_diligence_issue_id, "due-diligence-issue")?;
    persistence::list_due_diligence_issue_events(database.path(), &due_diligence_issue_id)
        .map(|records| records.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn create_due_diligence_issue_evidence_link(
    due_diligence_issue_id: String,
    due_diligence_issue_event_id: Option<String>,
    controlled_evidence_version_id: String,
    description: Option<String>,
    database: State<'_, persistence::DatabaseState>,
) -> Result<DueDiligenceIssueEvidenceDto, String> {
    validate_uuid(&due_diligence_issue_id, "due-diligence-issue")?;
    validate_optional_uuid(
        due_diligence_issue_event_id.as_deref(),
        "due-diligence-issue-event",
    )?;
    validate_uuid(
        &controlled_evidence_version_id,
        "controlled-evidence-version",
    )?;

    persistence::create_due_diligence_issue_evidence_link(
        database.path(),
        &due_diligence_issue_id,
        due_diligence_issue_event_id.as_deref(),
        &controlled_evidence_version_id,
        description.as_deref(),
    )
    .map(Into::into)
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_due_diligence_issue_evidence(
    due_diligence_issue_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<DueDiligenceIssueEvidenceDto>, String> {
    validate_uuid(&due_diligence_issue_id, "due-diligence-issue")?;
    persistence::list_due_diligence_issue_evidence(database.path(), &due_diligence_issue_id)
        .map(|records| records.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
}

fn validate_due_diligence_report_input(
    input: &DueDiligenceReportVersionInputDto,
) -> Result<(), String> {
    for issue_id in &input.issue_ids {
        validate_uuid(issue_id, "due-diligence-issue")?;
    }
    Ok(())
}

#[tauri::command]
fn create_due_diligence_report(
    due_diligence_workspace_id: String,
    input: DueDiligenceReportVersionInputDto,
    database: State<'_, persistence::DatabaseState>,
) -> Result<DueDiligenceReportDto, String> {
    validate_uuid(&due_diligence_workspace_id, "due-diligence-workspace")?;
    validate_due_diligence_report_input(&input)?;
    persistence::create_due_diligence_report(
        database.path(),
        &due_diligence_workspace_id,
        persistence::DueDiligenceReportVersionDefinition {
            title: &input.title,
            executive_summary: input.executive_summary.as_deref(),
            scope_summary: input.scope_summary.as_deref(),
            overall_conclusion: input.overall_conclusion.as_deref(),
            issue_ids: &input.issue_ids,
            created_by: input.created_by.as_deref(),
        },
    )
    .map(Into::into)
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn publish_due_diligence_report_version(
    due_diligence_report_id: String,
    input: DueDiligenceReportVersionInputDto,
    database: State<'_, persistence::DatabaseState>,
) -> Result<DueDiligenceReportDto, String> {
    validate_uuid(&due_diligence_report_id, "due-diligence-report")?;
    validate_due_diligence_report_input(&input)?;
    persistence::publish_due_diligence_report_version(
        database.path(),
        &due_diligence_report_id,
        persistence::DueDiligenceReportVersionDefinition {
            title: &input.title,
            executive_summary: input.executive_summary.as_deref(),
            scope_summary: input.scope_summary.as_deref(),
            overall_conclusion: input.overall_conclusion.as_deref(),
            issue_ids: &input.issue_ids,
            created_by: input.created_by.as_deref(),
        },
    )
    .map(Into::into)
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_due_diligence_reports(
    due_diligence_workspace_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<DueDiligenceReportDto>, String> {
    validate_uuid(&due_diligence_workspace_id, "due-diligence-workspace")?;
    persistence::list_due_diligence_reports(database.path(), &due_diligence_workspace_id)
        .map(|records| records.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_due_diligence_report_versions(
    due_diligence_report_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<DueDiligenceReportVersionDto>, String> {
    validate_uuid(&due_diligence_report_id, "due-diligence-report")?;
    persistence::list_due_diligence_report_versions(database.path(), &due_diligence_report_id)
        .map(|records| records.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_due_diligence_report_version_issues(
    due_diligence_report_version_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<DueDiligenceReportIssueDto>, String> {
    validate_uuid(
        &due_diligence_report_version_id,
        "due-diligence-report-version",
    )?;
    persistence::list_due_diligence_report_version_issues(
        database.path(),
        &due_diligence_report_version_id,
    )
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
fn list_ledger_account_summaries(
    ledger_import_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<LedgerAccountSummaryDto>, String> {
    validate_uuid(&ledger_import_id, "ledger-import")?;
    persistence::list_ledger_account_summaries(database.path(), &ledger_import_id)
        .map(|records| records.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn create_ledger_tb_mapping(
    ledger_import_id: String,
    trial_balance_import_id: String,
    ledger_account_key: String,
    trial_balance_account_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<LedgerTbMappingDto, String> {
    validate_uuid(&ledger_import_id, "ledger-import")?;
    validate_uuid(&trial_balance_import_id, "trial-balance-import")?;
    validate_uuid(&trial_balance_account_id, "trial-balance-account")?;
    persistence::create_ledger_tb_mapping(
        database.path(),
        &ledger_import_id,
        &trial_balance_import_id,
        &ledger_account_key,
        &trial_balance_account_id,
    )
    .map(Into::into)
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_current_ledger_tb_mappings(
    ledger_import_id: String,
    trial_balance_import_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<LedgerTbMappingDto>, String> {
    validate_uuid(&ledger_import_id, "ledger-import")?;
    validate_uuid(&trial_balance_import_id, "trial-balance-import")?;
    persistence::list_current_ledger_tb_mappings(
        database.path(),
        &ledger_import_id,
        &trial_balance_import_id,
    )
    .map(|records| records.into_iter().map(Into::into).collect())
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn create_financial_statement_schedule(
    engagement_id: String,
    reference: String,
    name: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<FinancialStatementScheduleDto, String> {
    validate_uuid(&engagement_id, "engagement")?;
    persistence::create_financial_statement_schedule(
        database.path(),
        &engagement_id,
        &reference,
        &name,
    )
    .map(Into::into)
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_financial_statement_schedules(
    engagement_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<FinancialStatementScheduleDto>, String> {
    validate_uuid(&engagement_id, "engagement")?;
    persistence::list_financial_statement_schedules(database.path(), &engagement_id)
        .map(|records| records.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn create_trial_balance_schedule_mapping(
    trial_balance_import_id: String,
    trial_balance_account_id: String,
    financial_statement_schedule_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<TrialBalanceScheduleMappingDto, String> {
    validate_uuid(&trial_balance_import_id, "trial-balance-import")?;
    validate_uuid(&trial_balance_account_id, "trial-balance-account")?;
    validate_uuid(
        &financial_statement_schedule_id,
        "financial-statement-schedule",
    )?;
    persistence::create_trial_balance_schedule_mapping(
        database.path(),
        &trial_balance_import_id,
        &trial_balance_account_id,
        &financial_statement_schedule_id,
    )
    .map(Into::into)
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_current_trial_balance_schedule_mappings(
    trial_balance_import_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<TrialBalanceScheduleMappingDto>, String> {
    validate_uuid(&trial_balance_import_id, "trial-balance-import")?;
    persistence::list_current_trial_balance_schedule_mappings(
        database.path(),
        &trial_balance_import_id,
    )
    .map(|records| records.into_iter().map(Into::into).collect())
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn create_financial_statement_schedule_link(
    financial_statement_schedule_id: String,
    controlled_evidence_version_id: String,
    location_kind: String,
    location_value: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<FinancialStatementScheduleLinkDto, String> {
    validate_uuid(
        &financial_statement_schedule_id,
        "financial-statement-schedule",
    )?;
    validate_uuid(
        &controlled_evidence_version_id,
        "controlled-evidence-version",
    )?;
    persistence::create_financial_statement_schedule_link(
        database.path(),
        &financial_statement_schedule_id,
        &controlled_evidence_version_id,
        &location_kind,
        &location_value,
    )
    .map(Into::into)
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_current_financial_statement_schedule_links(
    engagement_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<FinancialStatementScheduleLinkDto>, String> {
    validate_uuid(&engagement_id, "engagement")?;
    persistence::list_current_financial_statement_schedule_links(database.path(), &engagement_id)
        .map(|records| records.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_ledger_test_runs(
    ledger_import_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<LedgerTestRunDto>, String> {
    validate_uuid(&ledger_import_id, "ledger-import")?;
    persistence::list_ledger_test_runs(database.path(), &ledger_import_id)
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
async fn import_trial_balance_from_controlled_evidence(
    input: TrialBalanceImportInputDto,
    database: State<'_, persistence::DatabaseState>,
    evidence_state: State<'_, evidence::EvidenceState>,
) -> Result<TrialBalanceImportDto, String> {
    let TrialBalanceImportInputDto {
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
            trial_balance::MAX_TRIAL_BALANCE_IMPORT_BYTES,
        )
        .map_err(|error| error.to_string())?;

        let trial_balance_mapping = trial_balance::TrialBalanceColumnMapping {
            account_name_column: mapping.account_name_column,
            account_code_column: mapping.account_code_column,
            opening_balance_column: mapping.opening_balance_column,
            closing_balance_column: mapping.closing_balance_column,
        };
        let parsed = trial_balance::parse_trial_balance_workbook(
            &controlled.bytes,
            &sheet_name,
            header_row_number,
            amount_scale,
            &trial_balance_mapping,
        )?;
        let accounts = parsed
            .into_iter()
            .map(|account| persistence::TrialBalanceAccountInput {
                source_row_number: account.source_row_number,
                source_row_hash: account.source_row_hash,
                account_code_text: account.account_code_text,
                account_name_text: account.account_name_text,
                opening_minor: account.opening_minor,
                closing_minor: account.closing_minor,
            })
            .collect::<Vec<_>>();

        persistence::create_trial_balance_import(
            &database_path,
            persistence::TrialBalanceImportDefinition {
                engagement_id: &engagement_id,
                controlled_evidence_version_id: &controlled.record.controlled_evidence_version_id,
                sheet_name: &sheet_name,
                header_row_number: u64::from(header_row_number),
                account_name_column: mapping.account_name_column,
                account_code_column: mapping.account_code_column,
                opening_balance_column: mapping.opening_balance_column,
                closing_balance_column: mapping.closing_balance_column,
                amount_scale,
                accounts: &accounts,
            },
        )
        .map(Into::into)
        .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("Trial balance import task failed to join: {error}"))?
}

#[tauri::command]
fn list_trial_balance_imports(
    engagement_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<TrialBalanceImportDto>, String> {
    validate_uuid(&engagement_id, "engagement")?;
    persistence::list_trial_balance_imports(database.path(), &engagement_id)
        .map(|records| records.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_trial_balance_accounts(
    trial_balance_import_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<TrialBalanceAccountDto>, String> {
    validate_uuid(&trial_balance_import_id, "trial-balance-import")?;
    persistence::list_trial_balance_accounts(database.path(), &trial_balance_import_id)
        .map(|records| records.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn compare_trial_balance_opening_closing(
    trial_balance_import_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<TrialBalanceComparisonDto, String> {
    validate_uuid(&trial_balance_import_id, "trial-balance-import")?;
    persistence::compare_trial_balance_opening_closing(database.path(), &trial_balance_import_id)
        .map(Into::into)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn run_trial_balance_opening_closing_reconciliation(
    trial_balance_import_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<ReconciliationRunDto, String> {
    validate_uuid(&trial_balance_import_id, "trial-balance-import")?;
    persistence::run_trial_balance_opening_closing_reconciliation(
        database.path(),
        &trial_balance_import_id,
    )
    .map(Into::into)
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_reconciliation_runs(
    engagement_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<ReconciliationRunDto>, String> {
    validate_uuid(&engagement_id, "engagement")?;
    persistence::list_reconciliation_runs(database.path(), &engagement_id)
        .map(|records| records.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_reconciliation_exceptions(
    reconciliation_run_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<ReconciliationExceptionDto>, String> {
    validate_uuid(&reconciliation_run_id, "reconciliation-run")?;
    persistence::list_reconciliation_exceptions(database.path(), &reconciliation_run_id)
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
            let evidence_state = context
                .app_handle()
                .state::<evidence::EvidenceState>()
                .inner()
                .clone();

            std::thread::spawn(move || {
                let response = if request.uri().path().starts_with("/image/") {
                    preview::image_preview_response(&database_path, request)
                } else if request.uri().path().starts_with("/controlled-pdf/") {
                    preview::controlled_pdf_preview_response(
                        &database_path,
                        &evidence_state,
                        request,
                    )
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
            create_normal_data_workspace,
            list_normal_data_workspaces,
            create_normal_data_dataset,
            list_normal_data_datasets,
            declare_normal_data_column_semantic,
            list_normal_data_column_semantics,
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
            create_statutory_compliance_requirement,
            list_statutory_compliance_requirements,
            create_statutory_compliance_assessment,
            list_statutory_compliance_assessments,
            list_statutory_compliance_evidence,
            create_internal_audit_process,
            list_internal_audit_processes,
            create_internal_audit_objective,
            list_internal_audit_objectives,
            create_internal_audit_risk,
            list_internal_audit_risks,
            create_internal_audit_control,
            list_internal_audit_controls,
            create_internal_audit_test,
            list_internal_audit_tests,
            create_internal_audit_test_evidence_link,
            list_internal_audit_test_evidence,
            create_internal_audit_finding,
            list_internal_audit_findings_for_engagement,
            list_internal_audit_findings_for_client,
            create_internal_audit_finding_followup,
            list_internal_audit_finding_followups,
            create_internal_audit_finding_evidence_link,
            list_internal_audit_finding_evidence,
            create_due_diligence_workspace,
            get_due_diligence_workspace_for_engagement,
            create_due_diligence_section,
            list_due_diligence_sections,
            create_due_diligence_request,
            list_due_diligence_requests,
            create_due_diligence_request_event,
            list_due_diligence_request_events,
            create_due_diligence_request_evidence_link,
            list_due_diligence_request_evidence,
            create_due_diligence_issue,
            list_due_diligence_issues,
            create_due_diligence_issue_event,
            list_due_diligence_issue_events,
            create_due_diligence_issue_evidence_link,
            list_due_diligence_issue_evidence,
            create_due_diligence_report,
            publish_due_diligence_report_version,
            list_due_diligence_reports,
            list_due_diligence_report_versions,
            list_due_diligence_report_version_issues,
            import_ledger_from_controlled_evidence,
            list_ledger_imports,
            list_ledger_account_summaries,
            create_ledger_tb_mapping,
            list_current_ledger_tb_mappings,
            create_financial_statement_schedule,
            list_financial_statement_schedules,
            create_trial_balance_schedule_mapping,
            list_current_trial_balance_schedule_mappings,
            create_financial_statement_schedule_link,
            list_current_financial_statement_schedule_links,
            list_ledger_test_runs,
            run_high_value_ledger_test,
            list_ledger_exceptions,
            import_trial_balance_from_controlled_evidence,
            list_trial_balance_imports,
            list_trial_balance_accounts,
            compare_trial_balance_opening_closing,
            run_trial_balance_opening_closing_reconciliation,
            list_reconciliation_runs,
            list_reconciliation_exceptions,
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
