use crate::filesystem;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
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
const LATEST_SCHEMA_VERSION: i64 = 19;
const FIRM_LIBRARY_DEFINITION_MAX_BYTES: usize = 262_144;
const RECONCILIATION_PARAMETERS_MAX_BYTES: usize = 65_536;

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
    Migration {
        version: 8,
        name: "pbc_requests",
        sql: include_str!("../migrations/0008_pbc_requests.sql"),
    },
    Migration {
        version: 9,
        name: "workpaper_signoffs",
        sql: include_str!("../migrations/0009_workpaper_signoffs.sql"),
    },
    Migration {
        version: 10,
        name: "engagement_templates",
        sql: include_str!("../migrations/0010_engagement_templates.sql"),
    },
    Migration {
        version: 11,
        name: "firm_library",
        sql: include_str!("../migrations/0011_firm_library.sql"),
    },
    Migration {
        version: 12,
        name: "ledger_scrutiny",
        sql: include_str!("../migrations/0012_ledger_scrutiny.sql"),
    },
    Migration {
        version: 13,
        name: "narrow_ledger_provenance",
        sql: include_str!("../migrations/0013_narrow_ledger_provenance.sql"),
    },
    Migration {
        version: 14,
        name: "trial_balance",
        sql: include_str!("../migrations/0014_trial_balance.sql"),
    },
    Migration {
        version: 15,
        name: "ledger_tb_mappings",
        sql: include_str!("../migrations/0015_ledger_tb_mappings.sql"),
    },
    Migration {
        version: 16,
        name: "tb_schedule_mappings",
        sql: include_str!("../migrations/0016_tb_schedule_mappings.sql"),
    },
    Migration {
        version: 17,
        name: "fs_schedule_links",
        sql: include_str!("../migrations/0017_fs_schedule_links.sql"),
    },
    Migration {
        version: 18,
        name: "reconciliation_framework",
        sql: include_str!("../migrations/0018_reconciliation_framework.sql"),
    },
    Migration {
        version: 19,
        name: "statutory_compliance",
        sql: include_str!("../migrations/0019_statutory_compliance.sql"),
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
pub struct ControlledEvidenceReadRecord {
    pub controlled_evidence_version_id: String,
    pub document_id: String,
    pub controlled_storage_locator: String,
    pub sha256: Vec<u8>,
    pub size_bytes: u64,
    pub verification_state: String,
    pub retention_state: String,
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
pub struct EngagementTemplateRecord {
    pub engagement_template_id: String,
    pub name: String,
    pub description: Option<String>,
    pub latest_version_id: String,
    pub latest_version_number: u64,
    pub service_type_id: String,
    pub source_engagement_id: Option<String>,
    pub created_at_ms: i64,
}

#[derive(Debug, Clone)]
pub struct FirmLibraryItemRecord {
    pub firm_library_item_id: String,
    pub category: String,
    pub name: String,
    pub description: Option<String>,
    pub service_type_id: Option<String>,
    pub latest_version_id: String,
    pub latest_version_number: u64,
    pub latest_definition_hash: Vec<u8>,
    pub created_at_ms: i64,
}

#[derive(Debug, Clone)]
pub struct FirmLibraryVersionRecord {
    pub firm_library_version_id: String,
    pub firm_library_item_id: String,
    pub version_number: u64,
    pub definition_json: String,
    pub definition_hash: Vec<u8>,
    pub created_at_ms: i64,
}

#[derive(Debug, Clone)]
pub struct StatutoryComplianceRequirementRecord {
    pub statutory_compliance_requirement_id: String,
    pub engagement_id: String,
    pub firm_library_item_id: String,
    pub firm_library_version_id: String,
    pub requirement_name: String,
    pub requirement_description: Option<String>,
    pub version_number: u64,
    pub definition_json: String,
    pub definition_hash: Vec<u8>,
    pub created_at_ms: i64,
}

#[derive(Debug, Clone)]
pub struct StatutoryComplianceAssessmentRecord {
    pub statutory_compliance_assessment_id: String,
    pub statutory_compliance_requirement_id: String,
    pub version_number: u64,
    pub supersedes_assessment_id: Option<String>,
    pub applicability: String,
    pub due_date: Option<String>,
    pub actual_compliance_date: Option<String>,
    pub status: String,
    pub exception_text: Option<String>,
    pub conclusion: Option<String>,
    pub evidence_count: u64,
    pub assessed_at_ms: i64,
}

#[derive(Debug, Clone)]
pub struct StatutoryComplianceEvidenceRecord {
    pub statutory_compliance_evidence_link_id: String,
    pub statutory_compliance_assessment_id: String,
    pub controlled_evidence_version_id: String,
    pub document_id: String,
    pub source_content_version_id: String,
    pub source_sha256: Vec<u8>,
    pub linked_at_ms: i64,
}

pub struct StatutoryComplianceAssessmentDefinition<'a> {
    pub statutory_compliance_requirement_id: &'a str,
    pub applicability: &'a str,
    pub due_date: Option<&'a str>,
    pub actual_compliance_date: Option<&'a str>,
    pub status: &'a str,
    pub exception_text: Option<&'a str>,
    pub conclusion: Option<&'a str>,
    pub controlled_evidence_version_ids: &'a [String],
}

#[derive(Debug, Clone)]
pub struct LedgerImportRecord {
    pub ledger_import_id: String,
    pub engagement_id: String,
    pub controlled_evidence_version_id: String,
    pub document_id: String,
    pub source_content_version_id: String,
    pub source_sha256: Vec<u8>,
    pub sheet_name: String,
    pub header_row_number: u64,
    pub amount_column: u32,
    pub date_column: Option<u32>,
    pub account_column: Option<u32>,
    pub voucher_column: Option<u32>,
    pub narration_column: Option<u32>,
    pub amount_scale: u32,
    pub transaction_count: u64,
    pub imported_at_ms: i64,
}

#[derive(Debug, Clone)]
pub struct LedgerTransactionInput {
    pub source_row_number: u64,
    pub source_row_hash: Vec<u8>,
    pub transaction_date_text: Option<String>,
    pub account_text: Option<String>,
    pub voucher_text: Option<String>,
    pub narration_text: Option<String>,
    pub amount_minor: i64,
}

pub struct LedgerImportDefinition<'a> {
    pub engagement_id: &'a str,
    pub controlled_evidence_version_id: &'a str,
    pub sheet_name: &'a str,
    pub header_row_number: u64,
    pub amount_column: u32,
    pub date_column: Option<u32>,
    pub account_column: Option<u32>,
    pub voucher_column: Option<u32>,
    pub narration_column: Option<u32>,
    pub amount_scale: u32,
    pub transactions: &'a [LedgerTransactionInput],
}

#[derive(Debug, Clone)]
pub struct LedgerTestRunRecord {
    pub ledger_test_run_id: String,
    pub ledger_import_id: String,
    pub test_type: String,
    pub threshold_minor: i64,
    pub exception_count: u64,
    pub ran_at_ms: i64,
}

#[derive(Debug, Clone)]
pub struct LedgerExceptionRecord {
    pub ledger_exception_id: String,
    pub ledger_test_run_id: String,
    pub ledger_transaction_id: String,
    pub exception_code: String,
    pub amount_minor: i64,
    pub transaction_date_text: Option<String>,
    pub account_text: Option<String>,
    pub voucher_text: Option<String>,
    pub narration_text: Option<String>,
    pub controlled_evidence_version_id: String,
    pub document_id: String,
    pub source_content_version_id: String,
    pub source_sha256: Vec<u8>,
    pub sheet_name: String,
    pub source_row_number: u64,
    pub source_row_hash: Vec<u8>,
    pub created_at_ms: i64,
}

#[derive(Debug, Clone)]
pub struct TrialBalanceImportRecord {
    pub trial_balance_import_id: String,
    pub engagement_id: String,
    pub controlled_evidence_version_id: String,
    pub document_id: String,
    pub source_content_version_id: String,
    pub source_sha256: Vec<u8>,
    pub sheet_name: String,
    pub header_row_number: u64,
    pub account_name_column: u32,
    pub account_code_column: Option<u32>,
    pub opening_balance_column: Option<u32>,
    pub closing_balance_column: u32,
    pub amount_scale: u32,
    pub account_count: u64,
    pub opening_total_minor: i64,
    pub closing_total_minor: i64,
    pub imported_at_ms: i64,
}

#[derive(Debug, Clone)]
pub struct TrialBalanceAccountInput {
    pub source_row_number: u64,
    pub source_row_hash: Vec<u8>,
    pub account_code_text: Option<String>,
    pub account_name_text: String,
    pub opening_minor: i64,
    pub closing_minor: i64,
}

pub struct TrialBalanceImportDefinition<'a> {
    pub engagement_id: &'a str,
    pub controlled_evidence_version_id: &'a str,
    pub sheet_name: &'a str,
    pub header_row_number: u64,
    pub account_name_column: u32,
    pub account_code_column: Option<u32>,
    pub opening_balance_column: Option<u32>,
    pub closing_balance_column: u32,
    pub amount_scale: u32,
    pub accounts: &'a [TrialBalanceAccountInput],
}

#[derive(Debug, Clone)]
pub struct TrialBalanceAccountRecord {
    pub trial_balance_account_id: String,
    pub trial_balance_import_id: String,
    pub source_row_number: u64,
    pub source_row_hash: Vec<u8>,
    pub account_code_text: Option<String>,
    pub account_name_text: String,
    pub opening_minor: i64,
    pub closing_minor: i64,
    pub created_at_ms: i64,
}

#[derive(Debug, Clone)]
pub struct TrialBalanceMovementRecord {
    pub trial_balance_account_id: String,
    pub source_row_number: u64,
    pub source_row_hash: Vec<u8>,
    pub account_code_text: Option<String>,
    pub account_name_text: String,
    pub opening_minor: i64,
    pub closing_minor: i64,
    pub movement_minor: i64,
}

#[derive(Debug, Clone)]
pub struct TrialBalanceComparisonRecord {
    pub trial_balance_import_id: String,
    pub opening_total_minor: i64,
    pub closing_total_minor: i64,
    pub net_movement_minor: i64,
    pub account_count: u64,
    pub movements: Vec<TrialBalanceMovementRecord>,
}

#[derive(Debug, Clone)]
pub struct LedgerAccountSummaryRecord {
    pub ledger_import_id: String,
    pub account_key: String,
    pub account_text: String,
    pub transaction_count: u64,
    pub total_minor: i64,
    pub first_source_row_number: u64,
    pub last_source_row_number: u64,
}

#[derive(Debug, Clone)]
pub struct LedgerTbMappingRecord {
    pub ledger_tb_mapping_id: String,
    pub ledger_import_id: String,
    pub trial_balance_import_id: String,
    pub ledger_account_key: String,
    pub ledger_account_text: String,
    pub trial_balance_account_id: String,
    pub trial_balance_account_code_text: Option<String>,
    pub trial_balance_account_name_text: String,
    pub trial_balance_source_row_number: u64,
    pub trial_balance_source_row_hash: Vec<u8>,
    pub version_number: u64,
    pub supersedes_mapping_id: Option<String>,
    pub mapped_at_ms: i64,
}

#[derive(Debug, Clone)]
pub struct FinancialStatementScheduleRecord {
    pub financial_statement_schedule_id: String,
    pub engagement_id: String,
    pub reference: String,
    pub name: String,
    pub created_at_ms: i64,
}

#[derive(Debug, Clone)]
pub struct TrialBalanceScheduleMappingRecord {
    pub trial_balance_schedule_mapping_id: String,
    pub trial_balance_import_id: String,
    pub trial_balance_account_id: String,
    pub trial_balance_account_code_text: Option<String>,
    pub trial_balance_account_name_text: String,
    pub trial_balance_source_row_number: u64,
    pub trial_balance_source_row_hash: Vec<u8>,
    pub financial_statement_schedule_id: String,
    pub schedule_reference: String,
    pub schedule_name: String,
    pub version_number: u64,
    pub supersedes_mapping_id: Option<String>,
    pub mapped_at_ms: i64,
}

#[derive(Debug, Clone)]
pub struct FinancialStatementScheduleLinkRecord {
    pub financial_statement_schedule_link_id: String,
    pub financial_statement_schedule_id: String,
    pub schedule_reference: String,
    pub schedule_name: String,
    pub controlled_evidence_version_id: String,
    pub document_id: String,
    pub source_content_version_id: String,
    pub source_sha256: Vec<u8>,
    pub location_kind: String,
    pub location_value: String,
    pub version_number: u64,
    pub supersedes_link_id: Option<String>,
    pub linked_at_ms: i64,
}

#[derive(Debug, Clone)]
pub struct ReconciliationItemInput {
    pub match_key: String,
    pub amount_minor: i64,
    pub event_date_text: Option<String>,
    pub description_text: Option<String>,
    pub source_kind: String,
    pub source_entity_id: String,
    pub controlled_evidence_version_id: String,
    pub document_id: String,
    pub source_content_version_id: String,
    pub source_sha256: Vec<u8>,
    pub sheet_name: Option<String>,
    pub source_row_number: Option<u64>,
    pub source_row_hash: Option<Vec<u8>>,
}

pub struct ReconciliationRunDefinition<'a> {
    pub engagement_id: &'a str,
    pub reconciliation_type: &'a str,
    pub title: &'a str,
    pub parameters_json: &'a str,
    pub left_items: &'a [ReconciliationItemInput],
    pub right_items: &'a [ReconciliationItemInput],
}

#[derive(Debug, Clone)]
pub struct ReconciliationRunRecord {
    pub reconciliation_run_id: String,
    pub engagement_id: String,
    pub reconciliation_type: String,
    pub title: String,
    pub rule_code: String,
    pub parameters_json: String,
    pub left_item_count: u64,
    pub right_item_count: u64,
    pub matched_pair_count: u64,
    pub exception_count: u64,
    pub ran_at_ms: i64,
}

#[derive(Debug, Clone)]
pub struct ReconciliationExceptionRecord {
    pub reconciliation_exception_id: String,
    pub reconciliation_run_id: String,
    pub reconciliation_item_id: String,
    pub exception_code: String,
    pub side: String,
    pub match_key: String,
    pub amount_minor: i64,
    pub event_date_text: Option<String>,
    pub description_text: Option<String>,
    pub source_kind: String,
    pub source_entity_id: String,
    pub controlled_evidence_version_id: String,
    pub document_id: String,
    pub source_content_version_id: String,
    pub source_sha256: Vec<u8>,
    pub sheet_name: Option<String>,
    pub source_row_number: Option<u64>,
    pub source_row_hash: Option<Vec<u8>>,
    pub created_at_ms: i64,
}

struct TrialBalanceMappingTarget {
    trial_balance_import_id: String,
    account_code_text: Option<String>,
    account_name_text: String,
    source_row_number: i64,
    source_row_hash: Vec<u8>,
}

struct TrialBalanceReconciliationSource {
    engagement_id: String,
    controlled_evidence_version_id: String,
    document_id: String,
    source_content_version_id: String,
    source_sha256: Vec<u8>,
    sheet_name: String,
    amount_scale: u32,
}

struct FirmLibraryItemIdentity {
    category: String,
    name: String,
    description: Option<String>,
    service_type_id: Option<String>,
    created_at_ms: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct EngagementTemplateDefinition {
    areas: Vec<EngagementTemplateAreaDefinition>,
    procedures: Vec<EngagementTemplateProcedureDefinition>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct EngagementTemplateAreaDefinition {
    source_area_id: String,
    parent_source_area_id: Option<String>,
    name: String,
    code: Option<String>,
    display_order: i64,
    status: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct EngagementTemplateProcedureDefinition {
    source_area_id: Option<String>,
    reference: Option<String>,
    title: String,
    description: Option<String>,
    status: String,
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
pub struct WorkpaperSignoffRecord {
    pub signoff_id: String,
    pub workpaper_id: String,
    pub workpaper_revision_id: String,
    pub revision_number: u64,
    pub signoff_type: String,
    pub actor_id: String,
    pub actor_role: String,
    pub signed_at_ms: i64,
    pub comment: Option<String>,
    pub evidence_link_ids: Vec<String>,
    pub superseded_at_ms: Option<i64>,
    pub superseded_reason: Option<String>,
    pub superseded_by_revision_id: Option<String>,
}

pub struct NewWorkpaperSignoff<'a> {
    pub workpaper_id: &'a str,
    pub workpaper_revision_id: &'a str,
    pub signoff_type: &'a str,
    pub actor_id: &'a str,
    pub actor_role: &'a str,
    pub comment: Option<&'a str>,
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

#[derive(Debug, Clone)]
pub struct PbcRequestRecord {
    pub pbc_request_id: String,
    pub engagement_id: String,
    pub engagement_area_id: Option<String>,
    pub request_number: String,
    pub description: String,
    pub requested_from_party: String,
    pub due_at_ms: Option<i64>,
    pub status: String,
    pub client_visible_content: Option<String>,
    pub internal_notes: Option<String>,
    pub latest_assessment: Option<String>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Debug, Clone)]
pub struct PbcRequestEventRecord {
    pub pbc_request_event_id: String,
    pub pbc_request_id: String,
    pub event_type: String,
    pub actor_id: Option<String>,
    pub from_status: Option<String>,
    pub to_status: Option<String>,
    pub assessment_text: Option<String>,
    pub comment: Option<String>,
    pub occurred_at_ms: i64,
}

#[derive(Debug, Clone)]
pub struct PbcRequestEvidenceLinkRecord {
    pub pbc_request_evidence_link_id: String,
    pub pbc_request_id: String,
    pub document_id: String,
    pub document_name: String,
    pub content_version_id: Option<String>,
    pub content_observed_at_ms: Option<i64>,
    pub controlled_evidence_version_id: Option<String>,
    pub controlled_version_number: Option<u64>,
    pub controlled_captured_at_ms: Option<i64>,
    pub description: Option<String>,
    pub created_at_ms: i64,
}

pub struct NewPbcRequest<'a> {
    pub engagement_id: &'a str,
    pub engagement_area_id: Option<&'a str>,
    pub request_number: &'a str,
    pub description: &'a str,
    pub requested_from_party: &'a str,
    pub due_at_ms: Option<i64>,
    pub status: &'a str,
    pub client_visible_content: Option<&'a str>,
    pub internal_notes: Option<&'a str>,
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

pub fn get_controlled_evidence_read_record(
    database_path: &Path,
    controlled_evidence_version_id: &str,
) -> Result<Option<ControlledEvidenceReadRecord>, PersistenceError> {
    let connection = open_configured_connection(database_path)?;
    connection
        .query_row(
            "SELECT
                cev.controlled_evidence_version_id,
                cev.document_id,
                cev.controlled_storage_locator,
                cev.sha256,
                cev.size_bytes,
                cev.verification_state,
                cev.retention_state
             FROM controlled_evidence_versions cev
             JOIN documents d ON d.document_id = cev.document_id
             WHERE cev.controlled_evidence_version_id = ?1
               AND d.archived_at_ms IS NULL",
            [controlled_evidence_version_id],
            |row| {
                Ok(ControlledEvidenceReadRecord {
                    controlled_evidence_version_id: row.get(0)?,
                    document_id: row.get(1)?,
                    controlled_storage_locator: row.get(2)?,
                    sha256: row.get(3)?,
                    size_bytes: row.get::<_, i64>(4)?.max(0) as u64,
                    verification_state: row.get(5)?,
                    retention_state: row.get(6)?,
                })
            },
        )
        .optional()
        .map_err(PersistenceError::from)
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

fn normalize_ledger_account_key(value: &str) -> Result<String, PersistenceError> {
    let normalized = value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    if normalized.is_empty() || normalized.chars().count() > 500 {
        return Err(PersistenceError::Configuration(
            "ledger account key must contain 1 to 500 characters".to_string(),
        ));
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

fn normalize_firm_library_category(value: &str) -> Result<String, PersistenceError> {
    let category = workflow_state_key(value);
    if matches!(
        category.as_str(),
        "CHECKLIST"
            | "AUDIT_QUERY"
            | "RISK_TEMPLATE"
            | "CONTROL_TEMPLATE"
            | "LEDGER_SCRUTINY_TEST"
            | "REPORT_TEMPLATE"
            | "MANAGEMENT_LETTER_POINT"
            | "STATUTORY_COMPLIANCE_REQUIREMENT"
    ) {
        Ok(category)
    } else {
        Err(PersistenceError::Configuration(
            "firm library category is not supported".to_string(),
        ))
    }
}

fn normalize_firm_library_description(
    value: Option<&str>,
) -> Result<Option<String>, PersistenceError> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    if value.chars().count() > 2_000 {
        return Err(PersistenceError::Configuration(
            "firm library description must contain at most 2000 characters".to_string(),
        ));
    }
    Ok(Some(value.to_string()))
}

fn normalize_firm_library_definition(
    definition_json: &str,
) -> Result<(String, Vec<u8>), PersistenceError> {
    if definition_json.len() > FIRM_LIBRARY_DEFINITION_MAX_BYTES {
        return Err(PersistenceError::Configuration(format!(
            "firm library definition must be at most {FIRM_LIBRARY_DEFINITION_MAX_BYTES} bytes"
        )));
    }

    let definition: serde_json::Value = serde_json::from_str(definition_json).map_err(|error| {
        PersistenceError::Configuration(format!(
            "firm library definition must be valid JSON: {error}"
        ))
    })?;
    if !definition.is_object() {
        return Err(PersistenceError::Configuration(
            "firm library definition must be a JSON object".to_string(),
        ));
    }

    let canonical = serde_json::to_string(&definition).map_err(|error| {
        PersistenceError::Configuration(format!(
            "firm library definition could not be serialized: {error}"
        ))
    })?;
    let hash = Sha256::digest(canonical.as_bytes()).to_vec();
    Ok((canonical, hash))
}

fn normalize_statutory_compliance_state(
    value: &str,
    field_name: &str,
    allowed: &[&str],
) -> Result<String, PersistenceError> {
    let normalized = workflow_state_key(value);
    if allowed.iter().any(|candidate| *candidate == normalized) {
        Ok(normalized)
    } else {
        Err(PersistenceError::Configuration(format!(
            "{field_name} is not supported"
        )))
    }
}

fn normalize_statutory_compliance_text(
    value: Option<&str>,
    field_name: &str,
    max_chars: usize,
) -> Result<Option<String>, PersistenceError> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let count = value.chars().count();
    if count > max_chars
        || value.chars().any(|character| {
            character.is_control() && !matches!(character, '\n' | '\r' | '\t')
        })
    {
        return Err(PersistenceError::Configuration(format!(
            "{field_name} must contain at most {max_chars} printable characters"
        )));
    }
    Ok(Some(value.to_string()))
}

fn normalize_statutory_compliance_date(
    value: Option<&str>,
    field_name: &str,
) -> Result<Option<String>, PersistenceError> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let bytes = value.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || !bytes[..4].iter().all(u8::is_ascii_digit)
        || !bytes[5..7].iter().all(u8::is_ascii_digit)
        || !bytes[8..].iter().all(u8::is_ascii_digit)
    {
        return Err(PersistenceError::Configuration(format!(
            "{field_name} must use YYYY-MM-DD format"
        )));
    }

    let year = value[..4].parse::<u32>().map_err(|_| {
        PersistenceError::Configuration(format!("{field_name} year is invalid"))
    })?;
    let month = value[5..7].parse::<u32>().map_err(|_| {
        PersistenceError::Configuration(format!("{field_name} month is invalid"))
    })?;
    let day = value[8..].parse::<u32>().map_err(|_| {
        PersistenceError::Configuration(format!("{field_name} day is invalid"))
    })?;
    if year == 0 || !(1..=12).contains(&month) {
        return Err(PersistenceError::Configuration(format!(
            "{field_name} is not a valid calendar date"
        )));
    }

    let leap_year = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let max_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap_year => 29,
        2 => 28,
        _ => unreachable!("month range is validated above"),
    };
    if day == 0 || day > max_day {
        return Err(PersistenceError::Configuration(format!(
            "{field_name} is not a valid calendar date"
        )));
    }

    Ok(Some(value.to_string()))
}

fn normalize_statutory_compliance_assessment(
    applicability: &str,
    due_date: Option<&str>,
    actual_compliance_date: Option<&str>,
    status: &str,
    exception_text: Option<&str>,
    conclusion: Option<&str>,
    controlled_evidence_version_ids: &[String],
) -> Result<
    (
        String,
        Option<String>,
        Option<String>,
        String,
        Option<String>,
        Option<String>,
    ),
    PersistenceError,
> {
    let applicability = normalize_statutory_compliance_state(
        applicability,
        "statutory compliance applicability",
        &["UNDETERMINED", "APPLICABLE", "NOT_APPLICABLE"],
    )?;
    let status = normalize_statutory_compliance_state(
        status,
        "statutory compliance status",
        &[
            "UNASSESSED",
            "PENDING",
            "COMPLIANT",
            "EXCEPTION",
            "NOT_APPLICABLE",
        ],
    )?;
    let due_date = normalize_statutory_compliance_date(due_date, "statutory compliance due date")?;
    let actual_compliance_date = normalize_statutory_compliance_date(
        actual_compliance_date,
        "statutory compliance actual compliance date",
    )?;
    let exception_text = normalize_statutory_compliance_text(
        exception_text,
        "statutory compliance exception",
        10_000,
    )?;
    let conclusion = normalize_statutory_compliance_text(
        conclusion,
        "statutory compliance conclusion",
        10_000,
    )?;

    let mut evidence_ids = controlled_evidence_version_ids.to_vec();
    evidence_ids.sort();
    evidence_ids.dedup();
    if evidence_ids.len() != controlled_evidence_version_ids.len() {
        return Err(PersistenceError::Configuration(
            "statutory compliance evidence versions must be unique".to_string(),
        ));
    }

    match status.as_str() {
        "UNASSESSED" => {
            if applicability != "UNDETERMINED"
                || due_date.is_some()
                || actual_compliance_date.is_some()
                || exception_text.is_some()
                || conclusion.is_some()
                || !controlled_evidence_version_ids.is_empty()
            {
                return Err(PersistenceError::Configuration(
                    "UNASSESSED compliance must remain undetermined without dates, evidence, exception, or conclusion"
                        .to_string(),
                ));
            }
        }
        "PENDING" => {
            if applicability != "APPLICABLE"
                || actual_compliance_date.is_some()
                || exception_text.is_some()
            {
                return Err(PersistenceError::Configuration(
                    "PENDING compliance must be applicable, have no actual compliance date, and have no exception"
                        .to_string(),
                ));
            }
        }
        "COMPLIANT" => {
            if applicability != "APPLICABLE"
                || actual_compliance_date.is_none()
                || exception_text.is_some()
                || conclusion.is_none()
                || controlled_evidence_version_ids.is_empty()
            {
                return Err(PersistenceError::Configuration(
                    "COMPLIANT status requires applicable state, actual compliance date, conclusion, exact controlled evidence, and no exception"
                        .to_string(),
                ));
            }
            if let (Some(due_date), Some(actual_date)) =
                (due_date.as_deref(), actual_compliance_date.as_deref())
            {
                if actual_date > due_date {
                    return Err(PersistenceError::Configuration(
                        "late statutory compliance must be recorded as an EXCEPTION rather than COMPLIANT"
                            .to_string(),
                    ));
                }
            }
        }
        "EXCEPTION" => {
            if applicability != "APPLICABLE"
                || exception_text.is_none()
                || conclusion.is_none()
            {
                return Err(PersistenceError::Configuration(
                    "EXCEPTION status requires applicable state, exception text, and conclusion"
                        .to_string(),
                ));
            }
        }
        "NOT_APPLICABLE" => {
            if applicability != "NOT_APPLICABLE"
                || due_date.is_some()
                || actual_compliance_date.is_some()
                || exception_text.is_some()
                || conclusion.is_none()
                || !controlled_evidence_version_ids.is_empty()
            {
                return Err(PersistenceError::Configuration(
                    "NOT_APPLICABLE status requires a conclusion and cannot carry dates, exception text, or evidence"
                        .to_string(),
                ));
            }
        }
        _ => unreachable!("status allow-list is validated above"),
    }

    Ok((
        applicability,
        due_date,
        actual_compliance_date,
        status,
        exception_text,
        conclusion,
    ))
}

fn normalize_reconciliation_parameters(parameters_json: &str) -> Result<String, PersistenceError> {
    if parameters_json.len() > RECONCILIATION_PARAMETERS_MAX_BYTES {
        return Err(PersistenceError::Configuration(format!(
            "reconciliation parameters must be at most {RECONCILIATION_PARAMETERS_MAX_BYTES} bytes"
        )));
    }

    let parameters: serde_json::Value = serde_json::from_str(parameters_json).map_err(|error| {
        PersistenceError::Configuration(format!(
            "reconciliation parameters must be valid JSON: {error}"
        ))
    })?;
    if !parameters.is_object() {
        return Err(PersistenceError::Configuration(
            "reconciliation parameters must be a JSON object".to_string(),
        ));
    }

    serde_json::to_string(&parameters).map_err(|error| {
        PersistenceError::Configuration(format!(
            "reconciliation parameters could not be serialized: {error}"
        ))
    })
}

fn normalize_reconciliation_source_identifier(value: &str) -> Result<String, PersistenceError> {
    let normalized = value.trim();
    let count = normalized.chars().count();
    if count == 0 || count > 240 || normalized.chars().any(|character| character.is_control()) {
        return Err(PersistenceError::Configuration(
            "reconciliation source identifier must contain 1 to 240 printable characters"
                .to_string(),
        ));
    }
    Ok(normalized.to_string())
}

fn normalize_reconciliation_match_key(value: &str) -> Result<String, PersistenceError> {
    let normalized = value.trim();
    let count = normalized.chars().count();
    if count == 0 || count > 500 || normalized.chars().any(|character| character.is_control()) {
        return Err(PersistenceError::Configuration(
            "reconciliation match key must contain 1 to 500 printable characters".to_string(),
        ));
    }
    Ok(normalized.to_string())
}

fn normalize_reconciliation_sheet_name(
    value: Option<&str>,
) -> Result<Option<String>, PersistenceError> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let count = value.chars().count();
    if count > 255 || value.chars().any(|character| character.is_control()) {
        return Err(PersistenceError::Configuration(
            "reconciliation worksheet must contain at most 255 printable characters".to_string(),
        ));
    }
    Ok(Some(value.to_string()))
}

fn normalize_review_note_worksheet(value: &str) -> Result<String, PersistenceError> {
    let normalized = value.trim();
    let count = normalized.chars().count();
    if count == 0 || count > 255 || normalized.chars().any(|character| character.is_control()) {
        return Err(PersistenceError::Configuration(
            "review note worksheet must contain 1 to 255 printable characters".to_string(),
        ));
    }
    Ok(normalized.to_string())
}

fn is_a1_cell_reference(value: &str) -> bool {
    let bytes = value.trim().as_bytes();
    let letter_count = bytes
        .iter()
        .take_while(|byte| byte.is_ascii_alphabetic())
        .count();
    if !(1..=3).contains(&letter_count) || letter_count == bytes.len() {
        return false;
    }

    let mut column = 0u32;
    for byte in &bytes[..letter_count] {
        column = column
            .saturating_mul(26)
            .saturating_add(u32::from(byte.to_ascii_uppercase() - b'A' + 1));
    }
    if column == 0 || column > 16_384 {
        return false;
    }

    let row_bytes = &bytes[letter_count..];
    if row_bytes.is_empty() || !row_bytes.iter().all(|byte| byte.is_ascii_digit()) {
        return false;
    }
    let Ok(row_text) = std::str::from_utf8(row_bytes) else {
        return false;
    };
    let Ok(row) = row_text.parse::<u32>() else {
        return false;
    };
    (1..=1_048_576).contains(&row)
}

fn normalize_review_note_workbook_anchor(
    location_kind: &str,
    value: &str,
    expects_range: bool,
) -> Result<String, PersistenceError> {
    let (worksheet, address) = value.rsplit_once('!').ok_or_else(|| {
        let expected_format = if expects_range {
            "Worksheet!B12:D20"
        } else {
            "Worksheet!B12"
        };
        PersistenceError::Configuration(format!(
            "review note {location_kind} location must use {expected_format} format"
        ))
    })?;
    let worksheet = normalize_review_note_worksheet(worksheet)?;

    if expects_range {
        let Some((start, end)) = address.split_once(':') else {
            return Err(PersistenceError::Configuration(
                "review note RANGE location must use Worksheet!B12:D20 format".to_string(),
            ));
        };
        let start = start.trim();
        let end = end.trim();
        if end.contains(':') || !is_a1_cell_reference(start) || !is_a1_cell_reference(end) {
            return Err(PersistenceError::Configuration(
                "review note RANGE location must use Worksheet!B12:D20 format".to_string(),
            ));
        }
        Ok(format!(
            "{worksheet}!{}:{}",
            start.to_ascii_uppercase(),
            end.to_ascii_uppercase()
        ))
    } else {
        let address = address.trim();
        if address.contains(':') || !is_a1_cell_reference(address) {
            return Err(PersistenceError::Configuration(
                "review note CELL location must use Worksheet!B12 format".to_string(),
            ));
        }
        Ok(format!("{worksheet}!{}", address.to_ascii_uppercase()))
    }
}

fn normalize_review_note_location(
    evidence_link_id: Option<&str>,
    location_kind: Option<&str>,
    location_value: Option<&str>,
) -> Result<(Option<String>, Option<String>), PersistenceError> {
    let location_kind =
        normalize_optional_domain_text(location_kind, 80).map(|value| value.to_ascii_uppercase());
    let location_value = location_value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    if location_kind.is_some() != location_value.is_some() {
        return Err(PersistenceError::Configuration(
            "review note location kind and value must be supplied together".to_string(),
        ));
    }
    if location_kind.is_none() {
        return Ok((None, None));
    }
    if evidence_link_id.is_none() {
        return Err(PersistenceError::Configuration(
            "review note page/worksheet/cell/range location requires an exact evidence link"
                .to_string(),
        ));
    }

    let kind = location_kind.expect("location kind must exist after paired validation");
    let value = location_value.expect("location value must exist after paired validation");
    let normalized_value = match kind.as_str() {
        "PAGE" => {
            let page = value.parse::<u32>().map_err(|_| {
                PersistenceError::Configuration(
                    "review note PAGE location must be a positive page number".to_string(),
                )
            })?;
            if page == 0 {
                return Err(PersistenceError::Configuration(
                    "review note PAGE location must be a positive page number".to_string(),
                ));
            }
            page.to_string()
        }
        "WORKSHEET" => normalize_review_note_worksheet(&value)?,
        "CELL" => normalize_review_note_workbook_anchor("CELL", &value, false)?,
        "RANGE" => normalize_review_note_workbook_anchor("RANGE", &value, true)?,
        _ => {
            return Err(PersistenceError::Configuration(
                "review note location kind must be PAGE, WORKSHEET, CELL, or RANGE".to_string(),
            ))
        }
    };

    Ok((Some(kind), Some(normalized_value)))
}

fn normalize_exact_evidence_worksheet(
    value: &str,
    subject: &str,
) -> Result<String, PersistenceError> {
    let normalized = value.trim();
    let count = normalized.chars().count();
    if count == 0 || count > 255 || normalized.chars().any(|character| character.is_control()) {
        return Err(PersistenceError::Configuration(format!(
            "{subject} worksheet must contain 1 to 255 printable characters"
        )));
    }
    Ok(normalized.to_string())
}

fn normalize_exact_evidence_workbook_anchor(
    subject: &str,
    location_kind: &str,
    value: &str,
    expects_range: bool,
) -> Result<String, PersistenceError> {
    let (worksheet, address) = value.rsplit_once('!').ok_or_else(|| {
        let expected_format = if expects_range {
            "Worksheet!B12:D20"
        } else {
            "Worksheet!B12"
        };
        PersistenceError::Configuration(format!(
            "{subject} {location_kind} location must use {expected_format} format"
        ))
    })?;
    let worksheet = normalize_exact_evidence_worksheet(worksheet, subject)?;

    if expects_range {
        let Some((start, end)) = address.split_once(':') else {
            return Err(PersistenceError::Configuration(format!(
                "{subject} RANGE location must use Worksheet!B12:D20 format"
            )));
        };
        let start = start.trim();
        let end = end.trim();
        if end.contains(':') || !is_a1_cell_reference(start) || !is_a1_cell_reference(end) {
            return Err(PersistenceError::Configuration(format!(
                "{subject} RANGE location must use Worksheet!B12:D20 format"
            )));
        }
        Ok(format!(
            "{worksheet}!{}:{}",
            start.to_ascii_uppercase(),
            end.to_ascii_uppercase()
        ))
    } else {
        let address = address.trim();
        if address.contains(':') || !is_a1_cell_reference(address) {
            return Err(PersistenceError::Configuration(format!(
                "{subject} CELL location must use Worksheet!B12 format"
            )));
        }
        Ok(format!("{worksheet}!{}", address.to_ascii_uppercase()))
    }
}

fn normalize_exact_evidence_location(
    location_kind: &str,
    location_value: &str,
    subject: &str,
) -> Result<(String, String), PersistenceError> {
    let kind = location_kind.trim().to_ascii_uppercase();
    let value = location_value.trim();
    if kind.is_empty() || value.is_empty() {
        return Err(PersistenceError::Configuration(format!(
            "{subject} location kind and value are required"
        )));
    }

    let normalized_value = match kind.as_str() {
        "PAGE" => {
            let page = value.parse::<u32>().map_err(|_| {
                PersistenceError::Configuration(format!(
                    "{subject} PAGE location must be a positive page number"
                ))
            })?;
            if page == 0 {
                return Err(PersistenceError::Configuration(format!(
                    "{subject} PAGE location must be a positive page number"
                )));
            }
            page.to_string()
        }
        "WORKSHEET" => normalize_exact_evidence_worksheet(value, subject)?,
        "CELL" => normalize_exact_evidence_workbook_anchor(subject, "CELL", value, false)?,
        "RANGE" => normalize_exact_evidence_workbook_anchor(subject, "RANGE", value, true)?,
        _ => {
            return Err(PersistenceError::Configuration(format!(
                "{subject} location kind must be PAGE, WORKSHEET, CELL, or RANGE"
            )))
        }
    };

    Ok((kind, normalized_value))
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

fn capture_engagement_template_definition(
    transaction: &rusqlite::Transaction<'_>,
    source_engagement_id: &str,
) -> Result<(String, String), PersistenceError> {
    let service_type_id: Option<String> = transaction
        .query_row(
            "SELECT service_type_id
             FROM engagements
             WHERE engagement_id = ?1
               AND archived_at_ms IS NULL",
            [source_engagement_id],
            |row| row.get(0),
        )
        .optional()?;
    let service_type_id = service_type_id.ok_or_else(|| {
        PersistenceError::Configuration(format!(
            "source engagement {source_engagement_id} does not exist"
        ))
    })?;

    let areas = {
        let mut statement = transaction.prepare(
            "SELECT
                engagement_area_id,
                parent_area_id,
                name,
                code,
                display_order,
                status
             FROM engagement_areas
             WHERE engagement_id = ?1
               AND archived_at_ms IS NULL
             ORDER BY display_order, name COLLATE NOCASE, engagement_area_id",
        )?;
        let rows = statement.query_map([source_engagement_id], |row| {
            Ok(EngagementTemplateAreaDefinition {
                source_area_id: row.get(0)?,
                parent_source_area_id: row.get(1)?,
                name: row.get(2)?,
                code: row.get(3)?,
                display_order: row.get(4)?,
                status: row.get(5)?,
            })
        })?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        result
    };

    let procedures = {
        let mut statement = transaction.prepare(
            "SELECT
                engagement_area_id,
                reference,
                title,
                description,
                status
             FROM procedures
             WHERE engagement_id = ?1
               AND archived_at_ms IS NULL
             ORDER BY created_at_ms, procedure_id",
        )?;
        let rows = statement.query_map([source_engagement_id], |row| {
            Ok(EngagementTemplateProcedureDefinition {
                source_area_id: row.get(0)?,
                reference: row.get(1)?,
                title: row.get(2)?,
                description: row.get(3)?,
                status: row.get(4)?,
            })
        })?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        result
    };

    for procedure in &procedures {
        if let Some(source_area_id) = procedure.source_area_id.as_deref() {
            if !areas
                .iter()
                .any(|area| area.source_area_id == source_area_id)
            {
                return Err(PersistenceError::Configuration(
                    "active template procedure references an unavailable engagement area"
                        .to_string(),
                ));
            }
        }
    }

    let definition = EngagementTemplateDefinition { areas, procedures };
    let definition_json = serde_json::to_string(&definition).map_err(|error| {
        PersistenceError::Configuration(format!(
            "engagement template definition could not be serialized: {error}"
        ))
    })?;

    Ok((service_type_id, definition_json))
}

pub fn create_engagement_template_from_engagement(
    database_path: &Path,
    source_engagement_id: &str,
    name: &str,
    description: Option<&str>,
) -> Result<EngagementTemplateRecord, PersistenceError> {
    let name = normalize_domain_label(name, "engagement template name", 200)?;
    let normalized_name = normalize_search_text(&name);
    if normalized_name.is_empty() {
        return Err(PersistenceError::Configuration(
            "engagement template name must contain searchable characters".to_string(),
        ));
    }
    let description = description
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    let mut connection = open_configured_connection(database_path)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

    let (service_type_id, definition_json) =
        capture_engagement_template_definition(&transaction, source_engagement_id)?;

    let engagement_template_id = Uuid::new_v4().to_string();
    let engagement_template_version_id = Uuid::new_v4().to_string();
    let now = now_unix_ms()?;
    transaction.execute(
        "INSERT INTO engagement_templates (
            engagement_template_id,
            name,
            normalized_name,
            description,
            created_at_ms,
            archived_at_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, NULL)",
        params![
            &engagement_template_id,
            &name,
            &normalized_name,
            description.as_deref(),
            now
        ],
    )?;
    transaction.execute(
        "INSERT INTO engagement_template_versions (
            engagement_template_version_id,
            engagement_template_id,
            version_number,
            source_engagement_id,
            service_type_id,
            definition_json,
            created_at_ms
         ) VALUES (?1, ?2, 1, ?3, ?4, ?5, ?6)",
        params![
            &engagement_template_version_id,
            &engagement_template_id,
            source_engagement_id,
            &service_type_id,
            &definition_json,
            now
        ],
    )?;

    transaction.commit()?;
    Ok(EngagementTemplateRecord {
        engagement_template_id,
        name,
        description,
        latest_version_id: engagement_template_version_id,
        latest_version_number: 1,
        service_type_id,
        source_engagement_id: Some(source_engagement_id.to_string()),
        created_at_ms: now,
    })
}

pub fn create_engagement_template_version_from_engagement(
    database_path: &Path,
    engagement_template_id: &str,
    source_engagement_id: &str,
) -> Result<EngagementTemplateRecord, PersistenceError> {
    let mut connection = open_configured_connection(database_path)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

    let template: Option<(String, Option<String>, i64, String)> = transaction
        .query_row(
            "SELECT
                t.name,
                t.description,
                t.created_at_ms,
                v.service_type_id
             FROM engagement_templates t
             JOIN engagement_template_versions v
               ON v.engagement_template_id = t.engagement_template_id
              AND v.version_number = (
                  SELECT MAX(v2.version_number)
                  FROM engagement_template_versions v2
                  WHERE v2.engagement_template_id = t.engagement_template_id
              )
             WHERE t.engagement_template_id = ?1
               AND t.archived_at_ms IS NULL",
            [engagement_template_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()?;
    let (name, description, template_created_at_ms, expected_service_type_id) = template
        .ok_or_else(|| {
            PersistenceError::Configuration(format!(
                "engagement template {engagement_template_id} does not exist"
            ))
        })?;

    let (service_type_id, definition_json) =
        capture_engagement_template_definition(&transaction, source_engagement_id)?;
    if service_type_id != expected_service_type_id {
        return Err(PersistenceError::Configuration(
            "engagement template update must use the template service type".to_string(),
        ));
    }

    let next_version_number: i64 = transaction.query_row(
        "SELECT COALESCE(MAX(version_number), 0) + 1
         FROM engagement_template_versions
         WHERE engagement_template_id = ?1",
        [engagement_template_id],
        |row| row.get(0),
    )?;
    if next_version_number < 1 {
        return Err(PersistenceError::Configuration(
            "engagement template version sequence is invalid".to_string(),
        ));
    }

    let engagement_template_version_id = Uuid::new_v4().to_string();
    let now = now_unix_ms()?;
    transaction.execute(
        "INSERT INTO engagement_template_versions (
            engagement_template_version_id,
            engagement_template_id,
            version_number,
            source_engagement_id,
            service_type_id,
            definition_json,
            created_at_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            &engagement_template_version_id,
            engagement_template_id,
            next_version_number,
            source_engagement_id,
            &service_type_id,
            &definition_json,
            now
        ],
    )?;

    transaction.commit()?;
    Ok(EngagementTemplateRecord {
        engagement_template_id: engagement_template_id.to_string(),
        name,
        description,
        latest_version_id: engagement_template_version_id,
        latest_version_number: next_version_number as u64,
        service_type_id,
        source_engagement_id: Some(source_engagement_id.to_string()),
        created_at_ms: template_created_at_ms,
    })
}

pub fn list_engagement_templates(
    database_path: &Path,
) -> Result<Vec<EngagementTemplateRecord>, PersistenceError> {
    let connection = open_configured_connection(database_path)?;
    let mut statement = connection.prepare(
        "SELECT
            t.engagement_template_id,
            t.name,
            t.description,
            v.engagement_template_version_id,
            v.version_number,
            v.service_type_id,
            v.source_engagement_id,
            t.created_at_ms
         FROM engagement_templates t
         JOIN engagement_template_versions v
           ON v.engagement_template_id = t.engagement_template_id
          AND v.version_number = (
              SELECT MAX(v2.version_number)
              FROM engagement_template_versions v2
              WHERE v2.engagement_template_id = t.engagement_template_id
          )
         WHERE t.archived_at_ms IS NULL
         ORDER BY t.name COLLATE NOCASE, t.created_at_ms",
    )?;
    let rows = statement.query_map([], |row| {
        let version_number: i64 = row.get(4)?;
        Ok(EngagementTemplateRecord {
            engagement_template_id: row.get(0)?,
            name: row.get(1)?,
            description: row.get(2)?,
            latest_version_id: row.get(3)?,
            latest_version_number: version_number.max(0) as u64,
            service_type_id: row.get(5)?,
            source_engagement_id: row.get(6)?,
            created_at_ms: row.get(7)?,
        })
    })?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

pub fn create_firm_library_item(
    database_path: &Path,
    category: &str,
    name: &str,
    description: Option<&str>,
    service_type_id: Option<&str>,
    definition_json: &str,
) -> Result<FirmLibraryItemRecord, PersistenceError> {
    let category = normalize_firm_library_category(category)?;
    let name = normalize_domain_label(name, "firm library item name", 200)?;
    let normalized_name = normalize_search_text(&name);
    if normalized_name.is_empty() {
        return Err(PersistenceError::Configuration(
            "firm library item name must contain searchable characters".to_string(),
        ));
    }
    let description = normalize_firm_library_description(description)?;
    let (definition_json, definition_hash) = normalize_firm_library_definition(definition_json)?;

    let mut connection = open_configured_connection(database_path)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

    if let Some(service_type_id) = service_type_id {
        let service_exists: bool = transaction.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM service_types
                WHERE service_type_id = ?1
                  AND archived_at_ms IS NULL
            )",
            [service_type_id],
            |row| row.get(0),
        )?;
        if !service_exists {
            return Err(PersistenceError::Configuration(format!(
                "service type {service_type_id} does not exist"
            )));
        }
    }

    let firm_library_item_id = Uuid::new_v4().to_string();
    let firm_library_version_id = Uuid::new_v4().to_string();
    let now = now_unix_ms()?;

    transaction.execute(
        "INSERT INTO firm_library_items (
            firm_library_item_id,
            category,
            name,
            normalized_name,
            description,
            service_type_id,
            created_at_ms,
            archived_at_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL)",
        params![
            &firm_library_item_id,
            &category,
            &name,
            &normalized_name,
            description.as_deref(),
            service_type_id,
            now
        ],
    )?;
    transaction.execute(
        "INSERT INTO firm_library_versions (
            firm_library_version_id,
            firm_library_item_id,
            version_number,
            definition_json,
            definition_hash,
            created_at_ms
         ) VALUES (?1, ?2, 1, ?3, ?4, ?5)",
        params![
            &firm_library_version_id,
            &firm_library_item_id,
            &definition_json,
            &definition_hash,
            now
        ],
    )?;

    insert_domain_audit_event(
        &transaction,
        DomainAuditEvent {
            event_type: "FIRM_LIBRARY_ITEM_CREATED",
            entity_type: "FIRM_LIBRARY_ITEM",
            entity_id: &firm_library_item_id,
            related_entity_type: Some("FIRM_LIBRARY_VERSION"),
            related_entity_id: Some(&firm_library_version_id),
            occurred_at_ms: now,
            details: json!({
                "category": category.as_str(),
                "serviceTypeId": service_type_id,
                "versionNumber": 1,
                "definitionHash": bytes_to_lower_hex(&definition_hash)
            }),
        },
    )?;

    transaction.commit()?;
    Ok(FirmLibraryItemRecord {
        firm_library_item_id,
        category,
        name,
        description,
        service_type_id: service_type_id.map(str::to_string),
        latest_version_id: firm_library_version_id,
        latest_version_number: 1,
        latest_definition_hash: definition_hash,
        created_at_ms: now,
    })
}

pub fn publish_firm_library_version(
    database_path: &Path,
    firm_library_item_id: &str,
    definition_json: &str,
) -> Result<FirmLibraryItemRecord, PersistenceError> {
    let (definition_json, definition_hash) = normalize_firm_library_definition(definition_json)?;

    let mut connection = open_configured_connection(database_path)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

    let item: Option<FirmLibraryItemIdentity> = transaction
        .query_row(
            "SELECT
                category,
                name,
                description,
                service_type_id,
                created_at_ms
             FROM firm_library_items
             WHERE firm_library_item_id = ?1
               AND archived_at_ms IS NULL",
            [firm_library_item_id],
            |row| {
                Ok(FirmLibraryItemIdentity {
                    category: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    service_type_id: row.get(3)?,
                    created_at_ms: row.get(4)?,
                })
            },
        )
        .optional()?;
    let item = item.ok_or_else(|| {
        PersistenceError::Configuration(format!(
            "firm library item {firm_library_item_id} does not exist"
        ))
    })?;

    let next_version_number: i64 = transaction.query_row(
        "SELECT COALESCE(MAX(version_number), 0) + 1
         FROM firm_library_versions
         WHERE firm_library_item_id = ?1",
        [firm_library_item_id],
        |row| row.get(0),
    )?;
    if next_version_number < 1 {
        return Err(PersistenceError::Configuration(
            "firm library version sequence is invalid".to_string(),
        ));
    }

    let firm_library_version_id = Uuid::new_v4().to_string();
    let now = now_unix_ms()?;
    transaction.execute(
        "INSERT INTO firm_library_versions (
            firm_library_version_id,
            firm_library_item_id,
            version_number,
            definition_json,
            definition_hash,
            created_at_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            &firm_library_version_id,
            firm_library_item_id,
            next_version_number,
            &definition_json,
            &definition_hash,
            now
        ],
    )?;

    insert_domain_audit_event(
        &transaction,
        DomainAuditEvent {
            event_type: "FIRM_LIBRARY_VERSION_PUBLISHED",
            entity_type: "FIRM_LIBRARY_VERSION",
            entity_id: &firm_library_version_id,
            related_entity_type: Some("FIRM_LIBRARY_ITEM"),
            related_entity_id: Some(firm_library_item_id),
            occurred_at_ms: now,
            details: json!({
                "category": item.category.as_str(),
                "versionNumber": next_version_number,
                "definitionHash": bytes_to_lower_hex(&definition_hash)
            }),
        },
    )?;

    transaction.commit()?;
    Ok(FirmLibraryItemRecord {
        firm_library_item_id: firm_library_item_id.to_string(),
        category: item.category,
        name: item.name,
        description: item.description,
        service_type_id: item.service_type_id,
        latest_version_id: firm_library_version_id,
        latest_version_number: next_version_number as u64,
        latest_definition_hash: definition_hash,
        created_at_ms: item.created_at_ms,
    })
}

pub fn list_firm_library_items(
    database_path: &Path,
    category: Option<&str>,
    service_type_id: Option<&str>,
) -> Result<Vec<FirmLibraryItemRecord>, PersistenceError> {
    let category = category.map(normalize_firm_library_category).transpose()?;
    let connection = open_configured_connection(database_path)?;
    let mut statement = connection.prepare(
        "SELECT
            i.firm_library_item_id,
            i.category,
            i.name,
            i.description,
            i.service_type_id,
            v.firm_library_version_id,
            v.version_number,
            v.definition_hash,
            i.created_at_ms
         FROM firm_library_items i
         JOIN firm_library_versions v
           ON v.firm_library_item_id = i.firm_library_item_id
          AND v.version_number = (
              SELECT MAX(v2.version_number)
              FROM firm_library_versions v2
              WHERE v2.firm_library_item_id = i.firm_library_item_id
          )
         WHERE i.archived_at_ms IS NULL
           AND (?1 IS NULL OR i.category = ?1)
           AND (?2 IS NULL OR i.service_type_id = ?2)
         ORDER BY i.category, i.name COLLATE NOCASE, i.created_at_ms",
    )?;
    let rows = statement.query_map(params![category.as_deref(), service_type_id], |row| {
        let version_number: i64 = row.get(6)?;
        Ok(FirmLibraryItemRecord {
            firm_library_item_id: row.get(0)?,
            category: row.get(1)?,
            name: row.get(2)?,
            description: row.get(3)?,
            service_type_id: row.get(4)?,
            latest_version_id: row.get(5)?,
            latest_version_number: version_number.max(0) as u64,
            latest_definition_hash: row.get(7)?,
            created_at_ms: row.get(8)?,
        })
    })?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

pub fn list_firm_library_versions(
    database_path: &Path,
    firm_library_item_id: &str,
) -> Result<Vec<FirmLibraryVersionRecord>, PersistenceError> {
    let connection = open_configured_connection(database_path)?;

    let item_exists: bool = connection.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM firm_library_items
            WHERE firm_library_item_id = ?1
              AND archived_at_ms IS NULL
        )",
        [firm_library_item_id],
        |row| row.get(0),
    )?;
    if !item_exists {
        return Err(PersistenceError::Configuration(format!(
            "firm library item {firm_library_item_id} does not exist"
        )));
    }

    let mut statement = connection.prepare(
        "SELECT
            firm_library_version_id,
            firm_library_item_id,
            version_number,
            definition_json,
            definition_hash,
            created_at_ms
         FROM firm_library_versions
         WHERE firm_library_item_id = ?1
         ORDER BY version_number DESC",
    )?;
    let rows = statement.query_map([firm_library_item_id], |row| {
        let version_number: i64 = row.get(2)?;
        Ok(FirmLibraryVersionRecord {
            firm_library_version_id: row.get(0)?,
            firm_library_item_id: row.get(1)?,
            version_number: version_number.max(0) as u64,
            definition_json: row.get(3)?,
            definition_hash: row.get(4)?,
            created_at_ms: row.get(5)?,
        })
    })?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

pub fn create_statutory_compliance_requirement(
    database_path: &Path,
    engagement_id: &str,
    firm_library_version_id: &str,
) -> Result<StatutoryComplianceRequirementRecord, PersistenceError> {
    let mut connection = open_configured_connection(database_path)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

    let engagement_service_type_id: Option<String> = transaction
        .query_row(
            "SELECT service_type_id
             FROM engagements
             WHERE engagement_id = ?1
               AND archived_at_ms IS NULL",
            [engagement_id],
            |row| row.get(0),
        )
        .optional()?;
    let engagement_service_type_id = engagement_service_type_id.ok_or_else(|| {
        PersistenceError::Configuration(format!("engagement {engagement_id} does not exist"))
    })?;

    let library_version: Option<(String, String, Option<String>, Option<String>, i64, String, Vec<u8>)> =
        transaction
            .query_row(
                "SELECT
                    i.firm_library_item_id,
                    i.name,
                    i.description,
                    i.service_type_id,
                    v.version_number,
                    v.definition_json,
                    v.definition_hash
                 FROM firm_library_versions v
                 JOIN firm_library_items i
                   ON i.firm_library_item_id = v.firm_library_item_id
                 WHERE v.firm_library_version_id = ?1
                   AND i.category = 'STATUTORY_COMPLIANCE_REQUIREMENT'
                   AND i.archived_at_ms IS NULL",
                [firm_library_version_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                    ))
                },
            )
            .optional()?;
    let Some((
        firm_library_item_id,
        requirement_name,
        requirement_description,
        requirement_service_type_id,
        version_number,
        definition_json,
        definition_hash,
    )) = library_version
    else {
        return Err(PersistenceError::Configuration(format!(
            "statutory compliance library version {firm_library_version_id} does not exist"
        )));
    };

    if requirement_service_type_id
        .as_deref()
        .is_some_and(|value| value != engagement_service_type_id)
    {
        return Err(PersistenceError::Configuration(
            "statutory compliance requirement service type does not match the engagement"
                .to_string(),
        ));
    }
    if definition_hash.len() != 32 {
        return Err(PersistenceError::Configuration(
            "statutory compliance requirement definition hash is invalid".to_string(),
        ));
    }

    let existing: bool = transaction.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM statutory_compliance_requirements
            WHERE engagement_id = ?1
              AND firm_library_item_id = ?2
        )",
        params![engagement_id, &firm_library_item_id],
        |row| row.get(0),
    )?;
    if existing {
        return Err(PersistenceError::Configuration(
            "this statutory compliance requirement is already present in the engagement"
                .to_string(),
        ));
    }

    let requirement_id = Uuid::new_v4().to_string();
    let assessment_id = Uuid::new_v4().to_string();
    let now = now_unix_ms()?;
    transaction.execute(
        "INSERT INTO statutory_compliance_requirements (
            statutory_compliance_requirement_id,
            engagement_id,
            firm_library_item_id,
            firm_library_version_id,
            requirement_name,
            requirement_description,
            definition_hash,
            created_at_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            &requirement_id,
            engagement_id,
            &firm_library_item_id,
            firm_library_version_id,
            &requirement_name,
            requirement_description.as_deref(),
            &definition_hash,
            now
        ],
    )?;
    transaction.execute(
        "INSERT INTO statutory_compliance_assessments (
            statutory_compliance_assessment_id,
            statutory_compliance_requirement_id,
            version_number,
            supersedes_assessment_id,
            applicability,
            due_date,
            actual_compliance_date,
            status,
            exception_text,
            conclusion,
            assessed_at_ms
         ) VALUES (?1, ?2, 1, NULL, 'UNDETERMINED', NULL, NULL, 'UNASSESSED', NULL, NULL, ?3)",
        params![&assessment_id, &requirement_id, now],
    )?;

    insert_domain_audit_event(
        &transaction,
        DomainAuditEvent {
            event_type: "STATUTORY_COMPLIANCE_REQUIREMENT_ADDED",
            entity_type: "STATUTORY_COMPLIANCE_REQUIREMENT",
            entity_id: &requirement_id,
            related_entity_type: Some("ENGAGEMENT"),
            related_entity_id: Some(engagement_id),
            occurred_at_ms: now,
            details: json!({
                "firmLibraryItemId": firm_library_item_id,
                "firmLibraryVersionId": firm_library_version_id,
                "versionNumber": version_number,
                "definitionHash": bytes_to_lower_hex(&definition_hash),
                "initialAssessmentId": assessment_id
            }),
        },
    )?;

    transaction.commit()?;
    Ok(StatutoryComplianceRequirementRecord {
        statutory_compliance_requirement_id: requirement_id,
        engagement_id: engagement_id.to_string(),
        firm_library_item_id,
        firm_library_version_id: firm_library_version_id.to_string(),
        requirement_name,
        requirement_description,
        version_number: version_number.max(0) as u64,
        definition_json,
        definition_hash,
        created_at_ms: now,
    })
}

pub fn list_statutory_compliance_requirements(
    database_path: &Path,
    engagement_id: &str,
) -> Result<Vec<StatutoryComplianceRequirementRecord>, PersistenceError> {
    let connection = open_configured_connection(database_path)?;
    let mut statement = connection.prepare(
        "SELECT
            r.statutory_compliance_requirement_id,
            r.engagement_id,
            r.firm_library_item_id,
            r.firm_library_version_id,
            r.requirement_name,
            r.requirement_description,
            v.version_number,
            v.definition_json,
            r.definition_hash,
            r.created_at_ms
         FROM statutory_compliance_requirements r
         JOIN firm_library_versions v
           ON v.firm_library_version_id = r.firm_library_version_id
         WHERE r.engagement_id = ?1
         ORDER BY r.requirement_name COLLATE NOCASE, r.created_at_ms, r.statutory_compliance_requirement_id",
    )?;
    let rows = statement.query_map([engagement_id], |row| {
        let version_number: i64 = row.get(6)?;
        Ok(StatutoryComplianceRequirementRecord {
            statutory_compliance_requirement_id: row.get(0)?,
            engagement_id: row.get(1)?,
            firm_library_item_id: row.get(2)?,
            firm_library_version_id: row.get(3)?,
            requirement_name: row.get(4)?,
            requirement_description: row.get(5)?,
            version_number: version_number.max(0) as u64,
            definition_json: row.get(7)?,
            definition_hash: row.get(8)?,
            created_at_ms: row.get(9)?,
        })
    })?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

pub fn create_ledger_import(
    database_path: &Path,
    definition: LedgerImportDefinition<'_>,
) -> Result<LedgerImportRecord, PersistenceError> {
    let LedgerImportDefinition {
        engagement_id,
        controlled_evidence_version_id,
        sheet_name,
        header_row_number,
        amount_column,
        date_column,
        account_column,
        voucher_column,
        narration_column,
        amount_scale,
        transactions,
    } = definition;
    let sheet_name = sheet_name.trim();
    if sheet_name.is_empty()
        || sheet_name.chars().count() > 255
        || sheet_name.chars().any(|character| character.is_control())
    {
        return Err(PersistenceError::Configuration(
            "ledger worksheet name must contain 1 to 255 printable characters".to_string(),
        ));
    }
    if header_row_number == 0 {
        return Err(PersistenceError::Configuration(
            "ledger header row number must be at least 1".to_string(),
        ));
    }
    if amount_scale > 6 {
        return Err(PersistenceError::Configuration(
            "ledger amount scale must be between 0 and 6".to_string(),
        ));
    }

    for transaction in transactions {
        if transaction.source_row_number == 0 {
            return Err(PersistenceError::Configuration(
                "ledger transaction source row must be at least 1".to_string(),
            ));
        }
        if transaction.source_row_hash.len() != 32 {
            return Err(PersistenceError::Configuration(
                "ledger transaction provenance is incomplete".to_string(),
            ));
        }
    }

    let mut connection = open_configured_connection(database_path)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

    let engagement_exists: bool = transaction.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM engagements
            WHERE engagement_id = ?1
              AND archived_at_ms IS NULL
        )",
        [engagement_id],
        |row| row.get(0),
    )?;
    if !engagement_exists {
        return Err(PersistenceError::Configuration(format!(
            "engagement {engagement_id} does not exist"
        )));
    }

    let evidence: Option<(String, String, Vec<u8>, String, String)> = transaction
        .query_row(
            "SELECT
                document_id,
                source_content_version_id,
                sha256,
                verification_state,
                retention_state
             FROM controlled_evidence_versions
             WHERE controlled_evidence_version_id = ?1",
            [controlled_evidence_version_id],
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
    let Some((
        document_id,
        source_content_version_id,
        source_sha256,
        verification_state,
        retention_state,
    )) = evidence
    else {
        return Err(PersistenceError::Configuration(format!(
            "controlled evidence version {controlled_evidence_version_id} does not exist"
        )));
    };
    if verification_state != "HASH_VERIFIED" || retention_state != "RETAINED" {
        return Err(PersistenceError::Configuration(
            "ledger imports require retained hash-verified controlled evidence".to_string(),
        ));
    }
    if source_sha256.len() != 32 {
        return Err(PersistenceError::Configuration(
            "controlled ledger evidence hash is invalid".to_string(),
        ));
    }

    let ledger_import_id = Uuid::new_v4().to_string();
    let now = now_unix_ms()?;
    transaction.execute(
        "INSERT INTO ledger_imports (
            ledger_import_id,
            engagement_id,
            controlled_evidence_version_id,
            document_id,
            source_content_version_id,
            source_sha256,
            sheet_name,
            header_row_number,
            amount_column,
            date_column,
            account_column,
            voucher_column,
            narration_column,
            amount_scale,
            transaction_count,
            imported_at_ms
         ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16
         )",
        params![
            &ledger_import_id,
            engagement_id,
            controlled_evidence_version_id,
            &document_id,
            &source_content_version_id,
            &source_sha256,
            sheet_name,
            u64_to_i64(header_row_number)?,
            i64::from(amount_column),
            date_column.map(i64::from),
            account_column.map(i64::from),
            voucher_column.map(i64::from),
            narration_column.map(i64::from),
            i64::from(amount_scale),
            u64_to_i64(transactions.len() as u64)?,
            now
        ],
    )?;

    for ledger_transaction in transactions {
        transaction.execute(
            "INSERT INTO ledger_transactions (
                ledger_transaction_id,
                ledger_import_id,
                source_row_number,
                source_row_hash,
                transaction_date_text,
                account_text,
                voucher_text,
                narration_text,
                amount_minor,
                created_at_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                Uuid::new_v4().to_string(),
                &ledger_import_id,
                u64_to_i64(ledger_transaction.source_row_number)?,
                &ledger_transaction.source_row_hash,
                ledger_transaction.transaction_date_text.as_deref(),
                ledger_transaction.account_text.as_deref(),
                ledger_transaction.voucher_text.as_deref(),
                ledger_transaction.narration_text.as_deref(),
                ledger_transaction.amount_minor,
                now
            ],
        )?;
    }

    insert_domain_audit_event(
        &transaction,
        DomainAuditEvent {
            event_type: "LEDGER_IMPORTED",
            entity_type: "LEDGER_IMPORT",
            entity_id: &ledger_import_id,
            related_entity_type: Some("ENGAGEMENT"),
            related_entity_id: Some(engagement_id),
            occurred_at_ms: now,
            details: json!({
                "controlledEvidenceVersionId": controlled_evidence_version_id,
                "sourceContentVersionId": source_content_version_id,
                "sourceSha256": bytes_to_lower_hex(&source_sha256),
                "sheetName": sheet_name,
                "transactionCount": transactions.len(),
                "amountScale": amount_scale
            }),
        },
    )?;

    transaction.commit()?;
    Ok(LedgerImportRecord {
        ledger_import_id,
        engagement_id: engagement_id.to_string(),
        controlled_evidence_version_id: controlled_evidence_version_id.to_string(),
        document_id,
        source_content_version_id,
        source_sha256,
        sheet_name: sheet_name.to_string(),
        header_row_number,
        amount_column,
        date_column,
        account_column,
        voucher_column,
        narration_column,
        amount_scale,
        transaction_count: transactions.len() as u64,
        imported_at_ms: now,
    })
}

pub fn list_ledger_imports(
    database_path: &Path,
    engagement_id: &str,
) -> Result<Vec<LedgerImportRecord>, PersistenceError> {
    let connection = open_configured_connection(database_path)?;
    let mut statement = connection.prepare(
        "SELECT
            ledger_import_id,
            engagement_id,
            controlled_evidence_version_id,
            document_id,
            source_content_version_id,
            source_sha256,
            sheet_name,
            header_row_number,
            amount_column,
            date_column,
            account_column,
            voucher_column,
            narration_column,
            amount_scale,
            transaction_count,
            imported_at_ms
         FROM ledger_imports
         WHERE engagement_id = ?1
         ORDER BY imported_at_ms DESC, ledger_import_id",
    )?;
    let rows = statement.query_map([engagement_id], |row| {
        let header_row_number: i64 = row.get(7)?;
        let amount_column: i64 = row.get(8)?;
        let date_column: Option<i64> = row.get(9)?;
        let account_column: Option<i64> = row.get(10)?;
        let voucher_column: Option<i64> = row.get(11)?;
        let narration_column: Option<i64> = row.get(12)?;
        let amount_scale: i64 = row.get(13)?;
        let transaction_count: i64 = row.get(14)?;
        Ok(LedgerImportRecord {
            ledger_import_id: row.get(0)?,
            engagement_id: row.get(1)?,
            controlled_evidence_version_id: row.get(2)?,
            document_id: row.get(3)?,
            source_content_version_id: row.get(4)?,
            source_sha256: row.get(5)?,
            sheet_name: row.get(6)?,
            header_row_number: header_row_number.max(0) as u64,
            amount_column: amount_column.max(0) as u32,
            date_column: date_column.map(|value| value.max(0) as u32),
            account_column: account_column.map(|value| value.max(0) as u32),
            voucher_column: voucher_column.map(|value| value.max(0) as u32),
            narration_column: narration_column.map(|value| value.max(0) as u32),
            amount_scale: amount_scale.max(0) as u32,
            transaction_count: transaction_count.max(0) as u64,
            imported_at_ms: row.get(15)?,
        })
    })?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

fn ledger_account_summaries_from_connection(
    connection: &Connection,
    ledger_import_id: &str,
) -> Result<Vec<LedgerAccountSummaryRecord>, PersistenceError> {
    let import_exists: bool = connection.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM ledger_imports
            WHERE ledger_import_id = ?1
        )",
        [ledger_import_id],
        |row| row.get(0),
    )?;
    if !import_exists {
        return Err(PersistenceError::Configuration(format!(
            "ledger import {ledger_import_id} does not exist"
        )));
    }

    let mut statement = connection.prepare(
        "SELECT account_text, amount_minor, source_row_number
         FROM ledger_transactions
         WHERE ledger_import_id = ?1
           AND account_text IS NOT NULL
           AND length(trim(account_text)) > 0
         ORDER BY source_row_number, ledger_transaction_id",
    )?;
    let rows = statement.query_map([ledger_import_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, i64>(2)?,
        ))
    })?;

    #[derive(Debug)]
    struct SummaryAccumulator {
        account_text: String,
        transaction_count: u64,
        total_minor: i64,
        first_source_row_number: u64,
        last_source_row_number: u64,
    }

    let mut summaries = BTreeMap::<String, SummaryAccumulator>::new();
    for row in rows {
        let (account_text, amount_minor, source_row_number) = row?;
        let account_key = normalize_ledger_account_key(&account_text)?;
        let source_row_number = source_row_number.max(0) as u64;

        if let Some(summary) = summaries.get_mut(&account_key) {
            summary.transaction_count =
                summary.transaction_count.checked_add(1).ok_or_else(|| {
                    PersistenceError::Configuration(
                        "ledger account transaction count exceeds supported range".to_string(),
                    )
                })?;
            summary.total_minor =
                summary
                    .total_minor
                    .checked_add(amount_minor)
                    .ok_or_else(|| {
                        PersistenceError::Configuration(
                            "ledger account total exceeds supported range".to_string(),
                        )
                    })?;
            summary.last_source_row_number = source_row_number;
        } else {
            summaries.insert(
                account_key,
                SummaryAccumulator {
                    account_text,
                    transaction_count: 1,
                    total_minor: amount_minor,
                    first_source_row_number: source_row_number,
                    last_source_row_number: source_row_number,
                },
            );
        }
    }

    Ok(summaries
        .into_iter()
        .map(|(account_key, summary)| LedgerAccountSummaryRecord {
            ledger_import_id: ledger_import_id.to_string(),
            account_key,
            account_text: summary.account_text,
            transaction_count: summary.transaction_count,
            total_minor: summary.total_minor,
            first_source_row_number: summary.first_source_row_number,
            last_source_row_number: summary.last_source_row_number,
        })
        .collect())
}

pub fn list_ledger_account_summaries(
    database_path: &Path,
    ledger_import_id: &str,
) -> Result<Vec<LedgerAccountSummaryRecord>, PersistenceError> {
    let connection = open_configured_connection(database_path)?;
    ledger_account_summaries_from_connection(&connection, ledger_import_id)
}

pub fn create_ledger_tb_mapping(
    database_path: &Path,
    ledger_import_id: &str,
    trial_balance_import_id: &str,
    ledger_account_key: &str,
    trial_balance_account_id: &str,
) -> Result<LedgerTbMappingRecord, PersistenceError> {
    let ledger_account_key = normalize_ledger_account_key(ledger_account_key)?;
    let mut connection = open_configured_connection(database_path)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

    let import_pair: Option<(String, i64, String, i64)> = transaction
        .query_row(
            "SELECT
                li.engagement_id,
                li.amount_scale,
                tbi.engagement_id,
                tbi.amount_scale
             FROM ledger_imports li
             CROSS JOIN trial_balance_imports tbi
             WHERE li.ledger_import_id = ?1
               AND tbi.trial_balance_import_id = ?2",
            params![ledger_import_id, trial_balance_import_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()?;
    let Some((
        ledger_engagement_id,
        ledger_amount_scale,
        trial_balance_engagement_id,
        trial_balance_amount_scale,
    )) = import_pair
    else {
        return Err(PersistenceError::Configuration(
            "ledger and Trial Balance imports must both exist".to_string(),
        ));
    };
    if ledger_engagement_id != trial_balance_engagement_id {
        return Err(PersistenceError::Configuration(
            "ledger and Trial Balance imports must belong to the same engagement".to_string(),
        ));
    }
    if ledger_amount_scale != trial_balance_amount_scale {
        return Err(PersistenceError::Configuration(
            "ledger and Trial Balance imports must use the same amount scale".to_string(),
        ));
    }

    let summaries = ledger_account_summaries_from_connection(&transaction, ledger_import_id)?;
    let ledger_summary = summaries
        .into_iter()
        .find(|summary| summary.account_key == ledger_account_key)
        .ok_or_else(|| {
            PersistenceError::Configuration(
                "ledger account key does not exist in the selected immutable ledger import"
                    .to_string(),
            )
        })?;

    let trial_balance_account: Option<TrialBalanceMappingTarget> = transaction
        .query_row(
            "SELECT
                trial_balance_import_id,
                account_code_text,
                account_name_text,
                source_row_number,
                source_row_hash
             FROM trial_balance_accounts
             WHERE trial_balance_account_id = ?1",
            [trial_balance_account_id],
            |row| {
                Ok(TrialBalanceMappingTarget {
                    trial_balance_import_id: row.get(0)?,
                    account_code_text: row.get(1)?,
                    account_name_text: row.get(2)?,
                    source_row_number: row.get(3)?,
                    source_row_hash: row.get(4)?,
                })
            },
        )
        .optional()?;
    let Some(trial_balance_account) = trial_balance_account else {
        return Err(PersistenceError::Configuration(format!(
            "Trial Balance account {trial_balance_account_id} does not exist"
        )));
    };
    if trial_balance_account.trial_balance_import_id != trial_balance_import_id {
        return Err(PersistenceError::Configuration(
            "selected Trial Balance account does not belong to the selected immutable import"
                .to_string(),
        ));
    }

    let latest_mapping: Option<(String, String, i64)> = transaction
        .query_row(
            "SELECT
                ledger_tb_mapping_id,
                trial_balance_account_id,
                version_number
             FROM ledger_tb_mappings
             WHERE ledger_import_id = ?1
               AND trial_balance_import_id = ?2
               AND ledger_account_key = ?3
             ORDER BY version_number DESC
             LIMIT 1",
            params![
                ledger_import_id,
                trial_balance_import_id,
                &ledger_account_key
            ],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;

    if latest_mapping
        .as_ref()
        .is_some_and(|(_, account_id, _)| account_id == trial_balance_account_id)
    {
        return Err(PersistenceError::Configuration(
            "ledger account is already mapped to the selected Trial Balance account".to_string(),
        ));
    }

    let (supersedes_mapping_id, version_number) = match latest_mapping {
        Some((mapping_id, _, version_number)) => (
            Some(mapping_id),
            version_number.checked_add(1).ok_or_else(|| {
                PersistenceError::Configuration(
                    "ledger to Trial Balance mapping version exceeds supported range".to_string(),
                )
            })?,
        ),
        None => (None, 1),
    };
    let version_number_u64 = version_number.max(0) as u64;
    let trial_balance_source_row_number = trial_balance_account.source_row_number.max(0) as u64;
    let ledger_tb_mapping_id = Uuid::new_v4().to_string();
    let now = now_unix_ms()?;

    transaction.execute(
        "INSERT INTO ledger_tb_mappings (
            ledger_tb_mapping_id,
            ledger_import_id,
            trial_balance_import_id,
            ledger_account_key,
            ledger_account_text,
            trial_balance_account_id,
            version_number,
            supersedes_mapping_id,
            mapped_at_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            &ledger_tb_mapping_id,
            ledger_import_id,
            trial_balance_import_id,
            &ledger_account_key,
            &ledger_summary.account_text,
            trial_balance_account_id,
            version_number,
            supersedes_mapping_id.as_deref(),
            now
        ],
    )?;

    insert_domain_audit_event(
        &transaction,
        DomainAuditEvent {
            event_type: "LEDGER_TB_MAPPING_CREATED",
            entity_type: "LEDGER_TB_MAPPING",
            entity_id: &ledger_tb_mapping_id,
            related_entity_type: Some("ENGAGEMENT"),
            related_entity_id: Some(&ledger_engagement_id),
            occurred_at_ms: now,
            details: json!({
                "ledgerImportId": ledger_import_id,
                "trialBalanceImportId": trial_balance_import_id,
                "ledgerAccountKey": ledger_account_key,
                "ledgerAccountText": ledger_summary.account_text,
                "trialBalanceAccountId": trial_balance_account_id,
                "versionNumber": version_number_u64,
                "supersedesMappingId": supersedes_mapping_id
            }),
        },
    )?;

    transaction.commit()?;
    Ok(LedgerTbMappingRecord {
        ledger_tb_mapping_id,
        ledger_import_id: ledger_import_id.to_string(),
        trial_balance_import_id: trial_balance_import_id.to_string(),
        ledger_account_key,
        ledger_account_text: ledger_summary.account_text,
        trial_balance_account_id: trial_balance_account_id.to_string(),
        trial_balance_account_code_text: trial_balance_account.account_code_text,
        trial_balance_account_name_text: trial_balance_account.account_name_text,
        trial_balance_source_row_number,
        trial_balance_source_row_hash: trial_balance_account.source_row_hash,
        version_number: version_number_u64,
        supersedes_mapping_id,
        mapped_at_ms: now,
    })
}

pub fn list_current_ledger_tb_mappings(
    database_path: &Path,
    ledger_import_id: &str,
    trial_balance_import_id: &str,
) -> Result<Vec<LedgerTbMappingRecord>, PersistenceError> {
    let connection = open_configured_connection(database_path)?;
    let mut statement = connection.prepare(
        "SELECT
            m.ledger_tb_mapping_id,
            m.ledger_import_id,
            m.trial_balance_import_id,
            m.ledger_account_key,
            m.ledger_account_text,
            m.trial_balance_account_id,
            a.account_code_text,
            a.account_name_text,
            a.source_row_number,
            a.source_row_hash,
            m.version_number,
            m.supersedes_mapping_id,
            m.mapped_at_ms
         FROM ledger_tb_mappings m
         JOIN trial_balance_accounts a
           ON a.trial_balance_account_id = m.trial_balance_account_id
         WHERE m.ledger_import_id = ?1
           AND m.trial_balance_import_id = ?2
           AND NOT EXISTS (
               SELECT 1
               FROM ledger_tb_mappings newer
               WHERE newer.ledger_import_id = m.ledger_import_id
                 AND newer.trial_balance_import_id = m.trial_balance_import_id
                 AND newer.ledger_account_key = m.ledger_account_key
                 AND newer.version_number > m.version_number
           )
         ORDER BY m.ledger_account_key, m.version_number DESC",
    )?;
    let rows = statement.query_map(params![ledger_import_id, trial_balance_import_id], |row| {
        let trial_balance_source_row_number: i64 = row.get(8)?;
        let version_number: i64 = row.get(10)?;
        Ok(LedgerTbMappingRecord {
            ledger_tb_mapping_id: row.get(0)?,
            ledger_import_id: row.get(1)?,
            trial_balance_import_id: row.get(2)?,
            ledger_account_key: row.get(3)?,
            ledger_account_text: row.get(4)?,
            trial_balance_account_id: row.get(5)?,
            trial_balance_account_code_text: row.get(6)?,
            trial_balance_account_name_text: row.get(7)?,
            trial_balance_source_row_number: trial_balance_source_row_number.max(0) as u64,
            trial_balance_source_row_hash: row.get(9)?,
            version_number: version_number.max(0) as u64,
            supersedes_mapping_id: row.get(11)?,
            mapped_at_ms: row.get(12)?,
        })
    })?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

pub fn create_financial_statement_schedule(
    database_path: &Path,
    engagement_id: &str,
    reference: &str,
    name: &str,
) -> Result<FinancialStatementScheduleRecord, PersistenceError> {
    let reference =
        normalize_domain_label(reference, "financial statement schedule reference", 80)?;
    let normalized_reference = normalize_search_text(&reference);
    if normalized_reference.is_empty() {
        return Err(PersistenceError::Configuration(
            "financial statement schedule reference is invalid".to_string(),
        ));
    }
    let name = normalize_domain_label(name, "financial statement schedule name", 240)?;

    let mut connection = open_configured_connection(database_path)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

    let engagement_exists: bool = transaction.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM engagements
            WHERE engagement_id = ?1
              AND archived_at_ms IS NULL
        )",
        [engagement_id],
        |row| row.get(0),
    )?;
    if !engagement_exists {
        return Err(PersistenceError::Configuration(format!(
            "engagement {engagement_id} does not exist"
        )));
    }

    let duplicate_exists: bool = transaction.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM financial_statement_schedules
            WHERE engagement_id = ?1
              AND normalized_reference = ?2
        )",
        params![engagement_id, &normalized_reference],
        |row| row.get(0),
    )?;
    if duplicate_exists {
        return Err(PersistenceError::Configuration(format!(
            "financial statement schedule reference {reference} already exists in this engagement"
        )));
    }

    let financial_statement_schedule_id = Uuid::new_v4().to_string();
    let now = now_unix_ms()?;
    transaction.execute(
        "INSERT INTO financial_statement_schedules (
            financial_statement_schedule_id,
            engagement_id,
            reference,
            normalized_reference,
            name,
            created_at_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            &financial_statement_schedule_id,
            engagement_id,
            &reference,
            &normalized_reference,
            &name,
            now
        ],
    )?;

    insert_domain_audit_event(
        &transaction,
        DomainAuditEvent {
            event_type: "FINANCIAL_STATEMENT_SCHEDULE_CREATED",
            entity_type: "FINANCIAL_STATEMENT_SCHEDULE",
            entity_id: &financial_statement_schedule_id,
            related_entity_type: Some("ENGAGEMENT"),
            related_entity_id: Some(engagement_id),
            occurred_at_ms: now,
            details: json!({
                "reference": reference,
                "name": name
            }),
        },
    )?;

    transaction.commit()?;
    Ok(FinancialStatementScheduleRecord {
        financial_statement_schedule_id,
        engagement_id: engagement_id.to_string(),
        reference,
        name,
        created_at_ms: now,
    })
}

pub fn list_financial_statement_schedules(
    database_path: &Path,
    engagement_id: &str,
) -> Result<Vec<FinancialStatementScheduleRecord>, PersistenceError> {
    let connection = open_configured_connection(database_path)?;
    let engagement_exists: bool = connection.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM engagements
            WHERE engagement_id = ?1
              AND archived_at_ms IS NULL
        )",
        [engagement_id],
        |row| row.get(0),
    )?;
    if !engagement_exists {
        return Err(PersistenceError::Configuration(format!(
            "engagement {engagement_id} does not exist"
        )));
    }

    let mut statement = connection.prepare(
        "SELECT
            financial_statement_schedule_id,
            engagement_id,
            reference,
            name,
            created_at_ms
         FROM financial_statement_schedules
         WHERE engagement_id = ?1
         ORDER BY normalized_reference, financial_statement_schedule_id",
    )?;
    let rows = statement.query_map([engagement_id], |row| {
        Ok(FinancialStatementScheduleRecord {
            financial_statement_schedule_id: row.get(0)?,
            engagement_id: row.get(1)?,
            reference: row.get(2)?,
            name: row.get(3)?,
            created_at_ms: row.get(4)?,
        })
    })?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

pub fn create_trial_balance_schedule_mapping(
    database_path: &Path,
    trial_balance_import_id: &str,
    trial_balance_account_id: &str,
    financial_statement_schedule_id: &str,
) -> Result<TrialBalanceScheduleMappingRecord, PersistenceError> {
    let mut connection = open_configured_connection(database_path)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

    let trial_balance_engagement_id: Option<String> = transaction
        .query_row(
            "SELECT engagement_id
             FROM trial_balance_imports
             WHERE trial_balance_import_id = ?1",
            [trial_balance_import_id],
            |row| row.get(0),
        )
        .optional()?;
    let Some(trial_balance_engagement_id) = trial_balance_engagement_id else {
        return Err(PersistenceError::Configuration(format!(
            "Trial Balance import {trial_balance_import_id} does not exist"
        )));
    };

    let trial_balance_account: Option<TrialBalanceMappingTarget> = transaction
        .query_row(
            "SELECT
                trial_balance_import_id,
                account_code_text,
                account_name_text,
                source_row_number,
                source_row_hash
             FROM trial_balance_accounts
             WHERE trial_balance_account_id = ?1",
            [trial_balance_account_id],
            |row| {
                Ok(TrialBalanceMappingTarget {
                    trial_balance_import_id: row.get(0)?,
                    account_code_text: row.get(1)?,
                    account_name_text: row.get(2)?,
                    source_row_number: row.get(3)?,
                    source_row_hash: row.get(4)?,
                })
            },
        )
        .optional()?;
    let Some(trial_balance_account) = trial_balance_account else {
        return Err(PersistenceError::Configuration(format!(
            "Trial Balance account {trial_balance_account_id} does not exist"
        )));
    };
    if trial_balance_account.trial_balance_import_id != trial_balance_import_id {
        return Err(PersistenceError::Configuration(
            "selected Trial Balance account does not belong to the selected immutable import"
                .to_string(),
        ));
    }

    let schedule: Option<(String, String, String)> = transaction
        .query_row(
            "SELECT engagement_id, reference, name
             FROM financial_statement_schedules
             WHERE financial_statement_schedule_id = ?1",
            [financial_statement_schedule_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    let Some((schedule_engagement_id, schedule_reference, schedule_name)) = schedule else {
        return Err(PersistenceError::Configuration(format!(
            "financial statement schedule {financial_statement_schedule_id} does not exist"
        )));
    };
    if schedule_engagement_id != trial_balance_engagement_id {
        return Err(PersistenceError::Configuration(
            "Trial Balance import and financial statement schedule must belong to the same engagement"
                .to_string(),
        ));
    }

    let latest_mapping: Option<(String, String, i64)> = transaction
        .query_row(
            "SELECT
                trial_balance_schedule_mapping_id,
                financial_statement_schedule_id,
                version_number
             FROM trial_balance_schedule_mappings
             WHERE trial_balance_import_id = ?1
               AND trial_balance_account_id = ?2
             ORDER BY version_number DESC
             LIMIT 1",
            params![trial_balance_import_id, trial_balance_account_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;

    if latest_mapping
        .as_ref()
        .is_some_and(|(_, schedule_id, _)| schedule_id == financial_statement_schedule_id)
    {
        return Err(PersistenceError::Configuration(
            "Trial Balance account is already mapped to the selected schedule".to_string(),
        ));
    }

    let (supersedes_mapping_id, version_number) = match latest_mapping {
        Some((mapping_id, _, version_number)) => (
            Some(mapping_id),
            version_number.checked_add(1).ok_or_else(|| {
                PersistenceError::Configuration(
                    "Trial Balance schedule mapping version exceeds supported range".to_string(),
                )
            })?,
        ),
        None => (None, 1),
    };
    let version_number_u64 = version_number.max(0) as u64;
    let source_row_number = trial_balance_account.source_row_number.max(0) as u64;
    let trial_balance_schedule_mapping_id = Uuid::new_v4().to_string();
    let now = now_unix_ms()?;

    transaction.execute(
        "INSERT INTO trial_balance_schedule_mappings (
            trial_balance_schedule_mapping_id,
            trial_balance_import_id,
            trial_balance_account_id,
            financial_statement_schedule_id,
            version_number,
            supersedes_mapping_id,
            mapped_at_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            &trial_balance_schedule_mapping_id,
            trial_balance_import_id,
            trial_balance_account_id,
            financial_statement_schedule_id,
            version_number,
            supersedes_mapping_id.as_deref(),
            now
        ],
    )?;

    insert_domain_audit_event(
        &transaction,
        DomainAuditEvent {
            event_type: "TRIAL_BALANCE_SCHEDULE_MAPPING_CREATED",
            entity_type: "TRIAL_BALANCE_SCHEDULE_MAPPING",
            entity_id: &trial_balance_schedule_mapping_id,
            related_entity_type: Some("ENGAGEMENT"),
            related_entity_id: Some(&trial_balance_engagement_id),
            occurred_at_ms: now,
            details: json!({
                "trialBalanceImportId": trial_balance_import_id,
                "trialBalanceAccountId": trial_balance_account_id,
                "financialStatementScheduleId": financial_statement_schedule_id,
                "scheduleReference": schedule_reference,
                "versionNumber": version_number_u64,
                "supersedesMappingId": supersedes_mapping_id
            }),
        },
    )?;

    transaction.commit()?;
    Ok(TrialBalanceScheduleMappingRecord {
        trial_balance_schedule_mapping_id,
        trial_balance_import_id: trial_balance_import_id.to_string(),
        trial_balance_account_id: trial_balance_account_id.to_string(),
        trial_balance_account_code_text: trial_balance_account.account_code_text,
        trial_balance_account_name_text: trial_balance_account.account_name_text,
        trial_balance_source_row_number: source_row_number,
        trial_balance_source_row_hash: trial_balance_account.source_row_hash,
        financial_statement_schedule_id: financial_statement_schedule_id.to_string(),
        schedule_reference,
        schedule_name,
        version_number: version_number_u64,
        supersedes_mapping_id,
        mapped_at_ms: now,
    })
}

pub fn list_current_trial_balance_schedule_mappings(
    database_path: &Path,
    trial_balance_import_id: &str,
) -> Result<Vec<TrialBalanceScheduleMappingRecord>, PersistenceError> {
    let connection = open_configured_connection(database_path)?;
    let import_exists: bool = connection.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM trial_balance_imports
            WHERE trial_balance_import_id = ?1
        )",
        [trial_balance_import_id],
        |row| row.get(0),
    )?;
    if !import_exists {
        return Err(PersistenceError::Configuration(format!(
            "Trial Balance import {trial_balance_import_id} does not exist"
        )));
    }

    let mut statement = connection.prepare(
        "SELECT
            m.trial_balance_schedule_mapping_id,
            m.trial_balance_import_id,
            m.trial_balance_account_id,
            a.account_code_text,
            a.account_name_text,
            a.source_row_number,
            a.source_row_hash,
            m.financial_statement_schedule_id,
            s.reference,
            s.name,
            m.version_number,
            m.supersedes_mapping_id,
            m.mapped_at_ms
         FROM trial_balance_schedule_mappings m
         JOIN trial_balance_accounts a
           ON a.trial_balance_account_id = m.trial_balance_account_id
         JOIN financial_statement_schedules s
           ON s.financial_statement_schedule_id = m.financial_statement_schedule_id
         WHERE m.trial_balance_import_id = ?1
           AND NOT EXISTS (
               SELECT 1
               FROM trial_balance_schedule_mappings newer
               WHERE newer.trial_balance_import_id = m.trial_balance_import_id
                 AND newer.trial_balance_account_id = m.trial_balance_account_id
                 AND newer.version_number > m.version_number
           )
         ORDER BY a.source_row_number, m.trial_balance_account_id",
    )?;
    let rows = statement.query_map([trial_balance_import_id], |row| {
        let source_row_number: i64 = row.get(5)?;
        let version_number: i64 = row.get(10)?;
        Ok(TrialBalanceScheduleMappingRecord {
            trial_balance_schedule_mapping_id: row.get(0)?,
            trial_balance_import_id: row.get(1)?,
            trial_balance_account_id: row.get(2)?,
            trial_balance_account_code_text: row.get(3)?,
            trial_balance_account_name_text: row.get(4)?,
            trial_balance_source_row_number: source_row_number.max(0) as u64,
            trial_balance_source_row_hash: row.get(6)?,
            financial_statement_schedule_id: row.get(7)?,
            schedule_reference: row.get(8)?,
            schedule_name: row.get(9)?,
            version_number: version_number.max(0) as u64,
            supersedes_mapping_id: row.get(11)?,
            mapped_at_ms: row.get(12)?,
        })
    })?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

pub fn list_ledger_test_runs(
    database_path: &Path,
    ledger_import_id: &str,
) -> Result<Vec<LedgerTestRunRecord>, PersistenceError> {
    let connection = open_configured_connection(database_path)?;
    let import_exists: bool = connection.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM ledger_imports
            WHERE ledger_import_id = ?1
        )",
        [ledger_import_id],
        |row| row.get(0),
    )?;
    if !import_exists {
        return Err(PersistenceError::Configuration(format!(
            "ledger import {ledger_import_id} does not exist"
        )));
    }

    let mut statement = connection.prepare(
        "SELECT
            ledger_test_run_id,
            ledger_import_id,
            test_type,
            CAST(json_extract(parameters_json, '$.thresholdMinor') AS INTEGER),
            exception_count,
            ran_at_ms
         FROM ledger_test_runs
         WHERE ledger_import_id = ?1
         ORDER BY ran_at_ms DESC, ledger_test_run_id DESC",
    )?;
    let rows = statement.query_map([ledger_import_id], |row| {
        let exception_count: i64 = row.get(4)?;
        Ok(LedgerTestRunRecord {
            ledger_test_run_id: row.get(0)?,
            ledger_import_id: row.get(1)?,
            test_type: row.get(2)?,
            threshold_minor: row.get(3)?,
            exception_count: exception_count.max(0) as u64,
            ran_at_ms: row.get(5)?,
        })
    })?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

pub fn run_high_value_ledger_test(
    database_path: &Path,
    ledger_import_id: &str,
    threshold_minor: i64,
) -> Result<LedgerTestRunRecord, PersistenceError> {
    if threshold_minor <= 0 {
        return Err(PersistenceError::Configuration(
            "high-value threshold must be greater than zero".to_string(),
        ));
    }

    let mut connection = open_configured_connection(database_path)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

    let import_exists: bool = transaction.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM ledger_imports
            WHERE ledger_import_id = ?1
        )",
        [ledger_import_id],
        |row| row.get(0),
    )?;
    if !import_exists {
        return Err(PersistenceError::Configuration(format!(
            "ledger import {ledger_import_id} does not exist"
        )));
    }

    let transaction_ids = {
        let mut statement = transaction.prepare(
            "SELECT ledger_transaction_id
             FROM ledger_transactions
             WHERE ledger_import_id = ?1
               AND (amount_minor >= ?2 OR amount_minor <= -?2)
             ORDER BY source_row_number, ledger_transaction_id",
        )?;
        let rows = statement.query_map(params![ledger_import_id, threshold_minor], |row| {
            row.get::<_, String>(0)
        })?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        result
    };

    let ledger_test_run_id = Uuid::new_v4().to_string();
    let now = now_unix_ms()?;
    let parameters_json = json!({
        "thresholdMinor": threshold_minor
    })
    .to_string();
    transaction.execute(
        "INSERT INTO ledger_test_runs (
            ledger_test_run_id,
            ledger_import_id,
            test_type,
            parameters_json,
            exception_count,
            ran_at_ms
         ) VALUES (?1, ?2, 'HIGH_VALUE', ?3, ?4, ?5)",
        params![
            &ledger_test_run_id,
            ledger_import_id,
            &parameters_json,
            u64_to_i64(transaction_ids.len() as u64)?,
            now
        ],
    )?;

    for ledger_transaction_id in &transaction_ids {
        transaction.execute(
            "INSERT INTO ledger_exceptions (
                ledger_exception_id,
                ledger_test_run_id,
                ledger_transaction_id,
                exception_code,
                created_at_ms
             ) VALUES (?1, ?2, ?3, 'HIGH_VALUE', ?4)",
            params![
                Uuid::new_v4().to_string(),
                &ledger_test_run_id,
                ledger_transaction_id,
                now
            ],
        )?;
    }

    insert_domain_audit_event(
        &transaction,
        DomainAuditEvent {
            event_type: "LEDGER_TEST_RUN_COMPLETED",
            entity_type: "LEDGER_TEST_RUN",
            entity_id: &ledger_test_run_id,
            related_entity_type: Some("LEDGER_IMPORT"),
            related_entity_id: Some(ledger_import_id),
            occurred_at_ms: now,
            details: json!({
                "testType": "HIGH_VALUE",
                "thresholdMinor": threshold_minor,
                "exceptionCount": transaction_ids.len()
            }),
        },
    )?;

    transaction.commit()?;
    Ok(LedgerTestRunRecord {
        ledger_test_run_id,
        ledger_import_id: ledger_import_id.to_string(),
        test_type: "HIGH_VALUE".to_string(),
        threshold_minor,
        exception_count: transaction_ids.len() as u64,
        ran_at_ms: now,
    })
}

pub fn list_ledger_exceptions(
    database_path: &Path,
    ledger_test_run_id: &str,
) -> Result<Vec<LedgerExceptionRecord>, PersistenceError> {
    let connection = open_configured_connection(database_path)?;
    let mut statement = connection.prepare(
        "SELECT
            e.ledger_exception_id,
            e.ledger_test_run_id,
            t.ledger_transaction_id,
            e.exception_code,
            t.amount_minor,
            t.transaction_date_text,
            t.account_text,
            t.voucher_text,
            t.narration_text,
            i.controlled_evidence_version_id,
            i.document_id,
            i.source_content_version_id,
            i.source_sha256,
            i.sheet_name,
            t.source_row_number,
            t.source_row_hash,
            e.created_at_ms
         FROM ledger_exceptions e
         JOIN ledger_transactions t
           ON t.ledger_transaction_id = e.ledger_transaction_id
         JOIN ledger_imports i
           ON i.ledger_import_id = t.ledger_import_id
         WHERE e.ledger_test_run_id = ?1
         ORDER BY t.source_row_number, e.ledger_exception_id",
    )?;
    let rows = statement.query_map([ledger_test_run_id], |row| {
        let source_row_number: i64 = row.get(14)?;
        Ok(LedgerExceptionRecord {
            ledger_exception_id: row.get(0)?,
            ledger_test_run_id: row.get(1)?,
            ledger_transaction_id: row.get(2)?,
            exception_code: row.get(3)?,
            amount_minor: row.get(4)?,
            transaction_date_text: row.get(5)?,
            account_text: row.get(6)?,
            voucher_text: row.get(7)?,
            narration_text: row.get(8)?,
            controlled_evidence_version_id: row.get(9)?,
            document_id: row.get(10)?,
            source_content_version_id: row.get(11)?,
            source_sha256: row.get(12)?,
            sheet_name: row.get(13)?,
            source_row_number: source_row_number.max(0) as u64,
            source_row_hash: row.get(15)?,
            created_at_ms: row.get(16)?,
        })
    })?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

pub fn create_trial_balance_import(
    database_path: &Path,
    definition: TrialBalanceImportDefinition<'_>,
) -> Result<TrialBalanceImportRecord, PersistenceError> {
    let TrialBalanceImportDefinition {
        engagement_id,
        controlled_evidence_version_id,
        sheet_name,
        header_row_number,
        account_name_column,
        account_code_column,
        opening_balance_column,
        closing_balance_column,
        amount_scale,
        accounts,
    } = definition;

    let sheet_name = sheet_name.trim();
    if sheet_name.is_empty()
        || sheet_name.chars().count() > 255
        || sheet_name.chars().any(|character| character.is_control())
    {
        return Err(PersistenceError::Configuration(
            "trial balance worksheet name must contain 1 to 255 printable characters".to_string(),
        ));
    }
    if header_row_number == 0 {
        return Err(PersistenceError::Configuration(
            "trial balance header row number must be at least 1".to_string(),
        ));
    }
    if amount_scale > 6 {
        return Err(PersistenceError::Configuration(
            "trial balance amount scale must be between 0 and 6".to_string(),
        ));
    }
    if accounts.is_empty() {
        return Err(PersistenceError::Configuration(
            "trial balance import must contain at least one account".to_string(),
        ));
    }

    let mut opening_total_minor = 0_i64;
    let mut closing_total_minor = 0_i64;
    for account in accounts {
        if account.source_row_number <= header_row_number || account.source_row_hash.len() != 32 {
            return Err(PersistenceError::Configuration(
                "trial balance account provenance is incomplete".to_string(),
            ));
        }

        let account_name = account.account_name_text.trim();
        if account_name.is_empty()
            || account_name.chars().count() > 500
            || account_name.chars().any(|character| character.is_control())
        {
            return Err(PersistenceError::Configuration(
                "trial balance account name must contain 1 to 500 printable characters".to_string(),
            ));
        }

        if let Some(account_code) = account.account_code_text.as_deref() {
            let account_code = account_code.trim();
            if account_code.is_empty()
                || account_code.chars().count() > 200
                || account_code.chars().any(|character| character.is_control())
            {
                return Err(PersistenceError::Configuration(
                    "trial balance account code must contain 1 to 200 printable characters"
                        .to_string(),
                ));
            }
        }

        opening_total_minor = opening_total_minor
            .checked_add(account.opening_minor)
            .ok_or_else(|| {
                PersistenceError::Configuration(
                    "trial balance opening total exceeds supported range".to_string(),
                )
            })?;
        closing_total_minor = closing_total_minor
            .checked_add(account.closing_minor)
            .ok_or_else(|| {
                PersistenceError::Configuration(
                    "trial balance closing total exceeds supported range".to_string(),
                )
            })?;
    }

    let mut connection = open_configured_connection(database_path)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

    let engagement_exists: bool = transaction.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM engagements
            WHERE engagement_id = ?1
              AND archived_at_ms IS NULL
        )",
        [engagement_id],
        |row| row.get(0),
    )?;
    if !engagement_exists {
        return Err(PersistenceError::Configuration(format!(
            "engagement {engagement_id} does not exist"
        )));
    }

    let evidence: Option<(String, String, Vec<u8>, String, String)> = transaction
        .query_row(
            "SELECT
                document_id,
                source_content_version_id,
                sha256,
                verification_state,
                retention_state
             FROM controlled_evidence_versions
             WHERE controlled_evidence_version_id = ?1",
            [controlled_evidence_version_id],
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
    let Some((
        document_id,
        source_content_version_id,
        source_sha256,
        verification_state,
        retention_state,
    )) = evidence
    else {
        return Err(PersistenceError::Configuration(format!(
            "controlled evidence version {controlled_evidence_version_id} does not exist"
        )));
    };
    if verification_state != "HASH_VERIFIED" || retention_state != "RETAINED" {
        return Err(PersistenceError::Configuration(
            "trial balance imports require retained hash-verified controlled evidence".to_string(),
        ));
    }
    if source_sha256.len() != 32 {
        return Err(PersistenceError::Configuration(
            "controlled trial balance evidence hash is invalid".to_string(),
        ));
    }

    let trial_balance_import_id = Uuid::new_v4().to_string();
    let now = now_unix_ms()?;
    transaction.execute(
        "INSERT INTO trial_balance_imports (
            trial_balance_import_id,
            engagement_id,
            controlled_evidence_version_id,
            document_id,
            source_content_version_id,
            source_sha256,
            sheet_name,
            header_row_number,
            account_name_column,
            account_code_column,
            opening_balance_column,
            closing_balance_column,
            amount_scale,
            account_count,
            opening_total_minor,
            closing_total_minor,
            imported_at_ms
         ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17
         )",
        params![
            &trial_balance_import_id,
            engagement_id,
            controlled_evidence_version_id,
            &document_id,
            &source_content_version_id,
            &source_sha256,
            sheet_name,
            u64_to_i64(header_row_number)?,
            i64::from(account_name_column),
            account_code_column.map(i64::from),
            opening_balance_column.map(i64::from),
            i64::from(closing_balance_column),
            i64::from(amount_scale),
            u64_to_i64(accounts.len() as u64)?,
            opening_total_minor,
            closing_total_minor,
            now
        ],
    )?;

    for account in accounts {
        transaction.execute(
            "INSERT INTO trial_balance_accounts (
                trial_balance_account_id,
                trial_balance_import_id,
                source_row_number,
                source_row_hash,
                account_code_text,
                account_name_text,
                opening_minor,
                closing_minor,
                created_at_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                Uuid::new_v4().to_string(),
                &trial_balance_import_id,
                u64_to_i64(account.source_row_number)?,
                &account.source_row_hash,
                account.account_code_text.as_deref().map(str::trim),
                account.account_name_text.trim(),
                account.opening_minor,
                account.closing_minor,
                now
            ],
        )?;
    }

    insert_domain_audit_event(
        &transaction,
        DomainAuditEvent {
            event_type: "TRIAL_BALANCE_IMPORTED",
            entity_type: "TRIAL_BALANCE_IMPORT",
            entity_id: &trial_balance_import_id,
            related_entity_type: Some("ENGAGEMENT"),
            related_entity_id: Some(engagement_id),
            occurred_at_ms: now,
            details: json!({
                "controlledEvidenceVersionId": controlled_evidence_version_id,
                "sourceContentVersionId": source_content_version_id,
                "sourceSha256": bytes_to_lower_hex(&source_sha256),
                "sheetName": sheet_name,
                "accountCount": accounts.len(),
                "amountScale": amount_scale,
                "openingTotalMinor": opening_total_minor,
                "closingTotalMinor": closing_total_minor
            }),
        },
    )?;

    transaction.commit()?;
    Ok(TrialBalanceImportRecord {
        trial_balance_import_id,
        engagement_id: engagement_id.to_string(),
        controlled_evidence_version_id: controlled_evidence_version_id.to_string(),
        document_id,
        source_content_version_id,
        source_sha256,
        sheet_name: sheet_name.to_string(),
        header_row_number,
        account_name_column,
        account_code_column,
        opening_balance_column,
        closing_balance_column,
        amount_scale,
        account_count: accounts.len() as u64,
        opening_total_minor,
        closing_total_minor,
        imported_at_ms: now,
    })
}

pub fn list_trial_balance_imports(
    database_path: &Path,
    engagement_id: &str,
) -> Result<Vec<TrialBalanceImportRecord>, PersistenceError> {
    let connection = open_configured_connection(database_path)?;
    let mut statement = connection.prepare(
        "SELECT
            trial_balance_import_id,
            engagement_id,
            controlled_evidence_version_id,
            document_id,
            source_content_version_id,
            source_sha256,
            sheet_name,
            header_row_number,
            account_name_column,
            account_code_column,
            opening_balance_column,
            closing_balance_column,
            amount_scale,
            account_count,
            opening_total_minor,
            closing_total_minor,
            imported_at_ms
         FROM trial_balance_imports
         WHERE engagement_id = ?1
         ORDER BY imported_at_ms DESC, trial_balance_import_id",
    )?;
    let rows = statement.query_map([engagement_id], |row| {
        let header_row_number: i64 = row.get(7)?;
        let account_name_column: i64 = row.get(8)?;
        let account_code_column: Option<i64> = row.get(9)?;
        let opening_balance_column: Option<i64> = row.get(10)?;
        let closing_balance_column: i64 = row.get(11)?;
        let amount_scale: i64 = row.get(12)?;
        let account_count: i64 = row.get(13)?;
        Ok(TrialBalanceImportRecord {
            trial_balance_import_id: row.get(0)?,
            engagement_id: row.get(1)?,
            controlled_evidence_version_id: row.get(2)?,
            document_id: row.get(3)?,
            source_content_version_id: row.get(4)?,
            source_sha256: row.get(5)?,
            sheet_name: row.get(6)?,
            header_row_number: header_row_number.max(0) as u64,
            account_name_column: account_name_column.max(0) as u32,
            account_code_column: account_code_column.map(|value| value.max(0) as u32),
            opening_balance_column: opening_balance_column.map(|value| value.max(0) as u32),
            closing_balance_column: closing_balance_column.max(0) as u32,
            amount_scale: amount_scale.max(0) as u32,
            account_count: account_count.max(0) as u64,
            opening_total_minor: row.get(14)?,
            closing_total_minor: row.get(15)?,
            imported_at_ms: row.get(16)?,
        })
    })?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

pub fn list_trial_balance_accounts(
    database_path: &Path,
    trial_balance_import_id: &str,
) -> Result<Vec<TrialBalanceAccountRecord>, PersistenceError> {
    let connection = open_configured_connection(database_path)?;
    let import_exists: bool = connection.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM trial_balance_imports
            WHERE trial_balance_import_id = ?1
        )",
        [trial_balance_import_id],
        |row| row.get(0),
    )?;
    if !import_exists {
        return Err(PersistenceError::Configuration(format!(
            "trial balance import {trial_balance_import_id} does not exist"
        )));
    }

    let mut statement = connection.prepare(
        "SELECT
            trial_balance_account_id,
            trial_balance_import_id,
            source_row_number,
            source_row_hash,
            account_code_text,
            account_name_text,
            opening_minor,
            closing_minor,
            created_at_ms
         FROM trial_balance_accounts
         WHERE trial_balance_import_id = ?1
         ORDER BY source_row_number, trial_balance_account_id",
    )?;
    let rows = statement.query_map([trial_balance_import_id], |row| {
        let source_row_number: i64 = row.get(2)?;
        Ok(TrialBalanceAccountRecord {
            trial_balance_account_id: row.get(0)?,
            trial_balance_import_id: row.get(1)?,
            source_row_number: source_row_number.max(0) as u64,
            source_row_hash: row.get(3)?,
            account_code_text: row.get(4)?,
            account_name_text: row.get(5)?,
            opening_minor: row.get(6)?,
            closing_minor: row.get(7)?,
            created_at_ms: row.get(8)?,
        })
    })?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

pub fn compare_trial_balance_opening_closing(
    database_path: &Path,
    trial_balance_import_id: &str,
) -> Result<TrialBalanceComparisonRecord, PersistenceError> {
    let connection = open_configured_connection(database_path)?;

    let summary: Option<(i64, i64, i64)> = connection
        .query_row(
            "SELECT opening_total_minor, closing_total_minor, account_count
             FROM trial_balance_imports
             WHERE trial_balance_import_id = ?1",
            [trial_balance_import_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    let Some((opening_total_minor, closing_total_minor, account_count)) = summary else {
        return Err(PersistenceError::Configuration(format!(
            "trial balance import {trial_balance_import_id} does not exist"
        )));
    };
    let net_movement_minor = closing_total_minor
        .checked_sub(opening_total_minor)
        .ok_or_else(|| {
            PersistenceError::Configuration(
                "trial balance net movement exceeds supported range".to_string(),
            )
        })?;

    let accounts = list_trial_balance_accounts(database_path, trial_balance_import_id)?;
    let mut movements = Vec::with_capacity(accounts.len());
    let mut recomputed_movement_total = 0_i64;
    for account in accounts {
        let movement_minor = account
            .closing_minor
            .checked_sub(account.opening_minor)
            .ok_or_else(|| {
                PersistenceError::Configuration(format!(
                    "trial balance account movement exceeds supported range for row {}",
                    account.source_row_number
                ))
            })?;
        recomputed_movement_total = recomputed_movement_total
            .checked_add(movement_minor)
            .ok_or_else(|| {
                PersistenceError::Configuration(
                    "trial balance movement total exceeds supported range".to_string(),
                )
            })?;
        movements.push(TrialBalanceMovementRecord {
            trial_balance_account_id: account.trial_balance_account_id,
            source_row_number: account.source_row_number,
            source_row_hash: account.source_row_hash,
            account_code_text: account.account_code_text,
            account_name_text: account.account_name_text,
            opening_minor: account.opening_minor,
            closing_minor: account.closing_minor,
            movement_minor,
        });
    }

    if recomputed_movement_total != net_movement_minor {
        return Err(PersistenceError::Configuration(
            "trial balance movement comparison does not reconcile to imported totals".to_string(),
        ));
    }
    if account_count.max(0) as u64 != movements.len() as u64 {
        return Err(PersistenceError::Configuration(
            "trial balance account count does not match imported rows".to_string(),
        ));
    }

    movements.sort_by(|left, right| {
        right
            .movement_minor
            .unsigned_abs()
            .cmp(&left.movement_minor.unsigned_abs())
            .then_with(|| left.source_row_number.cmp(&right.source_row_number))
            .then_with(|| {
                left.trial_balance_account_id
                    .cmp(&right.trial_balance_account_id)
            })
    });

    Ok(TrialBalanceComparisonRecord {
        trial_balance_import_id: trial_balance_import_id.to_string(),
        opening_total_minor,
        closing_total_minor,
        net_movement_minor,
        account_count: movements.len() as u64,
        movements,
    })
}

pub fn create_financial_statement_schedule_link(
    database_path: &Path,
    financial_statement_schedule_id: &str,
    controlled_evidence_version_id: &str,
    location_kind: &str,
    location_value: &str,
) -> Result<FinancialStatementScheduleLinkRecord, PersistenceError> {
    let (location_kind, location_value) = normalize_exact_evidence_location(
        location_kind,
        location_value,
        "financial statement schedule",
    )?;

    let mut connection = open_configured_connection(database_path)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

    let schedule: Option<(String, String, String)> = transaction
        .query_row(
            "SELECT engagement_id, reference, name
             FROM financial_statement_schedules
             WHERE financial_statement_schedule_id = ?1",
            [financial_statement_schedule_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    let Some((engagement_id, schedule_reference, schedule_name)) = schedule else {
        return Err(PersistenceError::Configuration(format!(
            "financial statement schedule {financial_statement_schedule_id} does not exist"
        )));
    };

    let evidence: Option<(String, String, Vec<u8>, String, String)> = transaction
        .query_row(
            "SELECT
                cev.document_id,
                cev.source_content_version_id,
                cev.sha256,
                cev.verification_state,
                cev.retention_state
             FROM controlled_evidence_versions cev
             JOIN documents d ON d.document_id = cev.document_id
             WHERE cev.controlled_evidence_version_id = ?1
               AND d.archived_at_ms IS NULL",
            [controlled_evidence_version_id],
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
    let Some((
        document_id,
        source_content_version_id,
        source_sha256,
        verification_state,
        retention_state,
    )) = evidence
    else {
        return Err(PersistenceError::Configuration(format!(
            "controlled evidence version {controlled_evidence_version_id} does not exist"
        )));
    };
    if verification_state != "HASH_VERIFIED" || retention_state != "RETAINED" {
        return Err(PersistenceError::Configuration(
            "financial statement schedule links require retained hash-verified controlled evidence"
                .to_string(),
        ));
    }
    if source_sha256.len() != 32 {
        return Err(PersistenceError::Configuration(
            "controlled financial statement evidence hash is invalid".to_string(),
        ));
    }

    let latest_link: Option<(String, String, String, String, i64)> = transaction
        .query_row(
            "SELECT
                financial_statement_schedule_link_id,
                controlled_evidence_version_id,
                location_kind,
                location_value,
                version_number
             FROM financial_statement_schedule_links
             WHERE financial_statement_schedule_id = ?1
             ORDER BY version_number DESC
             LIMIT 1",
            [financial_statement_schedule_id],
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

    if latest_link
        .as_ref()
        .is_some_and(|(_, evidence_id, latest_kind, latest_value, _)| {
            evidence_id == controlled_evidence_version_id
                && latest_kind == &location_kind
                && latest_value == &location_value
        })
    {
        return Err(PersistenceError::Configuration(
            "financial statement schedule is already linked to the selected exact evidence location"
                .to_string(),
        ));
    }

    let (supersedes_link_id, version_number) = match latest_link {
        Some((link_id, _, _, _, version_number)) => (
            Some(link_id),
            version_number.checked_add(1).ok_or_else(|| {
                PersistenceError::Configuration(
                    "financial statement schedule link version exceeds supported range".to_string(),
                )
            })?,
        ),
        None => (None, 1),
    };
    let version_number_u64 = version_number.max(0) as u64;
    let financial_statement_schedule_link_id = Uuid::new_v4().to_string();
    let now = now_unix_ms()?;

    transaction.execute(
        "INSERT INTO financial_statement_schedule_links (
            financial_statement_schedule_link_id,
            financial_statement_schedule_id,
            controlled_evidence_version_id,
            document_id,
            source_content_version_id,
            source_sha256,
            location_kind,
            location_value,
            version_number,
            supersedes_link_id,
            linked_at_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            &financial_statement_schedule_link_id,
            financial_statement_schedule_id,
            controlled_evidence_version_id,
            &document_id,
            &source_content_version_id,
            &source_sha256,
            &location_kind,
            &location_value,
            version_number,
            supersedes_link_id.as_deref(),
            now
        ],
    )?;

    insert_domain_audit_event(
        &transaction,
        DomainAuditEvent {
            event_type: "FINANCIAL_STATEMENT_SCHEDULE_LINK_CREATED",
            entity_type: "FINANCIAL_STATEMENT_SCHEDULE_LINK",
            entity_id: &financial_statement_schedule_link_id,
            related_entity_type: Some("ENGAGEMENT"),
            related_entity_id: Some(&engagement_id),
            occurred_at_ms: now,
            details: json!({
                "financialStatementScheduleId": financial_statement_schedule_id,
                "scheduleReference": schedule_reference,
                "controlledEvidenceVersionId": controlled_evidence_version_id,
                "documentId": document_id,
                "sourceContentVersionId": source_content_version_id,
                "sourceSha256": bytes_to_lower_hex(&source_sha256),
                "locationKind": location_kind,
                "locationValue": location_value,
                "versionNumber": version_number_u64,
                "supersedesLinkId": supersedes_link_id
            }),
        },
    )?;

    transaction.commit()?;
    Ok(FinancialStatementScheduleLinkRecord {
        financial_statement_schedule_link_id,
        financial_statement_schedule_id: financial_statement_schedule_id.to_string(),
        schedule_reference,
        schedule_name,
        controlled_evidence_version_id: controlled_evidence_version_id.to_string(),
        document_id,
        source_content_version_id,
        source_sha256,
        location_kind,
        location_value,
        version_number: version_number_u64,
        supersedes_link_id,
        linked_at_ms: now,
    })
}

pub fn list_current_financial_statement_schedule_links(
    database_path: &Path,
    engagement_id: &str,
) -> Result<Vec<FinancialStatementScheduleLinkRecord>, PersistenceError> {
    let connection = open_configured_connection(database_path)?;
    let engagement_exists: bool = connection.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM engagements
            WHERE engagement_id = ?1
              AND archived_at_ms IS NULL
        )",
        [engagement_id],
        |row| row.get(0),
    )?;
    if !engagement_exists {
        return Err(PersistenceError::Configuration(format!(
            "engagement {engagement_id} does not exist"
        )));
    }

    let mut statement = connection.prepare(
        "SELECT
            l.financial_statement_schedule_link_id,
            l.financial_statement_schedule_id,
            s.reference,
            s.name,
            l.controlled_evidence_version_id,
            l.document_id,
            l.source_content_version_id,
            l.source_sha256,
            l.location_kind,
            l.location_value,
            l.version_number,
            l.supersedes_link_id,
            l.linked_at_ms
         FROM financial_statement_schedule_links l
         JOIN financial_statement_schedules s
           ON s.financial_statement_schedule_id = l.financial_statement_schedule_id
         WHERE s.engagement_id = ?1
           AND NOT EXISTS (
               SELECT 1
               FROM financial_statement_schedule_links newer
               WHERE newer.financial_statement_schedule_id =
                     l.financial_statement_schedule_id
                 AND newer.version_number > l.version_number
           )
         ORDER BY s.normalized_reference, l.financial_statement_schedule_link_id",
    )?;
    let rows = statement.query_map([engagement_id], |row| {
        let version_number: i64 = row.get(10)?;
        Ok(FinancialStatementScheduleLinkRecord {
            financial_statement_schedule_link_id: row.get(0)?,
            financial_statement_schedule_id: row.get(1)?,
            schedule_reference: row.get(2)?,
            schedule_name: row.get(3)?,
            controlled_evidence_version_id: row.get(4)?,
            document_id: row.get(5)?,
            source_content_version_id: row.get(6)?,
            source_sha256: row.get(7)?,
            location_kind: row.get(8)?,
            location_value: row.get(9)?,
            version_number: version_number.max(0) as u64,
            supersedes_link_id: row.get(11)?,
            linked_at_ms: row.get(12)?,
        })
    })?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

pub fn create_reconciliation_run(
    database_path: &Path,
    definition: ReconciliationRunDefinition<'_>,
) -> Result<ReconciliationRunRecord, PersistenceError> {
    let ReconciliationRunDefinition {
        engagement_id,
        reconciliation_type,
        title,
        parameters_json,
        left_items,
        right_items,
    } = definition;

    let reconciliation_type = workflow_state_key(reconciliation_type);
    if reconciliation_type.is_empty() || reconciliation_type.chars().count() > 80 {
        return Err(PersistenceError::Configuration(
            "reconciliation type must contain 1 to 80 normalized characters".to_string(),
        ));
    }
    let title = normalize_domain_label(title, "reconciliation title", 240)?;
    let parameters_json = normalize_reconciliation_parameters(parameters_json)?;

    struct NormalizedItem {
        stable_id: String,
        match_key: String,
        amount_minor: i64,
        event_date_text: Option<String>,
        description_text: Option<String>,
        source_kind: String,
        source_entity_id: String,
        controlled_evidence_version_id: String,
        document_id: String,
        source_content_version_id: String,
        source_sha256: Vec<u8>,
        sheet_name: Option<String>,
        source_row_number: Option<u64>,
        source_row_hash: Option<Vec<u8>>,
    }

    fn normalize_item(item: &ReconciliationItemInput) -> Result<NormalizedItem, PersistenceError> {
        let match_key = normalize_reconciliation_match_key(&item.match_key)?;
        let source_kind = workflow_state_key(&item.source_kind);
        if source_kind.is_empty() || source_kind.chars().count() > 80 {
            return Err(PersistenceError::Configuration(
                "reconciliation source kind must contain 1 to 80 normalized characters".to_string(),
            ));
        }
        let source_entity_id = normalize_reconciliation_source_identifier(&item.source_entity_id)?;

        let event_date_text = match item.event_date_text.as_deref() {
            Some(value) => {
                let value = value.trim();
                if value.is_empty() {
                    None
                } else if value.chars().count() > 80
                    || value.chars().any(|character| character.is_control())
                {
                    return Err(PersistenceError::Configuration(
                        "reconciliation event date text must contain at most 80 printable characters"
                            .to_string(),
                    ));
                } else {
                    Some(value.to_string())
                }
            }
            None => None,
        };
        let description_text = match item.description_text.as_deref() {
            Some(value) => {
                let value = value.trim();
                if value.is_empty() {
                    None
                } else if value.chars().count() > 1_000
                    || value.chars().any(|character| character.is_control())
                {
                    return Err(PersistenceError::Configuration(
                        "reconciliation description must contain at most 1000 printable characters"
                            .to_string(),
                    ));
                } else {
                    Some(value.to_string())
                }
            }
            None => None,
        };

        if item.source_sha256.len() != 32 {
            return Err(PersistenceError::Configuration(
                "reconciliation source SHA-256 must contain exactly 32 bytes".to_string(),
            ));
        }
        if item.source_row_number.is_some() != item.source_row_hash.is_some() {
            return Err(PersistenceError::Configuration(
                "reconciliation row provenance requires both row number and row hash".to_string(),
            ));
        }
        if let Some(source_row_hash) = item.source_row_hash.as_deref() {
            if source_row_hash.len() != 32 {
                return Err(PersistenceError::Configuration(
                    "reconciliation source row hash must contain exactly 32 bytes".to_string(),
                ));
            }
        }
        if matches!(item.source_row_number, Some(0)) {
            return Err(PersistenceError::Configuration(
                "reconciliation source row number must be at least 1".to_string(),
            ));
        }

        Ok(NormalizedItem {
            stable_id: format!("{source_kind}\u{1f}{source_entity_id}"),
            match_key,
            amount_minor: item.amount_minor,
            event_date_text,
            description_text,
            source_kind,
            source_entity_id,
            controlled_evidence_version_id: item.controlled_evidence_version_id.clone(),
            document_id: item.document_id.clone(),
            source_content_version_id: item.source_content_version_id.clone(),
            source_sha256: item.source_sha256.clone(),
            sheet_name: normalize_reconciliation_sheet_name(item.sheet_name.as_deref())?,
            source_row_number: item.source_row_number,
            source_row_hash: item.source_row_hash.clone(),
        })
    }

    let normalized_left = left_items
        .iter()
        .map(normalize_item)
        .collect::<Result<Vec<_>, _>>()?;
    let normalized_right = right_items
        .iter()
        .map(normalize_item)
        .collect::<Result<Vec<_>, _>>()?;

    let engine_left = normalized_left
        .iter()
        .map(|item| crate::reconciliation::ExactReconciliationItem {
            stable_id: item.stable_id.clone(),
            match_key: item.match_key.clone(),
            amount_minor: item.amount_minor,
        })
        .collect::<Vec<_>>();
    let engine_right = normalized_right
        .iter()
        .map(|item| crate::reconciliation::ExactReconciliationItem {
            stable_id: item.stable_id.clone(),
            match_key: item.match_key.clone(),
            amount_minor: item.amount_minor,
        })
        .collect::<Vec<_>>();
    let outcome = crate::reconciliation::reconcile_exact_key_amount(&engine_left, &engine_right)
        .map_err(PersistenceError::Configuration)?;

    let mut connection = open_configured_connection(database_path)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

    let engagement_exists: bool = transaction.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM engagements
            WHERE engagement_id = ?1
              AND archived_at_ms IS NULL
        )",
        [engagement_id],
        |row| row.get(0),
    )?;
    if !engagement_exists {
        return Err(PersistenceError::Configuration(format!(
            "engagement {engagement_id} does not exist"
        )));
    }

    let mut verified_evidence = BTreeMap::new();
    for item in normalized_left.iter().chain(normalized_right.iter()) {
        if verified_evidence.contains_key(&item.controlled_evidence_version_id) {
            continue;
        }

        let evidence: Option<(String, String, Vec<u8>, String, String)> = transaction
            .query_row(
                "SELECT
                    document_id,
                    source_content_version_id,
                    sha256,
                    verification_state,
                    retention_state
                 FROM controlled_evidence_versions
                 WHERE controlled_evidence_version_id = ?1",
                [&item.controlled_evidence_version_id],
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
        let Some(evidence) = evidence else {
            return Err(PersistenceError::Configuration(format!(
                "controlled evidence version {} does not exist",
                item.controlled_evidence_version_id
            )));
        };
        if evidence.3 != "HASH_VERIFIED" || evidence.4 != "RETAINED" {
            return Err(PersistenceError::Configuration(
                "reconciliation items require retained hash-verified controlled evidence"
                    .to_string(),
            ));
        }
        verified_evidence.insert(item.controlled_evidence_version_id.clone(), evidence);
    }

    for item in normalized_left.iter().chain(normalized_right.iter()) {
        let evidence = verified_evidence
            .get(&item.controlled_evidence_version_id)
            .ok_or_else(|| {
                PersistenceError::Configuration(
                    "reconciliation controlled evidence validation is incomplete".to_string(),
                )
            })?;
        if evidence.0 != item.document_id
            || evidence.1 != item.source_content_version_id
            || evidence.2.as_slice() != item.source_sha256.as_slice()
        {
            return Err(PersistenceError::Configuration(
                "reconciliation item provenance does not match controlled evidence".to_string(),
            ));
        }
    }

    let reconciliation_run_id = Uuid::new_v4().to_string();
    let now = now_unix_ms()?;
    let exception_count =
        outcome.unmatched_left_stable_ids.len() + outcome.unmatched_right_stable_ids.len();

    transaction.execute(
        "INSERT INTO reconciliation_runs (
            reconciliation_run_id,
            engagement_id,
            reconciliation_type,
            title,
            rule_code,
            parameters_json,
            left_item_count,
            right_item_count,
            matched_pair_count,
            exception_count,
            ran_at_ms
         ) VALUES (?1, ?2, ?3, ?4, 'EXACT_KEY_AMOUNT', ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            &reconciliation_run_id,
            engagement_id,
            &reconciliation_type,
            &title,
            &parameters_json,
            u64_to_i64(normalized_left.len() as u64)?,
            u64_to_i64(normalized_right.len() as u64)?,
            u64_to_i64(outcome.matches.len() as u64)?,
            u64_to_i64(exception_count as u64)?,
            now
        ],
    )?;

    let mut left_item_ids = BTreeMap::new();
    let mut right_item_ids = BTreeMap::new();

    let insert_items = |side: &str,
                        items: &[NormalizedItem],
                        item_ids: &mut BTreeMap<String, String>|
     -> Result<(), PersistenceError> {
        for item in items {
            let reconciliation_item_id = Uuid::new_v4().to_string();
            transaction.execute(
                "INSERT INTO reconciliation_items (
                    reconciliation_item_id,
                    reconciliation_run_id,
                    side,
                    match_key,
                    amount_minor,
                    event_date_text,
                    description_text,
                    source_kind,
                    source_entity_id,
                    controlled_evidence_version_id,
                    document_id,
                    source_content_version_id,
                    source_sha256,
                    sheet_name,
                    source_row_number,
                    source_row_hash,
                    created_at_ms
                 ) VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9,
                    ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17
                 )",
                params![
                    &reconciliation_item_id,
                    &reconciliation_run_id,
                    side,
                    &item.match_key,
                    item.amount_minor,
                    item.event_date_text.as_deref(),
                    item.description_text.as_deref(),
                    &item.source_kind,
                    &item.source_entity_id,
                    &item.controlled_evidence_version_id,
                    &item.document_id,
                    &item.source_content_version_id,
                    &item.source_sha256,
                    item.sheet_name.as_deref(),
                    item.source_row_number.map(u64_to_i64).transpose()?,
                    item.source_row_hash.as_deref(),
                    now
                ],
            )?;
            item_ids.insert(item.stable_id.clone(), reconciliation_item_id);
        }
        Ok(())
    };

    insert_items("LEFT", &normalized_left, &mut left_item_ids)?;
    insert_items("RIGHT", &normalized_right, &mut right_item_ids)?;

    for matched in &outcome.matches {
        let left_item_id = left_item_ids.get(&matched.left_stable_id).ok_or_else(|| {
            PersistenceError::Configuration(
                "reconciliation left match resolution failed".to_string(),
            )
        })?;
        let right_item_id = right_item_ids
            .get(&matched.right_stable_id)
            .ok_or_else(|| {
                PersistenceError::Configuration(
                    "reconciliation right match resolution failed".to_string(),
                )
            })?;
        transaction.execute(
            "INSERT INTO reconciliation_matches (
                reconciliation_match_id,
                reconciliation_run_id,
                left_item_id,
                right_item_id,
                matched_at_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                Uuid::new_v4().to_string(),
                &reconciliation_run_id,
                left_item_id,
                right_item_id,
                now
            ],
        )?;
    }

    for (exception_code, stable_ids, item_ids) in [
        (
            "UNMATCHED_LEFT",
            &outcome.unmatched_left_stable_ids,
            &left_item_ids,
        ),
        (
            "UNMATCHED_RIGHT",
            &outcome.unmatched_right_stable_ids,
            &right_item_ids,
        ),
    ] {
        for stable_id in stable_ids {
            let reconciliation_item_id = item_ids.get(stable_id).ok_or_else(|| {
                PersistenceError::Configuration(
                    "reconciliation exception resolution failed".to_string(),
                )
            })?;
            transaction.execute(
                "INSERT INTO reconciliation_exceptions (
                    reconciliation_exception_id,
                    reconciliation_run_id,
                    reconciliation_item_id,
                    exception_code,
                    created_at_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    Uuid::new_v4().to_string(),
                    &reconciliation_run_id,
                    reconciliation_item_id,
                    exception_code,
                    now
                ],
            )?;
        }
    }

    insert_domain_audit_event(
        &transaction,
        DomainAuditEvent {
            event_type: "RECONCILIATION_RUN_COMPLETED",
            entity_type: "RECONCILIATION_RUN",
            entity_id: &reconciliation_run_id,
            related_entity_type: Some("ENGAGEMENT"),
            related_entity_id: Some(engagement_id),
            occurred_at_ms: now,
            details: json!({
                "reconciliationType": reconciliation_type,
                "ruleCode": "EXACT_KEY_AMOUNT",
                "leftItemCount": normalized_left.len(),
                "rightItemCount": normalized_right.len(),
                "matchedPairCount": outcome.matches.len(),
                "exceptionCount": exception_count
            }),
        },
    )?;

    transaction.commit()?;

    Ok(ReconciliationRunRecord {
        reconciliation_run_id,
        engagement_id: engagement_id.to_string(),
        reconciliation_type,
        title,
        rule_code: "EXACT_KEY_AMOUNT".to_string(),
        parameters_json,
        left_item_count: normalized_left.len() as u64,
        right_item_count: normalized_right.len() as u64,
        matched_pair_count: outcome.matches.len() as u64,
        exception_count: exception_count as u64,
        ran_at_ms: now,
    })
}

pub fn run_trial_balance_opening_closing_reconciliation(
    database_path: &Path,
    trial_balance_import_id: &str,
) -> Result<ReconciliationRunRecord, PersistenceError> {
    let connection = open_configured_connection(database_path)?;
    let source = connection
        .query_row(
            "SELECT
                engagement_id,
                controlled_evidence_version_id,
                document_id,
                source_content_version_id,
                source_sha256,
                sheet_name,
                amount_scale
             FROM trial_balance_imports
             WHERE trial_balance_import_id = ?1",
            [trial_balance_import_id],
            |row| {
                let amount_scale: i64 = row.get(6)?;
                Ok(TrialBalanceReconciliationSource {
                    engagement_id: row.get(0)?,
                    controlled_evidence_version_id: row.get(1)?,
                    document_id: row.get(2)?,
                    source_content_version_id: row.get(3)?,
                    source_sha256: row.get(4)?,
                    sheet_name: row.get(5)?,
                    amount_scale: amount_scale.max(0) as u32,
                })
            },
        )
        .optional()?;
    let Some(source) = source else {
        return Err(PersistenceError::Configuration(format!(
            "trial balance import {trial_balance_import_id} does not exist"
        )));
    };
    drop(connection);

    let accounts = list_trial_balance_accounts(database_path, trial_balance_import_id)?;
    let mut left_items = Vec::with_capacity(accounts.len());
    let mut right_items = Vec::with_capacity(accounts.len());

    for account in &accounts {
        let account_label = account
            .account_code_text
            .as_deref()
            .map(|code| format!("{code} - {}", account.account_name_text))
            .unwrap_or_else(|| account.account_name_text.clone());

        let common = |amount_minor: i64, description_text: String| ReconciliationItemInput {
            match_key: account.trial_balance_account_id.clone(),
            amount_minor,
            event_date_text: None,
            description_text: Some(description_text),
            source_kind: "TRIAL_BALANCE_ACCOUNT".to_string(),
            source_entity_id: account.trial_balance_account_id.clone(),
            controlled_evidence_version_id: source.controlled_evidence_version_id.clone(),
            document_id: source.document_id.clone(),
            source_content_version_id: source.source_content_version_id.clone(),
            source_sha256: source.source_sha256.clone(),
            sheet_name: Some(source.sheet_name.clone()),
            source_row_number: Some(account.source_row_number),
            source_row_hash: Some(account.source_row_hash.clone()),
        };

        left_items.push(common(
            account.opening_minor,
            format!("Opening balance - {account_label}"),
        ));
        right_items.push(common(
            account.closing_minor,
            format!("Closing balance - {account_label}"),
        ));
    }

    let parameters_json = json!({
        "trialBalanceImportId": trial_balance_import_id,
        "amountScale": source.amount_scale,
        "leftSide": "OPENING",
        "rightSide": "CLOSING",
        "comparison": "EXACT_ACCOUNT_AND_AMOUNT"
    })
    .to_string();

    create_reconciliation_run(
        database_path,
        ReconciliationRunDefinition {
            engagement_id: &source.engagement_id,
            reconciliation_type: "TRIAL_BALANCE_OPENING_CLOSING",
            title: "Trial Balance opening vs closing exact reconciliation",
            parameters_json: &parameters_json,
            left_items: &left_items,
            right_items: &right_items,
        },
    )
}

pub fn list_reconciliation_runs(
    database_path: &Path,
    engagement_id: &str,
) -> Result<Vec<ReconciliationRunRecord>, PersistenceError> {
    let connection = open_configured_connection(database_path)?;
    let mut statement = connection.prepare(
        "SELECT
            reconciliation_run_id,
            engagement_id,
            reconciliation_type,
            title,
            rule_code,
            parameters_json,
            left_item_count,
            right_item_count,
            matched_pair_count,
            exception_count,
            ran_at_ms
         FROM reconciliation_runs
         WHERE engagement_id = ?1
         ORDER BY ran_at_ms DESC, reconciliation_run_id DESC",
    )?;
    let rows = statement.query_map([engagement_id], |row| {
        Ok(ReconciliationRunRecord {
            reconciliation_run_id: row.get(0)?,
            engagement_id: row.get(1)?,
            reconciliation_type: row.get(2)?,
            title: row.get(3)?,
            rule_code: row.get(4)?,
            parameters_json: row.get(5)?,
            left_item_count: row.get::<_, i64>(6)?.max(0) as u64,
            right_item_count: row.get::<_, i64>(7)?.max(0) as u64,
            matched_pair_count: row.get::<_, i64>(8)?.max(0) as u64,
            exception_count: row.get::<_, i64>(9)?.max(0) as u64,
            ran_at_ms: row.get(10)?,
        })
    })?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

pub fn list_reconciliation_exceptions(
    database_path: &Path,
    reconciliation_run_id: &str,
) -> Result<Vec<ReconciliationExceptionRecord>, PersistenceError> {
    let connection = open_configured_connection(database_path)?;
    let run_exists: bool = connection.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM reconciliation_runs
            WHERE reconciliation_run_id = ?1
        )",
        [reconciliation_run_id],
        |row| row.get(0),
    )?;
    if !run_exists {
        return Err(PersistenceError::Configuration(format!(
            "reconciliation run {reconciliation_run_id} does not exist"
        )));
    }

    let mut statement = connection.prepare(
        "SELECT
            e.reconciliation_exception_id,
            e.reconciliation_run_id,
            i.reconciliation_item_id,
            e.exception_code,
            i.side,
            i.match_key,
            i.amount_minor,
            i.event_date_text,
            i.description_text,
            i.source_kind,
            i.source_entity_id,
            i.controlled_evidence_version_id,
            i.document_id,
            i.source_content_version_id,
            i.source_sha256,
            i.sheet_name,
            i.source_row_number,
            i.source_row_hash,
            e.created_at_ms
         FROM reconciliation_exceptions e
         JOIN reconciliation_items i
           ON i.reconciliation_item_id = e.reconciliation_item_id
         WHERE e.reconciliation_run_id = ?1
         ORDER BY
            CASE e.exception_code
                WHEN 'UNMATCHED_LEFT' THEN 0
                ELSE 1
            END,
            i.match_key,
            i.amount_minor,
            i.source_kind,
            i.source_entity_id",
    )?;
    let rows = statement.query_map([reconciliation_run_id], |row| {
        let source_row_number: Option<i64> = row.get(16)?;
        Ok(ReconciliationExceptionRecord {
            reconciliation_exception_id: row.get(0)?,
            reconciliation_run_id: row.get(1)?,
            reconciliation_item_id: row.get(2)?,
            exception_code: row.get(3)?,
            side: row.get(4)?,
            match_key: row.get(5)?,
            amount_minor: row.get(6)?,
            event_date_text: row.get(7)?,
            description_text: row.get(8)?,
            source_kind: row.get(9)?,
            source_entity_id: row.get(10)?,
            controlled_evidence_version_id: row.get(11)?,
            document_id: row.get(12)?,
            source_content_version_id: row.get(13)?,
            source_sha256: row.get(14)?,
            sheet_name: row.get(15)?,
            source_row_number: source_row_number.map(|value| value.max(0) as u64),
            source_row_hash: row.get(17)?,
            created_at_ms: row.get(18)?,
        })
    })?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

pub fn create_engagement_from_template(
    database_path: &Path,
    engagement_template_version_id: &str,
    client_id: &str,
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

    let template_version: Option<(String, String)> = transaction
        .query_row(
            "SELECT
                v.service_type_id,
                v.definition_json
             FROM engagement_template_versions v
             JOIN engagement_templates t
               ON t.engagement_template_id = v.engagement_template_id
             WHERE v.engagement_template_version_id = ?1
               AND t.archived_at_ms IS NULL",
            [engagement_template_version_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let (service_type_id, definition_json) = template_version.ok_or_else(|| {
        PersistenceError::Configuration(format!(
            "engagement template version {engagement_template_version_id} does not exist"
        ))
    })?;

    let service_exists: bool = transaction.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM service_types
            WHERE service_type_id = ?1 AND archived_at_ms IS NULL
        )",
        [&service_type_id],
        |row| row.get(0),
    )?;
    if !service_exists {
        return Err(PersistenceError::Configuration(
            "engagement template service type is unavailable".to_string(),
        ));
    }

    let definition: EngagementTemplateDefinition =
        serde_json::from_str(&definition_json).map_err(|error| {
            PersistenceError::Configuration(format!(
                "engagement template definition is invalid: {error}"
            ))
        })?;

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
            &service_type_id,
            &name,
            period_start.as_deref(),
            period_end.as_deref(),
            &status,
            now
        ],
    )?;

    let mut area_id_map: Vec<(String, String)> = Vec::new();
    let mut pending_areas = definition.areas;
    while !pending_areas.is_empty() {
        let pending_count = pending_areas.len();
        let mut next_pending = Vec::new();

        for area in pending_areas {
            let parent_area_id = match area.parent_source_area_id.as_deref() {
                None => None,
                Some(parent_source_area_id) => {
                    let mapped = area_id_map
                        .iter()
                        .find(|(source_id, _)| source_id == parent_source_area_id)
                        .map(|(_, new_id)| new_id.clone());
                    let Some(mapped) = mapped else {
                        next_pending.push(area);
                        continue;
                    };
                    Some(mapped)
                }
            };

            let area_name = normalize_domain_label(&area.name, "template area name", 160)?;
            let area_code = normalize_optional_domain_text(area.code.as_deref(), 40);
            let area_status = normalize_domain_label(&area.status, "template area status", 80)?;
            let engagement_area_id = Uuid::new_v4().to_string();
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
                    &engagement_id,
                    parent_area_id.as_deref(),
                    &area_name,
                    area_code.as_deref(),
                    area.display_order,
                    &area_status,
                    now
                ],
            )?;
            area_id_map.push((area.source_area_id, engagement_area_id));
        }

        if next_pending.len() == pending_count {
            return Err(PersistenceError::Configuration(
                "engagement template area hierarchy is invalid".to_string(),
            ));
        }
        pending_areas = next_pending;
    }

    for procedure in definition.procedures {
        let engagement_area_id = match procedure.source_area_id.as_deref() {
            None => None,
            Some(source_area_id) => Some(
                area_id_map
                    .iter()
                    .find(|(source_id, _)| source_id == source_area_id)
                    .map(|(_, new_id)| new_id.clone())
                    .ok_or_else(|| {
                        PersistenceError::Configuration(
                            "engagement template procedure area is unavailable".to_string(),
                        )
                    })?,
            ),
        };
        let title = normalize_domain_label(&procedure.title, "template procedure title", 240)?;
        let reference = normalize_optional_domain_text(procedure.reference.as_deref(), 80);
        let description = procedure
            .description
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let procedure_status =
            normalize_domain_label(&procedure.status, "template procedure status", 80)?;
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
                Uuid::new_v4().to_string(),
                &engagement_id,
                engagement_area_id.as_deref(),
                reference.as_deref(),
                &title,
                description.as_deref(),
                &procedure_status,
                now
            ],
        )?;
    }

    transaction.commit()?;
    Ok(EngagementRecord {
        engagement_id,
        client_id: client_id.to_string(),
        service_type_id,
        name,
        period_start,
        period_end,
        status,
        created_at_ms: now,
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

    if let Some(previous_revision_id) = supersedes_revision_id.as_deref() {
        let supersede_reason = revision_reason
            .as_deref()
            .map(|reason| format!("New workpaper revision created: {reason}"))
            .unwrap_or_else(|| "New workpaper revision created.".to_string());
        supersede_revision_signoffs(
            &transaction,
            previous_revision_id,
            &revision_id,
            now,
            &supersede_reason,
            None,
        )?;
    }

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

    let active_signoff_count: i64 = transaction.query_row(
        "SELECT COUNT(*)
         FROM workpaper_signoffs s
         LEFT JOIN workpaper_signoff_supersessions ss ON ss.signoff_id = s.signoff_id
         WHERE s.workpaper_revision_id = ?1
           AND ss.signoff_id IS NULL",
        [workpaper_revision_id],
        |row| row.get(0),
    )?;
    if active_signoff_count > 0 {
        return Err(PersistenceError::Configuration(
            "signed workpaper revision cannot receive additional evidence; create a new revision"
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

fn signoff_type_key(value: &str) -> String {
    workflow_state_key(value)
}

fn signoff_requires_controlled_evidence(value: &str) -> bool {
    matches!(
        signoff_type_key(value).as_str(),
        "REVIEWED" | "FINAL_APPROVAL" | "FINAL"
    )
}

fn supersede_revision_signoffs(
    transaction: &rusqlite::Transaction<'_>,
    workpaper_revision_id: &str,
    superseded_by_revision_id: &str,
    superseded_at_ms: i64,
    reason: &str,
    actor_id: Option<&str>,
) -> Result<Vec<String>, PersistenceError> {
    let signoff_ids = {
        let mut statement = transaction.prepare(
            "SELECT s.signoff_id
             FROM workpaper_signoffs s
             LEFT JOIN workpaper_signoff_supersessions ss ON ss.signoff_id = s.signoff_id
             WHERE s.workpaper_revision_id = ?1
               AND ss.signoff_id IS NULL
             ORDER BY s.signed_at_ms, s.rowid",
        )?;
        let rows = statement.query_map([workpaper_revision_id], |row| row.get::<_, String>(0))?;
        let mut ids = Vec::new();
        for row in rows {
            ids.push(row?);
        }
        ids
    };

    for signoff_id in &signoff_ids {
        transaction.execute(
            "INSERT INTO workpaper_signoff_supersessions (
                signoff_supersession_id,
                signoff_id,
                superseded_by_revision_id,
                superseded_at_ms,
                superseded_reason,
                actor_id
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                Uuid::new_v4().to_string(),
                signoff_id,
                superseded_by_revision_id,
                superseded_at_ms,
                reason,
                actor_id
            ],
        )?;

        insert_domain_audit_event(
            transaction,
            DomainAuditEvent {
                event_type: "WORKPAPER_SIGNOFF_SUPERSEDED",
                entity_type: "WORKPAPER_SIGNOFF",
                entity_id: signoff_id,
                related_entity_type: Some("WORKPAPER_REVISION"),
                related_entity_id: Some(superseded_by_revision_id),
                occurred_at_ms: superseded_at_ms,
                details: json!({
                    "supersededRevisionId": workpaper_revision_id,
                    "supersededByRevisionId": superseded_by_revision_id,
                    "reason": reason,
                    "actorId": actor_id
                }),
            },
        )?;
    }

    Ok(signoff_ids)
}

pub fn create_workpaper_signoff(
    database_path: &Path,
    signoff: NewWorkpaperSignoff<'_>,
) -> Result<WorkpaperSignoffRecord, PersistenceError> {
    let signoff_type = normalize_domain_label(signoff.signoff_type, "sign-off type", 80)?;
    let actor_id = normalize_domain_label(signoff.actor_id, "sign-off actor", 160)?;
    let actor_role = normalize_domain_label(signoff.actor_role, "sign-off actor role", 160)?;
    let comment = signoff
        .comment
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
            [signoff.workpaper_revision_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    let Some((revision_workpaper_id, revision_number, latest_revision_number)) = revision_context
    else {
        return Err(PersistenceError::Configuration(format!(
            "workpaper revision {} does not exist",
            signoff.workpaper_revision_id
        )));
    };
    if revision_workpaper_id != signoff.workpaper_id {
        return Err(PersistenceError::Configuration(
            "sign-off revision must belong to the selected workpaper".to_string(),
        ));
    }
    if revision_number != latest_revision_number {
        return Err(PersistenceError::Configuration(
            "sign-off can only be recorded against the latest workpaper revision".to_string(),
        ));
    }

    if signoff_requires_controlled_evidence(&signoff_type) {
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
            [signoff.workpaper_revision_id],
            |row| row.get(0),
        )?;
        if unsafe_evidence_count > 0 {
            return Err(PersistenceError::Configuration(
                "reviewer/final sign-off requires all linked evidence to be immutable hash-verified controlled evidence"
                    .to_string(),
            ));
        }

        let unresolved_review_notes: i64 = transaction.query_row(
            "SELECT COUNT(*)
             FROM review_notes
             WHERE workpaper_revision_id = ?1
               AND current_state <> 'CLEARED'",
            [signoff.workpaper_revision_id],
            |row| row.get(0),
        )?;
        if unresolved_review_notes > 0 {
            return Err(PersistenceError::Configuration(
                "reviewer/final sign-off requires all review notes on the revision to be cleared"
                    .to_string(),
            ));
        }
    }

    let evidence_link_ids = {
        let mut statement = transaction.prepare(
            "SELECT evidence_link_id
             FROM workpaper_evidence_links
             WHERE workpaper_revision_id = ?1
             ORDER BY created_at_ms, rowid",
        )?;
        let rows = statement.query_map([signoff.workpaper_revision_id], |row| {
            row.get::<_, String>(0)
        })?;
        let mut ids = Vec::new();
        for row in rows {
            ids.push(row?);
        }
        ids
    };

    let signoff_id = Uuid::new_v4().to_string();
    let signed_at_ms = now_unix_ms()?;
    transaction.execute(
        "INSERT INTO workpaper_signoffs (
            signoff_id,
            workpaper_id,
            workpaper_revision_id,
            signoff_type,
            actor_id,
            actor_role,
            signed_at_ms,
            comment
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            &signoff_id,
            signoff.workpaper_id,
            signoff.workpaper_revision_id,
            &signoff_type,
            &actor_id,
            &actor_role,
            signed_at_ms,
            comment.as_deref()
        ],
    )?;

    for evidence_link_id in &evidence_link_ids {
        transaction.execute(
            "INSERT INTO workpaper_signoff_evidence (
                signoff_evidence_id,
                signoff_id,
                evidence_link_id,
                created_at_ms
             ) VALUES (?1, ?2, ?3, ?4)",
            params![
                Uuid::new_v4().to_string(),
                &signoff_id,
                evidence_link_id,
                signed_at_ms
            ],
        )?;
    }

    insert_domain_audit_event(
        &transaction,
        DomainAuditEvent {
            event_type: "WORKPAPER_SIGNOFF_CREATED",
            entity_type: "WORKPAPER_SIGNOFF",
            entity_id: &signoff_id,
            related_entity_type: Some("WORKPAPER_REVISION"),
            related_entity_id: Some(signoff.workpaper_revision_id),
            occurred_at_ms: signed_at_ms,
            details: json!({
                "workpaperId": signoff.workpaper_id,
                "revisionNumber": revision_number,
                "signoffType": signoff_type,
                "actorId": actor_id,
                "actorRole": actor_role,
                "evidenceLinkIds": evidence_link_ids
            }),
        },
    )?;

    transaction.commit()?;
    Ok(WorkpaperSignoffRecord {
        signoff_id,
        workpaper_id: signoff.workpaper_id.to_string(),
        workpaper_revision_id: signoff.workpaper_revision_id.to_string(),
        revision_number: revision_number.max(0) as u64,
        signoff_type,
        actor_id,
        actor_role,
        signed_at_ms,
        comment,
        evidence_link_ids,
        superseded_at_ms: None,
        superseded_reason: None,
        superseded_by_revision_id: None,
    })
}

pub fn list_workpaper_signoffs(
    database_path: &Path,
    workpaper_id: &str,
) -> Result<Vec<WorkpaperSignoffRecord>, PersistenceError> {
    let connection = open_configured_connection(database_path)?;
    let base_rows = {
        let mut statement = connection.prepare(
            "SELECT
                s.signoff_id,
                s.workpaper_id,
                s.workpaper_revision_id,
                wr.revision_number,
                s.signoff_type,
                s.actor_id,
                s.actor_role,
                s.signed_at_ms,
                s.comment,
                ss.superseded_at_ms,
                ss.superseded_reason,
                ss.superseded_by_revision_id
             FROM workpaper_signoffs s
             JOIN workpaper_revisions wr
               ON wr.workpaper_revision_id = s.workpaper_revision_id
             LEFT JOIN workpaper_signoff_supersessions ss
               ON ss.signoff_id = s.signoff_id
             WHERE s.workpaper_id = ?1
             ORDER BY wr.revision_number DESC, s.signed_at_ms DESC, s.rowid DESC",
        )?;
        let rows = statement.query_map([workpaper_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, i64>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, Option<i64>>(9)?,
                row.get::<_, Option<String>>(10)?,
                row.get::<_, Option<String>>(11)?,
            ))
        })?;
        let mut values = Vec::new();
        for row in rows {
            values.push(row?);
        }
        values
    };

    let mut result = Vec::new();
    for (
        signoff_id,
        row_workpaper_id,
        workpaper_revision_id,
        revision_number,
        signoff_type,
        actor_id,
        actor_role,
        signed_at_ms,
        comment,
        superseded_at_ms,
        superseded_reason,
        superseded_by_revision_id,
    ) in base_rows
    {
        let mut statement = connection.prepare(
            "SELECT evidence_link_id
             FROM workpaper_signoff_evidence
             WHERE signoff_id = ?1
             ORDER BY created_at_ms, rowid",
        )?;
        let rows = statement.query_map([&signoff_id], |row| row.get::<_, String>(0))?;
        let mut evidence_link_ids = Vec::new();
        for row in rows {
            evidence_link_ids.push(row?);
        }

        result.push(WorkpaperSignoffRecord {
            signoff_id,
            workpaper_id: row_workpaper_id,
            workpaper_revision_id,
            revision_number: revision_number.max(0) as u64,
            signoff_type,
            actor_id,
            actor_role,
            signed_at_ms,
            comment,
            evidence_link_ids,
            superseded_at_ms,
            superseded_reason,
            superseded_by_revision_id,
        });
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
    let (location_kind, location_value) = normalize_review_note_location(
        note.evidence_link_id,
        note.location_kind,
        note.location_value,
    )?;
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

pub fn create_pbc_request(
    database_path: &Path,
    request: NewPbcRequest<'_>,
) -> Result<PbcRequestRecord, PersistenceError> {
    let request_number = normalize_domain_label(request.request_number, "PBC request number", 80)?;
    let description = request.description.trim().to_string();
    if description.is_empty() {
        return Err(PersistenceError::Configuration(
            "PBC request description must not be empty".to_string(),
        ));
    }
    let requested_from_party =
        normalize_domain_label(request.requested_from_party, "requested-from party", 200)?;
    let status = normalize_domain_label(request.status, "PBC request status", 80)?;
    let client_visible_content = request
        .client_visible_content
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let internal_notes = request
        .internal_notes
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    let mut connection = open_configured_connection(database_path)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

    let engagement_exists: bool = transaction.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM engagements
            WHERE engagement_id = ?1
              AND archived_at_ms IS NULL
        )",
        [request.engagement_id],
        |row| row.get(0),
    )?;
    if !engagement_exists {
        return Err(PersistenceError::Configuration(format!(
            "engagement {} does not exist",
            request.engagement_id
        )));
    }

    if let Some(area_id) = request.engagement_area_id {
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
        if area_engagement.as_deref() != Some(request.engagement_id) {
            return Err(PersistenceError::Configuration(
                "PBC request area must belong to the same engagement".to_string(),
            ));
        }
    }

    let request_id = Uuid::new_v4().to_string();
    let event_id = Uuid::new_v4().to_string();
    let now = now_unix_ms()?;
    transaction.execute(
        "INSERT INTO pbc_requests (
            pbc_request_id,
            engagement_id,
            engagement_area_id,
            request_number,
            description,
            requested_from_party,
            due_at_ms,
            status,
            client_visible_content,
            internal_notes,
            created_at_ms,
            updated_at_ms,
            archived_at_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11, NULL)",
        params![
            &request_id,
            request.engagement_id,
            request.engagement_area_id,
            &request_number,
            &description,
            &requested_from_party,
            request.due_at_ms,
            &status,
            client_visible_content.as_deref(),
            internal_notes.as_deref(),
            now
        ],
    )?;
    transaction.execute(
        "INSERT INTO pbc_request_events (
            pbc_request_event_id,
            pbc_request_id,
            event_type,
            actor_id,
            from_status,
            to_status,
            assessment_text,
            comment,
            occurred_at_ms
         ) VALUES (?1, ?2, 'CREATED', NULL, NULL, ?3, NULL, NULL, ?4)",
        params![&event_id, &request_id, &status, now],
    )?;

    insert_domain_audit_event(
        &transaction,
        DomainAuditEvent {
            event_type: "PBC_REQUEST_CREATED",
            entity_type: "PBC_REQUEST",
            entity_id: &request_id,
            related_entity_type: Some("ENGAGEMENT"),
            related_entity_id: Some(request.engagement_id),
            occurred_at_ms: now,
            details: json!({
                "requestNumber": request_number,
                "engagementAreaId": request.engagement_area_id,
                "requestedFromParty": requested_from_party,
                "dueAtMs": request.due_at_ms,
                "status": status,
                "hasClientVisibleContent": client_visible_content.is_some(),
                "hasInternalNotes": internal_notes.is_some()
            }),
        },
    )?;

    transaction.commit()?;
    Ok(PbcRequestRecord {
        pbc_request_id: request_id,
        engagement_id: request.engagement_id.to_string(),
        engagement_area_id: request.engagement_area_id.map(str::to_string),
        request_number,
        description,
        requested_from_party,
        due_at_ms: request.due_at_ms,
        status,
        client_visible_content,
        internal_notes,
        latest_assessment: None,
        created_at_ms: now,
        updated_at_ms: now,
    })
}

pub fn list_pbc_requests(
    database_path: &Path,
    engagement_id: &str,
) -> Result<Vec<PbcRequestRecord>, PersistenceError> {
    let connection = open_configured_connection(database_path)?;
    let mut statement = connection.prepare(
        "SELECT
            p.pbc_request_id,
            p.engagement_id,
            p.engagement_area_id,
            p.request_number,
            p.description,
            p.requested_from_party,
            p.due_at_ms,
            p.status,
            p.client_visible_content,
            p.internal_notes,
            (
                SELECT e.assessment_text
                FROM pbc_request_events e
                WHERE e.pbc_request_id = p.pbc_request_id
                  AND e.event_type = 'ASSESSMENT_ADDED'
                ORDER BY e.occurred_at_ms DESC, e.rowid DESC
                LIMIT 1
            ),
            p.created_at_ms,
            p.updated_at_ms
         FROM pbc_requests p
         WHERE p.engagement_id = ?1
           AND p.archived_at_ms IS NULL
         ORDER BY
            CASE WHEN p.due_at_ms IS NULL THEN 1 ELSE 0 END,
            p.due_at_ms,
            p.request_number COLLATE NOCASE",
    )?;
    let rows = statement.query_map([engagement_id], |row| {
        Ok(PbcRequestRecord {
            pbc_request_id: row.get(0)?,
            engagement_id: row.get(1)?,
            engagement_area_id: row.get(2)?,
            request_number: row.get(3)?,
            description: row.get(4)?,
            requested_from_party: row.get(5)?,
            due_at_ms: row.get(6)?,
            status: row.get(7)?,
            client_visible_content: row.get(8)?,
            internal_notes: row.get(9)?,
            latest_assessment: row.get(10)?,
            created_at_ms: row.get(11)?,
            updated_at_ms: row.get(12)?,
        })
    })?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

pub fn transition_pbc_request_status(
    database_path: &Path,
    pbc_request_id: &str,
    to_status: &str,
    actor_id: Option<&str>,
    comment: Option<&str>,
) -> Result<PbcRequestEventRecord, PersistenceError> {
    let to_status = normalize_domain_label(to_status, "PBC request status", 80)?;
    let actor_id = normalize_optional_domain_text(actor_id, 160);
    let comment = comment
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    let mut connection = open_configured_connection(database_path)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let from_status: Option<String> = transaction
        .query_row(
            "SELECT status
             FROM pbc_requests
             WHERE pbc_request_id = ?1
               AND archived_at_ms IS NULL",
            [pbc_request_id],
            |row| row.get(0),
        )
        .optional()?;
    let Some(from_status) = from_status else {
        return Err(PersistenceError::Configuration(format!(
            "PBC request {pbc_request_id} does not exist"
        )));
    };

    let event_id = Uuid::new_v4().to_string();
    let now = now_unix_ms()?;
    transaction.execute(
        "INSERT INTO pbc_request_events (
            pbc_request_event_id,
            pbc_request_id,
            event_type,
            actor_id,
            from_status,
            to_status,
            assessment_text,
            comment,
            occurred_at_ms
         ) VALUES (?1, ?2, 'STATUS_CHANGED', ?3, ?4, ?5, NULL, ?6, ?7)",
        params![
            &event_id,
            pbc_request_id,
            actor_id.as_deref(),
            &from_status,
            &to_status,
            comment.as_deref(),
            now
        ],
    )?;
    transaction.execute(
        "UPDATE pbc_requests
         SET status = ?1,
             updated_at_ms = ?2
         WHERE pbc_request_id = ?3",
        params![&to_status, now, pbc_request_id],
    )?;

    insert_domain_audit_event(
        &transaction,
        DomainAuditEvent {
            event_type: "PBC_REQUEST_STATUS_CHANGED",
            entity_type: "PBC_REQUEST",
            entity_id: pbc_request_id,
            related_entity_type: None,
            related_entity_id: None,
            occurred_at_ms: now,
            details: json!({
                "fromStatus": from_status,
                "toStatus": to_status,
                "actorId": actor_id,
                "comment": comment
            }),
        },
    )?;
    transaction.commit()?;

    Ok(PbcRequestEventRecord {
        pbc_request_event_id: event_id,
        pbc_request_id: pbc_request_id.to_string(),
        event_type: "STATUS_CHANGED".to_string(),
        actor_id,
        from_status: Some(from_status),
        to_status: Some(to_status),
        assessment_text: None,
        comment,
        occurred_at_ms: now,
    })
}

pub fn add_pbc_request_assessment(
    database_path: &Path,
    pbc_request_id: &str,
    assessment_text: &str,
    actor_id: Option<&str>,
) -> Result<PbcRequestEventRecord, PersistenceError> {
    let assessment_text = assessment_text.trim().to_string();
    if assessment_text.is_empty() {
        return Err(PersistenceError::Configuration(
            "PBC auditor assessment must not be empty".to_string(),
        ));
    }
    let actor_id = normalize_optional_domain_text(actor_id, 160);

    let mut connection = open_configured_connection(database_path)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let request_exists: bool = transaction.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM pbc_requests
            WHERE pbc_request_id = ?1
              AND archived_at_ms IS NULL
        )",
        [pbc_request_id],
        |row| row.get(0),
    )?;
    if !request_exists {
        return Err(PersistenceError::Configuration(format!(
            "PBC request {pbc_request_id} does not exist"
        )));
    }

    let event_id = Uuid::new_v4().to_string();
    let now = now_unix_ms()?;
    transaction.execute(
        "INSERT INTO pbc_request_events (
            pbc_request_event_id,
            pbc_request_id,
            event_type,
            actor_id,
            from_status,
            to_status,
            assessment_text,
            comment,
            occurred_at_ms
         ) VALUES (?1, ?2, 'ASSESSMENT_ADDED', ?3, NULL, NULL, ?4, NULL, ?5)",
        params![
            &event_id,
            pbc_request_id,
            actor_id.as_deref(),
            &assessment_text,
            now
        ],
    )?;
    transaction.execute(
        "UPDATE pbc_requests
         SET updated_at_ms = ?1
         WHERE pbc_request_id = ?2",
        params![now, pbc_request_id],
    )?;

    insert_domain_audit_event(
        &transaction,
        DomainAuditEvent {
            event_type: "PBC_REQUEST_ASSESSMENT_ADDED",
            entity_type: "PBC_REQUEST",
            entity_id: pbc_request_id,
            related_entity_type: None,
            related_entity_id: None,
            occurred_at_ms: now,
            details: json!({
                "actorId": actor_id,
                "assessmentLength": assessment_text.chars().count()
            }),
        },
    )?;
    transaction.commit()?;

    Ok(PbcRequestEventRecord {
        pbc_request_event_id: event_id,
        pbc_request_id: pbc_request_id.to_string(),
        event_type: "ASSESSMENT_ADDED".to_string(),
        actor_id,
        from_status: None,
        to_status: None,
        assessment_text: Some(assessment_text),
        comment: None,
        occurred_at_ms: now,
    })
}

pub fn list_pbc_request_events(
    database_path: &Path,
    pbc_request_id: &str,
) -> Result<Vec<PbcRequestEventRecord>, PersistenceError> {
    let connection = open_configured_connection(database_path)?;
    let mut statement = connection.prepare(
        "SELECT
            pbc_request_event_id,
            pbc_request_id,
            event_type,
            actor_id,
            from_status,
            to_status,
            assessment_text,
            comment,
            occurred_at_ms
         FROM pbc_request_events
         WHERE pbc_request_id = ?1
         ORDER BY occurred_at_ms, rowid",
    )?;
    let rows = statement.query_map([pbc_request_id], |row| {
        Ok(PbcRequestEventRecord {
            pbc_request_event_id: row.get(0)?,
            pbc_request_id: row.get(1)?,
            event_type: row.get(2)?,
            actor_id: row.get(3)?,
            from_status: row.get(4)?,
            to_status: row.get(5)?,
            assessment_text: row.get(6)?,
            comment: row.get(7)?,
            occurred_at_ms: row.get(8)?,
        })
    })?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

pub fn create_pbc_request_evidence_link(
    database_path: &Path,
    pbc_request_id: &str,
    document_id: &str,
    content_version_id: Option<&str>,
    controlled_evidence_version_id: Option<&str>,
    description: Option<&str>,
) -> Result<PbcRequestEvidenceLinkRecord, PersistenceError> {
    if content_version_id.is_some() == controlled_evidence_version_id.is_some() {
        return Err(PersistenceError::Configuration(
            "PBC received evidence must reference exactly one content version or controlled evidence version"
                .to_string(),
        ));
    }
    let description = description
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    let mut connection = open_configured_connection(database_path)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let request_exists: bool = transaction.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM pbc_requests
            WHERE pbc_request_id = ?1
              AND archived_at_ms IS NULL
        )",
        [pbc_request_id],
        |row| row.get(0),
    )?;
    if !request_exists {
        return Err(PersistenceError::Configuration(format!(
            "PBC request {pbc_request_id} does not exist"
        )));
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

    let link_id = Uuid::new_v4().to_string();
    let now = now_unix_ms()?;
    transaction.execute(
        "INSERT INTO pbc_request_evidence_links (
            pbc_request_evidence_link_id,
            pbc_request_id,
            document_id,
            content_version_id,
            controlled_evidence_version_id,
            description,
            created_at_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            &link_id,
            pbc_request_id,
            document_id,
            content_version_id,
            controlled_evidence_version_id,
            description.as_deref(),
            now
        ],
    )?;

    insert_domain_audit_event(
        &transaction,
        DomainAuditEvent {
            event_type: "PBC_REQUEST_EVIDENCE_LINKED",
            entity_type: "PBC_REQUEST_EVIDENCE_LINK",
            entity_id: &link_id,
            related_entity_type: Some("PBC_REQUEST"),
            related_entity_id: Some(pbc_request_id),
            occurred_at_ms: now,
            details: json!({
                "documentId": document_id,
                "contentVersionId": content_version_id,
                "controlledEvidenceVersionId": controlled_evidence_version_id,
                "description": description
            }),
        },
    )?;
    transaction.commit()?;

    Ok(PbcRequestEvidenceLinkRecord {
        pbc_request_evidence_link_id: link_id,
        pbc_request_id: pbc_request_id.to_string(),
        document_id: document_id.to_string(),
        document_name,
        content_version_id: content_version_id.map(str::to_string),
        content_observed_at_ms,
        controlled_evidence_version_id: controlled_evidence_version_id.map(str::to_string),
        controlled_version_number,
        controlled_captured_at_ms,
        description,
        created_at_ms: now,
    })
}

pub fn list_pbc_request_evidence_links(
    database_path: &Path,
    pbc_request_id: &str,
) -> Result<Vec<PbcRequestEvidenceLinkRecord>, PersistenceError> {
    let connection = open_configured_connection(database_path)?;
    let mut statement = connection.prepare(
        "SELECT
            pel.pbc_request_evidence_link_id,
            pel.pbc_request_id,
            pel.document_id,
            d.display_name,
            pel.content_version_id,
            cv.observed_at_ms,
            pel.controlled_evidence_version_id,
            cev.version_number,
            cev.captured_at_ms,
            pel.description,
            pel.created_at_ms
         FROM pbc_request_evidence_links pel
         JOIN documents d ON d.document_id = pel.document_id
         LEFT JOIN content_versions cv ON cv.content_version_id = pel.content_version_id
         LEFT JOIN controlled_evidence_versions cev
           ON cev.controlled_evidence_version_id = pel.controlled_evidence_version_id
         WHERE pel.pbc_request_id = ?1
         ORDER BY pel.created_at_ms, pel.rowid",
    )?;
    let rows = statement.query_map([pbc_request_id], |row| {
        let controlled_version_number: Option<i64> = row.get(7)?;
        Ok(PbcRequestEvidenceLinkRecord {
            pbc_request_evidence_link_id: row.get(0)?,
            pbc_request_id: row.get(1)?,
            document_id: row.get(2)?,
            document_name: row.get(3)?,
            content_version_id: row.get(4)?,
            content_observed_at_ms: row.get(5)?,
            controlled_evidence_version_id: row.get(6)?,
            controlled_version_number: controlled_version_number.map(|value| value.max(0) as u64),
            controlled_captured_at_ms: row.get(8)?,
            description: row.get(9)?,
            created_at_ms: row.get(10)?,
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
    fn review_note_locations_require_exact_evidence_and_unambiguous_supported_anchors() {
        assert_eq!(
            normalize_review_note_location(Some("evidence-link"), Some("page"), Some(" 2 "))
                .expect("page anchor should normalize"),
            (Some("PAGE".to_string()), Some("2".to_string()))
        );
        assert_eq!(
            normalize_review_note_location(
                Some("evidence-link"),
                Some("CELL"),
                Some(" Trial Balance !b12 ")
            )
            .expect("cell anchor should normalize"),
            (
                Some("CELL".to_string()),
                Some("Trial Balance!B12".to_string())
            )
        );
        assert_eq!(
            normalize_review_note_location(
                Some("evidence-link"),
                Some("RANGE"),
                Some("Sheet1!b12:d20")
            )
            .expect("range anchor should normalize"),
            (
                Some("RANGE".to_string()),
                Some("Sheet1!B12:D20".to_string())
            )
        );

        let missing_evidence = normalize_review_note_location(None, Some("PAGE"), Some("2"))
            .expect_err("sublocation without exact evidence must be rejected");
        assert!(missing_evidence
            .to_string()
            .contains("requires an exact evidence link"));

        let ambiguous_cell =
            normalize_review_note_location(Some("evidence-link"), Some("CELL"), Some("B12"))
                .expect_err("cell anchor without worksheet must be rejected");
        assert!(ambiguous_cell.to_string().contains("Worksheet!B12"));

        let unsupported =
            normalize_review_note_location(Some("evidence-link"), Some("PARAGRAPH"), Some("4"))
                .expect_err("unsupported location kinds must be rejected");
        assert!(unsupported
            .to_string()
            .contains("PAGE, WORKSHEET, CELL, or RANGE"));
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

        assert_eq!(migration_count, 18);

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
                       'review_note_events',
                       'pbc_requests',
                       'pbc_request_events',
                       'pbc_request_evidence_links',
                       'workpaper_signoffs',
                       'workpaper_signoff_evidence',
                       'workpaper_signoff_supersessions',
                       'engagement_templates',
                       'engagement_template_versions',
                       'firm_library_items',
                       'firm_library_versions',
                       'ledger_imports',
                       'ledger_transactions',
                       'ledger_test_runs',
                       'ledger_exceptions',
                       'trial_balance_imports',
                       'trial_balance_accounts',
                       'ledger_tb_mappings',
                       'financial_statement_schedules',
                       'trial_balance_schedule_mappings',
                       'financial_statement_schedule_links',
                       'reconciliation_runs',
                       'reconciliation_items',
                       'reconciliation_matches',
                       'reconciliation_exceptions'
                   )",
                [],
                |row| row.get(0),
            )
            .expect("schema tables should be queryable");

        assert_eq!(table_count, 51);
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
        assert_eq!(user_version, 18);

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

        initialize_database(&database.path).expect("database should upgrade through version 17");

        let connection =
            open_configured_connection(&database.path).expect("upgraded database should open");
        let user_version: i64 = connection
            .query_row("PRAGMA user_version;", [], |row| row.get(0))
            .expect("version should be readable");
        assert_eq!(user_version, 18);

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

        initialize_database(&database.path).expect("database should upgrade through version 17");

        let connection =
            open_configured_connection(&database.path).expect("upgraded database should open");
        let user_version: i64 = connection
            .query_row("PRAGMA user_version;", [], |row| row.get(0))
            .expect("version should be readable");
        assert_eq!(user_version, 18);

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

        initialize_database(&database.path).expect("database should upgrade to version 18");

        let connection =
            open_configured_connection(&database.path).expect("upgraded database should open");
        let user_version: i64 = connection
            .query_row("PRAGMA user_version;", [], |row| row.get(0))
            .expect("version should be readable");
        assert_eq!(user_version, 18);

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

        initialize_database(&database.path).expect("database should upgrade to version 18");

        let connection =
            open_configured_connection(&database.path).expect("upgraded database should open");
        let user_version: i64 = connection
            .query_row("PRAGMA user_version;", [], |row| row.get(0))
            .expect("version should be readable");
        assert_eq!(user_version, 18);

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

        initialize_database(&database.path).expect("database should upgrade to version 18");

        let connection =
            open_configured_connection(&database.path).expect("upgraded database should open");
        let user_version: i64 = connection
            .query_row("PRAGMA user_version;", [], |row| row.get(0))
            .expect("version should be readable");
        assert_eq!(user_version, 18);

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
    fn eighth_migration_upgrades_existing_v7_database() {
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

            for migration in &MIGRATIONS[..7] {
                let checksum = migration_checksum(migration.sql);
                apply_migration(&mut connection, migration, &checksum)
                    .expect("prior migration should apply");
            }

            let user_version: i64 = connection
                .query_row("PRAGMA user_version;", [], |row| row.get(0))
                .expect("version should be readable");
            assert_eq!(user_version, 7);
        }

        initialize_database(&database.path).expect("database should upgrade to version 18");

        let connection =
            open_configured_connection(&database.path).expect("upgraded database should open");
        let user_version: i64 = connection
            .query_row("PRAGMA user_version;", [], |row| row.get(0))
            .expect("version should be readable");
        assert_eq!(user_version, 18);

        let table_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'table'
                   AND name IN (
                       'pbc_requests',
                       'pbc_request_events',
                       'pbc_request_evidence_links'
                   )",
                [],
                |row| row.get(0),
            )
            .expect("PBC request tables should exist");
        assert_eq!(table_count, 3);
    }

    #[test]
    fn ninth_migration_upgrades_existing_v8_database() {
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

            for migration in &MIGRATIONS[..8] {
                let checksum = migration_checksum(migration.sql);
                apply_migration(&mut connection, migration, &checksum)
                    .expect("prior migration should apply");
            }

            let user_version: i64 = connection
                .query_row("PRAGMA user_version;", [], |row| row.get(0))
                .expect("version should be readable");
            assert_eq!(user_version, 8);
        }

        initialize_database(&database.path).expect("database should upgrade to version 18");

        let connection =
            open_configured_connection(&database.path).expect("upgraded database should open");
        let user_version: i64 = connection
            .query_row("PRAGMA user_version;", [], |row| row.get(0))
            .expect("version should be readable");
        assert_eq!(user_version, 18);

        let table_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'table'
                   AND name IN (
                       'workpaper_signoffs',
                       'workpaper_signoff_evidence',
                       'workpaper_signoff_supersessions'
                   )",
                [],
                |row| row.get(0),
            )
            .expect("sign-off tables should exist");
        assert_eq!(table_count, 3);
    }

    #[test]
    fn tenth_migration_upgrades_existing_v9_database() {
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

            for migration in &MIGRATIONS[..9] {
                let checksum = migration_checksum(migration.sql);
                apply_migration(&mut connection, migration, &checksum)
                    .expect("prior migration should apply");
            }

            let user_version: i64 = connection
                .query_row("PRAGMA user_version;", [], |row| row.get(0))
                .expect("version should be readable");
            assert_eq!(user_version, 9);
        }

        initialize_database(&database.path).expect("database should upgrade to version 18");

        let connection =
            open_configured_connection(&database.path).expect("upgraded database should open");
        let user_version: i64 = connection
            .query_row("PRAGMA user_version;", [], |row| row.get(0))
            .expect("version should be readable");
        assert_eq!(user_version, 18);

        let table_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'table'
                   AND name IN (
                       'engagement_templates',
                       'engagement_template_versions'
                   )",
                [],
                |row| row.get(0),
            )
            .expect("engagement template tables should exist");
        assert_eq!(table_count, 2);
    }

    #[test]
    fn eleventh_migration_upgrades_existing_v10_database() {
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

            for migration in &MIGRATIONS[..10] {
                let checksum = migration_checksum(migration.sql);
                apply_migration(&mut connection, migration, &checksum)
                    .expect("prior migration should apply");
            }

            let user_version: i64 = connection
                .query_row("PRAGMA user_version;", [], |row| row.get(0))
                .expect("version should be readable");
            assert_eq!(user_version, 10);
        }

        initialize_database(&database.path).expect("database should upgrade to version 18");

        let connection =
            open_configured_connection(&database.path).expect("upgraded database should open");
        let user_version: i64 = connection
            .query_row("PRAGMA user_version;", [], |row| row.get(0))
            .expect("version should be readable");
        assert_eq!(user_version, 18);

        let table_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'table'
                   AND name IN (
                       'firm_library_items',
                       'firm_library_versions'
                   )",
                [],
                |row| row.get(0),
            )
            .expect("firm library tables should exist");
        assert_eq!(table_count, 2);
    }

    #[test]
    fn twelfth_migration_upgrades_existing_v11_database() {
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

            for migration in &MIGRATIONS[..11] {
                let checksum = migration_checksum(migration.sql);
                apply_migration(&mut connection, migration, &checksum)
                    .expect("prior migration should apply");
            }

            let user_version: i64 = connection
                .query_row("PRAGMA user_version;", [], |row| row.get(0))
                .expect("version should be readable");
            assert_eq!(user_version, 11);
        }

        initialize_database(&database.path).expect("database should upgrade to version 18");

        let connection =
            open_configured_connection(&database.path).expect("upgraded database should open");
        let user_version: i64 = connection
            .query_row("PRAGMA user_version;", [], |row| row.get(0))
            .expect("version should be readable");
        assert_eq!(user_version, 18);

        let table_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'table'
                   AND name IN (
                       'ledger_imports',
                       'ledger_transactions',
                       'ledger_test_runs',
                       'ledger_exceptions'
                   )",
                [],
                |row| row.get(0),
            )
            .expect("ledger scrutiny tables should exist");
        assert_eq!(table_count, 4);
    }

    #[test]
    fn thirteenth_migration_drops_redundant_ledger_row_payloads() {
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

            for migration in &MIGRATIONS[..12] {
                let checksum = migration_checksum(migration.sql);
                apply_migration(&mut connection, migration, &checksum)
                    .expect("prior migration should apply");
            }

            let user_version: i64 = connection
                .query_row("PRAGMA user_version;", [], |row| row.get(0))
                .expect("version should be readable");
            assert_eq!(user_version, 12);

            let source_row_json_column_count: i64 = connection
                .query_row(
                    "SELECT COUNT(*)
                     FROM pragma_table_info('ledger_transactions')
                     WHERE name = 'source_row_json'",
                    [],
                    |row| row.get(0),
                )
                .expect("v12 ledger schema should expose source row JSON");
            assert_eq!(source_row_json_column_count, 1);
        }

        initialize_database(&database.path).expect("database should upgrade to version 18");

        let connection =
            open_configured_connection(&database.path).expect("upgraded database should open");
        let user_version: i64 = connection
            .query_row("PRAGMA user_version;", [], |row| row.get(0))
            .expect("version should be readable");
        assert_eq!(user_version, 18);

        let source_row_json_column_count: i64 = connection
            .query_row(
                "SELECT COUNT(*)
                 FROM pragma_table_info('ledger_transactions')
                 WHERE name = 'source_row_json'",
                [],
                |row| row.get(0),
            )
            .expect("v13 ledger schema should be queryable");
        assert_eq!(source_row_json_column_count, 0);

        let source_row_hash_column_count: i64 = connection
            .query_row(
                "SELECT COUNT(*)
                 FROM pragma_table_info('ledger_transactions')
                 WHERE name = 'source_row_hash'",
                [],
                |row| row.get(0),
            )
            .expect("row hash column should remain");
        assert_eq!(source_row_hash_column_count, 1);
    }

    #[test]
    fn fourteenth_migration_adds_trial_balance_tables() {
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

            for migration in &MIGRATIONS[..13] {
                let checksum = migration_checksum(migration.sql);
                apply_migration(&mut connection, migration, &checksum)
                    .expect("prior migration should apply");
            }

            let user_version: i64 = connection
                .query_row("PRAGMA user_version;", [], |row| row.get(0))
                .expect("version should be readable");
            assert_eq!(user_version, 13);
        }

        initialize_database(&database.path).expect("database should upgrade to version 18");

        let connection =
            open_configured_connection(&database.path).expect("upgraded database should open");
        let user_version: i64 = connection
            .query_row("PRAGMA user_version;", [], |row| row.get(0))
            .expect("version should be readable");
        assert_eq!(user_version, 18);

        let table_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'table'
                   AND name IN (
                       'trial_balance_imports',
                       'trial_balance_accounts'
                   )",
                [],
                |row| row.get(0),
            )
            .expect("trial balance tables should exist");
        assert_eq!(table_count, 2);
    }

    #[test]
    fn fifteenth_migration_adds_immutable_ledger_tb_mappings() {
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

            for migration in &MIGRATIONS[..14] {
                let checksum = migration_checksum(migration.sql);
                apply_migration(&mut connection, migration, &checksum)
                    .expect("prior migration should apply");
            }

            let user_version: i64 = connection
                .query_row("PRAGMA user_version;", [], |row| row.get(0))
                .expect("version should be readable");
            assert_eq!(user_version, 14);
        }

        initialize_database(&database.path).expect("database should upgrade to version 18");

        let connection =
            open_configured_connection(&database.path).expect("upgraded database should open");
        let user_version: i64 = connection
            .query_row("PRAGMA user_version;", [], |row| row.get(0))
            .expect("version should be readable");
        assert_eq!(user_version, 18);

        let table_exists: bool = connection
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM sqlite_master
                    WHERE type = 'table' AND name = 'ledger_tb_mappings'
                 )",
                [],
                |row| row.get(0),
            )
            .expect("ledger to Trial Balance mapping table should exist");
        assert!(table_exists);
    }

    #[test]
    fn sixteenth_migration_adds_immutable_trial_balance_schedule_mappings() {
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

            for migration in &MIGRATIONS[..15] {
                let checksum = migration_checksum(migration.sql);
                apply_migration(&mut connection, migration, &checksum)
                    .expect("prior migration should apply");
            }

            let user_version: i64 = connection
                .query_row("PRAGMA user_version;", [], |row| row.get(0))
                .expect("version should be readable");
            assert_eq!(user_version, 15);
        }

        initialize_database(&database.path).expect("database should upgrade to version 18");

        let connection =
            open_configured_connection(&database.path).expect("upgraded database should open");
        let user_version: i64 = connection
            .query_row("PRAGMA user_version;", [], |row| row.get(0))
            .expect("version should be readable");
        assert_eq!(user_version, 18);

        let table_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'table'
                   AND name IN (
                       'financial_statement_schedules',
                       'trial_balance_schedule_mappings'
                   )",
                [],
                |row| row.get(0),
            )
            .expect("schedule mapping tables should exist");
        assert_eq!(table_count, 2);
    }

    #[test]
    fn seventeenth_migration_adds_immutable_financial_statement_schedule_links() {
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

            for migration in &MIGRATIONS[..16] {
                let checksum = migration_checksum(migration.sql);
                apply_migration(&mut connection, migration, &checksum)
                    .expect("prior migration should apply");
            }

            let user_version: i64 = connection
                .query_row("PRAGMA user_version;", [], |row| row.get(0))
                .expect("version should be readable");
            assert_eq!(user_version, 16);
        }

        initialize_database(&database.path).expect("database should upgrade to version 18");

        let connection =
            open_configured_connection(&database.path).expect("upgraded database should open");
        let user_version: i64 = connection
            .query_row("PRAGMA user_version;", [], |row| row.get(0))
            .expect("version should be readable");
        assert_eq!(user_version, 18);

        let table_exists: bool = connection
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM sqlite_master
                    WHERE type = 'table'
                      AND name = 'financial_statement_schedule_links'
                 )",
                [],
                |row| row.get(0),
            )
            .expect("financial statement schedule link table should exist");
        assert!(table_exists);
    }

    #[test]
    fn eighteenth_migration_adds_immutable_reconciliation_framework() {
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

            for migration in &MIGRATIONS[..17] {
                let checksum = migration_checksum(migration.sql);
                apply_migration(&mut connection, migration, &checksum)
                    .expect("prior migration should apply");
            }

            let user_version: i64 = connection
                .query_row("PRAGMA user_version;", [], |row| row.get(0))
                .expect("version should be readable");
            assert_eq!(user_version, 17);
        }

        initialize_database(&database.path).expect("database should upgrade to version 18");

        let connection =
            open_configured_connection(&database.path).expect("upgraded database should open");
        let user_version: i64 = connection
            .query_row("PRAGMA user_version;", [], |row| row.get(0))
            .expect("version should be readable");
        assert_eq!(user_version, 18);

        let table_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'table'
                   AND name IN (
                       'reconciliation_runs',
                       'reconciliation_items',
                       'reconciliation_matches',
                       'reconciliation_exceptions'
                   )",
                [],
                |row| row.get(0),
            )
            .expect("reconciliation framework tables should exist");
        assert_eq!(table_count, 4);
    }

    #[test]
    fn controlled_accounting_imports_are_deterministic_and_source_traceable() {
        let database = TestDatabase::new();
        initialize_database(&database.path).expect("database initialization should succeed");

        let client = create_client(&database.path, "Ledger Client").expect("client");
        let service = create_service_type(&database.path, "Statutory Audit").expect("service");
        let engagement = create_engagement(
            &database.path,
            &client.client_id,
            &service.service_type_id,
            "FY 2026-27 statutory audit",
            None,
            None,
            "ACTIVE",
        )
        .expect("engagement");

        let storage_root_id = Uuid::new_v4().to_string();
        let document_id = Uuid::new_v4().to_string();
        let file_instance_id = Uuid::new_v4().to_string();
        let content_version_id = Uuid::new_v4().to_string();
        let capture_job_id = Uuid::new_v4().to_string();
        let controlled_evidence_version_id = Uuid::new_v4().to_string();
        let source_sha256 = vec![0xAB; 32];

        {
            let connection =
                open_configured_connection(&database.path).expect("database should open");
            connection
                .execute(
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
                     ) VALUES (?1, 'LOCAL', ?2, 'TEST', '/controlled-test', NULL, NULL, 'AVAILABLE', 1, NULL, 1, 1)",
                    params![&storage_root_id, vec![0_u8]],
                )
                .expect("storage root should insert");
            connection
                .execute(
                    "INSERT INTO documents (
                        document_id,
                        storage_state,
                        display_name,
                        created_at_ms,
                        created_by,
                        archived_at_ms
                     ) VALUES (?1, 'CONTROLLED_EVIDENCE', 'Ledger.xlsx', 1, NULL, NULL)",
                    [&document_id],
                )
                .expect("document should insert");
            connection
                .execute(
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
                        ?1, ?2, ?3, ?4, 'TEST', 'Ledger.xlsx', 'ledger xlsx',
                        NULL, NULL, NULL, NULL, 128, NULL, NULL, 1, 1, NULL, NULL, 'AVAILABLE'
                     )",
                    params![
                        &file_instance_id,
                        &document_id,
                        &storage_root_id,
                        b"Ledger.xlsx".to_vec()
                    ],
                )
                .expect("file instance should insert");
            connection
                .execute(
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
                     ) VALUES (?1, ?2, ?3, 1, 128, NULL, NULL, ?4, 'HASH_VERIFIED', 1)",
                    params![
                        &content_version_id,
                        &document_id,
                        &file_instance_id,
                        &source_sha256
                    ],
                )
                .expect("content version should insert");
            connection
                .execute(
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
                     ) VALUES (?1, ?2, ?3, 'COMPLETE', 1, 1, 1, 'TEST', 'TEST', NULL, NULL)",
                    params![&capture_job_id, &file_instance_id, &document_id],
                )
                .expect("capture job should insert");
            let locator = format!("{document_id}/{controlled_evidence_version_id}");
            connection
                .execute(
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
                        ?1, ?2, ?3, ?4, ?5, 1, ?6, ?7, 128, 1, NULL, 'TEST', 'TEST',
                        'RETAINED', 'HASH_VERIFIED', 1
                     )",
                    params![
                        &controlled_evidence_version_id,
                        &document_id,
                        &file_instance_id,
                        &content_version_id,
                        &capture_job_id,
                        &locator,
                        &source_sha256
                    ],
                )
                .expect("controlled evidence should insert");
        }

        let ledger_rows = [
            (2_u64, 10_000_i64),
            (3_u64, 100_000_i64),
            (4_u64, -200_000_i64),
            (5_u64, 99_999_i64),
        ]
        .into_iter()
        .map(|(source_row_number, amount_minor)| {
            let source_row_json = json!({
                "rowNumber": source_row_number,
                "cells": [format!("row-{source_row_number}"), amount_minor]
            })
            .to_string();
            LedgerTransactionInput {
                source_row_number,
                source_row_hash: Sha256::digest(source_row_json.as_bytes()).to_vec(),
                transaction_date_text: Some("2026-03-31".to_string()),
                account_text: Some("Revenue".to_string()),
                voucher_text: Some(format!("JV-{source_row_number}")),
                narration_text: Some("Ledger test row".to_string()),
                amount_minor,
            }
        })
        .collect::<Vec<_>>();

        let ledger_import = create_ledger_import(
            &database.path,
            LedgerImportDefinition {
                engagement_id: &engagement.engagement_id,
                controlled_evidence_version_id: &controlled_evidence_version_id,
                sheet_name: "Ledger",
                header_row_number: 1,
                amount_column: 4,
                date_column: Some(0),
                account_column: Some(1),
                voucher_column: Some(2),
                narration_column: Some(3),
                amount_scale: 2,
                transactions: &ledger_rows,
            },
        )
        .expect("ledger import should succeed");
        assert_eq!(ledger_import.transaction_count, 4);
        assert_eq!(ledger_import.source_sha256, source_sha256);
        assert_eq!(
            ledger_import.controlled_evidence_version_id,
            controlled_evidence_version_id
        );

        let imports =
            list_ledger_imports(&database.path, &engagement.engagement_id).expect("imports");
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].ledger_import_id, ledger_import.ledger_import_id);

        let run =
            run_high_value_ledger_test(&database.path, &ledger_import.ledger_import_id, 100_000)
                .expect("high-value test should run");
        assert_eq!(run.test_type, "HIGH_VALUE");
        assert_eq!(run.threshold_minor, 100_000);
        assert_eq!(run.exception_count, 2);

        let runs = list_ledger_test_runs(&database.path, &ledger_import.ledger_import_id)
            .expect("test run history");
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].ledger_test_run_id, run.ledger_test_run_id);
        assert_eq!(runs[0].ledger_import_id, ledger_import.ledger_import_id);
        assert_eq!(runs[0].test_type, "HIGH_VALUE");
        assert_eq!(runs[0].threshold_minor, 100_000);
        assert_eq!(runs[0].exception_count, 2);

        let exceptions =
            list_ledger_exceptions(&database.path, &run.ledger_test_run_id).expect("exceptions");
        assert_eq!(exceptions.len(), 2);
        assert_eq!(
            exceptions
                .iter()
                .map(|exception| exception.source_row_number)
                .collect::<Vec<_>>(),
            vec![3, 4]
        );
        for exception in &exceptions {
            assert_eq!(
                exception.controlled_evidence_version_id,
                controlled_evidence_version_id
            );
            assert_eq!(exception.document_id, document_id);
            assert_eq!(exception.source_content_version_id, content_version_id);
            assert_eq!(exception.source_sha256, source_sha256);
            assert_eq!(exception.sheet_name, "Ledger");
            assert_eq!(exception.source_row_hash.len(), 32);
        }

        assert!(
            run_high_value_ledger_test(&database.path, &ledger_import.ledger_import_id, 0)
                .expect_err("zero threshold should fail")
                .to_string()
                .contains("greater than zero")
        );

        let connection =
            open_configured_connection(&database.path).expect("database should reopen");
        let mutation_error = connection
            .execute(
                "UPDATE ledger_transactions
                 SET amount_minor = amount_minor + 1
                 WHERE ledger_import_id = ?1",
                [&ledger_import.ledger_import_id],
            )
            .expect_err("imported transactions must be immutable");
        assert!(mutation_error
            .to_string()
            .contains("ledger transactions are immutable"));

        let trial_balance_rows = vec![
            TrialBalanceAccountInput {
                source_row_number: 2,
                source_row_hash: Sha256::digest(b"tb-row-2").to_vec(),
                account_code_text: Some("1000".to_string()),
                account_name_text: "Cash".to_string(),
                opening_minor: 100_000,
                closing_minor: 120_000,
            },
            TrialBalanceAccountInput {
                source_row_number: 3,
                source_row_hash: Sha256::digest(b"tb-row-3").to_vec(),
                account_code_text: Some("4000".to_string()),
                account_name_text: "Revenue".to_string(),
                opening_minor: -200_000,
                closing_minor: -250_000,
            },
        ];
        let trial_balance_import = create_trial_balance_import(
            &database.path,
            TrialBalanceImportDefinition {
                engagement_id: &engagement.engagement_id,
                controlled_evidence_version_id: &controlled_evidence_version_id,
                sheet_name: "Trial Balance",
                header_row_number: 1,
                account_name_column: 1,
                account_code_column: Some(0),
                opening_balance_column: Some(2),
                closing_balance_column: 3,
                amount_scale: 2,
                accounts: &trial_balance_rows,
            },
        )
        .expect("trial balance import should succeed");

        assert_eq!(trial_balance_import.account_count, 2);
        assert_eq!(trial_balance_import.opening_total_minor, -100_000);
        assert_eq!(trial_balance_import.closing_total_minor, -130_000);
        assert_eq!(trial_balance_import.source_sha256, source_sha256);
        assert_eq!(
            trial_balance_import.controlled_evidence_version_id,
            controlled_evidence_version_id
        );

        let trial_balance_imports =
            list_trial_balance_imports(&database.path, &engagement.engagement_id)
                .expect("trial balance imports");
        assert_eq!(trial_balance_imports.len(), 1);
        assert_eq!(
            trial_balance_imports[0].trial_balance_import_id,
            trial_balance_import.trial_balance_import_id
        );

        let trial_balance_accounts = list_trial_balance_accounts(
            &database.path,
            &trial_balance_import.trial_balance_import_id,
        )
        .expect("trial balance accounts");
        assert_eq!(trial_balance_accounts.len(), 2);
        assert_eq!(trial_balance_accounts[0].account_name_text, "Cash");
        assert_eq!(trial_balance_accounts[0].source_row_number, 2);
        assert_eq!(trial_balance_accounts[0].source_row_hash.len(), 32);
        assert_eq!(trial_balance_accounts[1].account_name_text, "Revenue");
        assert_eq!(trial_balance_accounts[1].closing_minor, -250_000);

        let comparison = compare_trial_balance_opening_closing(
            &database.path,
            &trial_balance_import.trial_balance_import_id,
        )
        .expect("opening and closing comparison");
        assert_eq!(comparison.opening_total_minor, -100_000);
        assert_eq!(comparison.closing_total_minor, -130_000);
        assert_eq!(comparison.net_movement_minor, -30_000);
        assert_eq!(comparison.account_count, 2);
        assert_eq!(comparison.movements[0].account_name_text, "Revenue");
        assert_eq!(comparison.movements[0].movement_minor, -50_000);
        assert_eq!(comparison.movements[0].source_row_number, 3);
        assert_eq!(comparison.movements[1].account_name_text, "Cash");
        assert_eq!(comparison.movements[1].movement_minor, 20_000);

        let ledger_account_summaries =
            list_ledger_account_summaries(&database.path, &ledger_import.ledger_import_id)
                .expect("ledger account summaries");
        assert_eq!(ledger_account_summaries.len(), 1);
        assert_eq!(ledger_account_summaries[0].account_key, "revenue");
        assert_eq!(ledger_account_summaries[0].account_text, "Revenue");
        assert_eq!(ledger_account_summaries[0].transaction_count, 4);
        assert_eq!(ledger_account_summaries[0].total_minor, 9_999);
        assert_eq!(ledger_account_summaries[0].first_source_row_number, 2);
        assert_eq!(ledger_account_summaries[0].last_source_row_number, 5);

        let first_mapping = create_ledger_tb_mapping(
            &database.path,
            &ledger_import.ledger_import_id,
            &trial_balance_import.trial_balance_import_id,
            "  Revenue ",
            &trial_balance_accounts[1].trial_balance_account_id,
        )
        .expect("initial ledger to Trial Balance mapping");
        assert_eq!(first_mapping.version_number, 1);
        assert_eq!(first_mapping.ledger_account_key, "revenue");
        assert_eq!(first_mapping.trial_balance_account_name_text, "Revenue");
        assert_eq!(first_mapping.supersedes_mapping_id, None);

        let duplicate_mapping_error = create_ledger_tb_mapping(
            &database.path,
            &ledger_import.ledger_import_id,
            &trial_balance_import.trial_balance_import_id,
            "REVENUE",
            &trial_balance_accounts[1].trial_balance_account_id,
        )
        .expect_err("mapping the same account to the same TB account should be rejected");
        assert!(duplicate_mapping_error
            .to_string()
            .contains("already mapped"));

        let second_mapping = create_ledger_tb_mapping(
            &database.path,
            &ledger_import.ledger_import_id,
            &trial_balance_import.trial_balance_import_id,
            "revenue",
            &trial_balance_accounts[0].trial_balance_account_id,
        )
        .expect("ledger account remap should append a new version");
        assert_eq!(second_mapping.version_number, 2);
        assert_eq!(
            second_mapping.supersedes_mapping_id.as_deref(),
            Some(first_mapping.ledger_tb_mapping_id.as_str())
        );
        assert_eq!(second_mapping.trial_balance_account_name_text, "Cash");

        let current_mappings = list_current_ledger_tb_mappings(
            &database.path,
            &ledger_import.ledger_import_id,
            &trial_balance_import.trial_balance_import_id,
        )
        .expect("current ledger to Trial Balance mappings");
        assert_eq!(current_mappings.len(), 1);
        assert_eq!(
            current_mappings[0].ledger_tb_mapping_id,
            second_mapping.ledger_tb_mapping_id
        );
        assert_eq!(current_mappings[0].version_number, 2);
        assert_eq!(current_mappings[0].trial_balance_account_name_text, "Cash");

        let mapping_mutation_error = connection
            .execute(
                "UPDATE ledger_tb_mappings
                 SET ledger_account_text = 'Changed'
                 WHERE ledger_tb_mapping_id = ?1",
                [&first_mapping.ledger_tb_mapping_id],
            )
            .expect_err("ledger to Trial Balance mapping history must be immutable");
        assert!(mapping_mutation_error
            .to_string()
            .contains("ledger to trial balance mappings are immutable"));

        let revenue_schedule = create_financial_statement_schedule(
            &database.path,
            &engagement.engagement_id,
            "SCH-REV",
            "Revenue",
        )
        .expect("financial statement schedule should be created");
        let cash_schedule = create_financial_statement_schedule(
            &database.path,
            &engagement.engagement_id,
            "SCH-CASH",
            "Cash and bank balances",
        )
        .expect("second financial statement schedule should be created");
        let schedules =
            list_financial_statement_schedules(&database.path, &engagement.engagement_id)
                .expect("financial statement schedules");
        assert_eq!(schedules.len(), 2);
        assert_eq!(schedules[0].reference, "SCH-CASH");
        assert_eq!(schedules[1].reference, "SCH-REV");

        let first_schedule_mapping = create_trial_balance_schedule_mapping(
            &database.path,
            &trial_balance_import.trial_balance_import_id,
            &trial_balance_accounts[1].trial_balance_account_id,
            &revenue_schedule.financial_statement_schedule_id,
        )
        .expect("initial Trial Balance schedule mapping");
        assert_eq!(first_schedule_mapping.version_number, 1);
        assert_eq!(first_schedule_mapping.schedule_reference, "SCH-REV");
        assert_eq!(
            first_schedule_mapping.trial_balance_account_name_text,
            "Revenue"
        );
        assert_eq!(first_schedule_mapping.supersedes_mapping_id, None);
        assert_eq!(first_schedule_mapping.trial_balance_source_row_number, 3);
        assert_eq!(
            first_schedule_mapping.trial_balance_source_row_hash.len(),
            32
        );

        let duplicate_schedule_mapping_error = create_trial_balance_schedule_mapping(
            &database.path,
            &trial_balance_import.trial_balance_import_id,
            &trial_balance_accounts[1].trial_balance_account_id,
            &revenue_schedule.financial_statement_schedule_id,
        )
        .expect_err("duplicate current schedule mapping should be rejected");
        assert!(duplicate_schedule_mapping_error
            .to_string()
            .contains("already mapped"));

        let second_schedule_mapping = create_trial_balance_schedule_mapping(
            &database.path,
            &trial_balance_import.trial_balance_import_id,
            &trial_balance_accounts[1].trial_balance_account_id,
            &cash_schedule.financial_statement_schedule_id,
        )
        .expect("Trial Balance schedule remap should append a version");
        assert_eq!(second_schedule_mapping.version_number, 2);
        assert_eq!(
            second_schedule_mapping.supersedes_mapping_id.as_deref(),
            Some(
                first_schedule_mapping
                    .trial_balance_schedule_mapping_id
                    .as_str()
            )
        );
        assert_eq!(second_schedule_mapping.schedule_reference, "SCH-CASH");

        let current_schedule_mappings = list_current_trial_balance_schedule_mappings(
            &database.path,
            &trial_balance_import.trial_balance_import_id,
        )
        .expect("current Trial Balance schedule mappings");
        assert_eq!(current_schedule_mappings.len(), 1);
        assert_eq!(
            current_schedule_mappings[0].trial_balance_schedule_mapping_id,
            second_schedule_mapping.trial_balance_schedule_mapping_id
        );
        assert_eq!(current_schedule_mappings[0].version_number, 2);
        assert_eq!(current_schedule_mappings[0].schedule_reference, "SCH-CASH");

        let first_statement_link = create_financial_statement_schedule_link(
            &database.path,
            &revenue_schedule.financial_statement_schedule_id,
            &controlled_evidence_version_id,
            "page",
            " 2 ",
        )
        .expect("financial statement schedule should link to exact evidence");
        assert_eq!(first_statement_link.version_number, 1);
        assert_eq!(first_statement_link.location_kind, "PAGE");
        assert_eq!(first_statement_link.location_value, "2");
        assert_eq!(first_statement_link.source_sha256, source_sha256);
        assert_eq!(first_statement_link.supersedes_link_id, None);

        let duplicate_statement_link_error = create_financial_statement_schedule_link(
            &database.path,
            &revenue_schedule.financial_statement_schedule_id,
            &controlled_evidence_version_id,
            "PAGE",
            "2",
        )
        .expect_err("duplicate exact financial statement link should be rejected");
        assert!(duplicate_statement_link_error
            .to_string()
            .contains("already linked"));

        let second_statement_link = create_financial_statement_schedule_link(
            &database.path,
            &revenue_schedule.financial_statement_schedule_id,
            &controlled_evidence_version_id,
            "PAGE",
            "3",
        )
        .expect("financial statement relink should append a version");
        assert_eq!(second_statement_link.version_number, 2);
        assert_eq!(
            second_statement_link.supersedes_link_id.as_deref(),
            Some(
                first_statement_link
                    .financial_statement_schedule_link_id
                    .as_str()
            )
        );
        assert_eq!(second_statement_link.location_value, "3");

        let current_statement_links = list_current_financial_statement_schedule_links(
            &database.path,
            &engagement.engagement_id,
        )
        .expect("current financial statement schedule links");
        assert_eq!(current_statement_links.len(), 1);
        assert_eq!(
            current_statement_links[0].financial_statement_schedule_link_id,
            second_statement_link.financial_statement_schedule_link_id
        );
        assert_eq!(current_statement_links[0].schedule_reference, "SCH-REV");
        assert_eq!(current_statement_links[0].location_kind, "PAGE");
        assert_eq!(current_statement_links[0].location_value, "3");

        let invalid_statement_anchor = create_financial_statement_schedule_link(
            &database.path,
            &cash_schedule.financial_statement_schedule_id,
            &controlled_evidence_version_id,
            "CELL",
            "B12",
        )
        .expect_err("unqualified workbook anchor should be rejected");
        assert!(invalid_statement_anchor
            .to_string()
            .contains("Worksheet!B12"));

        let statement_link_mutation_error = connection
            .execute(
                "UPDATE financial_statement_schedule_links
                 SET location_value = '4'
                 WHERE financial_statement_schedule_link_id = ?1",
                [&first_statement_link.financial_statement_schedule_link_id],
            )
            .expect_err("financial statement schedule link history must be immutable");
        assert!(statement_link_mutation_error
            .to_string()
            .contains("financial statement schedule links are immutable"));

        let schedule_mutation_error = connection
            .execute(
                "UPDATE financial_statement_schedules
                 SET name = 'Changed'
                 WHERE financial_statement_schedule_id = ?1",
                [&revenue_schedule.financial_statement_schedule_id],
            )
            .expect_err("financial statement schedules must be immutable");
        assert!(schedule_mutation_error
            .to_string()
            .contains("financial statement schedules are immutable"));

        let schedule_mapping_mutation_error = connection
            .execute(
                "UPDATE trial_balance_schedule_mappings
                 SET version_number = version_number + 1
                 WHERE trial_balance_schedule_mapping_id = ?1",
                [&first_schedule_mapping.trial_balance_schedule_mapping_id],
            )
            .expect_err("Trial Balance schedule mapping history must be immutable");
        assert!(schedule_mapping_mutation_error
            .to_string()
            .contains("trial balance schedule mappings are immutable"));

        let reconciliation_row_hash = |label: &str| Sha256::digest(label.as_bytes()).to_vec();
        let reconciliation_item =
            |source_entity_id: &str, match_key: &str, amount_minor: i64, source_row_number: u64| {
                ReconciliationItemInput {
                    match_key: match_key.to_string(),
                    amount_minor,
                    event_date_text: Some("2026-03-31".to_string()),
                    description_text: Some(format!("Reconciliation item {source_entity_id}")),
                    source_kind: "TEST_ROW".to_string(),
                    source_entity_id: source_entity_id.to_string(),
                    controlled_evidence_version_id: controlled_evidence_version_id.clone(),
                    document_id: document_id.clone(),
                    source_content_version_id: content_version_id.clone(),
                    source_sha256: source_sha256.clone(),
                    sheet_name: Some("Recon".to_string()),
                    source_row_number: Some(source_row_number),
                    source_row_hash: Some(reconciliation_row_hash(source_entity_id)),
                }
            };

        let left_reconciliation_items = vec![
            reconciliation_item("L-A", "INV-100", 10_000, 10),
            reconciliation_item("L-B", "INV-100", 10_000, 11),
            reconciliation_item("L-C", "INV-200", 20_000, 12),
        ];
        let right_reconciliation_items = vec![
            reconciliation_item("R-A", "INV-100", 10_000, 20),
            reconciliation_item("R-C", "INV-300", 30_000, 21),
            reconciliation_item("R-B", "INV-200", 20_000, 22),
        ];

        let reconciliation_run = create_reconciliation_run(
            &database.path,
            ReconciliationRunDefinition {
                engagement_id: &engagement.engagement_id,
                reconciliation_type: "generic test",
                title: "Exact key and amount reconciliation",
                parameters_json: r#"{"comparison":"EXACT","amountScale":2}"#,
                left_items: &left_reconciliation_items,
                right_items: &right_reconciliation_items,
            },
        )
        .expect("reconciliation run should succeed");
        assert_eq!(reconciliation_run.reconciliation_type, "GENERIC_TEST");
        assert_eq!(reconciliation_run.rule_code, "EXACT_KEY_AMOUNT");
        assert_eq!(reconciliation_run.left_item_count, 3);
        assert_eq!(reconciliation_run.right_item_count, 3);
        assert_eq!(reconciliation_run.matched_pair_count, 2);
        assert_eq!(reconciliation_run.exception_count, 2);

        let reconciliation_runs =
            list_reconciliation_runs(&database.path, &engagement.engagement_id)
                .expect("reconciliation runs");
        assert_eq!(reconciliation_runs.len(), 1);
        assert_eq!(
            reconciliation_runs[0].reconciliation_run_id,
            reconciliation_run.reconciliation_run_id
        );

        let reconciliation_exceptions = list_reconciliation_exceptions(
            &database.path,
            &reconciliation_run.reconciliation_run_id,
        )
        .expect("reconciliation exceptions");
        assert_eq!(reconciliation_exceptions.len(), 2);
        assert_eq!(
            reconciliation_exceptions
                .iter()
                .map(|exception| (
                    exception.exception_code.as_str(),
                    exception.source_entity_id.as_str()
                ))
                .collect::<Vec<_>>(),
            vec![("UNMATCHED_LEFT", "L-B"), ("UNMATCHED_RIGHT", "R-C")]
        );
        for exception in &reconciliation_exceptions {
            assert_eq!(
                exception.controlled_evidence_version_id,
                controlled_evidence_version_id
            );
            assert_eq!(exception.document_id, document_id);
            assert_eq!(exception.source_content_version_id, content_version_id);
            assert_eq!(exception.source_sha256, source_sha256);
            assert_eq!(exception.sheet_name.as_deref(), Some("Recon"));
            assert_eq!(exception.source_row_hash.as_ref().map(Vec::len), Some(32));
        }

        let mut invalid_reconciliation_left = left_reconciliation_items.clone();
        invalid_reconciliation_left[0].source_sha256 = vec![0xCD; 32];
        let provenance_error = create_reconciliation_run(
            &database.path,
            ReconciliationRunDefinition {
                engagement_id: &engagement.engagement_id,
                reconciliation_type: "generic test",
                title: "Invalid provenance reconciliation",
                parameters_json: "{}",
                left_items: &invalid_reconciliation_left,
                right_items: &right_reconciliation_items,
            },
        )
        .expect_err("mismatched controlled evidence provenance must fail");
        assert!(provenance_error
            .to_string()
            .contains("does not match controlled evidence"));

        let reconciliation_run_mutation_error = connection
            .execute(
                "UPDATE reconciliation_runs
                 SET exception_count = 0
                 WHERE reconciliation_run_id = ?1",
                [&reconciliation_run.reconciliation_run_id],
            )
            .expect_err("reconciliation runs must be immutable");
        assert!(reconciliation_run_mutation_error
            .to_string()
            .contains("reconciliation runs are immutable"));

        let reconciliation_item_id: String = connection
            .query_row(
                "SELECT reconciliation_item_id
                 FROM reconciliation_items
                 WHERE reconciliation_run_id = ?1
                 ORDER BY reconciliation_item_id
                 LIMIT 1",
                [&reconciliation_run.reconciliation_run_id],
                |row| row.get(0),
            )
            .expect("reconciliation item should exist");
        let reconciliation_item_mutation_error = connection
            .execute(
                "UPDATE reconciliation_items
                 SET amount_minor = amount_minor + 1
                 WHERE reconciliation_item_id = ?1",
                [&reconciliation_item_id],
            )
            .expect_err("reconciliation items must be immutable");
        assert!(reconciliation_item_mutation_error
            .to_string()
            .contains("reconciliation items are immutable"));

        let reconciliation_match_id: String = connection
            .query_row(
                "SELECT reconciliation_match_id
                 FROM reconciliation_matches
                 WHERE reconciliation_run_id = ?1
                 ORDER BY reconciliation_match_id
                 LIMIT 1",
                [&reconciliation_run.reconciliation_run_id],
                |row| row.get(0),
            )
            .expect("reconciliation match should exist");
        let reconciliation_match_mutation_error = connection
            .execute(
                "UPDATE reconciliation_matches
                 SET matched_at_ms = matched_at_ms + 1
                 WHERE reconciliation_match_id = ?1",
                [&reconciliation_match_id],
            )
            .expect_err("reconciliation matches must be immutable");
        assert!(reconciliation_match_mutation_error
            .to_string()
            .contains("reconciliation matches are immutable"));

        let reconciliation_exception_mutation_error = connection
            .execute(
                "UPDATE reconciliation_exceptions
                 SET exception_code = 'UNMATCHED_RIGHT'
                 WHERE reconciliation_exception_id = ?1",
                [&reconciliation_exceptions[0].reconciliation_exception_id],
            )
            .expect_err("reconciliation exceptions must be immutable");
        assert!(reconciliation_exception_mutation_error
            .to_string()
            .contains("reconciliation exceptions are immutable"));

        let trial_balance_mutation_error = connection
            .execute(
                "UPDATE trial_balance_accounts
                 SET closing_minor = closing_minor + 1
                 WHERE trial_balance_import_id = ?1",
                [&trial_balance_import.trial_balance_import_id],
            )
            .expect_err("imported trial balance accounts must be immutable");
        assert!(trial_balance_mutation_error
            .to_string()
            .contains("trial balance accounts are immutable"));
    }

    #[test]
    fn firm_library_versions_are_exact_immutable_and_scoped() {
        let database = TestDatabase::new();
        initialize_database(&database.path).expect("database initialization should succeed");

        for category in [
            "CHECKLIST",
            "AUDIT_QUERY",
            "RISK_TEMPLATE",
            "CONTROL_TEMPLATE",
            "LEDGER_SCRUTINY_TEST",
            "REPORT_TEMPLATE",
            "MANAGEMENT_LETTER_POINT",
            "STATUTORY_COMPLIANCE_REQUIREMENT",
        ] {
            assert_eq!(
                normalize_firm_library_category(category).expect("category should normalize"),
                category
            );
        }
        assert!(normalize_firm_library_category("unsupported")
            .expect_err("unsupported category should fail")
            .to_string()
            .contains("not supported"));

        let service =
            create_service_type(&database.path, "Firm Library Assurance").expect("service type");

        let checklist = create_firm_library_item(
            &database.path,
            "checklist",
            "Revenue completion checklist",
            Some("Firm completion steps for revenue testing."),
            Some(&service.service_type_id),
            r#"{"items":[{"id":"rev-1","text":"Agree final revenue schedule"}],"required":true}"#,
        )
        .expect("checklist should be created");
        assert_eq!(checklist.category, "CHECKLIST");
        assert_eq!(checklist.latest_version_number, 1);
        assert_eq!(checklist.latest_definition_hash.len(), 32);
        assert_eq!(
            checklist.service_type_id.as_deref(),
            Some(service.service_type_id.as_str())
        );

        let global_query = create_firm_library_item(
            &database.path,
            "audit query",
            "Unexpected journal descriptions",
            None,
            None,
            r#"{"query":"description contains unusual terms","severity":"review"}"#,
        )
        .expect("global audit query should be created");
        assert_eq!(global_query.category, "AUDIT_QUERY");
        assert_eq!(global_query.service_type_id, None);

        let scoped_checklists = list_firm_library_items(
            &database.path,
            Some("CHECKLIST"),
            Some(&service.service_type_id),
        )
        .expect("scoped checklist list should load");
        assert_eq!(scoped_checklists.len(), 1);
        assert_eq!(
            scoped_checklists[0].firm_library_item_id,
            checklist.firm_library_item_id
        );

        let all_items =
            list_firm_library_items(&database.path, None, None).expect("library should load");
        assert_eq!(all_items.len(), 2);

        let invalid_definition = create_firm_library_item(
            &database.path,
            "RISK_TEMPLATE",
            "Invalid definition",
            None,
            None,
            r#"["not","an","object"]"#,
        )
        .expect_err("array definitions must be rejected");
        assert!(invalid_definition
            .to_string()
            .contains("must be a JSON object"));

        let invalid_category = create_firm_library_item(
            &database.path,
            "OTHER",
            "Unsupported category",
            None,
            None,
            r#"{"value":1}"#,
        )
        .expect_err("unsupported categories must be rejected");
        assert!(invalid_category.to_string().contains("not supported"));

        let updated = publish_firm_library_version(
            &database.path,
            &checklist.firm_library_item_id,
            r#"{"items":[{"id":"rev-1","text":"Agree final revenue schedule"},{"id":"rev-2","text":"Document cut-off conclusion"}],"required":true}"#,
        )
        .expect("new library version should publish");
        assert_eq!(updated.latest_version_number, 2);
        assert_ne!(updated.latest_version_id, checklist.latest_version_id);
        assert_ne!(
            updated.latest_definition_hash,
            checklist.latest_definition_hash
        );

        let versions = list_firm_library_versions(&database.path, &checklist.firm_library_item_id)
            .expect("version history should load");
        assert_eq!(versions.len(), 2);
        assert_eq!(versions[0].version_number, 2);
        assert_eq!(versions[1].version_number, 1);
        assert_eq!(versions[0].definition_hash.len(), 32);
        assert_eq!(versions[1].definition_hash.len(), 32);
        assert!(versions[0]
            .definition_json
            .contains("Document cut-off conclusion"));
        assert!(!versions[1]
            .definition_json
            .contains("Document cut-off conclusion"));

        let connection =
            open_configured_connection(&database.path).expect("database should reopen");
        let mutation_error = connection
            .execute(
                "UPDATE firm_library_versions
                 SET definition_json = '{}'
                 WHERE firm_library_version_id = ?1",
                [&versions[1].firm_library_version_id],
            )
            .expect_err("firm library version updates must be rejected");
        assert!(mutation_error
            .to_string()
            .contains("firm library versions are immutable"));
    }

    #[test]
    fn engagement_templates_copy_exact_methodology_without_live_linkage() {
        let database = TestDatabase::new();
        initialize_database(&database.path).expect("database initialization should succeed");

        let source_client =
            create_client(&database.path, "Template Source Client").expect("source client");
        let target_client =
            create_client(&database.path, "Template Target Client").expect("target client");
        let service =
            create_service_type(&database.path, "Reusable Assurance").expect("service type");

        let source_engagement = create_engagement(
            &database.path,
            &source_client.client_id,
            &service.service_type_id,
            "Source methodology engagement",
            None,
            None,
            "ACTIVE",
        )
        .expect("source engagement");

        let root_area = create_engagement_area(
            &database.path,
            &source_engagement.engagement_id,
            None,
            "Revenue",
            Some("REV"),
            10,
            "ACTIVE",
        )
        .expect("root area");
        let child_area = create_engagement_area(
            &database.path,
            &source_engagement.engagement_id,
            Some(&root_area.engagement_area_id),
            "Cut-off",
            Some("CUT"),
            20,
            "ACTIVE",
        )
        .expect("child area");
        create_procedure(
            &database.path,
            &source_engagement.engagement_id,
            Some(&child_area.engagement_area_id),
            Some("REV-01"),
            "Test revenue cut-off",
            Some("Inspect transactions around period end."),
            "ACTIVE",
        )
        .expect("source procedure");

        let template = create_engagement_template_from_engagement(
            &database.path,
            &source_engagement.engagement_id,
            "Core revenue methodology",
            Some("Reusable revenue areas and procedures."),
        )
        .expect("template should be captured");
        assert_eq!(template.latest_version_number, 1);
        assert_eq!(template.service_type_id, service.service_type_id);
        assert_eq!(
            template.source_engagement_id.as_deref(),
            Some(source_engagement.engagement_id.as_str())
        );

        let templates =
            list_engagement_templates(&database.path).expect("template list should load");
        assert_eq!(templates.len(), 1);
        assert_eq!(templates[0].latest_version_id, template.latest_version_id);

        create_procedure(
            &database.path,
            &source_engagement.engagement_id,
            Some(&root_area.engagement_area_id),
            Some("REV-LATER"),
            "Procedure added after template capture",
            None,
            "ACTIVE",
        )
        .expect("later source procedure");

        let updated_template = create_engagement_template_version_from_engagement(
            &database.path,
            &template.engagement_template_id,
            &source_engagement.engagement_id,
        )
        .expect("updated methodology should publish a new template version");
        assert_eq!(updated_template.latest_version_number, 2);
        assert_ne!(
            updated_template.latest_version_id,
            template.latest_version_id
        );
        let latest_templates =
            list_engagement_templates(&database.path).expect("latest template list should load");
        assert_eq!(latest_templates[0].latest_version_number, 2);
        assert_eq!(
            latest_templates[0].latest_version_id,
            updated_template.latest_version_id
        );

        let updated_target = create_engagement_from_template(
            &database.path,
            &updated_template.latest_version_id,
            &target_client.client_id,
            "Target engagement from updated template",
            None,
            None,
            "ACTIVE",
        )
        .expect("new exact methodology version should instantiate");
        assert_eq!(
            list_procedures(&database.path, &updated_target.engagement_id)
                .expect("updated target procedures")
                .len(),
            2
        );

        let target_engagement = create_engagement_from_template(
            &database.path,
            &template.latest_version_id,
            &target_client.client_id,
            "Target engagement from template",
            Some("2026-04-01"),
            Some("2027-03-31"),
            "ACTIVE",
        )
        .expect("engagement should instantiate from exact template version");
        assert_eq!(target_engagement.service_type_id, service.service_type_id);

        let target_areas = list_engagement_areas(&database.path, &target_engagement.engagement_id)
            .expect("target areas should load");
        assert_eq!(target_areas.len(), 2);
        let target_root = target_areas
            .iter()
            .find(|area| area.name == "Revenue")
            .expect("target root area");
        let target_child = target_areas
            .iter()
            .find(|area| area.name == "Cut-off")
            .expect("target child area");
        assert_eq!(
            target_child.parent_area_id.as_deref(),
            Some(target_root.engagement_area_id.as_str())
        );
        assert_ne!(target_root.engagement_area_id, root_area.engagement_area_id);
        assert_ne!(
            target_child.engagement_area_id,
            child_area.engagement_area_id
        );

        let target_procedures = list_procedures(&database.path, &target_engagement.engagement_id)
            .expect("target procedures should load");
        assert_eq!(target_procedures.len(), 1);
        assert_eq!(target_procedures[0].reference.as_deref(), Some("REV-01"));

        create_engagement_area(
            &database.path,
            &target_engagement.engagement_id,
            None,
            "Client-specific addition",
            None,
            30,
            "ACTIVE",
        )
        .expect("instantiated engagement should remain customizable");

        let second_target = create_engagement_from_template(
            &database.path,
            &template.latest_version_id,
            &target_client.client_id,
            "Second target from same version",
            None,
            None,
            "ACTIVE",
        )
        .expect("same exact template version should remain reusable");
        assert_eq!(
            list_engagement_areas(&database.path, &second_target.engagement_id)
                .expect("second target areas")
                .len(),
            2
        );
        assert_eq!(
            list_procedures(&database.path, &second_target.engagement_id)
                .expect("second target procedures")
                .len(),
            1
        );

        let connection =
            open_configured_connection(&database.path).expect("database should reopen");
        let mutation_error = connection
            .execute(
                "UPDATE engagement_template_versions
                 SET definition_json = '{}'
                 WHERE engagement_template_version_id = ?1",
                [&template.latest_version_id],
            )
            .expect_err("template versions must be immutable");
        assert!(mutation_error
            .to_string()
            .contains("engagement template versions are immutable"));
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

        let invalid_pbc = create_pbc_request(
            &database.path,
            NewPbcRequest {
                engagement_id: &second_engagement.engagement_id,
                engagement_area_id: Some(&child_area.engagement_area_id),
                request_number: "PBC-X",
                description: "Invalid cross-engagement request",
                requested_from_party: "Client finance team",
                due_at_ms: None,
                status: "REQUESTED",
                client_visible_content: None,
                internal_notes: None,
            },
        )
        .expect_err("PBC area from another engagement must be rejected");
        assert!(invalid_pbc
            .to_string()
            .contains("PBC request area must belong to the same engagement"));

        let pbc_request = create_pbc_request(
            &database.path,
            NewPbcRequest {
                engagement_id: &engagement.engagement_id,
                engagement_area_id: Some(&child_area.engagement_area_id),
                request_number: "PBC-001",
                description: "Provide vendor onboarding approvals for the selected sample.",
                requested_from_party: "Client finance team",
                due_at_ms: Some(12_345_678),
                status: "REQUESTED",
                client_visible_content: Some("Please upload the approval records for the sample."),
                internal_notes: Some("Do not disclose internal sampling rationale."),
            },
        )
        .expect("PBC request should be created");
        assert_eq!(
            pbc_request.client_visible_content.as_deref(),
            Some("Please upload the approval records for the sample.")
        );
        assert_eq!(
            pbc_request.internal_notes.as_deref(),
            Some("Do not disclose internal sampling rationale.")
        );

        transition_pbc_request_status(
            &database.path,
            &pbc_request.pbc_request_id,
            "RECEIVED",
            Some("auditor@example.test"),
            Some("Client provided the requested records."),
        )
        .expect("PBC status transition should be recorded");
        add_pbc_request_assessment(
            &database.path,
            &pbc_request.pbc_request_id,
            "Received evidence is complete for the selected sample.",
            Some("auditor@example.test"),
        )
        .expect("PBC assessment should be recorded");

        let pbc_evidence = create_pbc_request_evidence_link(
            &database.path,
            &pbc_request.pbc_request_id,
            &support_a.document_id,
            Some(&version_a),
            None,
            Some("Received vendor onboarding support"),
        )
        .expect("PBC received evidence should bind to an exact content version");
        assert_eq!(pbc_evidence.document_name, "Support A.pdf");
        assert_eq!(
            pbc_evidence.content_version_id.as_deref(),
            Some(version_a.as_str())
        );

        let pbc_requests = list_pbc_requests(&database.path, &engagement.engagement_id)
            .expect("PBC requests should list");
        assert_eq!(pbc_requests.len(), 1);
        assert_eq!(pbc_requests[0].status, "RECEIVED");
        assert_eq!(
            pbc_requests[0].latest_assessment.as_deref(),
            Some("Received evidence is complete for the selected sample.")
        );

        let pbc_events = list_pbc_request_events(&database.path, &pbc_request.pbc_request_id)
            .expect("PBC request event history should load");
        assert_eq!(pbc_events.len(), 3);
        assert_eq!(pbc_events[0].event_type, "CREATED");
        assert_eq!(pbc_events[1].event_type, "STATUS_CHANGED");
        assert_eq!(pbc_events[2].event_type, "ASSESSMENT_ADDED");

        let pbc_links =
            list_pbc_request_evidence_links(&database.path, &pbc_request.pbc_request_id)
                .expect("PBC received evidence should list");
        assert_eq!(pbc_links.len(), 1);
        assert_eq!(
            pbc_links[0].pbc_request_evidence_link_id,
            pbc_evidence.pbc_request_evidence_link_id
        );

        let connection =
            open_configured_connection(&database.path).expect("database should reopen");
        let pbc_event_mutation_error = connection
            .execute(
                "UPDATE pbc_request_events
                 SET comment = 'tampered'
                 WHERE pbc_request_event_id = ?1",
                [&pbc_events[1].pbc_request_event_id],
            )
            .expect_err("PBC request event history must be immutable");
        assert!(pbc_event_mutation_error
            .to_string()
            .contains("PBC request events are immutable"));

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

        let reviewer_signoff_error = create_workpaper_signoff(
            &database.path,
            NewWorkpaperSignoff {
                workpaper_id: &workpaper.workpaper_id,
                workpaper_revision_id: &revision_two.workpaper_revision_id,
                signoff_type: "REVIEWED",
                actor_id: "manager@example.test",
                actor_role: "Engagement Manager",
                comment: Some("Attempted reviewer sign-off."),
            },
        )
        .expect_err("reviewer sign-off must reject raw observed evidence");
        assert!(reviewer_signoff_error
            .to_string()
            .contains("immutable hash-verified controlled evidence"));

        let prepared_signoff = create_workpaper_signoff(
            &database.path,
            NewWorkpaperSignoff {
                workpaper_id: &workpaper.workpaper_id,
                workpaper_revision_id: &revision_two.workpaper_revision_id,
                signoff_type: "PREPARED",
                actor_id: "preparer@example.test",
                actor_role: "Senior Associate",
                comment: Some("Prepared revision 2 for review."),
            },
        )
        .expect("preparer sign-off should succeed before formal reviewer reliance");
        assert_eq!(prepared_signoff.revision_number, 2);
        assert_eq!(
            prepared_signoff.evidence_link_ids,
            vec![evidence_link.evidence_link_id.clone()]
        );
        assert_eq!(prepared_signoff.superseded_at_ms, None);

        let post_signoff_link_error = create_workpaper_evidence_link(
            &database.path,
            &revision_two.workpaper_revision_id,
            &support_a.document_id,
            Some(&version_a),
            None,
            "SUPPORTS",
            Some("Attempted evidence mutation after sign-off"),
        )
        .expect_err("signed revision must reject additional evidence");
        assert!(post_signoff_link_error
            .to_string()
            .contains("signed workpaper revision cannot receive additional evidence"));

        let connection =
            open_configured_connection(&database.path).expect("database should reopen");
        let signoff_mutation_error = connection
            .execute(
                "UPDATE workpaper_signoffs
                 SET actor_role = 'tampered'
                 WHERE signoff_id = ?1",
                [&prepared_signoff.signoff_id],
            )
            .expect_err("sign-off row must be immutable");
        assert!(signoff_mutation_error
            .to_string()
            .contains("workpaper sign-offs are immutable"));

        let revision_three = create_workpaper_revision(
            &database.path,
            &workpaper.workpaper_id,
            NewWorkpaperRevision {
                revision_reason: Some("Rework after review and controlled evidence capture"),
                objective: "Confirm onboarding approvals operate as designed.",
                procedure_performed: "Reperformed the expanded onboarding review.",
                population: "All vendors added during the period.",
                sample: "Eight judgmentally selected vendors.",
                exceptions: "One delayed approval noted and evaluated.",
                management_explanation: "Approval was completed the following business day.",
                conclusion: "Control operated with one documented timing exception.",
            },
        )
        .expect("new material revision should be created");

        let signoffs_after_revision_three =
            list_workpaper_signoffs(&database.path, &workpaper.workpaper_id)
                .expect("sign-off history should load");
        assert_eq!(signoffs_after_revision_three.len(), 1);
        assert_eq!(
            signoffs_after_revision_three[0].signoff_id,
            prepared_signoff.signoff_id
        );
        assert_eq!(
            signoffs_after_revision_three[0]
                .superseded_by_revision_id
                .as_deref(),
            Some(revision_three.workpaper_revision_id.as_str())
        );
        assert!(signoffs_after_revision_three[0].superseded_at_ms.is_some());

        let capture_job_id = Uuid::new_v4().to_string();
        let controlled_version_id = Uuid::new_v4().to_string();
        let controlled_locator = format!("test-controlled://{controlled_version_id}");
        let connection =
            open_configured_connection(&database.path).expect("database should reopen");
        connection
            .execute(
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
                 ) VALUES (?1, ?2, ?3, 'COMPLETE', 3000, 3001, 3002, ?4, ?5, NULL, NULL)",
                params![
                    &capture_job_id,
                    &support_a.file_instance_id,
                    &support_a.document_id,
                    "Formal review support",
                    "TEST_CONTROLLED"
                ],
            )
            .expect("controlled evidence capture job should insert");
        connection
            .execute(
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
                 ) VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?7, 64, 3002, ?8, ?9, ?10, 'RETAINED', 'HASH_VERIFIED', 1)",
                params![
                    &controlled_version_id,
                    &support_a.document_id,
                    &support_a.file_instance_id,
                    &version_a,
                    &capture_job_id,
                    &controlled_locator,
                    vec![0xA5_u8; 32],
                    "preparer@example.test",
                    "Formal review support",
                    "TEST_CONTROLLED"
                ],
            )
            .expect("hash-verified controlled evidence should insert");

        let controlled_link = create_workpaper_evidence_link(
            &database.path,
            &revision_three.workpaper_revision_id,
            &support_a.document_id,
            None,
            Some(&controlled_version_id),
            "SUPPORTS",
            Some("Controlled onboarding evidence"),
        )
        .expect("new revision should accept controlled evidence");
        assert_eq!(
            controlled_link.controlled_evidence_version_id.as_deref(),
            Some(controlled_version_id.as_str())
        );

        let reviewer_signoff = create_workpaper_signoff(
            &database.path,
            NewWorkpaperSignoff {
                workpaper_id: &workpaper.workpaper_id,
                workpaper_revision_id: &revision_three.workpaper_revision_id,
                signoff_type: "REVIEWED",
                actor_id: "manager@example.test",
                actor_role: "Engagement Manager",
                comment: Some("Reviewed controlled evidence and revision 3."),
            },
        )
        .expect("reviewer sign-off should succeed on controlled evidence");
        assert_eq!(reviewer_signoff.revision_number, 3);
        assert_eq!(
            reviewer_signoff.evidence_link_ids,
            vec![controlled_link.evidence_link_id.clone()]
        );

        let revision_four = create_workpaper_revision(
            &database.path,
            &workpaper.workpaper_id,
            NewWorkpaperRevision {
                revision_reason: Some("Post-review conclusion refinement"),
                objective: "Confirm onboarding approvals operate as designed.",
                procedure_performed: "Reperformed the expanded onboarding review.",
                population: "All vendors added during the period.",
                sample: "Eight judgmentally selected vendors.",
                exceptions: "One delayed approval noted and evaluated.",
                management_explanation: "Approval was completed the following business day.",
                conclusion:
                    "Control operated effectively subject to one documented timing exception.",
            },
        )
        .expect("post-review material change should create a new revision");

        let signoff_history = list_workpaper_signoffs(&database.path, &workpaper.workpaper_id)
            .expect("sign-off history should load");
        assert_eq!(signoff_history.len(), 2);
        let reviewed_history = signoff_history
            .iter()
            .find(|entry| entry.signoff_id == reviewer_signoff.signoff_id)
            .expect("reviewer sign-off should remain historical");
        assert_eq!(
            reviewed_history.superseded_by_revision_id.as_deref(),
            Some(revision_four.workpaper_revision_id.as_str())
        );
        assert!(reviewed_history.superseded_at_ms.is_some());
        let prepared_history = signoff_history
            .iter()
            .find(|entry| entry.signoff_id == prepared_signoff.signoff_id)
            .expect("preparer sign-off should remain historical");
        assert_eq!(
            prepared_history.superseded_by_revision_id.as_deref(),
            Some(revision_three.workpaper_revision_id.as_str())
        );

        assert_eq!(
            count_audit_events_for_test(
                &database.path,
                "WORKPAPER_SIGNOFF_CREATED",
                &prepared_signoff.signoff_id,
            )
            .expect("preparer sign-off audit count should load"),
            1
        );
        assert_eq!(
            count_audit_events_for_test(
                &database.path,
                "WORKPAPER_SIGNOFF_SUPERSEDED",
                &reviewer_signoff.signoff_id,
            )
            .expect("reviewer sign-off supersession audit count should load"),
            1
        );

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
        assert_eq!(workpapers[0].latest_revision_number, Some(4));

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
