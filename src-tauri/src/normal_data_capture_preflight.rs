//! Native-only transaction-bound exact three-artifact *working-data* reads.
//! Not controlled-evidence capture, durable custody, a permission token,
//! a trusted identity issuer, an audit event, or specialist linkage.
use crate::{
    normal_data_promotion_policy::{self, PromotionIntent, VerifiedPrincipal},
    normal_data_retention_store::{self, CandidateArtifactSet},
    persistence::PersistenceError,
};
use std::path::Path;

/// Exact roles and current target inspected; metadata is NOT a capture grant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CapturePreflightObservation {
    pub run_id: String,
    pub workspace_id: String,
    pub recipe_version_id: String,
    pub candidate_stage_id: String,
    pub target_engagement_id: String,
    pub target_workpaper_id: String,
    pub target_workpaper_revision_id: String,
    pub artifacts: CandidateArtifactSet,
}

/// Exact in-memory bytes from ONE registered candidate verification pass.
/// Three role-specific buffers are bounded, rehashed and bound to immutable
/// run/source/result provenance inside the CURRENT authorization transaction.
/// No file paths, database evidence versions or link mutations are returned.
/// Future formal capture MUST perform its own final transactional authorization
/// and durable, authenticated controlled-retention operation.
pub(crate) struct CaptureMaterialObservation {
    pub preflight: CapturePreflightObservation,
    pub original_a_bytes: Vec<u8>,
    pub original_b_bytes: Vec<u8>,
    pub frozen_result_json_bytes: Vec<u8>,
}

fn mismatch() -> PersistenceError {
    PersistenceError::Configuration(
        "capture material does not match the exact authorized candidate".to_string(),
    )
}

/// Read source A, source B, and the exact stored result JSON once each, then
/// return those SAME verified buffers. The VerifiedPrincipal is not constructible
/// in a production build. No Tauri API exposes this operation. SQLite does NOT
/// isolate external filesystem writes and returning buffers never grants later
/// permission or certifies secure controlled evidence storage.
pub(crate) fn read_exact_capture_material(
    database_path: &Path,
    retention_root: &Path,
    stage_id: &str,
    intent: &PromotionIntent<'_>,
    principal: Option<&VerifiedPrincipal>,
) -> Result<CaptureMaterialObservation, PersistenceError> {
    normal_data_promotion_policy::with_authorized_transaction(
        database_path,
        intent,
        principal,
        |tx| {
            let verified = normal_data_retention_store::read_registered_candidate_bytes_on_connection(
                tx,
                retention_root,
                stage_id,
                intent.run_id,
            )?;
            let record = &verified.record;
            if record.run_id != intent.run_id
                || record.workspace_id != intent.expected_workspace_id
                || record.stage_id != stage_id
            {
                return Err(mismatch());
            }
            Ok(CaptureMaterialObservation {
                preflight: CapturePreflightObservation {
                    run_id: record.run_id.clone(),
                    workspace_id: record.workspace_id.clone(),
                    recipe_version_id: record.recipe_version_id.clone(),
                    candidate_stage_id: record.stage_id.clone(),
                    target_engagement_id: intent.target_engagement_id.to_string(),
                    target_workpaper_id: intent.target_workpaper_id.to_string(),
                    target_workpaper_revision_id: intent.target_workpaper_revision_id.to_string(),
                    artifacts: record.describe_three_artifacts(),
                },
                original_a_bytes: verified.source_a_bytes,
                original_b_bytes: verified.source_b_bytes,
                frozen_result_json_bytes: verified.result_json_bytes,
            })
        },
    )
}

/// Metadata-only projection of a fresh three-buffer inspection. It is never
/// an authorization token, a retained version or an enduring integrity claim.
pub(crate) fn inspect_exact_capture_preflight(
    database_path: &Path,
    retention_root: &Path,
    stage_id: &str,
    intent: &PromotionIntent<'_>,
    principal: Option<&VerifiedPrincipal>,
) -> Result<CapturePreflightObservation, PersistenceError> {
    Ok(read_exact_capture_material(
        database_path,
        retention_root,
        stage_id,
        intent,
        principal,
    )?
    .preflight)
}
