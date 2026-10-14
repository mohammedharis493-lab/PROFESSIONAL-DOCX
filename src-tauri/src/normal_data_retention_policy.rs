//! A retained candidate and v31 ledger remain working-data only.
//! No production principal issuer, Tauri command, evidence or linkage.
use crate::{
    normal_data_promotion_policy::{self, PromotionIntent, VerifiedPrincipal},
    normal_data_retention_store::{self, RetentionCandidateRecord},
    persistence::PersistenceError,
};
use std::path::Path;

fn denied() -> PersistenceError {
    PersistenceError::Configuration(
        "Normal Data candidate is not bound to the exact target".to_string(),
    )
}

/// Revalidate candidate bytes and ledger on the same SQLite transaction as
/// live grants, frozen-run provenance and latest unsigned draft target state.
/// Filesystem writers are NOT isolated by SQLite. This is a non-reusable,
/// read-only observation; formal capture must redo checks before writing.
pub(crate) fn inspect_authorized_retention_candidate(
    database_path: &Path,
    retention_root: &Path,
    stage_id: &str,
    intent: &PromotionIntent<'_>,
    principal: Option<&VerifiedPrincipal>,
) -> Result<RetentionCandidateRecord, PersistenceError> {
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
            if record.run_id != intent.run_id || record.workspace_id != intent.expected_workspace_id
            {
                return Err(denied());
            }
            Ok(record)
        },
    )
}
