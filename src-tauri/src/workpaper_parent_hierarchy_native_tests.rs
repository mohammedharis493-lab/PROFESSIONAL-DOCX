//! Test-only characterization of the existing native parent-hierarchy creators.
//! Uses actual production migrations with disposable SQLite, not mock handlers.
//! This does NOT call registered Tauri commands or test professional permissions.

use crate::persistence::{self, initialize_database};
use rusqlite::{params, Connection};
use std::path::PathBuf;
use uuid::Uuid;

struct HierarchyFixture {
    root: PathBuf,
    path: PathBuf,
    client_id: String,
    service_type_id: String,
}

impl HierarchyFixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "professional-docx-hierarchy-{}",
            Uuid::new_v4()
        ));
        let path = root.join("metadata.sqlite");
        initialize_database(&path).expect("apply actual production migrations");
        let client_id = Uuid::new_v4().to_string();
        let service_type_id = Uuid::new_v4().to_string();
        let db = Connection::open(&path).expect("open isolated fixture");
        db.execute(
            "INSERT INTO clients (client_id, name, created_at_ms) VALUES (?1, 'Fixture client', 1)",
            [&client_id],
        )
        .expect("create client fixture");
        db.execute(
            "INSERT INTO service_types
             (service_type_id, name, normalized_name, created_at_ms)
             VALUES (?1, 'Fixture audit', 'fixture audit', 1)",
            [&service_type_id],
        )
        .expect("create service fixture");
        Self {
            root,
            path,
            client_id,
            service_type_id,
        }
    }

    fn engagement(&self, name: &str) -> String {
        persistence::create_engagement(
            &self.path,
            &self.client_id,
            &self.service_type_id,
            name,
            None,
            None,
            "ACTIVE",
        )
        .expect("legacy engagement creator succeeds without principal input")
        .engagement_id
    }

    fn counts(&self) -> [i64; 7] {
        let db = Connection::open(&self.path).expect("read test fixture");
        db.query_row(
            "SELECT
               (SELECT COUNT(*) FROM engagements),
               (SELECT COUNT(*) FROM engagement_areas),
               (SELECT COUNT(*) FROM procedures),
               (SELECT COUNT(*) FROM workpapers),
               (SELECT COUNT(*) FROM workpaper_revisions),
               (SELECT COUNT(*) FROM workpaper_signoffs),
               (SELECT COUNT(*) FROM audit_events)",
            [],
            |row| {
                Ok([
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                ])
            },
        )
        .expect("capture cross-hierarchy row counts")
    }
}

impl Drop for HierarchyFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn legacy_engagement_creation_checks_live_client_and_service_but_not_an_issuer() {
    let fixture = HierarchyFixture::new();
    let before = fixture.counts();

    let missing_client = persistence::create_engagement(
        &fixture.path,
        &Uuid::new_v4().to_string(),
        &fixture.service_type_id,
        "Missing client",
        None,
        None,
        "ACTIVE",
    );
    assert!(missing_client.is_err());
    assert_eq!(fixture.counts(), before, "missing client must not mutate rows");

    let missing_service = persistence::create_engagement(
        &fixture.path,
        &fixture.client_id,
        &Uuid::new_v4().to_string(),
        "Missing service",
        None,
        None,
        "ACTIVE",
    );
    assert!(missing_service.is_err());
    assert_eq!(fixture.counts(), before, "missing service must not mutate rows");

    let created = fixture.engagement("Authorized-looking label is not a grant");
    assert!(Uuid::parse_str(&created).is_ok());
    let mut expected = before;
    expected[0] += 1;
    assert_eq!(fixture.counts(), expected);
    // No issuer/actor/grant parameter is supplied; successful fixture creation
    // is a CURRENT LEGACY observation, not an authenticated permission test.
    assert_eq!(fixture.counts()[6], before[6]);
}

#[test]
fn legacy_area_creation_rejects_cross_engagement_parent_without_writes() {
    let fixture = HierarchyFixture::new();
    let one = fixture.engagement("Engagement one");
    let two = fixture.engagement("Engagement two");
    let parent = persistence::create_engagement_area(
        &fixture.path,
        &one,
        None,
        "Root area",
        Some("A"),
        1,
        "ACTIVE",
    )
    .expect("create first engagement area");
    let before = fixture.counts();

    let foreign = persistence::create_engagement_area(
        &fixture.path,
        &two,
        Some(&parent.engagement_area_id),
        "Foreign child",
        None,
        2,
        "ACTIVE",
    );
    assert!(foreign.is_err(), "foreign parent must be rejected");
    assert_eq!(fixture.counts(), before);

    let child = persistence::create_engagement_area(
        &fixture.path,
        &one,
        Some(&parent.engagement_area_id),
        "Same engagement child",
        None,
        2,
        "ACTIVE",
    )
    .expect("current legacy scope check permits same-engagement child");
    assert_eq!(child.engagement_id, one);
    assert_eq!(child.parent_area_id.as_deref(), Some(parent.engagement_area_id.as_str()));
    let mut expected = before;
    expected[1] += 1;
    assert_eq!(fixture.counts(), expected);
}

#[test]
fn legacy_procedure_creation_rejects_foreign_area_without_writes() {
    let fixture = HierarchyFixture::new();
    let one = fixture.engagement("Engagement one");
    let two = fixture.engagement("Engagement two");
    let area = persistence::create_engagement_area(
        &fixture.path,
        &one,
        None,
        "Area one",
        None,
        0,
        "ACTIVE",
    )
    .expect("seed area");
    let before = fixture.counts();

    let foreign = persistence::create_procedure(
        &fixture.path,
        &two,
        Some(&area.engagement_area_id),
        Some("P-FOREIGN"),
        "Must not be inserted",
        None,
        "ACTIVE",
    );
    assert!(foreign.is_err(), "procedure cannot reference another engagement's area");
    assert_eq!(fixture.counts(), before);

    let valid = persistence::create_procedure(
        &fixture.path,
        &one,
        Some(&area.engagement_area_id),
        Some("P-LOCAL"),
        "Local procedure",
        None,
        "ACTIVE",
    )
    .expect("current legacy scope check permits local procedure");
    assert_eq!(valid.engagement_id, one);
    assert_eq!(valid.engagement_area_id.as_deref(), Some(area.engagement_area_id.as_str()));
    let mut expected = before;
    expected[2] += 1;
    assert_eq!(fixture.counts(), expected);
}

#[test]
fn legacy_hierarchy_creation_rejects_archived_ancestors_atomically() {
    let fixture = HierarchyFixture::new();
    let engagement = fixture.engagement("Archived parent engagement");
    let area = persistence::create_engagement_area(
        &fixture.path,
        &engagement,
        None,
        "Archived parent area",
        None,
        0,
        "ACTIVE",
    )
    .expect("seed active area");

    let db = Connection::open(&fixture.path).expect("fixture archive");
    db.execute(
        "UPDATE engagement_areas SET archived_at_ms = 2 WHERE engagement_area_id = ?1",
        [&area.engagement_area_id],
    )
    .expect("archive test area");
    drop(db);
    let before = fixture.counts();

    assert!(persistence::create_engagement_area(
        &fixture.path,
        &engagement,
        Some(&area.engagement_area_id),
        "Child of archived area",
        None,
        0,
        "ACTIVE",
    )
    .is_err());
    assert!(persistence::create_procedure(
        &fixture.path,
        &engagement,
        Some(&area.engagement_area_id),
        None,
        "Procedure under archived area",
        None,
        "ACTIVE",
    )
    .is_err());
    assert_eq!(fixture.counts(), before);

    let db = Connection::open(&fixture.path).expect("fixture archive engagement");
    db.execute(
        "UPDATE engagements SET archived_at_ms = 3 WHERE engagement_id = ?1",
        params![engagement],
    )
    .expect("archive test engagement");
    drop(db);
    let after_archive = fixture.counts();
    assert!(persistence::create_engagement_area(
        &fixture.path,
        &engagement,
        None,
        "Area on archived engagement",
        None,
        0,
        "ACTIVE",
    )
    .is_err());
    assert!(persistence::create_procedure(
        &fixture.path,
        &engagement,
        None,
        None,
        "Procedure on archived engagement",
        None,
        "ACTIVE",
    )
    .is_err());
    assert_eq!(fixture.counts(), after_archive);
}
