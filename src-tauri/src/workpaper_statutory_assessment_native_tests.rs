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
