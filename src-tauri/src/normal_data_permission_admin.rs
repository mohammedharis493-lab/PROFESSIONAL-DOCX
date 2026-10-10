//! Native-only, transactionally audited permission administration primitives.
//! No production trusted administrator constructor or Tauri exposure exists.
//! A UUID or asserted role string is never an administrator credential.
use crate::{
    normal_data_promotion_policy::PromotionPermission,
    persistence::{self, PersistenceError},
};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use std::path::Path;
use uuid::Uuid;

/// Only an external trusted native authentication and administration adapter
/// may create this capability in production. Its bootstrap is NOT implemented.
pub(crate) struct VerifiedGrantAdministrator {
    subject_id: String,
}

#[cfg(test)]
impl VerifiedGrantAdministrator {
    fn fixture(subject_id: &str) -> Self {
        Self {
            subject_id: subject_id.to_string(),
        }
    }
}

fn denied() -> PersistenceError {
    PersistenceError::Configuration(
        "Normal Data permission administration requires trusted administrator authority"
            .to_string(),
    )
}

fn valid_uuid(value: &str) -> Result<(), PersistenceError> {
    Uuid::parse_str(value).map(|_| ()).map_err(|_| denied())
}

fn administrator_id(admin: &VerifiedGrantAdministrator) -> Result<&str, PersistenceError> {
    valid_uuid(&admin.subject_id)?;
    Ok(&admin.subject_id)
}

fn append_admin_event(
    tx: &rusqlite::Transaction<'_>,
    actor: &str,
    subject_id: &str,
    grant_id: Option<&str>,
    event_type: &str,
    now: i64,
) -> Result<(), PersistenceError> {
    tx.execute(
        "INSERT INTO normal_data_permission_admin_events (
            event_id, event_type, administrator_subject_id, subject_id, grant_id, occurred_at_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            Uuid::new_v4().to_string(),
            event_type,
            actor,
            subject_id,
            grant_id,
            now
        ],
    )?;
    Ok(())
}

/// Associate an externally attested subject UUID with the trusted native issuer.
/// Cannot be used by frontend clients. A DB row alone never authenticates.
pub(crate) fn enroll_subject(
    database_path: &Path,
    admin: &VerifiedGrantAdministrator,
    subject_id: &str,
) -> Result<(), PersistenceError> {
    let actor = administrator_id(admin)?;
    valid_uuid(subject_id)?;
    let mut connection = persistence::open_configured_connection(database_path)?;
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let now = persistence::now_unix_ms()?;
    tx.execute(
        "INSERT INTO normal_data_permission_subjects (
            subject_id, identity_issuer, registered_at_ms
         ) VALUES (?1, 'TRUSTED_NATIVE_IDP', ?2)",
        params![subject_id, now],
    )?;
    append_admin_event(&tx, actor, subject_id, None, "SUBJECT_ENROLLED", now)?;
    tx.commit()?;
    Ok(())
}

fn resource_exists(
    tx: &rusqlite::Transaction<'_>,
    permission: PromotionPermission,
    resource_id: &str,
) -> Result<bool, PersistenceError> {
    let query = match permission {
        PromotionPermission::ReadNormalDataWorkspace => {
            "SELECT EXISTS(
                 SELECT 1 FROM normal_data_workspaces
                 WHERE normal_data_workspace_id = ?1
             )"
        }
        PromotionPermission::AttachEvidenceToEngagement => {
            "SELECT EXISTS(
                 SELECT 1 FROM engagements e
                 JOIN clients c ON c.client_id = e.client_id
                 WHERE e.engagement_id = ?1
                   AND e.archived_at_ms IS NULL AND c.archived_at_ms IS NULL
             )"
        }
        PromotionPermission::ModifyWorkpaperRevision => {
            "SELECT EXISTS(
                 SELECT 1 FROM workpaper_revisions wr
                 JOIN workpapers w ON w.workpaper_id = wr.workpaper_id
                 JOIN engagements e ON e.engagement_id = w.engagement_id
                 JOIN clients c ON c.client_id = e.client_id
                 WHERE wr.workpaper_revision_id = ?1
                   AND w.archived_at_ms IS NULL
                   AND e.archived_at_ms IS NULL AND c.archived_at_ms IS NULL
                   AND wr.revision_number = (
                       SELECT MAX(latest.revision_number)
                       FROM workpaper_revisions latest
                       WHERE latest.workpaper_id = wr.workpaper_id
                   )
                   AND w.workflow_state IN ('NOT_STARTED', 'IN_PROGRESS', 'DRAFT')
                   AND NOT EXISTS (
                       SELECT 1 FROM workpaper_signoffs s
                       WHERE s.workpaper_revision_id = wr.workpaper_revision_id
                   )
                   AND NOT EXISTS (
                       SELECT 1 FROM review_notes n
                       WHERE n.workpaper_revision_id = wr.workpaper_revision_id
                         AND n.current_state <> 'CLEARED'
                   )
             )"
        }
    };
    let exists: i64 = tx.query_row(query, [resource_id], |row| row.get(0))?;
    Ok(exists == 1)
}

/// Issue exactly one scoped, non-transferable grant, with optional expiry.
/// A future trusted administrator adapter must decide whether an actor can
/// issue this kind of grant; this module neither verifies identity nor grants
/// administrator powers to an ordinary enrolled subject.
pub(crate) fn issue_grant(
    database_path: &Path,
    admin: &VerifiedGrantAdministrator,
    subject_id: &str,
    permission: PromotionPermission,
    resource_id: &str,
    expires_at_ms: Option<i64>,
) -> Result<String, PersistenceError> {
    let actor = administrator_id(admin)?;
    valid_uuid(subject_id)?;
    valid_uuid(resource_id)?;
    let mut connection = persistence::open_configured_connection(database_path)?;
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let now = persistence::now_unix_ms()?;
    if expires_at_ms.is_some_and(|expiry| expiry <= now) {
        return Err(denied());
    }
    let active: Option<i64> = tx
        .query_row(
            "SELECT registered_at_ms
             FROM normal_data_permission_subjects
             WHERE subject_id = ?1
               AND identity_issuer = 'TRUSTED_NATIVE_IDP'
               AND registered_at_ms <= ?2 AND disabled_at_ms IS NULL",
            params![subject_id, now],
            |row| row.get(0),
        )
        .optional()?;
    if active.is_none() || !resource_exists(&tx, permission, resource_id)? {
        return Err(denied());
    }
    let grant_id = Uuid::new_v4().to_string();
    tx.execute(
        "INSERT INTO normal_data_permission_grants (
            grant_id, subject_id, permission, resource_id, granted_at_ms, expires_at_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            &grant_id,
            subject_id,
            permission.database_key(),
            resource_id,
            now,
            expires_at_ms
        ],
    )?;
    append_admin_event(&tx, actor, subject_id, Some(&grant_id), "GRANT_ISSUED", now)?;
    tx.commit()?;
    Ok(grant_id)
}

/// Revoke an exact grant once; a second revocation is a denied operation.
/// The read-only resolver observes revocation on its next query.
pub(crate) fn revoke_grant(
    database_path: &Path,
    admin: &VerifiedGrantAdministrator,
    grant_id: &str,
) -> Result<(), PersistenceError> {
    let actor = administrator_id(admin)?;
    valid_uuid(grant_id)?;
    let mut connection = persistence::open_configured_connection(database_path)?;
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let now = persistence::now_unix_ms()?;
    let subject_id: Option<String> = tx
        .query_row(
            "SELECT subject_id FROM normal_data_permission_grants
             WHERE grant_id = ?1 AND revoked_at_ms IS NULL",
            [grant_id],
            |row| row.get(0),
        )
        .optional()?;
    let subject_id = subject_id.ok_or_else(denied)?;
    let updated = tx.execute(
        "UPDATE normal_data_permission_grants SET revoked_at_ms = ?1
         WHERE grant_id = ?2 AND revoked_at_ms IS NULL",
        params![now, grant_id],
    )?;
    if updated != 1 {
        return Err(denied());
    }
    append_admin_event(
        &tx,
        actor,
        &subject_id,
        Some(grant_id),
        "GRANT_REVOKED",
        now,
    )?;
    tx.commit()?;
    Ok(())
}

/// Permanently disable an enrolled subject, invalidating all its grants.
/// The old subject cannot be enabled or reenrolled in-place.
pub(crate) fn disable_subject(
    database_path: &Path,
    admin: &VerifiedGrantAdministrator,
    subject_id: &str,
) -> Result<(), PersistenceError> {
    let actor = administrator_id(admin)?;
    valid_uuid(subject_id)?;
    let mut connection = persistence::open_configured_connection(database_path)?;
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let now = persistence::now_unix_ms()?;
    let updated = tx.execute(
        "UPDATE normal_data_permission_subjects
         SET disabled_at_ms = ?1
         WHERE subject_id = ?2 AND disabled_at_ms IS NULL",
        params![now, subject_id],
    )?;
    if updated != 1 {
        return Err(denied());
    }
    append_admin_event(&tx, actor, subject_id, None, "SUBJECT_DISABLED", now)?;
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::normal_data_promotion_policy::{
        PromotionPermissionPolicy, SqlitePromotionPermissions,
    };
    use rusqlite::params;
    use std::fs;

    struct Fixture {
        folder: std::path::PathBuf,
        path: std::path::PathBuf,
        workspace: String,
        engagement: String,
        revision: String,
    }

    impl Fixture {
        fn new() -> Self {
            let folder =
                std::env::temp_dir().join(format!("pdox-permission-admin-{}", Uuid::new_v4()));
            fs::create_dir_all(&folder).expect("fixture folder");
            let path = folder.join("metadata.sqlite");
            persistence::initialize_database(&path).expect("migrate database");
            let workspace = Uuid::new_v4().to_string();
            let client = Uuid::new_v4().to_string();
            let service = Uuid::new_v4().to_string();
            let engagement = Uuid::new_v4().to_string();
            let workpaper = Uuid::new_v4().to_string();
            let revision = Uuid::new_v4().to_string();
            let conn = persistence::open_configured_connection(&path).expect("fixture connection");
            conn.execute(
                "INSERT INTO normal_data_workspaces
                 (normal_data_workspace_id, created_at_ms) VALUES (?1, 1)",
                [&workspace],
            )
            .expect("workspace");
            conn.execute(
                "INSERT INTO clients (client_id, name, created_at_ms)
                 VALUES (?1, 'Client', 1)",
                [&client],
            )
            .expect("client");
            conn.execute(
                "INSERT INTO service_types (service_type_id, name, normalized_name, created_at_ms)
                 VALUES (?1, 'Audit', ?2, 1)",
                params![&service, &service],
            )
            .expect("service");
            conn.execute(
                "INSERT INTO engagements (
                    engagement_id, client_id, service_type_id, name, status, created_at_ms
                 ) VALUES (?1, ?2, ?3, 'Engagement', 'ACTIVE', 1)",
                params![&engagement, &client, &service],
            )
            .expect("engagement");
            conn.execute(
                "INSERT INTO workpapers (
                    workpaper_id, engagement_id, reference, title,
                    workflow_state, created_at_ms
                 ) VALUES (?1, ?2, ?3, 'Workpaper', 'DRAFT', 1)",
                params![&workpaper, &engagement, &workpaper],
            )
            .expect("workpaper");
            conn.execute(
                "INSERT INTO workpaper_revisions (
                    workpaper_revision_id, workpaper_id, revision_number, created_at_ms
                 ) VALUES (?1, ?2, 1, 1)",
                params![&revision, &workpaper],
            )
            .expect("revision");
            Self {
                folder,
                path,
                workspace,
                engagement,
                revision,
            }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.folder);
        }
    }

    #[test]
    fn administrator_actions_are_atomic_audited_and_immediately_revocable() {
        let fixture = Fixture::new();
        let actor = VerifiedGrantAdministrator::fixture(&Uuid::new_v4().to_string());
        let subject = Uuid::new_v4().to_string();
        let provider = SqlitePromotionPermissions::new(&fixture.path);
        assert!(
            issue_grant(
                &fixture.path,
                &actor,
                &subject,
                PromotionPermission::ReadNormalDataWorkspace,
                &fixture.workspace,
                None
            )
            .is_err(),
            "unknown subject cannot receive a grant"
        );
        enroll_subject(&fixture.path, &actor, &subject).expect("trusted fixture enrollment");
        assert!(!provider.is_allowed(
            &subject,
            PromotionPermission::ReadNormalDataWorkspace,
            &fixture.workspace
        ));
        let read_id = issue_grant(
            &fixture.path,
            &actor,
            &subject,
            PromotionPermission::ReadNormalDataWorkspace,
            &fixture.workspace,
            None,
        )
        .expect("exact workspace grant");
        let engagement_id = issue_grant(
            &fixture.path,
            &actor,
            &subject,
            PromotionPermission::AttachEvidenceToEngagement,
            &fixture.engagement,
            None,
        )
        .expect("exact engagement grant");
        let revision_id = issue_grant(
            &fixture.path,
            &actor,
            &subject,
            PromotionPermission::ModifyWorkpaperRevision,
            &fixture.revision,
            None,
        )
        .expect("exact revision grant");
        assert!(provider.is_allowed(
            &subject,
            PromotionPermission::ReadNormalDataWorkspace,
            &fixture.workspace
        ));
        assert!(provider.is_allowed(
            &subject,
            PromotionPermission::AttachEvidenceToEngagement,
            &fixture.engagement
        ));
        assert!(provider.is_allowed(
            &subject,
            PromotionPermission::ModifyWorkpaperRevision,
            &fixture.revision
        ));
        assert!(!provider.is_allowed(
            &subject,
            PromotionPermission::ModifyWorkpaperRevision,
            &fixture.workspace
        ));
        assert!(
            issue_grant(
                &fixture.path,
                &actor,
                &subject,
                PromotionPermission::ReadNormalDataWorkspace,
                &Uuid::new_v4().to_string(),
                None
            )
            .is_err(),
            "nonexistent resource cannot be authorized"
        );
        assert!(
            issue_grant(
                &fixture.path,
                &actor,
                &subject,
                PromotionPermission::ReadNormalDataWorkspace,
                &fixture.workspace,
                Some(1)
            )
            .is_err(),
            "expired grant denied"
        );
        revoke_grant(&fixture.path, &actor, &revision_id).expect("revoke exact revision grant");
        assert!(!provider.is_allowed(
            &subject,
            PromotionPermission::ModifyWorkpaperRevision,
            &fixture.revision
        ));
        assert!(revoke_grant(&fixture.path, &actor, &revision_id).is_err());
        disable_subject(&fixture.path, &actor, &subject).expect("disable subject");
        assert!(!provider.is_allowed(
            &subject,
            PromotionPermission::ReadNormalDataWorkspace,
            &fixture.workspace
        ));
        assert!(!provider.is_allowed(
            &subject,
            PromotionPermission::AttachEvidenceToEngagement,
            &fixture.engagement
        ));
        assert!(disable_subject(&fixture.path, &actor, &subject).is_err());
        assert!(enroll_subject(&fixture.path, &actor, &subject).is_err());
        assert!(
            issue_grant(
                &fixture.path,
                &actor,
                &subject,
                PromotionPermission::ModifyWorkpaperRevision,
                &fixture.revision,
                None
            )
            .is_err(),
            "disabled subject cannot be reauthorized"
        );

        let conn = persistence::open_configured_connection(&fixture.path).expect("check audit");
        let events: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM normal_data_permission_admin_events
             WHERE subject_id = ?1",
                [&subject],
                |row| row.get(0),
            )
            .expect("count administrator events");
        assert_eq!(events, 6, "enroll + three issue + revoke + disable");
        let issued: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM normal_data_permission_admin_events
             WHERE grant_id IN (?1, ?2) AND event_type = 'GRANT_ISSUED'",
                params![&read_id, &engagement_id],
                |row| row.get(0),
            )
            .expect("issued grant receipts");
        assert_eq!(issued, 2);
        assert!(
            conn.execute(
                "DELETE FROM normal_data_permission_admin_events WHERE subject_id = ?1",
                [&subject]
            )
            .is_err(),
            "administrator receipts cannot be deleted"
        );
        assert!(
            conn.execute(
                "UPDATE normal_data_permission_admin_events SET event_type = 'SUBJECT_DISABLED'
             WHERE subject_id = ?1",
                [&subject]
            )
            .is_err(),
            "administrator receipts cannot be altered"
        );
        let links: i64 = conn
            .query_row("SELECT COUNT(*) FROM workpaper_evidence_links", [], |row| {
                row.get(0)
            })
            .expect("zero evidence links");
        assert_eq!(links, 0);
    }

    #[test]
    fn administrator_mutation_rolls_back_if_audit_append_fails() {
        let fixture = Fixture::new();
        let actor = VerifiedGrantAdministrator::fixture(&Uuid::new_v4().to_string());
        let subject = Uuid::new_v4().to_string();
        let conn = persistence::open_configured_connection(&fixture.path).expect("database");
        conn.execute_batch(
            "CREATE TRIGGER test_block_permission_admin_events
             BEFORE INSERT ON normal_data_permission_admin_events
             BEGIN SELECT RAISE(ABORT, 'audit store unavailable'); END;",
        )
        .expect("inject audit failure");
        assert!(enroll_subject(&fixture.path, &actor, &subject).is_err());
        let enrolled: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM normal_data_permission_subjects
             WHERE subject_id = ?1",
                [&subject],
                |row| row.get(0),
            )
            .expect("rollback count");
        assert_eq!(enrolled, 0, "enrollment and audit must commit atomically");
        conn.execute_batch("DROP TRIGGER test_block_permission_admin_events;")
            .expect("restore audit");
        enroll_subject(&fixture.path, &actor, &subject).expect("enroll");
        let grant_id = issue_grant(
            &fixture.path,
            &actor,
            &subject,
            PromotionPermission::ReadNormalDataWorkspace,
            &fixture.workspace,
            None,
        )
        .expect("issue grant");
        conn.execute_batch(
            "CREATE TRIGGER test_block_permission_admin_events
             BEFORE INSERT ON normal_data_permission_admin_events
             BEGIN SELECT RAISE(ABORT, 'audit store unavailable'); END;",
        )
        .expect("block audit again");
        assert!(revoke_grant(&fixture.path, &actor, &grant_id).is_err());
        let revoked: Option<i64> = conn
            .query_row(
                "SELECT revoked_at_ms FROM normal_data_permission_grants
             WHERE grant_id = ?1",
                [&grant_id],
                |row| row.get(0),
            )
            .expect("revoke rollback");
        assert_eq!(revoked, None, "revocation must roll back with its audit");
    }
}
