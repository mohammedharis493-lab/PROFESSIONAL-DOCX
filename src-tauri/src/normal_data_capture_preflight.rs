//! Read-only, transaction-bound three-artifact capture *preflight*.
//! This is NOT a controlled-evidence capture plan approval, transfer token,
//! retained evidence, audit event, trusted principal or workpaper linkage.
//! The candidate filesystem can change immediately after inspection.
use crate::{
    normal_data_promotion_policy::{self, PromotionIntent, VerifiedPrincipal},
    normal_data_retention_store::{self, CandidateArtifactSet},
    persistence::PersistenceError,
};
use std::path::Path;

/// Exact three roles and the specific, currently eligible target inspected.
/// The caller MUST repeat authorization and byte verification before *any*
/// later write. These strings never identify controlled-evidence versions.
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

/// Requires the unavailable-in-production VerifiedPrincipal. Within the same
/// IMMEDIATE SQLite transaction, rechecks scoped grants, frozen-run identities,
/// unsigned/current target and all three candidate bytes + ledger + manifest.
/// Returns descriptive metadata only; no paths, bytes, evidence IDs or reusable
/// permission receipt. Passing a stale observation to a later capture is unsafe.
pub(crate) fn inspect_exact_capture_preflight(
    database_path: &Path,
    retention_root: &Path,
    stage_id: &str,
    intent: &PromotionIntent<'_>,
    principal: Option<&VerifiedPrincipal>,
) -> Result<CapturePreflightObservation, PersistenceError> {
    normal_data_promotion_policy::with_authorized_transaction(
        database_path,
        intent,
        principal,
        |tx| {
            let record = normal_data_retention_store::inspect_registered_candidate_on_connection(
                tx,
                retention_root,
                stage_id,
                intent.run_id,
            )?;
            if record.run_id != intent.run_id
                || record.workspace_id != intent.expected_workspace_id
                || record.stage_id != stage_id
            {
                return Err(PersistenceError::Configuration(
                    "capture preflight candidate does not match exact requested run".to_string(),
                ));
            }
            Ok(CapturePreflightObservation {
                run_id: record.run_id.clone(),
                workspace_id: record.workspace_id.clone(),
                recipe_version_id: record.recipe_version_id.clone(),
                candidate_stage_id: record.stage_id.clone(),
                target_engagement_id: intent.target_engagement_id.to_string(),
                target_workpaper_id: intent.target_workpaper_id.to_string(),
                target_workpaper_revision_id: intent.target_workpaper_revision_id.to_string(),
                artifacts: record.describe_three_artifacts(),
            })
        },
    )
}
