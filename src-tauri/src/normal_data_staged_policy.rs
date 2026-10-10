//! Transaction-bound, *read-only* reconciliation of a private staging package
//! to one exact Normal Data run and eligible specialist target.
//!
//! This is NOT authenticated promotion. No trusted principal constructor exists
//! in production, no Tauri command exposes these routines, and no evidence or
//! signoff is created. A ready staging package is never an approval token.
use crate::{
    normal_data_promotion_policy::{self, PromotionIntent, VerifiedPrincipal},
    normal_data_provenance,
    normal_data_staging::{self, ExpectedStageRun, StageReceipt},
    persistence::PersistenceError,
};
use rusqlite::{params, OptionalExtension, Transaction};
use sha2::{Digest, Sha256};
use std::{fmt::Write, path::Path};

const MAX_STORED_RESULT_BYTES: usize = 8 * 1024 * 1024;

fn deny() -> PersistenceError {
    PersistenceError::Configuration(
        "Normal Data staging does not match an authorized exact frozen run".to_string(),
    )
}

fn frozen_document_id(
    tx: &Transaction<'_>,
    dataset_version_id: &str,
    content_version_id: &str,
) -> Result<String, PersistenceError> {
    tx.query_row(
        "SELECT v.document_id
         FROM normal_data_dataset_versions v
         JOIN content_versions cv
           ON cv.content_version_id = v.content_version_id
          AND cv.document_id = v.document_id
          AND cv.file_instance_id = v.file_instance_id
         WHERE v.normal_data_dataset_version_id = ?1
           AND v.content_version_id = ?2
           AND v.source_verification_state = 'HASH_VERIFIED'
           AND cv.verification_state = 'HASH_VERIFIED'
           AND v.source_sha256 = cv.sha256",
        params![dataset_version_id, content_version_id],
        |row| row.get(0),
    )
    .optional()?
    .ok_or_else(deny)
}

fn exact_stored_result_byte_hash(
    tx: &Transaction<'_>,
    run_id: &str,
    recipe_version_id: &str,
) -> Result<String, PersistenceError> {
    let frozen: Option<String> = tx
        .query_row(
            "SELECT result_json FROM normal_data_comparison_runs
             WHERE normal_data_comparison_run_id = ?1
               AND normal_data_comparison_recipe_version_id = ?2",
            params![run_id, recipe_version_id],
            |row| row.get(0),
        )
        .optional()?;
    let json = frozen.ok_or_else(deny)?;
    if json.is_empty() || json.len() > MAX_STORED_RESULT_BYTES {
        return Err(deny());
    }
    let mut hex = String::with_capacity(64);
    for byte in Sha256::digest(json.as_bytes()) {
        write!(&mut hex, "{byte:02x}").expect("writing digest to String");
    }
    Ok(hex)
}

/// Tests a staging package against the *same SQLite transaction* that checks
/// a verified identity, all three live scoped grants, current workpaper state,
/// and the immutable comparison run. All run/source/document/content bindings
/// and exact persisted result JSON bytes are independently checked against
/// the staging manifest, and all three staged files are hash-verified again.
///
/// This returns a read-only observation, not a privileged handle: filesystem
/// bytes may change after return, and neither this method nor StageReceipt may
/// authorize later writes. Future D3 linkage must repeat checks inside its
/// own final write transaction, and must independently persist the exact
/// original + result bytes in an immutable controlled-evidence store.
pub(crate) fn inspect_authorized_staged_run(
    database_path: &Path,
    staging_root: &Path,
    stage_id: &str,
    intent: &PromotionIntent<'_>,
    principal: Option<&VerifiedPrincipal>,
) -> Result<StageReceipt, PersistenceError> {
    normal_data_promotion_policy::with_authorized_transaction(
        database_path,
        intent,
        principal,
        |tx| {
            let receipt = normal_data_provenance::inspect_run_on_connection(tx, intent.run_id)?;
            let document_a = frozen_document_id(
                tx,
                &receipt.source_a.dataset_version_id,
                &receipt.source_a.content_version_id,
            )?;
            let document_b = frozen_document_id(
                tx,
                &receipt.source_b.dataset_version_id,
                &receipt.source_b.content_version_id,
            )?;
            let result_artifact_hash = exact_stored_result_byte_hash(
                tx,
                &receipt.normal_data_comparison_run_id,
                &receipt.normal_data_comparison_recipe_version_id,
            )?;
            let expected = ExpectedStageRun {
                provenance: &receipt,
                source_a_document_id: &document_a,
                source_b_document_id: &document_b,
                result_artifact_sha256_hex: &result_artifact_hash,
            };
            let inspected =
                normal_data_staging::inspect_stage_against_run(staging_root, stage_id, &expected)?;
            if inspected.run_id != intent.run_id
                || inspected.workspace_id != intent.expected_workspace_id
            {
                return Err(deny());
            }
            Ok(inspected)
        },
    )
}
