//! Native-only, append-only SQLite history of private staging *observations*.
//! Not an evidence-capture log, authorization grant, trust attestation or
//! promotion command. A local database owner can tamper with these records.
//!
//! The scan reads the filesystem BEFORE the SQLite write transaction. The
//! resulting statuses are historical observations only: they can already be
//! stale by commit time and can NEVER authorize a later evidence linkage.
use crate::{
    normal_data_staging::{self, StageInventoryEntry, StageStatus},
    persistence::{self, PersistenceError},
};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use std::path::Path;
use uuid::Uuid;

const MAX_RECOVERY_ENTRIES: usize = 10_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecoverySnapshot {
    pub recovery_scan_id: String,
    pub observed_at_ms: i64,
    pub entries: Vec<StageInventoryEntry>,
}

fn denied(message: &str) -> PersistenceError {
    PersistenceError::Configuration(format!(
        "Normal Data recovery observation unavailable: {message}"
    ))
}

fn state_key(state: StageStatus) -> &'static str {
    match state {
        StageStatus::ReadyVerified => "READY_LOCAL_HASH_VALID",
        StageStatus::Corrupt => "READY_CORRUPT",
        StageStatus::Interrupted => "PARTIAL_INTERRUPTED",
    }
}

fn state_from_key(value: &str) -> Result<StageStatus, PersistenceError> {
    match value {
        "READY_LOCAL_HASH_VALID" => Ok(StageStatus::ReadyVerified),
        "READY_CORRUPT" => Ok(StageStatus::Corrupt),
        "PARTIAL_INTERRUPTED" => Ok(StageStatus::Interrupted),
        _ => Err(denied("unknown stored recovery state")),
    }
}

/// Scan a native-selected existing private root, then commit one complete
/// snapshot header and all stage observations under a single IMMEDIATE
/// transaction. A failure rolls back both tables. No raw paths, source bytes,
/// content, hashes or principals enter this journal.
///
/// READY_LOCAL_HASH_VALID only says the package matched its OWN manifest
/// during this scan. It does NOT mean it matches trusted database provenance,
/// that the bytes remain unchanged, or that any retention/capture occurred.
pub(crate) fn record_recovery_snapshot(
    database_path: &Path,
    staging_root: &Path,
) -> Result<RecoverySnapshot, PersistenceError> {
    let entries = normal_data_staging::scan_stages(staging_root)?;
    if entries.len() > MAX_RECOVERY_ENTRIES {
        return Err(denied("too many observed packages"));
    }
    let observed_at_ms = persistence::now_unix_ms()?;
    let recovery_scan_id = Uuid::new_v4().to_string();
    let mut connection = persistence::open_configured_connection(database_path)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    transaction.execute(
        "INSERT INTO normal_data_stage_recovery_scans
             (recovery_scan_id, observed_at_ms, entry_count)
         VALUES (?1, ?2, ?3)",
        params![recovery_scan_id, observed_at_ms, entries.len() as i64],
    )?;
    for entry in &entries {
        Uuid::parse_str(&entry.stage_id).map_err(|_| denied("stage ID is not a UUID"))?;
        transaction.execute(
            "INSERT INTO normal_data_stage_recovery_entries
                 (recovery_scan_id, stage_id, observed_state)
             VALUES (?1, ?2, ?3)",
            params![recovery_scan_id, entry.stage_id, state_key(entry.status)],
        )?;
    }
    transaction.commit()?;
    Ok(RecoverySnapshot {
        recovery_scan_id,
        observed_at_ms,
        entries,
    })
}

/// Historical inventory only. Never treats previous observations as current
/// package presence, authorization, immutable evidence or a validation cache.
pub(crate) fn read_recovery_snapshot(
    database_path: &Path,
    scan_id: &str,
) -> Result<RecoverySnapshot, PersistenceError> {
    Uuid::parse_str(scan_id).map_err(|_| denied("scan ID is not a UUID"))?;
    let connection = persistence::open_configured_connection(database_path)?;
    let header: Option<(i64, i64)> = connection
        .query_row(
            "SELECT observed_at_ms, entry_count
             FROM normal_data_stage_recovery_scans
             WHERE recovery_scan_id = ?1",
            [scan_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let (observed_at_ms, expected_count) = header.ok_or_else(|| denied("scan does not exist"))?;
    if !(0..=MAX_RECOVERY_ENTRIES as i64).contains(&expected_count) {
        return Err(denied("recorded entry count out of bounds"));
    }
    let mut statement = connection.prepare(
        "SELECT stage_id, observed_state
         FROM normal_data_stage_recovery_entries
         WHERE recovery_scan_id = ?1
         ORDER BY stage_id LIMIT 10001",
    )?;
    let iter = statement.query_map([scan_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut entries = Vec::new();
    for record in iter {
        let (stage_id, state) = record?;
        Uuid::parse_str(&stage_id).map_err(|_| denied("stored stage ID invalid"))?;
        entries.push(StageInventoryEntry {
            stage_id,
            status: state_from_key(&state)?,
        });
        if entries.len() > MAX_RECOVERY_ENTRIES {
            return Err(denied("too many stored entries"));
        }
    }
    if entries.len() as i64 != expected_count {
        return Err(denied("stored recovery inventory is incomplete"));
    }
    Ok(RecoverySnapshot {
        recovery_scan_id: scan_id.to_owned(),
        observed_at_ms,
        entries,
    })
}


#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecoveryDrift {
    pub stage_id: String,
    pub recorded: Option<StageStatus>,
    pub observed_now: Option<StageStatus>,
}

/// Re-scan a native-chosen private root and compare it with an immutable
/// historical journal snapshot. Absence, additions and state transitions are
/// reported rather than silently assuming that an old valid stage is still
/// valid. This check intentionally writes NOTHING and is not an authorization,
/// retained-evidence, or crash-atomic filesystem/database snapshot.
pub(crate) fn compare_recovery_snapshot_to_live(
    database_path: &Path,
    staging_root: &Path,
    scan_id: &str,
) -> Result<Vec<RecoveryDrift>, PersistenceError> {
    use std::collections::BTreeMap;

    let historical = read_recovery_snapshot(database_path, scan_id)?;
    let live = normal_data_staging::scan_stages(staging_root)?;
    if live.len() > MAX_RECOVERY_ENTRIES {
        return Err(denied("too many current packages"));
    }
    let mut old = BTreeMap::new();
    for entry in historical.entries {
        if old.insert(entry.stage_id, entry.status).is_some() {
            return Err(denied("ambiguous historical package identity"));
        }
    }
    let mut current = BTreeMap::new();
    for entry in live {
        if current.insert(entry.stage_id, entry.status).is_some() {
            return Err(denied("ambiguous current package identity"));
        }
    }
    let mut result = Vec::new();
    for (stage_id, old_state) in &old {
        let new_state = current.remove(stage_id);
        if new_state != Some(*old_state) {
            result.push(RecoveryDrift {
                stage_id: stage_id.clone(),
                recorded: Some(*old_state),
                observed_now: new_state,
            });
        }
    }
    for (stage_id, new_state) in current {
        result.push(RecoveryDrift {
            stage_id,
            recorded: None,
            observed_now: Some(new_state),
        });
    }
    result.sort_by(|a, b| a.stage_id.cmp(&b.stage_id));
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::PathBuf};

    struct Fixture {
        root: PathBuf,
        db: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("pdox-stage-recovery-{}", Uuid::new_v4()));
            fs::create_dir_all(&root).expect("new fixture directory");
            let db = root.join("state.sqlite");
            persistence::initialize_database(&db).expect("schema v30");
            let stage = root.join("private-stage");
            fs::create_dir(&stage).expect("private staging root");
            Self { root, db }
        }
        fn stage_root(&self) -> PathBuf {
            self.root.join("private-stage")
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn recovery_snapshots_are_append_only_and_reflect_new_integrity_observations() {
        let fixture = Fixture::new();
        let root = fixture.stage_root();
        let interrupted_id = Uuid::new_v4().to_string();
        let corrupt_id = Uuid::new_v4().to_string();
        fs::create_dir(root.join(format!("{interrupted_id}.partial")))
            .expect("interrupted capture");
        fs::create_dir(root.join(format!("{corrupt_id}.ready"))).expect("malformed ready package");
        let first = record_recovery_snapshot(&fixture.db, &root).expect("initial recovery scan");
        assert_eq!(first.entries.len(), 2);
        assert!(first.entries.contains(&StageInventoryEntry {
            stage_id: interrupted_id.clone(),
            status: StageStatus::Interrupted,
        }));
        assert!(first.entries.contains(&StageInventoryEntry {
            stage_id: corrupt_id.clone(),
            status: StageStatus::Corrupt,
        }));
        assert_eq!(
            read_recovery_snapshot(&fixture.db, &first.recovery_scan_id)
                .expect("reopen committed first snapshot"),
            first,
        );
        assert!(read_recovery_snapshot(&fixture.db, "bad-scan-id").is_err());
        fs::remove_dir(root.join(format!("{interrupted_id}.partial")))
            .expect("simulate cleanup of interrupted stage");
        let second = record_recovery_snapshot(&fixture.db, &root).expect("new recovery scan");
        assert_eq!(second.entries.len(), 1);
        assert_ne!(first.recovery_scan_id, second.recovery_scan_id);
        assert_eq!(
            read_recovery_snapshot(&fixture.db, &first.recovery_scan_id)
                .expect("old observation still present"),
            first,
        );
        let connection = persistence::open_configured_connection(&fixture.db).expect("database");
        let counts: (i64, i64) = (
            connection
                .query_row(
                    "SELECT COUNT(*) FROM normal_data_stage_recovery_scans",
                    [],
                    |row| row.get(0),
                )
                .expect("scans"),
            connection
                .query_row(
                    "SELECT COUNT(*) FROM normal_data_stage_recovery_entries",
                    [],
                    |row| row.get(0),
                )
                .expect("entries"),
        );
        assert_eq!(counts, (2, 3));
        assert!(connection.execute(
            "UPDATE normal_data_stage_recovery_entries SET observed_state = 'READY_LOCAL_HASH_VALID'",
            [],
        ).is_err(), "old observations cannot be rewritten");
        assert!(
            connection
                .execute("DELETE FROM normal_data_stage_recovery_scans", [],)
                .is_err(),
            "old scans cannot be deleted"
        );
        for table in ["workpaper_evidence_links", "controlled_evidence_versions"] {
            let count: i64 = connection
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .expect("still no formal capture or linkage");
            assert_eq!(count, 0);
        }
    }

    #[test]
    fn failed_entry_insert_rolls_back_header_and_all_prior_entries() {
        let fixture = Fixture::new();
        let root = fixture.stage_root();
        for _ in 0..2 {
            let stage = Uuid::new_v4().to_string();
            fs::create_dir(root.join(format!("{stage}.partial"))).expect("recovery candidate");
        }
        let connection = persistence::open_configured_connection(&fixture.db).expect("database");
        connection
            .execute_batch(
                "CREATE TRIGGER test_recovery_observation_failure
             BEFORE INSERT ON normal_data_stage_recovery_entries
             BEGIN SELECT RAISE(ABORT, 'simulated disk inventory journal fault'); END;",
            )
            .expect("fault injection");
        assert!(record_recovery_snapshot(&fixture.db, &root).is_err());
        let scans: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM normal_data_stage_recovery_scans",
                [],
                |row| row.get(0),
            )
            .expect("no snapshot header committed");
        let entries: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM normal_data_stage_recovery_entries",
                [],
                |row| row.get(0),
            )
            .expect("no observation row committed");
        assert_eq!((scans, entries), (0, 0));
        connection
            .execute_batch("DROP TRIGGER test_recovery_observation_failure")
            .expect("repair fault");
        assert_eq!(
            record_recovery_snapshot(&fixture.db, &root)
                .expect("record after rollback")
                .entries
                .len(),
            2,
        );
    }

    #[test]
    fn drift_detects_missing_added_and_changed_packages_without_mutating_history() {
        let fixture = Fixture::new();
        let root = fixture.stage_root();
        let missing = Uuid::new_v4().to_string();
        let unchanged = Uuid::new_v4().to_string();
        let changed = Uuid::new_v4().to_string();
        let added = Uuid::new_v4().to_string();
        for id in [&missing, &unchanged, &changed] {
            fs::create_dir(root.join(format!("{id}.partial"))).expect("partial stage");
        }
        let snapshot = record_recovery_snapshot(&fixture.db, &root).expect("historical snapshot");
        assert!(compare_recovery_snapshot_to_live(&fixture.db, &root, &snapshot.recovery_scan_id)
            .expect("no drift").is_empty());
        fs::remove_dir(root.join(format!("{missing}.partial"))).expect("remove partial");
        fs::remove_dir(root.join(format!("{changed}.partial"))).expect("remove old state");
        fs::create_dir(root.join(format!("{changed}.ready"))).expect("replace with corrupt ready");
        fs::create_dir(root.join(format!("{added}.partial"))).expect("new partial");
        let drift = compare_recovery_snapshot_to_live(&fixture.db, &root, &snapshot.recovery_scan_id)
            .expect("fresh local inspection");
        assert_eq!(drift.len(), 3);
        assert!(drift.contains(&RecoveryDrift {
            stage_id: missing,
            recorded: Some(StageStatus::Interrupted),
            observed_now: None,
        }));
        assert!(drift.contains(&RecoveryDrift {
            stage_id: changed,
            recorded: Some(StageStatus::Interrupted),
            observed_now: Some(StageStatus::Corrupt),
        }));
        assert!(drift.contains(&RecoveryDrift {
            stage_id: added,
            recorded: None,
            observed_now: Some(StageStatus::Interrupted),
        }));
        assert_eq!(read_recovery_snapshot(&fixture.db, &snapshot.recovery_scan_id)
            .expect("historical scan is untouched"), snapshot);
        let connection = persistence::open_configured_connection(&fixture.db).expect("database");
        let scans: i64 = connection.query_row(
            "SELECT COUNT(*) FROM normal_data_stage_recovery_scans", [], |row| row.get(0)
        ).expect("read count");
        assert_eq!(scans, 1, "differential inspection never appends a scan");
        for table in ["controlled_evidence_versions", "workpaper_evidence_links"] {
            let count: i64 = connection.query_row(
                &format!("SELECT COUNT(*) FROM {table}"), [], |row| row.get(0)
            ).expect("no formal evidence");
            assert_eq!(count, 0);
        }
        assert!(compare_recovery_snapshot_to_live(&fixture.db, &root, "not-a-uuid").is_err());
    }

    #[test]
    fn drift_fails_closed_on_ambiguous_partial_and_ready_identity() {
        let fixture = Fixture::new();
        let root = fixture.stage_root();
        let same = Uuid::new_v4().to_string();
        fs::create_dir(root.join(format!("{same}.partial"))).expect("partial");
        let snapshot = record_recovery_snapshot(&fixture.db, &root).expect("initial");
        fs::create_dir(root.join(format!("{same}.ready"))).expect("conflicting ready");
        assert!(compare_recovery_snapshot_to_live(
            &fixture.db, &root, &snapshot.recovery_scan_id
        ).is_err(), "duplicate stage identifiers must not be resolved arbitrarily");
        assert!(record_recovery_snapshot(&fixture.db, &root).is_err());
        assert_eq!(read_recovery_snapshot(&fixture.db, &snapshot.recovery_scan_id)
            .expect("initial is immutable"), snapshot);
    }

}
