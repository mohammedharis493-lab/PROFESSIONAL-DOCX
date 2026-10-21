//! Nonshipping real-migration characterization of current firm-library
//! publication and exact statutory-compliance version consumption.
//! No registered Tauri IPC, issuer, trusted publisher, grants or professional
//! approval is created or simulated by these tests.

use crate::persistence::{self, initialize_database};
use rusqlite::Connection;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use uuid::Uuid;

struct FirmLibraryFixture {
    root: PathBuf,
    path: PathBuf,
    service_id: String,
    engagement_id: String,
}

impl FirmLibraryFixture {
    fn new() -> Self {
        let root =
            std::env::temp_dir().join(format!("professional-docx-library-{}", Uuid::new_v4()));
        let path = root.join("metadata.sqlite");
        initialize_database(&path).expect("apply actual production migrations");
        let service_id = persistence::create_service_type(&path, "Library fixture audit")
            .expect("test service")
            .service_type_id;
        let client = persistence::create_client(&path, "Library fixture client")
            .expect("test client");
        let engagement_id = persistence::create_engagement(
            &path,
            &client.client_id,
            &service_id,
            "Library fixture engagement",
            None,
            None,
            "ACTIVE",
        )
        .expect("test engagement")
        .engagement_id;
        Self {
            root,
            path,
            service_id,
            engagement_id,
        }
    }

    fn counts(&self) -> [i64; 5] {
        let db = Connection::open(&self.path).expect("open isolated test database");
        db.query_row(
            "SELECT
               (SELECT COUNT(*) FROM firm_library_items),
               (SELECT COUNT(*) FROM firm_library_versions),
               (SELECT COUNT(*) FROM statutory_compliance_requirements),
               (SELECT COUNT(*) FROM statutory_compliance_assessments),
               (SELECT COUNT(*) FROM audit_events)",
            [],
            |row| {
                Ok([
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ])
            },
        )
        .expect("read exact aggregate fixture counts")
    }

    fn requirement(&self, definition: &str) -> persistence::FirmLibraryItemRecord {
        persistence::create_firm_library_item(
            &self.path,
            "STATUTORY_COMPLIANCE_REQUIREMENT",
            "Fixture regulatory control",
            None,
            Some(&self.service_id),
            definition,
        )
        .expect("publish synthetic compliance requirement")
    }
}

impl Drop for FirmLibraryFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn legacy_firm_library_create_preserves_canonical_hash_audit_and_immutable_version() {
    let fixture = FirmLibraryFixture::new();
    let before = fixture.counts();
    let item = persistence::create_firm_library_item(
        &fixture.path,
        "CHECKLIST",
        "  Synthetic  checklist ",
        Some("Current legacy fixture"),
        Some(&fixture.service_id),
        r#"{ "z": 1, "a": "exact source" }"#,
    )
    .expect("legacy unverified publisher can create a library row");
    assert_eq!(item.name, "Synthetic checklist");
    assert_eq!(item.latest_version_number, 1);

    let versions = persistence::list_firm_library_versions(
        &fixture.path,
        &item.firm_library_item_id,
    )
    .expect("list immutable versions");
    assert_eq!(versions.len(), 1);
    assert_eq!(versions[0].definition_json, r#"{"a":"exact source","z":1}"#);
    assert_eq!(
        versions[0].definition_hash,
        Sha256::digest(versions[0].definition_json.as_bytes()).to_vec()
    );
    assert_eq!(versions[0].definition_hash, item.latest_definition_hash);

    let db = Connection::open(&fixture.path).expect("read audit and triggers");
    let audit_count: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM audit_events
             WHERE event_type = 'FIRM_LIBRARY_ITEM_CREATED'
               AND entity_id = ?1 AND related_entity_id = ?2",
            [&item.firm_library_item_id, &item.latest_version_id],
            |row| row.get(0),
        )
        .expect("read actual audit event");
    assert_eq!(audit_count, 1);
    assert!(db
        .execute(
            "UPDATE firm_library_versions SET definition_json = '{}' WHERE firm_library_version_id = ?1",
            [&item.latest_version_id],
        )
        .unwrap_err()
        .to_string()
        .contains("immutable"));
    assert!(db
        .execute(
            "DELETE FROM firm_library_versions WHERE firm_library_version_id = ?1",
            [&item.latest_version_id],
        )
        .unwrap_err()
        .to_string()
        .contains("immutable"));
    drop(db);

    let mut expected = before;
    expected[0] += 1;
    expected[1] += 1;
    expected[4] += 1;
    assert_eq!(fixture.counts(), expected);
    // A domain audit event is not an independently verified publisher.
}

#[test]
fn legacy_firm_library_rejects_invalid_definition_and_foreign_service_without_writes() {
    let fixture = FirmLibraryFixture::new();
    let before = fixture.counts();
    assert!(persistence::create_firm_library_item(
        &fixture.path,
        "CHECKLIST",
        "Wrong service",
        None,
        Some(&Uuid::new_v4().to_string()),
        r#"{"content":"not inserted"}"#,
    )
    .is_err());
    assert!(persistence::create_firm_library_item(
        &fixture.path,
        "CHECKLIST",
        "Bad JSON",
        None,
        None,
        "[1,2,3]",
    )
    .is_err());
    assert!(persistence::create_firm_library_item(
        &fixture.path,
        "INVALID",
        "Bad category",
        None,
        None,
        r#"{"x":1}"#,
    )
    .is_err());
    assert_eq!(fixture.counts(), before);

    let item = fixture.requirement(r#"{"content":"first"}"#);
    let before_archival = fixture.counts();
    let db = Connection::open(&fixture.path).expect("fixture archive");
    db.execute(
        "UPDATE firm_library_items SET archived_at_ms = 2 WHERE firm_library_item_id = ?1",
        [&item.firm_library_item_id],
    )
    .expect("archive only synthetic item");
    drop(db);
    assert!(persistence::publish_firm_library_version(
        &fixture.path,
        &item.firm_library_item_id,
        r#"{"content":"should fail"}"#,
    )
    .is_err());
    assert_eq!(fixture.counts(), before_archival);
}

#[test]
fn legacy_firm_library_publish_appends_version_without_issuer_or_compare_and_swap() {
    let fixture = FirmLibraryFixture::new();
    let item = fixture.requirement(r#"{"content":"same text"}"#);
    let initial = persistence::list_firm_library_versions(
        &fixture.path,
        &item.firm_library_item_id,
    )
    .expect("initial version");
    let before = fixture.counts();
    let published = persistence::publish_firm_library_version(
        &fixture.path,
        &item.firm_library_item_id,
        r#"{"content":"same text"}"#,
    )
    .expect("legacy publication accepts same bytes as a new version");
    assert_eq!(published.latest_version_number, 2);
    assert_ne!(published.latest_version_id, item.latest_version_id);
    assert_eq!(published.latest_definition_hash, item.latest_definition_hash);
    let versions = persistence::list_firm_library_versions(
        &fixture.path,
        &item.firm_library_item_id,
    )
    .expect("read both exact versions");
    assert_eq!(versions.len(), 2);
    assert_eq!(versions[1].firm_library_version_id, initial[0].firm_library_version_id);
    assert_eq!(versions[1].definition_json, initial[0].definition_json);
    assert_eq!(versions[1].definition_hash, initial[0].definition_hash);
    let mut expected = before;
    expected[1] += 1;
    expected[4] += 1;
    assert_eq!(fixture.counts(), expected);
    // Current persistence accepts no expected-latest ID or authenticated actor.
}

#[test]
fn legacy_statutory_consumer_pins_exact_library_version_and_rejects_wrong_scopes() {
    let fixture = FirmLibraryFixture::new();
    let requirement = fixture.requirement(r#"{"text":"initial statutory methodology"}"#);
    let checklist = persistence::create_firm_library_item(
        &fixture.path,
        "CHECKLIST",
        "Not statutory",
        None,
        Some(&fixture.service_id),
        r#"{"content":"wrong category"}"#,
    )
    .expect("seed nonstatutory fixture");

    let before = fixture.counts();
    assert!(persistence::create_statutory_compliance_requirement(
        &fixture.path,
        &fixture.engagement_id,
        &checklist.latest_version_id,
    )
    .is_err());
    assert_eq!(fixture.counts(), before);

    let other_service = persistence::create_service_type(&fixture.path, "Unrelated fixture service")
        .expect("seed unrelated service");
    let foreign = persistence::create_firm_library_item(
        &fixture.path,
        "STATUTORY_COMPLIANCE_REQUIREMENT",
        "Unrelated service requirement",
        None,
        Some(&other_service.service_type_id),
        r#"{"text":"wrong engagement service"}"#,
    )
    .expect("seed foreign service requirement");
    let before_foreign = fixture.counts();
    assert!(persistence::create_statutory_compliance_requirement(
        &fixture.path,
        &fixture.engagement_id,
        &foreign.latest_version_id,
    )
    .is_err());
    assert_eq!(fixture.counts(), before_foreign);

    let recorded = persistence::create_statutory_compliance_requirement(
        &fixture.path,
        &fixture.engagement_id,
        &requirement.latest_version_id,
    )
    .expect("legacy command consumes current exact version");
    assert_eq!(recorded.firm_library_version_id, requirement.latest_version_id);
    assert_eq!(recorded.definition_hash, requirement.latest_definition_hash);
    let after_record = fixture.counts();
    assert!(persistence::create_statutory_compliance_requirement(
        &fixture.path,
        &fixture.engagement_id,
        &requirement.latest_version_id,
    )
    .is_err());
    assert_eq!(fixture.counts(), after_record);

    let later = persistence::publish_firm_library_version(
        &fixture.path,
        &requirement.firm_library_item_id,
        r#"{"text":"later methodology is not retroactive"}"#,
    )
    .expect("publish later library version");
    assert_ne!(later.latest_version_id, requirement.latest_version_id);
    let existing = persistence::list_statutory_compliance_requirements(
        &fixture.path,
        &fixture.engagement_id,
    )
    .expect("read pinned requirement");
    assert_eq!(existing.len(), 1);
    assert_eq!(existing[0].firm_library_version_id, requirement.latest_version_id);
    assert_eq!(existing[0].definition_hash, requirement.latest_definition_hash);
    assert_eq!(existing[0].definition_json, r#"{"text":"initial statutory methodology"}"#);
    let now = fixture.counts();
    assert_eq!(now[2], after_record[2]);
    assert_eq!(now[3], after_record[3]);
    // Neither a stored exact version nor a recorded audit authenticates a human.
}

#[test]
fn legacy_statutory_consumer_only_checks_claimed_hash_length_not_recomputed_digest() {
    let fixture = FirmLibraryFixture::new();
    let item = fixture.requirement(r#"{"text":"genuine first version"}"#);
    let new_version_id = Uuid::new_v4().to_string();
    let forged_digest = vec![0xA5_u8; 32];
    let forged_definition = r#"{"text":"synthetic database-tamper input"}"#;
    assert_ne!(
        Sha256::digest(forged_definition.as_bytes()).to_vec(),
        forged_digest
    );

    // This is a NEW synthetic row injected only in disposable SQLite.
    // Existing native publish always computes its own digest, so this is
    // NOT a demonstrated registered-Tauri injection or ordinary UI vector.
    let db = Connection::open(&fixture.path).expect("open disposable fixture");
    db.execute(
        "INSERT INTO firm_library_versions (
            firm_library_version_id, firm_library_item_id, version_number,
            definition_json, definition_hash, created_at_ms
         ) VALUES (?1, ?2, 2, ?3, ?4, 2)",
        rusqlite::params![
            &new_version_id,
            &item.firm_library_item_id,
            forged_definition,
            &forged_digest
        ],
    )
    .expect("seed intentionally inconsistent new fixture version");
    drop(db);
    let before = fixture.counts();

    let recorded = persistence::create_statutory_compliance_requirement(
        &fixture.path,
        &fixture.engagement_id,
        &new_version_id,
    )
    .expect("current downstream verifies hash byte length, not recomputed digest");
    assert_eq!(recorded.firm_library_version_id, new_version_id);
    assert_eq!(recorded.definition_json, forged_definition);
    assert_eq!(recorded.definition_hash, forged_digest);
    let mut expected = before;
    expected[2] += 1;
    expected[3] += 1;
    expected[4] += 1;
    assert_eq!(fixture.counts(), expected);
    let initial = persistence::list_firm_library_versions(
        &fixture.path,
        &item.firm_library_item_id,
    )
    .expect("original historic version still exists");
    assert_eq!(initial[1].firm_library_version_id, item.latest_version_id);
    assert_eq!(initial[1].definition_hash, item.latest_definition_hash);
}
