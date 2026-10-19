//! Nonshipping, test-only native transaction-denial reference.
//! This is NOT a Tauri handler, production issuer, grant database, nor a
//! test of the currently registered legacy command dispatch. In particular,
//! the 34 direct-Tauri scenarios in the deferred matrix remain NOT RUN.
//! This prototype checks that a denial is evaluated while SQLite holds the
//! writer lock and stops execution BEFORE any proposed state mutation.
//! Only DenyAllLegacyWriter is provided: no fixture or production allow policy.

use rusqlite::{Connection, Transaction, TransactionBehavior};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LegacyWriteIntent {
    WorkpaperCreate,
    Revision,
    EvidenceLink,
    Signoff,
    WorkflowTransition,
    ReviewNoteCreate,
    ReviewNoteRespond,
    ReviewNoteClear,
    ReviewNoteReopen,
}

const ALL_LEGACY_WRITES: [LegacyWriteIntent; 9] = [
    LegacyWriteIntent::WorkpaperCreate,
    LegacyWriteIntent::Revision,
    LegacyWriteIntent::EvidenceLink,
    LegacyWriteIntent::Signoff,
    LegacyWriteIntent::WorkflowTransition,
    LegacyWriteIntent::ReviewNoteCreate,
    LegacyWriteIntent::ReviewNoteRespond,
    LegacyWriteIntent::ReviewNoteClear,
    LegacyWriteIntent::ReviewNoteReopen,
];

trait LegacyNativeWriteAuthority {
    fn check_under_immediate_lock(
        &self,
        transaction: &Transaction<'_>,
        target_workpaper: &str,
        intent: LegacyWriteIntent,
    ) -> Result<(), &'static str>;
}

/// There is no real issuer, authenticated session or permission store.
/// Caller-provided names/roles are intentionally NOT part of this contract.
struct DenyAllLegacyWriter;

impl LegacyNativeWriteAuthority for DenyAllLegacyWriter {
    fn check_under_immediate_lock(
        &self,
        _transaction: &Transaction<'_>,
        _target_workpaper: &str,
        _intent: LegacyWriteIntent,
    ) -> Result<(), &'static str> {
        Err("NATIVE_LEGACY_WRITE_AUTHORITY_NOT_IMPLEMENTED")
    }
}

/// This explicitly synthetic probe demonstrates check-before-write ordering;
/// it does NOT call, wrap or replace any registered Tauri command. It may
/// insert only a marked synthetic audit event into an isolated test database
/// if a hypothetical authority returned Ok, which DenyAllLegacyWriter cannot.
fn test_only_under_lock_write_probe(
    database_path: &Path,
    workpaper_id: &str,
    intent: LegacyWriteIntent,
    caller_claimed_actor: Option<&str>,
    authority: &dyn LegacyNativeWriteAuthority,
) -> Result<(), String> {
    let _untrusted_actor = caller_claimed_actor;
    let mut db = Connection::open(database_path).map_err(|e| e.to_string())?;
    db.busy_timeout(std::time::Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    db.pragma_update(None, "foreign_keys", "ON")
        .map_err(|e| e.to_string())?;
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    // Fail closed without disclosing whether a target workpaper exists.
    authority
        .check_under_immediate_lock(&tx, workpaper_id, intent)
        .map_err(str::to_string)?;

    // UNREACHABLE under DenyAll; tests ensure refusal is atomic and leaves
    // revisions, sign-offs, review notes, events and audits untouched.
    tx.execute(
        "INSERT INTO audit_events
         (audit_event_id, event_type, entity_type, entity_id,
          occurred_at_ms, details_json)
         VALUES (?1, 'TEST_ONLY_DENIAL_PROBE', 'TEST_ONLY',
                 ?2, 1, '{}')",
        rusqlite::params![uuid::Uuid::new_v4().to_string(), workpaper_id],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::initialize_database;
    use uuid::Uuid;

    #[derive(Debug, PartialEq, Eq)]
    struct FixtureWriteSurface {
        workpapers: i64,
        revisions: i64,
        evidence_links: i64,
        signoffs: i64,
        supersessions: i64,
        workflow_events: i64,
        review_notes: i64,
        review_events: i64,
        audit_events: i64,
    }

    struct Fixture {
        root: std::path::PathBuf,
        path: std::path::PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "professional-docx-denied-legacy-command-{}",
                Uuid::new_v4()
            ));
            let path = root.join("metadata.sqlite");
            initialize_database(&path).expect("apply actual native migrations");
            let db = Connection::open(&path).expect("open disposable fixture");
            db.execute_batch(
                "PRAGMA foreign_keys = ON;
                 INSERT INTO clients (client_id, name, created_at_ms)
                   VALUES ('client-denial', 'Fixture', 1);
                 INSERT INTO service_types
                   (service_type_id, name, normalized_name, created_at_ms)
                   VALUES ('service-denial', 'Fixture', 'fixture', 1);
                 INSERT INTO engagements
                   (engagement_id, client_id, service_type_id, name, status, created_at_ms)
                   VALUES ('engagement-denial', 'client-denial', 'service-denial',
                           'Synthetic', 'ACTIVE', 1);
                 INSERT INTO workpapers
                   (workpaper_id, engagement_id, reference, title, workflow_state, created_at_ms)
                   VALUES ('workpaper-denial', 'engagement-denial', 'D-01',
                           'Synthetic denial scope', 'IN_PROGRESS', 1);",
            )
            .expect("seed isolated source workpaper");
            Self { root, path }
        }

        fn entire_write_surface(&self) -> FixtureWriteSurface {
            let db = Connection::open(&self.path).expect("read fixture");
            db.query_row(
                "SELECT
                   (SELECT COUNT(*) FROM workpapers),
                   (SELECT COUNT(*) FROM workpaper_revisions),
                   (SELECT COUNT(*) FROM workpaper_evidence_links),
                   (SELECT COUNT(*) FROM workpaper_signoffs),
                   (SELECT COUNT(*) FROM workpaper_signoff_supersessions),
                   (SELECT COUNT(*) FROM workpaper_workflow_events),
                   (SELECT COUNT(*) FROM review_notes),
                   (SELECT COUNT(*) FROM review_note_events),
                   (SELECT COUNT(*) FROM audit_events)",
                [],
                |row| {
                    Ok(FixtureWriteSurface {
                        workpapers: row.get(0)?,
                        revisions: row.get(1)?,
                        evidence_links: row.get(2)?,
                        signoffs: row.get(3)?,
                        supersessions: row.get(4)?,
                        workflow_events: row.get(5)?,
                        review_notes: row.get(6)?,
                        review_events: row.get(7)?,
                        audit_events: row.get(8)?,
                    })
                },
            )
            .expect("count all synthetic write-surface records")
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn denied_under_writer_lock_for_each_of_nine_legacy_intents() {
        let fixture = Fixture::new();
        let before = fixture.entire_write_surface();
        for intent in ALL_LEGACY_WRITES {
            let result = test_only_under_lock_write_probe(
                &fixture.path,
                "workpaper-denial",
                intent,
                Some("spoofed-admin-and-reviewer"),
                &DenyAllLegacyWriter,
            );
            assert_eq!(
                result,
                Err("NATIVE_LEGACY_WRITE_AUTHORITY_NOT_IMPLEMENTED".to_string()),
                "{intent:?}"
            );
            assert_eq!(fixture.entire_write_surface(), before, "{intent:?}");
        }
    }

    #[test]
    fn denial_does_not_disclose_foreign_or_missing_workpaper_existence() {
        let fixture = Fixture::new();
        let before = fixture.entire_write_surface();
        for workpaper in ["workpaper-denial", "foreign-workpaper", "", "malformed"] {
            for intent in ALL_LEGACY_WRITES {
                let result = test_only_under_lock_write_probe(
                    &fixture.path,
                    workpaper,
                    intent,
                    Some("claimed-approved-professional"),
                    &DenyAllLegacyWriter,
                );
                assert_eq!(
                    result,
                    Err("NATIVE_LEGACY_WRITE_AUTHORITY_NOT_IMPLEMENTED".to_string())
                );
                assert_eq!(fixture.entire_write_surface(), before);
            }
        }
    }

    #[test]
    fn no_caller_attribution_can_turn_the_native_denial_into_allow() {
        let fixture = Fixture::new();
        let before = fixture.entire_write_surface();
        for actor in [None, Some(""), Some("admin"), Some("FINAL_APPROVAL")] {
            for intent in ALL_LEGACY_WRITES {
                let denied = test_only_under_lock_write_probe(
                    &fixture.path,
                    "workpaper-denial",
                    intent,
                    actor,
                    &DenyAllLegacyWriter,
                );
                assert_eq!(
                    denied,
                    Err("NATIVE_LEGACY_WRITE_AUTHORITY_NOT_IMPLEMENTED".to_string())
                );
            }
        }
        assert_eq!(fixture.entire_write_surface(), before);
    }
}
