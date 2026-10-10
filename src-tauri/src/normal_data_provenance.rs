//! Read-only historical provenance for one completed Normal Data comparison.
//! This is a working-data receipt, NOT source recapture, specialist attachment,
//! authorization to promote, or evidence eligible for audit sign-off.
use crate::{
    normal_data_comparison::{self, ComparisonResult, PeriodBasis},
    normal_data_source_reader,
    persistence::{self, PersistenceError},
};
use rusqlite::OptionalExtension;
use serde::Serialize;
use std::{fmt::Write, path::Path};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunSourceReceipt {
    pub dataset_version_id: String,
    pub content_version_id: String,
    pub sha256_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunProvenanceReceipt {
    pub normal_data_comparison_run_id: String,
    pub normal_data_workspace_id: String,
    pub normal_data_comparison_recipe_version_id: String,
    pub source_a: RunSourceReceipt,
    pub source_b: RunSourceReceipt,
    pub result_sha256_hex: String,
    pub period_basis: String,
    pub started_at_ms: i64,
    pub completed_at_ms: i64,
    /// Historical statement about verified bytes read during the recorded run;
    /// not a fresh file read, certificate of retention, or current file status.
    pub verification_at_execution: String,
    pub current_source_bytes_checked: bool,
    pub is_controlled_evidence: bool,
    pub specialist_promotion_authorized: bool,
}

/// A separate live read of one immutable dataset source. The read-time
/// observation is not a retained capture and may become stale immediately.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceReadObservation {
    pub dataset_version_id: String,
    pub content_version_id: String,
    pub sha256_hex: String,
    pub observed_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunSourcePreflight {
    pub normal_data_comparison_run_id: String,
    pub source_a: SourceReadObservation,
    pub source_b: SourceReadObservation,
    /// Two independent, sequential hash-verified reads—not an atomic snapshot
    /// of both files and not a guarantee about subsequent capture operations.
    pub exact_sources_verified_at_read: bool,
    pub is_controlled_evidence: bool,
    pub specialist_promotion_authorized: bool,
}

struct FrozenRun {
    recipe_version_id: String,
    workspace_id: String,
    dataset_a_id: String,
    dataset_b_id: String,
    content_a_id: String,
    content_b_id: String,
    frozen_a_hash: Vec<u8>,
    frozen_b_hash: Vec<u8>,
    run_a_hash: Vec<u8>,
    run_b_hash: Vec<u8>,
    result_hash: Vec<u8>,
    result_json: String,
    recipe_period_basis: String,
    recipe_amount_columns_json: String,
    recipe_tolerance: i64,
    started_at_ms: i64,
    completed_at_ms: i64,
}

fn invalid(message: &str) -> PersistenceError {
    PersistenceError::Configuration(message.to_string())
}

fn encode_hash(bytes: &[u8]) -> Result<String, PersistenceError> {
    if bytes.len() != 32 {
        return Err(invalid(
            "stored Normal Data provenance has an invalid digest length",
        ));
    }
    let mut text = String::with_capacity(64);
    for byte in bytes {
        write!(&mut text, "{byte:02x}").expect("hex encoding cannot fail");
    }
    Ok(text)
}

/// Returns a bounded identity/hash receipt without touching linked source files
/// or creating any new evidence or audit event. No current-source verification
/// or authenticated promotion authority is inferred from database identity.
pub fn inspect_run(
    database_path: &Path,
    run_id: &str,
) -> Result<RunProvenanceReceipt, PersistenceError> {
    Uuid::parse_str(run_id).map_err(|_| invalid("comparison run ID must be a UUID"))?;
    let connection = persistence::open_configured_connection(database_path)?;
    let frozen: FrozenRun = connection
        .query_row(
            "SELECT r.normal_data_comparison_recipe_version_id,
                cr.normal_data_workspace_id,
                rv.dataset_a_version_id, rv.dataset_b_version_id,
                av.content_version_id, bv.content_version_id,
                av.source_sha256, bv.source_sha256,
                r.dataset_a_source_sha256, r.dataset_b_source_sha256,
                r.result_sha256, r.result_json, rv.period_basis,
                rv.amount_columns_json, rv.tolerance_minor_units,
                r.started_at_ms, r.completed_at_ms
         FROM normal_data_comparison_runs r
         JOIN normal_data_comparison_recipe_versions rv
           ON rv.normal_data_comparison_recipe_version_id =
              r.normal_data_comparison_recipe_version_id
         JOIN normal_data_comparison_recipes cr
           ON cr.normal_data_comparison_recipe_id = rv.normal_data_comparison_recipe_id
         JOIN normal_data_dataset_versions av
           ON av.normal_data_dataset_version_id = rv.dataset_a_version_id
         JOIN normal_data_dataset_versions bv
           ON bv.normal_data_dataset_version_id = rv.dataset_b_version_id
         JOIN normal_data_datasets ad ON ad.normal_data_dataset_id = av.normal_data_dataset_id
         JOIN normal_data_datasets bd ON bd.normal_data_dataset_id = bv.normal_data_dataset_id
         JOIN content_versions ac ON ac.content_version_id = av.content_version_id
           AND ac.document_id = av.document_id AND ac.file_instance_id = av.file_instance_id
         JOIN content_versions bc ON bc.content_version_id = bv.content_version_id
           AND bc.document_id = bv.document_id AND bc.file_instance_id = bv.file_instance_id
         WHERE r.normal_data_comparison_run_id = ?1
           AND ad.normal_data_workspace_id = cr.normal_data_workspace_id
           AND bd.normal_data_workspace_id = cr.normal_data_workspace_id
           AND av.source_verification_state = 'HASH_VERIFIED'
           AND bv.source_verification_state = 'HASH_VERIFIED'
           AND av.source_stable_during_read = 1
           AND bv.source_stable_during_read = 1
           AND ac.verification_state = 'HASH_VERIFIED'
           AND bc.verification_state = 'HASH_VERIFIED'
           AND ac.source_stable_during_read = 1
           AND bc.source_stable_during_read = 1
           AND ac.sha256 = av.source_sha256
           AND bc.sha256 = bv.source_sha256
           AND ac.size_bytes = av.source_size_bytes
           AND bc.size_bytes = bv.source_size_bytes",
            [run_id],
            |row| {
                Ok(FrozenRun {
                    recipe_version_id: row.get(0)?,
                    workspace_id: row.get(1)?,
                    dataset_a_id: row.get(2)?,
                    dataset_b_id: row.get(3)?,
                    content_a_id: row.get(4)?,
                    content_b_id: row.get(5)?,
                    frozen_a_hash: row.get(6)?,
                    frozen_b_hash: row.get(7)?,
                    run_a_hash: row.get(8)?,
                    run_b_hash: row.get(9)?,
                    result_hash: row.get(10)?,
                    result_json: row.get(11)?,
                    recipe_period_basis: row.get(12)?,
                    recipe_amount_columns_json: row.get(13)?,
                    recipe_tolerance: row.get(14)?,
                    started_at_ms: row.get(15)?,
                    completed_at_ms: row.get(16)?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| invalid("verified historical comparison run was not found"))?;

    let a_hash = encode_hash(&frozen.run_a_hash)?;
    let b_hash = encode_hash(&frozen.run_b_hash)?;
    if frozen.run_a_hash != frozen.frozen_a_hash || frozen.run_b_hash != frozen.frozen_b_hash {
        return Err(invalid(
            "stored run source hashes do not match immutable dataset versions",
        ));
    }
    let result_hash = encode_hash(&frozen.result_hash)?;
    let result: ComparisonResult = serde_json::from_str(&frozen.result_json)
        .map_err(|_| invalid("stored comparison result JSON is invalid"))?;
    normal_data_comparison::verify_stored_result_digest(&result)
        .map_err(|_| invalid("stored comparison result has a mismatched canonical digest"))?;
    if result.result_sha256_hex != result_hash {
        return Err(invalid("stored result digest differs from run metadata"));
    }
    let expected_basis = match result.period_basis {
        PeriodBasis::FilingPeriod => "FILING_PERIOD",
        PeriodBasis::InvoiceMonth => "INVOICE_MONTH",
        PeriodBasis::AccountingPeriod => "ACCOUNTING_PERIOD",
    };
    let frozen_fields: Vec<String> = serde_json::from_str(&frozen.recipe_amount_columns_json)
        .map_err(|_| invalid("stored comparison recipe numeric columns are invalid"))?;
    if frozen.recipe_period_basis != expected_basis
        || frozen.recipe_tolerance != result.tolerance_minor_units
        || frozen_fields != result.amount_columns
    {
        return Err(invalid(
            "stored run settings do not match frozen recipe version",
        ));
    }

    Ok(RunProvenanceReceipt {
        normal_data_comparison_run_id: run_id.to_string(),
        normal_data_workspace_id: frozen.workspace_id,
        normal_data_comparison_recipe_version_id: frozen.recipe_version_id,
        source_a: RunSourceReceipt {
            dataset_version_id: frozen.dataset_a_id,
            content_version_id: frozen.content_a_id,
            sha256_hex: a_hash,
        },
        source_b: RunSourceReceipt {
            dataset_version_id: frozen.dataset_b_id,
            content_version_id: frozen.content_b_id,
            sha256_hex: b_hash,
        },
        result_sha256_hex: result_hash,
        period_basis: frozen.recipe_period_basis,
        started_at_ms: frozen.started_at_ms,
        completed_at_ms: frozen.completed_at_ms,
        verification_at_execution: "HASH_VERIFIED_RECORDED".to_string(),
        current_source_bytes_checked: false,
        is_controlled_evidence: false,
        specialist_promotion_authorized: false,
    })
}

fn verify_one_current_source(
    database_path: &Path,
    frozen: &RunSourceReceipt,
) -> Result<SourceReadObservation, PersistenceError> {
    // The existing approved-root reader enforces the indexed document / file /
    // content-version binding, a stable source read, 32 MiB maximum size, and
    // SHA-256 equality. It returns no user-controlled filesystem path.
    let verified = normal_data_source_reader::read_verified_dataset(
        database_path,
        &frozen.dataset_version_id,
    )?;
    if verified.dataset_version_id != frozen.dataset_version_id
        || verified.content_version_id != frozen.content_version_id
        || encode_hash(&verified.sha256)? != frozen.sha256_hex
    {
        return Err(invalid(
            "live source does not match the exact frozen comparison-run input",
        ));
    }
    // Read has finished before the observation is stamped. Discard its bytes:
    // preflight must never persist or expose the originals.
    Ok(SourceReadObservation {
        dataset_version_id: verified.dataset_version_id,
        content_version_id: verified.content_version_id,
        sha256_hex: frozen.sha256_hex.clone(),
        observed_at_ms: persistence::now_unix_ms()?,
    })
}

/// Optional explicit, bounded live input recheck. Fails closed if either input
/// is missing, has changed (including same-size tampering), or no longer passes
/// the approved-root / stable-read / SHA-256 checks. This command is **not**
/// evidence capture, caller authorization, or promotion approval; a subsequent
/// operation must re-check its own sources and access policy.
pub fn recheck_run_sources(
    database_path: &Path,
    run_id: &str,
) -> Result<RunSourcePreflight, PersistenceError> {
    let historical = inspect_run(database_path, run_id)?;
    // Individual observations occur at different times; no atomicity across
    // files and no promise that bytes remain identical after these reads.
    let source_a = verify_one_current_source(database_path, &historical.source_a)?;
    let source_b = verify_one_current_source(database_path, &historical.source_b)?;
    Ok(RunSourcePreflight {
        normal_data_comparison_run_id: historical.normal_data_comparison_run_id,
        source_a,
        source_b,
        exact_sources_verified_at_read: true,
        is_controlled_evidence: false,
        specialist_promotion_authorized: false,
    })
}
