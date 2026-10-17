//! Regression-only tests of the ACTUAL native persistence create/read paths.
//!
//! The synthetic Node 22 experimental node:sqlite TEXT getter truncates at
//! embedded NUL. Do not apply that driver limitation to Rust/rusqlite without
//! running these native, real-migration tests on Linux AND Windows.
//!
//! These tests DO NOT assert that the legacy revision/sign-off commands are
//! authorized, that revisions copy exact evidence, or that a trusted Save
//! flow exists. In particular, signer labels remain caller-supplied in the
//! legacy command; a test fixture's PREPARED row is not professional approval.

use crate::persistence::{
    create_workpaper_revision, create_workpaper_signoff, initialize_database,
    list_workpaper_revisions, list_workpaper_signoffs, NewWorkpaperRevision,
    NewWorkpaperSignoff, WorkpaperRevisionRecord,
};
use rusqlite::Connection;
use std::path::PathBuf;
use uuid::Uuid;

struct NativeRevisionFixture {
    root: PathBuf,
    path: PathBuf,
}

impl NativeRevisionFixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "professional-docx-native-revision-{}",
            Uuid::new_v4()
        ));
        let path = root.join("metadata.sqlite");
        initialize_database(&path).expect("apply actual production migrations");

        // Only the minimal parent records needed to call the real production
        // persistence functions. Never create or modify user workpapers.
        let connection = Connection::open(&path).expect("open isolated fixture");
        connection
            .execute_batch(
                "INSERT INTO clients (client_id, name, created_at_ms)
                   VALUES ('client-native', 'Test client', 1);
                 INSERT INTO service_types
                   (service_type_id, name, normalized_name, created_at_ms)
                   VALUES ('service-native', 'Audit', 'audit', 1);
                 INSERT INTO engagements
                   (engagement_id, client_id, service_type_id, name, status, created_at_ms)
                   VALUES ('engagement-native', 'client-native', 'service-native',
                           'Test engagement', 'ACTIVE', 1);
                 INSERT INTO workpapers
                   (workpaper_id, engagement_id, reference, title, workflow_state, created_at_ms)
                   VALUES ('workpaper-native', 'engagement-native', 'WP-01',
                           'Canonical UTF-8 regression', 'IN_PROGRESS', 1);",
            )
            .expect("seed native test workpaper");
        Self { root, path }
    }

    fn insert(&self, fields: [&str; 7]) -> WorkpaperRevisionRecord {
        create_workpaper_revision(
            &self.path,
            "workpaper-native",
            NewWorkpaperRevision {
                revision_reason: Some("Native UTF-8 round-trip regression"),
                objective: fields[0],
                procedure_performed: fields[1],
                population: fields[2],
                sample: fields[3],
                exceptions: fields[4],
                management_explanation: fields[5],
                conclusion: fields[6],
            },
        )
        .expect("legacy native revision function should persist exact test input")
    }

    fn rows(&self) -> Vec<WorkpaperRevisionRecord> {
        list_workpaper_revisions(&self.path, "workpaper-native")
            .expect("read revisions with real rusqlite TEXT accessors")
    }
}

impl Drop for NativeRevisionFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn as_fields(row: &WorkpaperRevisionRecord) -> [&str; 7] {
    [
        &row.objective,
        &row.procedure_performed,
        &row.population,
        &row.sample,
        &row.exceptions,
        &row.management_explanation,
        &row.conclusion,
    ]
}

fn uppercase_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02X}")).collect()
}

#[test]
fn actual_rust_sqlite_roundtrips_nul_unicode_blank_and_whitespace_in_all_fields() {
    let fixture = NativeRevisionFixture::new();
    let fields = [
        "Objective\u{0000}after\r\n🔎",
        "e\u{0301} é",
        "",
        "\r\n",
        "line 1\nline 2\u{0000}",
        "A\u{00a0} ",
        "Conclusion 😀",
    ];
    let created = fixture.insert(fields);
    let listed = fixture.rows();
    assert_eq!(listed.len(), 1);
    assert_eq!(created.workpaper_revision_id, listed[0].workpaper_revision_id);
    assert_eq!(as_fields(&created), fields);
    assert_eq!(as_fields(&listed[0]), fields);
    assert_eq!(listed[0].revision_number, 1);
    assert_eq!(listed[0].supersedes_revision_id, None);

    // Independent Python-calculated SHA-256 over the CURRENT production Rust
    // framing: [u64-LE length in UTF-8 bytes][UTF-8 text], seven times.
    const GOLDEN: &str = "ca435f9ed0145d2438ee59d144f0ddc744a1d70c17769abbba455e058f42dd95";
    let hash = created.content_hash.expect("actual write computes digest");
    assert_eq!(hash.len(), 32);
    assert_eq!(uppercase_hex(&hash), GOLDEN.to_uppercase());
    assert_eq!(listed[0].content_hash.as_deref(), Some(hash.as_slice()));

    // Query SQLite's raw byte representation, not SQLite length(TEXT),
    // which stops at embedded NUL by documented SQLite string semantics.
    let connection = Connection::open(&fixture.path).expect("inspect SQLite bytes");
    let (objective_hex, exception_hex, bytes): (String, String, i64) = connection
        .query_row(
            "SELECT hex(objective), hex(exceptions),
                    length(CAST(objective AS BLOB))
             FROM workpaper_revisions
             WHERE workpaper_revision_id=?1",
            [&listed[0].workpaper_revision_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("inspect immutable source bytes");
    assert_eq!(objective_hex, uppercase_hex(fields[0].as_bytes()));
    assert_eq!(exception_hex, uppercase_hex(fields[4].as_bytes()));
    assert_eq!(bytes as usize, fields[0].len());
}

#[test]
fn actual_rust_sqlite_handles_embedded_nul_at_start_middle_end_of_each_canonical_field() {
    let fixture = NativeRevisionFixture::new();
    let fields = [
        "\u{0000}a",
        "a\u{0000}z",
        "a\u{0000}",
        "\u{0000}",
        "\u{0000}\u{0000}",
        " \u{0000}\r\n ",
        "🔎\u{0000}é\u{0000}",
    ];
    let result = fixture.insert(fields);
    assert_eq!(as_fields(&result), fields);
    assert_eq!(as_fields(&fixture.rows()[0]), fields);
    let connection = Connection::open(&fixture.path).expect("inspect real SQLite bytes");
    for (column, value) in [
        ("objective", fields[0]),
        ("procedure_performed", fields[1]),
        ("population", fields[2]),
        ("sample", fields[3]),
        ("exceptions", fields[4]),
        ("management_explanation", fields[5]),
        ("conclusion", fields[6]),
    ] {
        let query = format!("SELECT hex({column}) FROM workpaper_revisions LIMIT 1");
        let stored: String = connection
            .query_row(&query, [], |row| row.get(0))
            .expect("read exact source byte hex");
        assert_eq!(stored, uppercase_hex(value.as_bytes()), "{column}");
    }
}

#[test]
fn native_one_character_correction_preserves_old_text_and_signed_history() {
    let fixture = NativeRevisionFixture::new();
    let base_fields = [
        "Objective\u{0000}after\r\n🔎",
        "e\u{0301} é",
        "",
        "\r\n",
        "line 1\nline 2\u{0000}",
        "A\u{00a0} ",
        "Conclusion 😀",
    ];
    let first = fixture.insert(base_fields);

    // Exercise today's real persistence sign-off/supersession behavior only.
    // Its actor ID/role are fixture data, NOT a verified native identity.
    let signed = create_workpaper_signoff(
        &fixture.path,
        NewWorkpaperSignoff {
            workpaper_id: "workpaper-native",
            workpaper_revision_id: &first.workpaper_revision_id,
            signoff_type: "PREPARED",
            actor_id: "untrusted-fixture",
            actor_role: "PREPARER",
            comment: Some("Local legacy API semantics only"),
        },
    )
    .expect("create a synthetic legacy PREPARED sign-off");

    let mut revised_fields = base_fields;
    revised_fields[6] = "Conclusion 😀!";
    let next = fixture.insert(revised_fields);

    assert_ne!(first.workpaper_revision_id, next.workpaper_revision_id);
    assert_eq!(next.revision_number, 2);
    assert_eq!(
        next.supersedes_revision_id.as_deref(),
        Some(first.workpaper_revision_id.as_str())
    );
    assert_eq!(as_fields(&next), revised_fields);
    assert_ne!(next.content_hash, first.content_hash);

    let rows = fixture.rows();
    assert_eq!(rows.len(), 2);
    assert_eq!(as_fields(&rows[0]), revised_fields);
    assert_eq!(as_fields(&rows[1]), base_fields);
    assert_eq!(rows[1].workpaper_revision_id, first.workpaper_revision_id);

    let signoffs = list_workpaper_signoffs(&fixture.path, "workpaper-native")
        .expect("read exact legacy sign-off history");
    assert_eq!(signoffs.len(), 1);
    assert_eq!(signoffs[0].signoff_id, signed.signoff_id);
    assert_eq!(
        signoffs[0].workpaper_revision_id,
        first.workpaper_revision_id
    );
    assert_eq!(
        signoffs[0].superseded_by_revision_id.as_deref(),
        Some(next.workpaper_revision_id.as_str())
    );
    assert!(signoffs[0].evidence_link_ids.is_empty());
    assert!(
        signoffs
            .iter()
            .all(|signoff| signoff.workpaper_revision_id != next.workpaper_revision_id)
    );
}
