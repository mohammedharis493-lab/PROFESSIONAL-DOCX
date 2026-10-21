//! Test-only observations of legacy client/service and template-derived
//! engagement creation against the actual production SQLite migrations.
//! Calls persistence functions, NEVER registered Tauri handlers, principals,
//! grants, professional signers or protected evidence.

use crate::persistence::{self, initialize_database};
use rusqlite::{params, Connection};
use std::path::PathBuf;
use uuid::Uuid;

struct TemplateFixture {
    root: PathBuf,
    path: PathBuf,
    client_id: String,
    service_type_id: String,
}

impl TemplateFixture {
    fn new() -> Self {
        let root =
            std::env::temp_dir().join(format!("professional-docx-template-{}", Uuid::new_v4()));
        let path = root.join("metadata.sqlite");
        initialize_database(&path).expect("apply actual production migrations");
        let client_id = persistence::create_client(&path, "Fixture client")
            .expect("create test-only client")
            .client_id;
        let service_type_id = persistence::create_service_type(&path, "Fixture audit")
            .expect("create test-only service type")
            .service_type_id;
        Self {
            root,
            path,
            client_id,
            service_type_id,
        }
    }

    fn source_engagement(&self) -> String {
        persistence::create_engagement(
            &self.path,
            &self.client_id,
            &self.service_type_id,
            "Source engagement",
            None,
            None,
            "ACTIVE",
        )
        .expect("create fixture engagement")
        .engagement_id
    }

    fn counts(&self) -> [i64; 9] {
        let db = Connection::open(&self.path).expect("read isolated test database");
        db.query_row(
            "SELECT
                (SELECT COUNT(*) FROM clients),
                (SELECT COUNT(*) FROM service_types),
                (SELECT COUNT(*) FROM engagements),
                (SELECT COUNT(*) FROM engagement_templates),
                (SELECT COUNT(*) FROM engagement_template_versions),
                (SELECT COUNT(*) FROM engagement_areas),
                (SELECT COUNT(*) FROM procedures),
                (SELECT COUNT(*) FROM workpapers),
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
                    row.get(7)?,
                    row.get(8)?,
                ])
            },
        )
        .expect("count hierarchy and audit rows")
    }
}

impl Drop for TemplateFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn legacy_client_and_service_provisioning_has_no_issuer_input() {
    let fixture = TemplateFixture::new();
    let before = fixture.counts();

    let client = persistence::create_client(&fixture.path, "  Additional   client  ")
        .expect("legacy client creation permits caller label");
    assert_eq!(client.name, "Additional client");
    let service = persistence::create_service_type(&fixture.path, "  Forensic   audit  ")
        .expect("legacy service creation permits caller label");
    assert_eq!(service.name, "Forensic audit");

    let mut expected = before;
    expected[0] += 1;
    expected[1] += 1;
    assert_eq!(fixture.counts(), expected);
    assert_eq!(fixture.counts()[8], before[8]);

    let after = fixture.counts();
    assert!(persistence::create_client(&fixture.path, "   ").is_err());
    assert!(persistence::create_service_type(&fixture.path, "   ").is_err());
    assert_eq!(fixture.counts(), after);
    // No trusted issuer, grant or human approval is supplied to these legacy calls.
}

#[test]
fn legacy_template_clone_reparents_nested_areas_across_clients_without_copying_workpapers() {
    let fixture = TemplateFixture::new();
    let source_id = fixture.source_engagement();
    let root = persistence::create_engagement_area(
        &fixture.path,
        &source_id,
        None,
        "Planning",
        Some("PL"),
        1,
        "ACTIVE",
    )
    .expect("create root");
    let child = persistence::create_engagement_area(
        &fixture.path,
        &source_id,
        Some(&root.engagement_area_id),
        "Risk",
        Some("RK"),
        2,
        "ACTIVE",
    )
    .expect("create nested area");
    persistence::create_procedure(
        &fixture.path,
        &source_id,
        Some(&child.engagement_area_id),
        Some("PR-1"),
        "Procedure one",
        Some("Fixture only"),
        "ACTIVE",
    )
    .expect("create linked procedure");
    persistence::create_workpaper(
        &fixture.path,
        &source_id,
        Some(&child.engagement_area_id),
        None,
        "WP-SOURCE",
        "Source only",
        "IN_PROGRESS",
    )
    .expect("create source workpaper");

    let template = persistence::create_engagement_template_from_engagement(
        &fixture.path,
        &source_id,
        "Unverified template",
        None,
    )
    .expect("snapshot source hierarchy");
    let other_client = persistence::create_client(&fixture.path, "Other test client")
        .expect("seed different destination client");
    let before = fixture.counts();

    let cloned = persistence::create_engagement_from_template(
        &fixture.path,
        &template.latest_version_id,
        &other_client.client_id,
        "Cross-client legacy clone",
        None,
        None,
        "REVIEWED",
    )
    .expect("current legacy template clone accepts unrelated live destination client");
    assert_eq!(cloned.client_id, other_client.client_id);
    assert_eq!(cloned.service_type_id, fixture.service_type_id);
    assert_eq!(cloned.status, "REVIEWED");

    let areas = persistence::list_engagement_areas(&fixture.path, &cloned.engagement_id)
        .expect("read cloned areas");
    assert_eq!(areas.len(), 2);
    let new_root = areas
        .iter()
        .find(|area| area.name == "Planning")
        .expect("root copied");
    let new_child = areas
        .iter()
        .find(|area| area.name == "Risk")
        .expect("child copied");
    assert_ne!(new_root.engagement_area_id, root.engagement_area_id);
    assert_ne!(new_child.engagement_area_id, child.engagement_area_id);
    assert_eq!(
        new_child.parent_area_id.as_deref(),
        Some(new_root.engagement_area_id.as_str())
    );

    let procedures = persistence::list_procedures(&fixture.path, &cloned.engagement_id)
        .expect("read cloned procedures");
    assert_eq!(procedures.len(), 1);
    assert_eq!(
        procedures[0].engagement_area_id.as_deref(),
        Some(new_child.engagement_area_id.as_str())
    );
    assert_eq!(
        persistence::list_workpapers(&fixture.path, &cloned.engagement_id)
            .expect("read cloned workpapers")
            .len(),
        0
    );
    assert_eq!(
        persistence::list_workpapers(&fixture.path, &source_id)
            .expect("read original workpapers")
            .len(),
        1
    );

    let mut expected = before;
    expected[2] += 1;
    expected[5] += 2;
    expected[6] += 1;
    assert_eq!(fixture.counts(), expected);
    // Cross-client creation and REVIEWED are unverified LEGACY behaviors,
    // NOT authorization, professional review or a recommended future policy.
    assert_eq!(fixture.counts()[8], before[8]);
}

#[test]
fn legacy_template_clone_rolls_back_staged_engagement_on_invalid_definition() {
    let fixture = TemplateFixture::new();
    let source_id = fixture.source_engagement();
    let template = persistence::create_engagement_template_from_engagement(
        &fixture.path,
        &source_id,
        "Malformed fixture template",
        None,
    )
    .expect("create genuine template version from disposable migrated database");

    let before_protected_update = fixture.counts();
    let db = Connection::open(&fixture.path).expect("open isolated fixture");
    let protected_update = db.execute(
        "UPDATE engagement_template_versions
         SET definition_json = '{}' WHERE engagement_template_version_id = ?1",
        [&template.latest_version_id],
    );
    assert!(
        protected_update
            .unwrap_err()
            .to_string()
            .contains("immutable"),
        "actual production migration must prevent rewriting historical template versions"
    );
    drop(db);
    assert_eq!(fixture.counts(), before_protected_update);

    let cases = [
        serde_json::json!({
            "areas": [
                {
                    "source_area_id": "root",
                    "parent_source_area_id": null,
                    "name": "Root",
                    "code": null,
                    "display_order": 0,
                    "status": "ACTIVE"
                },
                {
                    "source_area_id": "orphan",
                    "parent_source_area_id": "absent-parent",
                    "name": "Orphan",
                    "code": null,
                    "display_order": 1,
                    "status": "ACTIVE"
                }
            ],
            "procedures": []
        }),
        serde_json::json!({
            "areas": [
                {
                    "source_area_id": "root",
                    "parent_source_area_id": null,
                    "name": "Root",
                    "code": null,
                    "display_order": 0,
                    "status": "ACTIVE"
                }
            ],
            "procedures": [
                {
                    "source_area_id": "missing-procedure-area",
                    "reference": "PR",
                    "title": "Orphan procedure",
                    "description": null,
                    "status": "ACTIVE"
                }
            ]
        }),
    ];
    for (i, bad_definition) in cases.iter().enumerate() {
        // A separate, explicitly fabricated fixture version is inserted for
        // adversarial input coverage. No existing immutable version is edited,
        // and the real installed application exposes no such fixture insertion.
        let fabricated_version_id = Uuid::new_v4().to_string();
        let db = Connection::open(&fixture.path).expect("open disposable fixture");
        db.execute(
            "INSERT INTO engagement_template_versions (
                engagement_template_version_id, engagement_template_id,
                version_number, source_engagement_id, service_type_id,
                definition_json, created_at_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 2)",
            params![
                &fabricated_version_id,
                &template.engagement_template_id,
                (i as i64) + 2,
                &source_id,
                &fixture.service_type_id,
                bad_definition.to_string()
            ],
        )
        .expect("insert only a new synthetic malformed fixture version");
        drop(db);

        let before = fixture.counts();
        let failed = persistence::create_engagement_from_template(
            &fixture.path,
            &fabricated_version_id,
            &fixture.client_id,
            &format!("Rejected attempt {i}"),
            None,
            None,
            "ACTIVE",
        );
        assert!(failed.is_err(), "malformed template must reject");
        assert_eq!(
            fixture.counts(),
            before,
            "staged engagement/area/procedure and audit counts must roll back"
        );
    }
    let original = persistence::create_engagement_from_template(
        &fixture.path,
        &template.latest_version_id,
        &fixture.client_id,
        "Historical original remains usable",
        None,
        None,
        "ACTIVE",
    )
    .expect("initial historical version remains readable and usable");
    assert!(Uuid::parse_str(&original.engagement_id).is_ok());
}

#[test]
fn legacy_template_version_rejects_foreign_service_type_without_new_version() {
    let fixture = TemplateFixture::new();
    let source_id = fixture.source_engagement();
    let template = persistence::create_engagement_template_from_engagement(
        &fixture.path,
        &source_id,
        "Template version service scope",
        None,
    )
    .expect("capture first version");
    let second_service = persistence::create_service_type(&fixture.path, "Tax fixture")
        .expect("create distinct test service");
    let foreign_source = persistence::create_engagement(
        &fixture.path,
        &fixture.client_id,
        &second_service.service_type_id,
        "Other service engagement",
        None,
        None,
        "ACTIVE",
    )
    .expect("create other service engagement");
    let before = fixture.counts();
    assert!(
        persistence::create_engagement_template_version_from_engagement(
            &fixture.path,
            &template.engagement_template_id,
            &foreign_source.engagement_id
        )
        .is_err()
    );
    assert_eq!(fixture.counts(), before);
    let good = persistence::create_engagement_template_version_from_engagement(
        &fixture.path,
        &template.engagement_template_id,
        &source_id,
    )
    .expect("same-service template source permitted by existing code");
    assert_eq!(good.latest_version_number, 2);
    assert_ne!(good.latest_version_id, template.latest_version_id);
    let mut expected = before;
    expected[4] += 1;
    assert_eq!(fixture.counts(), expected);
}
