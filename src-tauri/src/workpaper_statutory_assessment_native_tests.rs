//! Nonshipping characterization of legacy statutory-compliance assessment writes.
//! All fixtures use actual production migrations in disposable local SQLite.
//! No direct registered-Tauri IPC, authenticated user, grant or professional
//! reviewer is supplied by these persistence-level tests.

use crate::persistence::{self, initialize_database};
use rusqlite::Connection;
use std::path::PathBuf;
use uuid::Uuid;

struct StatutoryFixture {
    root: PathBuf,
    path: PathBuf,
    engagement_id: String,
    requirement_id: String,
}

impl StatutoryFixture {
    fn new() -> Self {
        let root =
            std::env::temp_dir().join(format!("professional-docx-statutory-{}", Uuid::new_v4()));
        let path = root.join("metadata.sqlite");
        initialize_database(&path).expect("run actual migrations");
        let service = persistence::create_service_type(&path, "Statutory fixture service")
            .expect("test service");
        let client =
            persistence::create_client(&path, "Statutory fixture client").expect("test client");
        let engagement_id = persistence::create_engagement(
            &path,
            &client.client_id,
            &service.service_type_id,
            "Statutory fixture engagement",
            None,
            None,
            "ACTIVE",
        )
        .expect("test engagement")
        .engagement_id;
        let library = persistence::create_firm_library_item(
            &path,
            "STATUTORY_COMPLIANCE_REQUIREMENT",
            "Statutory fixture obligation",
            None,
            Some(&service.service_type_id),
            r#"{"content":"synthetic only"}"#,
        )
        .expect("test library version");
        let requirement_id = persistence::create_statutory_compliance_requirement(
            &path,
            &engagement_id,
            &library.latest_version_id,
        )
        .expect("test installed-methodology requirement")
        .statutory_compliance_requirement_id;
        Self {
            root,
            path,
            engagement_id,
            requirement_id,
        }
    }

    fn counts(&self) -> [i64; 4] {
        let db = Connection::open(&self.path).expect("open isolated database");
        db.query_row(
            "SELECT
                 (SELECT COUNT(*) FROM statutory_compliance_requirements),
                 (SELECT COUNT(*) FROM statutory_compliance_assessments),
                 (SELECT COUNT(*) FROM statutory_compliance_evidence_links),
                 (SELECT COUNT(*) FROM audit_events)",
            [],
            |row| Ok([row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?]),
        )
        .expect("capture requirements/assessments/evidence/audit")
    }

    // Test-only helper mirrors the seven native assessment fields explicitly;
    // scope this style exception to the disposable fixture, never production.
    #[allow(clippy::too_many_arguments)]
    fn assess(
        &self,
        applicability: &str,
        status: &str,
        due_date: Option<&str>,
        actual_date: Option<&str>,
        exception_text: Option<&str>,
        conclusion: Option<&str>,
        evidence: &[String],
    ) -> Result<persistence::StatutoryComplianceAssessmentRecord, persistence::PersistenceError>
    {
        persistence::create_statutory_compliance_assessment(
            &self.path,
            persistence::StatutoryComplianceAssessmentDefinition {
                statutory_compliance_requirement_id: &self.requirement_id,
                applicability,
                due_date,
                actual_compliance_date: actual_date,
                status,
                exception_text,
                conclusion,
                controlled_evidence_version_ids: evidence,
            },
        )
    }
}

impl Drop for StatutoryFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn legacy_assessment_appends_caller_claimed_exception_without_evidence_or_verified_actor() {
    let fixture = StatutoryFixture::new();
    let before = fixture.counts();
    let result = fixture
        .assess(
            "APPLICABLE",
            "EXCEPTION",
            Some("2026-10-01"),
            Some("2026-10-15"),
            Some("Late statutory submission"),
            Some("Requires follow-up"),
            &[],
        )
        .expect("legacy exception status supports no evidence");
    assert_eq!(result.status, "EXCEPTION");
    assert_eq!(result.version_number, 2);
    assert_eq!(result.evidence_count, 0);

    let rows =
        persistence::list_statutory_compliance_assessments(&fixture.path, &fixture.requirement_id)
            .expect("read actual immutable assessment history");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].version_number, 2);
    assert_eq!(rows[1].version_number, 1);
    assert_eq!(
        rows[0].supersedes_assessment_id.as_deref(),
        Some(rows[1].statutory_compliance_assessment_id.as_str())
    );
    assert_eq!(
        persistence::list_statutory_compliance_evidence(
            &fixture.path,
            &result.statutory_compliance_assessment_id
        )
        .expect("read assessment evidence")
        .len(),
        0
    );

    let db = Connection::open(&fixture.path).expect("inspect migrated audit");
    let actors: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM audit_events
             WHERE event_type = 'STATUTORY_COMPLIANCE_ASSESSMENT_RECORDED'
               AND entity_id = ?1 AND actor_id IS NULL",
            [&result.statutory_compliance_assessment_id],
            |row| row.get(0),
        )
        .expect("count un-attributed event");
    assert_eq!(actors, 1);
    let mutation = db.execute(
        "UPDATE statutory_compliance_assessments SET status = 'COMPLIANT'
         WHERE statutory_compliance_assessment_id = ?1",
        [&result.statutory_compliance_assessment_id],
    );
    assert!(mutation.unwrap_err().to_string().contains("immutable"));
    drop(db);

    let mut expected = before;
    expected[1] += 1;
    expected[3] += 1;
    assert_eq!(fixture.counts(), expected);
    // This is a current legacy behavior, not a professionally reviewed claim.
}

#[test]
fn legacy_assessment_invalid_state_duplicate_evidence_and_missing_evidence_do_not_write() {
    let fixture = StatutoryFixture::new();
    let before = fixture.counts();
    assert!(fixture
        .assess(
            "APPLICABLE",
            "COMPLIANT",
            Some("2026-10-01"),
            Some("2026-10-01"),
            None,
            Some("Caller conclusion"),
            &[],
        )
        .is_err());
    assert_eq!(fixture.counts(), before);

    let duplicate = Uuid::new_v4().to_string();
    assert!(fixture
        .assess(
            "APPLICABLE",
            "COMPLIANT",
            Some("2026-10-01"),
            Some("2026-10-01"),
            None,
            Some("Caller conclusion"),
            &[duplicate.clone(), duplicate],
        )
        .is_err());
    assert_eq!(fixture.counts(), before);

    let missing = Uuid::new_v4().to_string();
    assert!(fixture
        .assess(
            "APPLICABLE",
            "COMPLIANT",
            Some("2026-10-01"),
            Some("2026-10-01"),
            None,
            Some("Caller conclusion"),
            &[missing],
        )
        .is_err());
    assert_eq!(fixture.counts(), before);

    assert!(fixture
        .assess(
            "APPLICABLE",
            "NOT_APPLICABLE",
            None,
            None,
            None,
            Some("Wrong applicability"),
            &[],
        )
        .is_err());
    assert_eq!(fixture.counts(), before);
}

#[test]
fn legacy_assessment_allows_new_version_after_archiving_parent_engagement() {
    let fixture = StatutoryFixture::new();
    let db = Connection::open(&fixture.path).expect("open isolated fixture");
    db.execute(
        "UPDATE engagements SET archived_at_ms = 22 WHERE engagement_id = ?1",
        [&fixture.engagement_id],
    )
    .expect("archive synthetic parent engagement");
    drop(db);
    let before = fixture.counts();

    let recorded = fixture
        .assess(
            "NOT_APPLICABLE",
            "NOT_APPLICABLE",
            None,
            None,
            None,
            Some("Caller says this control does not apply"),
            &[],
        )
        .expect("current assessment path does not recheck archived parent");
    assert_eq!(recorded.version_number, 2);
    assert_eq!(recorded.status, "NOT_APPLICABLE");
    let mut expected = before;
    expected[1] += 1;
    expected[3] += 1;
    assert_eq!(fixture.counts(), expected);
    // An archived parent must not be interpreted as a permission grant.
}

#[test]
fn legacy_assessment_version_numbers_advance_with_immutable_prior_state() {
    let fixture = StatutoryFixture::new();
    let first = fixture
        .assess(
            "APPLICABLE",
            "PENDING",
            Some("2026-12-31"),
            None,
            None,
            Some("Pending filing"),
            &[],
        )
        .expect("pending legacy state");
    let second = fixture
        .assess(
            "APPLICABLE",
            "EXCEPTION",
            Some("2026-12-31"),
            Some("2027-01-01"),
            Some("One-day late"),
            Some("Document exception"),
            &[],
        )
        .expect("later exception legacy state");
    assert_eq!(first.version_number, 2);
    assert_eq!(second.version_number, 3);
    assert_eq!(
        second.supersedes_assessment_id.as_deref(),
        Some(first.statutory_compliance_assessment_id.as_str())
    );
    let history =
        persistence::list_statutory_compliance_assessments(&fixture.path, &fixture.requirement_id)
            .expect("inspect immutable earlier states");
    assert_eq!(
        history
            .iter()
            .map(|row| row.version_number)
            .collect::<Vec<_>>(),
        vec![3, 2, 1]
    );
    assert_eq!(history[1].status, "PENDING");
    assert_eq!(history[2].status, "UNASSESSED");
    assert_eq!(fixture.counts()[2], 0);
}


struct SyntheticEvidence {
    version_id: String,
    document_id: String,
    source_content_version_id: String,
    sha256: Vec<u8>,
}

// Deliberately insert a full FK-valid source/capture lineage into disposable
// migrated SQLite. The invented HASH_VERIFIED flag and digest are NOT proof
// that the native capture pipeline captured or verified any real bytes.
fn seed_synthetic_evidence(fixture: &StatutoryFixture, verification: &str) -> SyntheticEvidence {
    let storage_root_id = Uuid::new_v4().to_string();
    let document_id = Uuid::new_v4().to_string();
    let file_id = Uuid::new_v4().to_string();
    let source_content_version_id = Uuid::new_v4().to_string();
    let job_id = Uuid::new_v4().to_string();
    let version_id = Uuid::new_v4().to_string();
    let sha256 = vec![0xB7_u8; 32];
    let db = Connection::open(&fixture.path).expect("open isolated migrated database");
    db.execute(
        "INSERT INTO storage_roots (
            storage_root_id, kind, native_locator, native_locator_encoding,
            display_locator, canonical_native_locator, canonical_display_locator,
            availability_state, approved_at_ms, approved_by, created_at_ms, updated_at_ms
         ) VALUES (?1, 'LOCAL', ?2, 'TEST', '/synthetic-evidence', NULL,
                   NULL, 'AVAILABLE', 1, NULL, 1, 1)",
        rusqlite::params![&storage_root_id, vec![0_u8]],
    )
    .expect("seed synthetic root");
    db.execute(
        "INSERT INTO documents (
            document_id, storage_state, display_name, created_at_ms,
            created_by, archived_at_ms
         ) VALUES (?1, 'CONTROLLED_EVIDENCE', 'Synthetic provenance.xlsx', 1, NULL, NULL)",
        [&document_id],
    )
    .expect("seed synthetic document with no engagement association");
    db.execute(
        "INSERT INTO file_instances (
            file_instance_id, document_id, storage_root_id, relative_path_native,
            path_native_encoding, relative_path_display, relative_path_search,
            filesystem_identity, volume_identity, creation_time_ms, last_write_time_ms,
            size_bytes, file_attributes, reparse_tag, first_seen_at_ms, last_seen_at_ms,
            first_seen_generation_id, last_seen_generation_id, availability_state
         ) VALUES (
            ?1, ?2, ?3, ?4, 'TEST', 'Synthetic provenance.xlsx',
            'synthetic provenance xlsx', NULL, NULL, NULL, NULL, 128,
            NULL, NULL, 1, 1, NULL, NULL, 'AVAILABLE'
         )",
        rusqlite::params![
            &file_id,
            &document_id,
            &storage_root_id,
            b"Synthetic provenance.xlsx".to_vec()
        ],
    )
    .expect("seed synthetic file");
    db.execute(
        "INSERT INTO content_versions (
            content_version_id, document_id, file_instance_id, observed_at_ms,
            size_bytes, last_write_time_ms, quick_fingerprint, sha256,
            verification_state, source_stable_during_read
         ) VALUES (?1, ?2, ?3, 1, 128, NULL, NULL, ?4, 'HASH_VERIFIED', 1)",
        rusqlite::params![&source_content_version_id, &document_id, &file_id, &sha256],
    )
    .expect("seed synthetic source version");
    db.execute(
        "INSERT INTO evidence_capture_jobs (
            evidence_capture_job_id, file_instance_id, document_id, status,
            requested_at_ms, started_at_ms, completed_at_ms, capture_reason,
            capture_policy, failure_code, failure_message
         ) VALUES (?1, ?2, ?3, 'COMPLETE', 1, 1, 1, 'TEST', 'TEST', NULL, NULL)",
        rusqlite::params![&job_id, &file_id, &document_id],
    )
    .expect("seed synthetic completed capture record");
    db.execute(
        "INSERT INTO controlled_evidence_versions (
            controlled_evidence_version_id, document_id, source_file_instance_id,
            source_content_version_id, evidence_capture_job_id, version_number,
            controlled_storage_locator, sha256, size_bytes, captured_at_ms,
            captured_by, capture_reason, capture_policy, retention_state,
            verification_state, source_stable_during_read
         ) VALUES (
            ?1, ?2, ?3, ?4, ?5, 1, ?6, ?7, 128, 1, NULL,
            'TEST', 'TEST', 'RETAINED', ?8, 1
         )",
        rusqlite::params![
            &version_id,
            &document_id,
            &file_id,
            &source_content_version_id,
            &job_id,
            format!("{document_id}/{version_id}"),
            &sha256,
            verification
        ],
    )
    .expect("seed synthetic controlled evidence metadata");
    SyntheticEvidence {
        version_id,
        document_id,
        source_content_version_id,
        sha256,
    }
}

#[test]
fn legacy_assessment_uses_controlled_metadata_snapshot_without_document_scope_grant() {
    let fixture = StatutoryFixture::new();
    let evidence = seed_synthetic_evidence(&fixture, "HASH_VERIFIED");
    let before = fixture.counts();
    let result = fixture
        .assess(
            "APPLICABLE",
            "COMPLIANT",
            Some("2026-12-31"),
            Some("2026-12-30"),
            None,
            Some("Caller assertion supported by synthetic metadata"),
            std::slice::from_ref(&evidence.version_id),
        )
        .expect("current persistence accepts FK-valid retained verified metadata");
    assert_eq!(result.status, "COMPLIANT");
    assert_eq!(result.evidence_count, 1);

    let links = persistence::list_statutory_compliance_evidence(
        &fixture.path,
        &result.statutory_compliance_assessment_id,
    )
    .expect("read exact version-link tuple");
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].controlled_evidence_version_id, evidence.version_id);
    assert_eq!(links[0].document_id, evidence.document_id);
    assert_eq!(
        links[0].source_content_version_id,
        evidence.source_content_version_id
    );
    assert_eq!(links[0].source_sha256, evidence.sha256);

    let db = Connection::open(&fixture.path).expect("inspect migrated constraints");
    let error = db
        .execute(
            "UPDATE statutory_compliance_evidence_links
             SET linked_at_ms = linked_at_ms + 1
             WHERE statutory_compliance_evidence_link_id = ?1",
            [&links[0].statutory_compliance_evidence_link_id],
        )
        .expect_err("migrated evidence-link tuple must be immutable");
    assert!(error.to_string().contains("immutable"));
    let audit_actors: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM audit_events
             WHERE event_type = 'STATUTORY_COMPLIANCE_ASSESSMENT_RECORDED'
               AND entity_id = ?1 AND actor_id IS NULL",
            [&result.statutory_compliance_assessment_id],
            |row| row.get(0),
        )
        .expect("audit actor remains unspecified");
    assert_eq!(audit_actors, 1);
    drop(db);

    let mut expected = before;
    expected[1] += 1;
    expected[2] += 1;
    expected[3] += 1;
    assert_eq!(fixture.counts(), expected);
    // No real controlled bytes, custody, Tauri issuer or cross-scope grant.
}

#[test]
fn legacy_assessment_can_attach_archived_document_hidden_by_evidence_read_path() {
    let fixture = StatutoryFixture::new();
    let evidence = seed_synthetic_evidence(&fixture, "HASH_VERIFIED");
    let db = Connection::open(&fixture.path).expect("open isolated migrated database");
    db.execute(
        "UPDATE documents SET archived_at_ms = 77 WHERE document_id = ?1",
        [&evidence.document_id],
    )
    .expect("archive only the synthetic source document");
    drop(db);

    assert!(persistence::get_controlled_evidence_read_record(
        &fixture.path,
        &evidence.version_id,
    )
    .expect("read record access path returns without SQL error")
    .is_none());
    let before = fixture.counts();
    let result = fixture
        .assess(
            "APPLICABLE",
            "COMPLIANT",
            None,
            Some("2026-12-30"),
            None,
            Some("Claim despite archived synthetic source document"),
            std::slice::from_ref(&evidence.version_id),
        )
        .expect("assessment lookup does not join documents or enforce archive state");
    let links = persistence::list_statutory_compliance_evidence(
        &fixture.path,
        &result.statutory_compliance_assessment_id,
    )
    .expect("inspect newly linked archived-document reference");
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].document_id, evidence.document_id);
    let mut expected = before;
    expected[1] += 1;
    expected[2] += 1;
    expected[3] += 1;
    assert_eq!(fixture.counts(), expected);
    // This establishes a metadata/read-path inconsistency only, not webview exploitability.
}

#[test]
fn legacy_assessment_rejects_quarantined_metadata_without_partial_evidence_or_audit() {
    let fixture = StatutoryFixture::new();
    let evidence = seed_synthetic_evidence(&fixture, "QUARANTINED");
    let before = fixture.counts();
    let err = fixture
        .assess(
            "APPLICABLE",
            "COMPLIANT",
            None,
            Some("2026-12-30"),
            None,
            Some("Claim based on quarantined fixture data"),
            std::slice::from_ref(&evidence.version_id),
        )
        .expect_err("quarantined evidence must be refused");
    assert!(err.to_string().contains("not retained and hash verified"));
    assert_eq!(fixture.counts(), before);
}
