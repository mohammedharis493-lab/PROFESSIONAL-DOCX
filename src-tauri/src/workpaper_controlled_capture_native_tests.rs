//! Nonshipping characterization of the persistence completion boundary.
//! Synthetic CAPTURING jobs are directly staged in actual migrated SQLite.
//! The production evidence::capture_controlled_evidence file-copy verifier is
//! NOT bypassed via a registered Tauri IPC by any case below. No real bytes,
//! authenticated issuer, retention approval, or client-scope grant is created.

use crate::persistence::{self, initialize_database};
use rusqlite::{params, Connection};
use std::path::PathBuf;
use uuid::Uuid;

struct CaptureFixture {
    root: PathBuf,
    path: PathBuf,
    document_id: String,
    file_id: String,
    job_id: String,
}

impl CaptureFixture {
    fn new() -> Self {
        let root =
            std::env::temp_dir().join(format!("professional-docx-capture-{}", Uuid::new_v4()));
        let path = root.join("metadata.sqlite");
        initialize_database(&path).expect("apply actual production migrations");
        let fixture = Self {
            root,
            path,
            document_id: Uuid::new_v4().to_string(),
            file_id: Uuid::new_v4().to_string(),
            job_id: Uuid::new_v4().to_string(),
        };
        let db = Connection::open(&fixture.path).expect("open synthetic test database");
        let root_id = Uuid::new_v4().to_string();
        db.execute(
            "INSERT INTO storage_roots (
                storage_root_id, kind, native_locator, native_locator_encoding,
                display_locator, canonical_native_locator, canonical_display_locator,
                availability_state, approved_at_ms, approved_by, created_at_ms, updated_at_ms
             ) VALUES (?1, 'LOCAL', ?2, 'TEST', '/capture-fixture', NULL,
                       NULL, 'AVAILABLE', 1, NULL, 1, 1)",
            params![root_id, vec![0_u8]],
        )
        .expect("seed synthetic root");
        db.execute(
            "INSERT INTO documents (
                document_id, storage_state, display_name, created_at_ms,
                created_by, archived_at_ms
             ) VALUES (?1, 'LINKED', 'Synthetic source.txt', 1, NULL, NULL)",
            [&fixture.document_id],
        )
        .expect("seed synthetic document");
        db.execute(
            "INSERT INTO file_instances (
                file_instance_id, document_id, storage_root_id, relative_path_native,
                path_native_encoding, relative_path_display, relative_path_search,
                filesystem_identity, volume_identity, creation_time_ms, last_write_time_ms,
                size_bytes, file_attributes, reparse_tag, first_seen_at_ms, last_seen_at_ms,
                first_seen_generation_id, last_seen_generation_id, availability_state
             ) VALUES (
                ?1, ?2, ?3, ?4, 'TEST', 'Synthetic source.txt', 'synthetic source txt',
                NULL, NULL, NULL, NULL, 5, NULL, NULL, 1, 1, NULL, NULL, 'AVAILABLE'
             )",
            params![
                &fixture.file_id,
                &fixture.document_id,
                &root_id,
                b"Synthetic source.txt".to_vec()
            ],
        )
        .expect("seed synthetic source file metadata");
        drop(db);
        fixture.seed_job(&fixture.job_id);
        fixture
    }

    fn seed_job(&self, job_id: &str) {
        let db = Connection::open(&self.path).expect("open isolated migrated SQLite");
        db.execute(
            "INSERT INTO evidence_capture_jobs (
                evidence_capture_job_id, file_instance_id, document_id,
                status, requested_at_ms, started_at_ms, completed_at_ms,
                capture_reason, capture_policy, failure_code, failure_message
             ) VALUES (?1, ?2, ?3, 'CAPTURING', 1, 1, NULL,
                       'SYNTHETIC_TEST', 'SYNTHETIC_ONLY', NULL, NULL)",
            params![job_id, &self.file_id, &self.document_id],
        )
        .expect("stage synthetic CAPTURING row");
    }

    fn status(&self, job_id: &str) -> String {
        let db = Connection::open(&self.path).expect("open migrated fixture");
        db.query_row(
            "SELECT status FROM evidence_capture_jobs WHERE evidence_capture_job_id = ?1",
            [job_id],
            |row| row.get(0),
        )
        .expect("read job status")
    }

    fn counts(&self) -> [i64; 4] {
        let db = Connection::open(&self.path).expect("open migrated fixture");
        db.query_row(
            "SELECT
               (SELECT COUNT(*) FROM content_versions),
               (SELECT COUNT(*) FROM controlled_evidence_versions),
               (SELECT COUNT(*) FROM audit_events),
               (SELECT COUNT(*) FROM evidence_capture_jobs)",
            [],
            |row| Ok([row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?]),
        )
        .expect("read content, controlled, audit, job counts")
    }

    fn complete(
        &self,
        job_id: &str,
        version_id: &str,
        document_id: &str,
        file_id: &str,
    ) -> Result<persistence::ControlledEvidenceVersionRecord, persistence::PersistenceError> {
        let supplied_hash = [0xA5_u8; 32];
        persistence::complete_evidence_capture(
            &self.path,
            persistence::EvidenceCaptureCompletion {
                capture_job_id: job_id,
                controlled_evidence_version_id: version_id,
                document_id,
                file_instance_id: file_id,
                controlled_storage_locator: "synthetic://no-real-controlled-file",
                sha256: &supplied_hash,
                size_bytes: 5,
                last_write_time_ms: None,
            },
        )
    }
}

impl Drop for CaptureFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn legacy_capture_completion_rejects_job_identity_mismatch_without_writes() {
    let fixture = CaptureFixture::new();
    let before = fixture.counts();
    let version_id = Uuid::new_v4().to_string();
    assert!(fixture
        .complete(
            &fixture.job_id,
            &version_id,
            &Uuid::new_v4().to_string(),
            &fixture.file_id,
        )
        .is_err());
    assert_eq!(fixture.counts(), before);
    assert!(fixture
        .complete(
            &fixture.job_id,
            &version_id,
            &fixture.document_id,
            &Uuid::new_v4().to_string(),
        )
        .is_err());
    assert_eq!(fixture.counts(), before);
    assert_eq!(fixture.status(&fixture.job_id), "CAPTURING");
}

#[test]
fn legacy_capture_failure_terminal_state_is_idempotent_and_blocks_completion() {
    let fixture = CaptureFixture::new();
    let before = fixture.counts();
    persistence::finish_evidence_capture_failure(
        &fixture.path,
        &fixture.job_id,
        "FAILED",
        "FIXTURE_SOURCE_CHANGED",
        "Synthetic source changed",
    )
    .expect("legacy failure transition");
    assert_eq!(fixture.status(&fixture.job_id), "FAILED");
    let after = fixture.counts();
    assert_eq!(after[0], before[0]);
    assert_eq!(after[1], before[1]);
    assert_eq!(after[2], before[2] + 1);
    assert_eq!(after[3], before[3]);
    persistence::finish_evidence_capture_failure(
        &fixture.path,
        &fixture.job_id,
        "QUARANTINED",
        "REPEATED_FAILURE",
        "No second event",
    )
    .expect("already terminal is a no-op");
    assert_eq!(fixture.counts(), after);
    let version_id = Uuid::new_v4().to_string();
    assert!(fixture
        .complete(
            &fixture.job_id,
            &version_id,
            &fixture.document_id,
            &fixture.file_id,
        )
        .is_err());
    assert_eq!(fixture.counts(), after);

    let db = Connection::open(&fixture.path).expect("inspect terminal audit");
    let null_actor_count: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM audit_events
             WHERE event_type = 'EVIDENCE_CAPTURE_FAILED'
               AND entity_id = ?1 AND actor_id IS NULL",
            [&fixture.job_id],
            |row| row.get(0),
        )
        .expect("read un-attributed synthetic failure audit");
    assert_eq!(null_actor_count, 1);
    drop(db);
    let other_job_id = Uuid::new_v4().to_string();
    fixture.seed_job(&other_job_id);
    persistence::finish_evidence_capture_failure(
        &fixture.path,
        &other_job_id,
        "QUARANTINED",
        "SYNTHETIC_QUARANTINE",
        "No bytes accepted",
    )
    .expect("quarantine transition");
    assert_eq!(fixture.status(&other_job_id), "QUARANTINED");
}

#[test]
fn legacy_capture_completion_persists_claimed_hash_even_after_document_archive() {
    let fixture = CaptureFixture::new();
    let db = Connection::open(&fixture.path).expect("archive fixture document");
    db.execute(
        "UPDATE documents SET archived_at_ms = 77 WHERE document_id = ?1",
        [&fixture.document_id],
    )
    .expect("synthetic document archive");
    drop(db);
    let before = fixture.counts();
    let version_id = Uuid::new_v4().to_string();
    let recorded = fixture
        .complete(
            &fixture.job_id,
            &version_id,
            &fixture.document_id,
            &fixture.file_id,
        )
        .expect("legacy persistence accepts staged job despite archived source");
    assert_eq!(recorded.version_number, 1);
    assert_eq!(recorded.sha256, vec![0xA5_u8; 32]);
    assert_eq!(recorded.verification_state, "HASH_VERIFIED");
    assert_eq!(fixture.status(&fixture.job_id), "COMPLETE");
    assert!(persistence::get_controlled_evidence_read_record(&fixture.path, &version_id)
        .expect("read record query")
        .is_none());
    assert!(!fixture.root.join("synthetic:").exists());

    let db = Connection::open(&fixture.path).expect("read synthetic completion audit");
    let completed_event_count: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM audit_events
             WHERE event_type = 'EVIDENCE_CAPTURE_COMPLETED'
               AND entity_id = ?1 AND actor_id IS NULL",
            [&version_id],
            |row| row.get(0),
        )
        .expect("audit completion event");
    assert_eq!(completed_event_count, 1);
    let content_sha: Vec<u8> = db
        .query_row(
            "SELECT sha256 FROM content_versions
             WHERE content_version_id = ?1",
            [&recorded.source_content_version_id],
            |row| row.get(0),
        )
        .expect("read claimed source digest");
    assert_eq!(content_sha, vec![0xA5_u8; 32]);
    drop(db);

    let mut expected = before;
    expected[0] += 1;
    expected[1] += 1;
    expected[2] += 1;
    assert_eq!(fixture.counts(), expected);
    // The ordinary evidence::perform_capture verifies bytes before calling
    // this lower-level function. Here there is NO managed file or real hash.
}

#[test]
fn legacy_capture_duplicate_version_id_rolls_back_content_and_job_completion() {
    let fixture = CaptureFixture::new();
    let version_id = Uuid::new_v4().to_string();
    fixture
        .complete(
            &fixture.job_id,
            &version_id,
            &fixture.document_id,
            &fixture.file_id,
        )
        .expect("first purely synthetic metadata completion");
    let second_job = Uuid::new_v4().to_string();
    fixture.seed_job(&second_job);
    let before = fixture.counts();
    assert!(fixture
        .complete(
            &second_job,
            &version_id,
            &fixture.document_id,
            &fixture.file_id,
        )
        .is_err());
    assert_eq!(fixture.status(&second_job), "CAPTURING");
    assert_eq!(fixture.counts(), before);
    let db = Connection::open(&fixture.path).expect("inspect unchanged original version");
    let original_count: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM controlled_evidence_versions
             WHERE controlled_evidence_version_id = ?1",
            [&version_id],
            |row| row.get(0),
        )
        .expect("original metadata version must remain");
    assert_eq!(original_count, 1);
    // The failed second completion did not leave orphan source-content rows.
}
