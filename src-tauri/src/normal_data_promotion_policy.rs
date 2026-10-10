//! Internal, fail-closed Normal Data -> workpaper authorization boundary.
//!
//! This is not a Tauri command and performs no capture or attachment. No
//! authenticated principal provider is installed yet. Only a future trusted
//! native identity provider may construct VerifiedPrincipal in production;
//! calling this with no principal always fails. The test-only constructor
//! cannot be compiled into the shipping application.
use crate::{
    normal_data_provenance,
    persistence::{self, PersistenceError},
};
use rusqlite::{params, OptionalExtension};
use std::path::Path;
use uuid::Uuid;

/// Identity verified by an authentication mechanism outside any frontend
/// payload. An actor_id string or client association does not construct this.
pub(crate) struct VerifiedPrincipal {
    subject_id: String,
}

#[cfg(test)]
impl VerifiedPrincipal {
    pub(crate) fn fixture(subject_id: &str) -> Self {
        Self {
            subject_id: subject_id.to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PromotionPermission {
    ReadNormalDataWorkspace,
    AttachEvidenceToEngagement,
    ModifyWorkpaperRevision,
}

/// Future adapter to an authenticated, enforced permission system. Do not use
/// client-supplied role labels as implementations of this policy.
pub(crate) trait PromotionPermissionPolicy {
    fn is_allowed(
        &self,
        principal_id: &str,
        permission: PromotionPermission,
        resource_id: &str,
    ) -> bool;
}

/// Deliberate default while no trusted authenticator and RBAC exist.
pub(crate) struct DenyAllPromotionPermissions;

impl PromotionPermissionPolicy for DenyAllPromotionPermissions {
    fn is_allowed(
        &self,
        _principal_id: &str,
        _permission: PromotionPermission,
        _resource_id: &str,
    ) -> bool {
        false
    }
}

/// Caller-selected IDs must each be independently verified against immutable
/// database relationships and trusted permissions, never against a path or
/// the "latest" data version.
pub(crate) struct PromotionIntent<'a> {
    pub run_id: &'a str,
    pub expected_workspace_id: &'a str,
    pub target_engagement_id: &'a str,
    pub target_workpaper_id: &'a str,
    pub target_workpaper_revision_id: &'a str,
}

fn deny() -> PersistenceError {
    PersistenceError::Configuration(
        "Normal Data specialist promotion is unavailable without an authenticated, authorized caller and eligible target"
            .to_string(),
    )
}

fn eligible_draft_state(value: &str) -> bool {
    // Fail closed on unusual custom workflow states. Formal/reviewed/final
    // states, including their aliases, are never suitable for new attachments.
    matches!(value, "NOT_STARTED" | "IN_PROGRESS" | "DRAFT")
}

/// Preflight only. No persistent approval token is returned: a future D3
/// mutation must repeat authorization and state verification *inside its own
/// write transaction* immediately before linking retained evidence.
pub(crate) fn check_policy(
    database_path: &Path,
    intent: &PromotionIntent<'_>,
    principal: Option<&VerifiedPrincipal>,
    permissions: &dyn PromotionPermissionPolicy,
) -> Result<(), PersistenceError> {
    // Deny before database lookups to avoid leaking another workspace/target
    // to an unauthenticated frontend. A UUID or actor string is NOT identity.
    let principal = principal.ok_or_else(deny)?;
    for id in [
        intent.run_id,
        intent.expected_workspace_id,
        intent.target_engagement_id,
        intent.target_workpaper_id,
        intent.target_workpaper_revision_id,
    ] {
        Uuid::parse_str(id).map_err(|_| deny())?;
    }
    for (permission, resource_id) in [
        (
            PromotionPermission::ReadNormalDataWorkspace,
            intent.expected_workspace_id,
        ),
        (
            PromotionPermission::AttachEvidenceToEngagement,
            intent.target_engagement_id,
        ),
        (
            PromotionPermission::ModifyWorkpaperRevision,
            intent.target_workpaper_revision_id,
        ),
    ] {
        if !permissions.is_allowed(&principal.subject_id, permission, resource_id) {
            return Err(deny());
        }
    }

    let source = normal_data_provenance::inspect_run(database_path, intent.run_id)?;
    if source.normal_data_workspace_id != intent.expected_workspace_id {
        return Err(deny());
    }

    let connection = persistence::open_configured_connection(database_path)?;
    let target: Option<(String, String, String, bool, bool)> = connection
        .query_row(
            "SELECT w.engagement_id, w.workflow_state,
                    (SELECT latest.workpaper_revision_id
                     FROM workpaper_revisions latest
                     WHERE latest.workpaper_id = w.workpaper_id
                     ORDER BY latest.revision_number DESC LIMIT 1),
                    EXISTS(SELECT 1 FROM workpaper_signoffs s
                           WHERE s.workpaper_revision_id = wr.workpaper_revision_id),
                    EXISTS(SELECT 1 FROM review_notes n
                           WHERE n.workpaper_revision_id = wr.workpaper_revision_id
                             AND n.current_state <> 'CLEARED')
             FROM workpaper_revisions wr
             JOIN workpapers w ON w.workpaper_id = wr.workpaper_id
                              AND w.archived_at_ms IS NULL
             JOIN engagements e ON e.engagement_id = w.engagement_id
                               AND e.archived_at_ms IS NULL
             JOIN clients c ON c.client_id = e.client_id
                           AND c.archived_at_ms IS NULL
             WHERE wr.workpaper_revision_id = ?1
               AND wr.workpaper_id = ?2",
            params![intent.target_workpaper_revision_id, intent.target_workpaper_id],
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
    let Some((actual_engagement, workflow_state, latest_revision, signed, unresolved)) = target
    else {
        return Err(deny());
    };
    if actual_engagement != intent.target_engagement_id
        || latest_revision != intent.target_workpaper_revision_id
        || !eligible_draft_state(&workflow_state)
        || signed
        || unresolved
    {
        return Err(deny());
    }
    Ok(())
}
