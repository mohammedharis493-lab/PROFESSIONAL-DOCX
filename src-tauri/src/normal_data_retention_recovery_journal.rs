//! Append-only local journal of private candidate recovery observations.
//! Neither an authorization decision nor controlled-evidence custody.
//! Scanning and SQLite commit are NOT one atomic filesystem snapshot.
use crate::{
    normal_data_retention_store::{
        self, RetentionCandidateInventoryEntry, RetentionCandidateStatus,
    },
    persistence::{self, PersistenceError},
};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fmt::Write, fs, path::Path};
use uuid::Uuid;

const MAX_ENTRIES: usize = 10_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RetentionRecoverySnapshot {
    pub recovery_scan_id: String,
    pub observed_at_ms: i64,
    /// None only for older v32 scans that could not record their root.
    pub canonical_root_sha256_hex: Option<String>,
    pub entries: Vec<RetentionCandidateInventoryEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RetentionRecoveryDrift {
    pub stage_id: String,
    pub recorded: Option<RetentionCandidateStatus>,
    pub observed_now: Option<RetentionCandidateStatus>,
}

fn denied(why: &str) -> PersistenceError {
    PersistenceError::Configuration(format!(
        "Normal Data candidate observation unavailable: {why}"
    ))
}

/// Bind snapshots to the canonical root path without persisting an absolute
/// path. This is a comparison scope, not a filesystem ownership attestation.
/// It cannot detect replacement of the contents at the SAME path by a hostile
/// local owner; formal controlled-evidence capture remains disabled.
fn canonical_root_digest(root: &Path) -> Result<String, PersistenceError> {
    let meta = fs::symlink_metadata(root)?;
    if !meta.is_dir() || meta.file_type().is_symlink() {
        return Err(denied("retention root must be a real directory"));
    }
    let canonical = fs::canonicalize(root)?;
    let mut hash = Sha256::new();
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        hash.update(b"pdox:retention-root:unix:v1\0");
        hash.update(canonical.as_os_str().as_bytes());
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        hash.update(b"pdox:retention-root:windows:v1\0");
        for unit in canonical.as_os_str().encode_wide() {
            hash.update(unit.to_le_bytes());
        }
    }
    let mut output = String::with_capacity(64);
    for byte in hash.finalize() {
        write!(&mut output, "{byte:02x}").expect("hex encoding is infallible");
    }
    Ok(output)
}

fn valid_scope_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
}

fn state_key(state: RetentionCandidateStatus) -> &'static str {
    match state {
        RetentionCandidateStatus::RecordedValid => "RECORDED_VALID",
        RetentionCandidateStatus::OrphanValid => "ORPHAN_VALID",
        RetentionCandidateStatus::Interrupted => "PARTIAL_INTERRUPTED",
        RetentionCandidateStatus::Corrupt => "CANDIDATE_CORRUPT",
        RetentionCandidateStatus::RecordedMissing => "RECORDED_MISSING",
    }
}

fn state_from_key(key: &str) -> Result<RetentionCandidateStatus, PersistenceError> {
    match key {
        "RECORDED_VALID" => Ok(RetentionCandidateStatus::RecordedValid),
        "ORPHAN_VALID" => Ok(RetentionCandidateStatus::OrphanValid),
        "PARTIAL_INTERRUPTED" => Ok(RetentionCandidateStatus::Interrupted),
        "CANDIDATE_CORRUPT" => Ok(RetentionCandidateStatus::Corrupt),
        "RECORDED_MISSING" => Ok(RetentionCandidateStatus::RecordedMissing),
        _ => Err(denied("unknown stored candidate state")),
    }
}

/// Observe the candidate filesystem and registration ledger, then append the
/// bounded identity/state-only snapshot as a single IMMEDIATE transaction.
/// Failed entries roll back their header and all previous entries.
/// No actor identity, raw paths, source bytes/hashes, evidence IDs or grants
/// stored; the canonical root *path digest* is scoped metadata only.
/// Validity is true only AT inspection; never use this journal to authorize.
pub(crate) fn record_retention_recovery_snapshot(
    database_path: &Path,
    retention_root: &Path,
) -> Result<RetentionRecoverySnapshot, PersistenceError> {
    let root_scope = canonical_root_digest(retention_root)?;
    let entries =
        normal_data_retention_store::scan_retention_candidates(database_path, retention_root)?;
    if canonical_root_digest(retention_root)? != root_scope {
        return Err(denied("retention root changed during observation"));
    }
    if entries.len() > MAX_ENTRIES {
        return Err(denied("too many observed candidates"));
    }
    let observed_at_ms = persistence::now_unix_ms()?;
    let recovery_scan_id = Uuid::new_v4().to_string();
    let mut connection = persistence::open_configured_connection(database_path)?;
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    tx.execute(
        "INSERT INTO normal_data_retention_recovery_scans
             (recovery_scan_id, observed_at_ms, entry_count)
         VALUES (?1, ?2, ?3)",
        params![recovery_scan_id, observed_at_ms, entries.len() as i64],
    )?;
    tx.execute(
        "INSERT INTO normal_data_retention_recovery_root_scopes
             (recovery_scan_id, canonical_root_sha256_hex) VALUES (?1, ?2)",
        params![recovery_scan_id, root_scope],
    )?;
    for entry in &entries {
        Uuid::parse_str(&entry.stage_id).map_err(|_| denied("invalid candidate stage ID"))?;
        tx.execute(
            "INSERT INTO normal_data_retention_recovery_entries
                (recovery_scan_id, stage_id, observed_state)
             VALUES (?1, ?2, ?3)",
            params![recovery_scan_id, entry.stage_id, state_key(entry.status)],
        )?;
    }
    let recorded_count: i64 = tx.query_row(
        "SELECT COUNT(*) FROM normal_data_retention_recovery_entries
         WHERE recovery_scan_id = ?1",
        [&recovery_scan_id],
        |row| row.get(0),
    )?;
    if recorded_count != entries.len() as i64 {
        return Err(denied("candidate snapshot insert incomplete"));
    }
    tx.commit()?;
    Ok(RetentionRecoverySnapshot {
        recovery_scan_id,
        observed_at_ms,
        canonical_root_sha256_hex: Some(root_scope),
        entries,
    })
}

/// Read historical identities and states. Never reclassify a historical
/// observation as authorization, a current inventory, or custody evidence.
pub(crate) fn read_retention_recovery_snapshot(
    database_path: &Path,
    scan_id: &str,
) -> Result<RetentionRecoverySnapshot, PersistenceError> {
    Uuid::parse_str(scan_id).map_err(|_| denied("invalid snapshot identifier"))?;
    let connection = persistence::open_configured_connection(database_path)?;
    let header: Option<(i64, i64, Option<String>)> = connection
        .query_row(
            "SELECT s.observed_at_ms, s.entry_count, roots.canonical_root_sha256_hex
             FROM normal_data_retention_recovery_scans s
             LEFT JOIN normal_data_retention_recovery_root_scopes roots
               ON roots.recovery_scan_id = s.recovery_scan_id
             WHERE s.recovery_scan_id = ?1",
            [scan_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    let (observed_at_ms, expected_count, root_scope) =
        header.ok_or_else(|| denied("snapshot not found"))?;
    if root_scope
        .as_deref()
        .is_some_and(|value| !valid_scope_hash(value))
    {
        return Err(denied("invalid saved root scope digest"));
    }
    if !(0..=MAX_ENTRIES as i64).contains(&expected_count) {
        return Err(denied("stored entry count exceeds limit"));
    }
    let mut statement = connection.prepare(
        "SELECT stage_id, observed_state
         FROM normal_data_retention_recovery_entries
         WHERE recovery_scan_id = ?1 ORDER BY stage_id LIMIT 10001",
    )?;
    let rows = statement.query_map([scan_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut entries = Vec::new();
    for row in rows {
        let (stage_id, state) = row?;
        Uuid::parse_str(&stage_id).map_err(|_| denied("invalid saved stage ID"))?;
        entries.push(RetentionCandidateInventoryEntry {
            stage_id,
            status: state_from_key(&state)?,
        });
        if entries.len() > MAX_ENTRIES {
            return Err(denied("stored candidate list exceeds limit"));
        }
    }
    if entries.len() as i64 != expected_count {
        return Err(denied("snapshot count does not match entries"));
    }
    Ok(RetentionRecoverySnapshot {
        recovery_scan_id: scan_id.to_owned(),
        observed_at_ms,
        canonical_root_sha256_hex: root_scope,
        entries,
    })
}

/// Observe current disk + v31 registry and compare to a historical snapshot.
/// Detect additions, removals and status transitions. Writes no records.
/// Reject a different canonical root or a historical v32 scan without
/// root scope. A path hash is NOT proof of directory ownership or integrity:
/// malicious substitution under the SAME path remains outside this model.
pub(crate) fn compare_retention_recovery_snapshot_to_live(
    database_path: &Path,
    retention_root: &Path,
    scan_id: &str,
) -> Result<Vec<RetentionRecoveryDrift>, PersistenceError> {
    let historical = read_retention_recovery_snapshot(database_path, scan_id)?;
    let root_scope = canonical_root_digest(retention_root)?;
    if historical.canonical_root_sha256_hex.as_deref() != Some(root_scope.as_str()) {
        return Err(denied(
            "snapshot does not bind this canonical retention root",
        ));
    }
    let current =
        normal_data_retention_store::scan_retention_candidates(database_path, retention_root)?;
    if canonical_root_digest(retention_root)? != root_scope {
        return Err(denied("retention root changed during live comparison"));
    }
    if current.len() > MAX_ENTRIES {
        return Err(denied("current candidate list exceeds limit"));
    }
    let mut prior = BTreeMap::new();
    for entry in historical.entries {
        if prior.insert(entry.stage_id, entry.status).is_some() {
            return Err(denied("ambiguous historical candidate"));
        }
    }
    let mut now = BTreeMap::new();
    for entry in current {
        if now.insert(entry.stage_id, entry.status).is_some() {
            return Err(denied("ambiguous current candidate"));
        }
    }
    let mut drift = Vec::new();
    for (stage_id, state) in prior {
        let new_state = now.remove(&stage_id);
        if new_state != Some(state) {
            drift.push(RetentionRecoveryDrift {
                stage_id,
                recorded: Some(state),
                observed_now: new_state,
            });
        }
    }
    for (stage_id, state) in now {
        drift.push(RetentionRecoveryDrift {
            stage_id,
            recorded: None,
            observed_now: Some(state),
        });
    }
    drift.sort_by(|a, b| a.stage_id.cmp(&b.stage_id));
    Ok(drift)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::PathBuf};

    struct Fixture {
        root: PathBuf,
        database: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let root =
                std::env::temp_dir().join(format!("pdox-retention-recovery-{}", Uuid::new_v4()));
            fs::create_dir_all(&root).expect("fixture root");
            let database = root.join("state.sqlite");
            persistence::initialize_database(&database).expect("v32 schema");
            fs::create_dir(root.join("private-candidates")).expect("private candidate root");
            Self { root, database }
        }

        fn candidates(&self) -> PathBuf {
            self.root.join("private-candidates")
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn canonical_root_scope_rejects_cross_root_and_unbound_legacy_comparisons() {
        let fixture = Fixture::new();
        let root = fixture.candidates();
        let another = fixture.root.join("another-private-root");
        fs::create_dir(&another).expect("other existing root");
        let recorded = record_retention_recovery_snapshot(&fixture.database, &root)
            .expect("record even an empty inventory");
        let fingerprint = recorded
            .canonical_root_sha256_hex
            .as_ref()
            .expect("v33 observation must record canonical root scope");
        assert!(valid_scope_hash(fingerprint));
        assert_eq!(recorded.entries.len(), 0);
        let other = record_retention_recovery_snapshot(&fixture.database, &another)
            .expect("separate root observation");
        assert_ne!(
            recorded.canonical_root_sha256_hex,
            other.canonical_root_sha256_hex
        );
        assert!(
            compare_retention_recovery_snapshot_to_live(
                &fixture.database,
                &another,
                &recorded.recovery_scan_id,
            )
            .is_err(),
            "different empty roots cannot appear to have no drift"
        );
        assert!(
            compare_retention_recovery_snapshot_to_live(
                &fixture.database,
                &root,
                &other.recovery_scan_id,
            )
            .is_err(),
            "root mismatch must fail in either direction"
        );
        assert!(compare_retention_recovery_snapshot_to_live(
            &fixture.database,
            &root,
            &recorded.recovery_scan_id,
        )
        .expect("same canonical root")
        .is_empty());
        assert_eq!(
            read_retention_recovery_snapshot(&fixture.database, &recorded.recovery_scan_id)
                .expect("immutable scope persisted"),
            recorded,
        );

        let conn =
            persistence::open_configured_connection(&fixture.database).expect("v33 connection");
        let legacy_scan_id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO normal_data_retention_recovery_scans
             (recovery_scan_id, observed_at_ms, entry_count) VALUES (?1, 1, 0)",
            [&legacy_scan_id],
        )
        .expect("a pre-v33 header has no root fingerprint");
        let legacy = read_retention_recovery_snapshot(&fixture.database, &legacy_scan_id)
            .expect("old unscoped header remains readable");
        assert_eq!(legacy.canonical_root_sha256_hex, None);
        assert!(
            compare_retention_recovery_snapshot_to_live(&fixture.database, &root, &legacy_scan_id,)
                .is_err(),
            "legacy unscoped observations cannot be cross-root drift inputs"
        );
        assert!(
            conn.execute(
                "UPDATE normal_data_retention_recovery_root_scopes
             SET canonical_root_sha256_hex = ?1
             WHERE recovery_scan_id = ?2",
                params![fingerprint, recorded.recovery_scan_id],
            )
            .is_err(),
            "root scope is immutable"
        );
        assert!(
            conn.execute(
                "DELETE FROM normal_data_retention_recovery_root_scopes
             WHERE recovery_scan_id = ?1",
                [&recorded.recovery_scan_id],
            )
            .is_err(),
            "saved root binding cannot be deleted"
        );
        let scope_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM normal_data_retention_recovery_root_scopes",
                [],
                |row| row.get(0),
            )
            .expect("scope count");
        assert_eq!(
            scope_count, 2,
            "neither comparisons nor legacy read append scope"
        );
        for table in [
            "controlled_evidence_versions",
            "workpaper_evidence_links",
            "normal_data_permission_grants",
        ] {
            let count: i64 = conn
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .expect("formal side effects");
            assert_eq!(count, 0, "{table} cannot be created by a scope observation");
        }
    }

    #[test]
    fn snapshot_append_only_and_drift_reports_added_removed_changed_candidates() {
        let fixture = Fixture::new();
        let root = fixture.candidates();
        let partial = Uuid::new_v4().to_string();
        let corrupt = Uuid::new_v4().to_string();
        let added = Uuid::new_v4().to_string();
        fs::create_dir(root.join(format!("{partial}.partial"))).expect("partial");
        fs::create_dir(root.join(format!("{corrupt}.candidate"))).expect("corrupt candidate");
        let first = record_retention_recovery_snapshot(&fixture.database, &root)
            .expect("first journal observation");
        assert_eq!(first.entries.len(), 2);
        assert!(first.entries.contains(&RetentionCandidateInventoryEntry {
            stage_id: partial.clone(),
            status: RetentionCandidateStatus::Interrupted,
        }));
        assert!(first.entries.contains(&RetentionCandidateInventoryEntry {
            stage_id: corrupt.clone(),
            status: RetentionCandidateStatus::Corrupt,
        }));
        assert_eq!(
            read_retention_recovery_snapshot(&fixture.database, &first.recovery_scan_id)
                .expect("saved snapshot"),
            first,
        );
        assert!(compare_retention_recovery_snapshot_to_live(
            &fixture.database,
            &root,
            &first.recovery_scan_id
        )
        .expect("no drift")
        .is_empty());
        fs::remove_dir(root.join(format!("{partial}.partial"))).expect("partial removed");
        fs::remove_dir(root.join(format!("{corrupt}.candidate"))).expect("corrupt removed");
        fs::create_dir(root.join(format!("{corrupt}.partial"))).expect("new interrupted state");
        fs::create_dir(root.join(format!("{added}.candidate"))).expect("new corrupt candidate");
        let drift = compare_retention_recovery_snapshot_to_live(
            &fixture.database,
            &root,
            &first.recovery_scan_id,
        )
        .expect("compare current inventory");
        assert_eq!(drift.len(), 3);
        assert!(drift.contains(&RetentionRecoveryDrift {
            stage_id: partial,
            recorded: Some(RetentionCandidateStatus::Interrupted),
            observed_now: None,
        }));
        assert!(drift.contains(&RetentionRecoveryDrift {
            stage_id: corrupt,
            recorded: Some(RetentionCandidateStatus::Corrupt),
            observed_now: Some(RetentionCandidateStatus::Interrupted),
        }));
        assert!(drift.contains(&RetentionRecoveryDrift {
            stage_id: added,
            recorded: None,
            observed_now: Some(RetentionCandidateStatus::Corrupt),
        }));
        let second = record_retention_recovery_snapshot(&fixture.database, &root)
            .expect("second history entry");
        assert_ne!(first.recovery_scan_id, second.recovery_scan_id);
        assert_eq!(
            read_retention_recovery_snapshot(&fixture.database, &first.recovery_scan_id)
                .expect("previous snapshot unchanged"),
            first,
        );
        let connection =
            persistence::open_configured_connection(&fixture.database).expect("connection");
        assert!(
            connection
                .execute(
                    "UPDATE normal_data_retention_recovery_entries
             SET observed_state = 'RECORDED_VALID'",
                    [],
                )
                .is_err(),
            "previous observations cannot change"
        );
        assert!(
            connection
                .execute("DELETE FROM normal_data_retention_recovery_scans", [],)
                .is_err(),
            "historical scan cannot be deleted"
        );
        for table in [
            "normal_data_retention_candidates",
            "controlled_evidence_versions",
            "workpaper_evidence_links",
            "normal_data_permission_grants",
        ] {
            let count: i64 = connection
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .expect("read no-capture count");
            assert_eq!(count, 0, "{table} is not mutated by observation");
        }
        assert!(read_retention_recovery_snapshot(&fixture.database, "invalid").is_err());
    }

    #[test]
    fn failure_during_entry_insert_rolls_back_complete_snapshot() {
        let fixture = Fixture::new();
        let root = fixture.candidates();
        for _ in 0..2 {
            let id = Uuid::new_v4().to_string();
            fs::create_dir(root.join(format!("{id}.partial"))).expect("interrupted candidate");
        }
        let connection =
            persistence::open_configured_connection(&fixture.database).expect("connection");
        connection
            .execute_batch(
                "CREATE TRIGGER test_retention_recovery_insert_failure
             BEFORE INSERT ON normal_data_retention_recovery_entries
             BEGIN SELECT RAISE(ABORT, 'injected recovery journal failure'); END;",
            )
            .expect("fault injection");
        assert!(record_retention_recovery_snapshot(&fixture.database, &root).is_err());
        for table in [
            "normal_data_retention_recovery_scans",
            "normal_data_retention_recovery_entries",
            "normal_data_retention_recovery_root_scopes",
        ] {
            let count: i64 = connection
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .expect("rollback count");
            assert_eq!(count, 0, "{table} rolled back");
        }
        connection
            .execute_batch("DROP TRIGGER test_retention_recovery_insert_failure")
            .expect("remove fault");
        let retry = record_retention_recovery_snapshot(&fixture.database, &root)
            .expect("retry after rollback");
        assert_eq!(retry.entries.len(), 2);
    }

    #[test]
    fn ambiguous_partial_and_candidate_stage_identity_fails_closed() {
        let fixture = Fixture::new();
        let root = fixture.candidates();
        let same = Uuid::new_v4().to_string();
        fs::create_dir(root.join(format!("{same}.partial"))).expect("partial");
        let first = record_retention_recovery_snapshot(&fixture.database, &root)
            .expect("before ambiguous package");
        fs::create_dir(root.join(format!("{same}.candidate"))).expect("conflicting candidate");
        assert!(record_retention_recovery_snapshot(&fixture.database, &root).is_err());
        assert!(compare_retention_recovery_snapshot_to_live(
            &fixture.database,
            &root,
            &first.recovery_scan_id
        )
        .is_err());
        assert_eq!(
            read_retention_recovery_snapshot(&fixture.database, &first.recovery_scan_id)
                .expect("prior observation unchanged"),
            first,
        );
    }
}
