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
    list_workpaper_revisions, list_workpaper_signoffs, NewWorkpaperRevision, NewWorkpaperSignoff,
    WorkpaperRevisionRecord,
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
    assert_eq!(
        created.workpaper_revision_id,
        listed[0].workpaper_revision_id
    );
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
    assert!(signoffs
        .iter()
        .all(|signoff| signoff.workpaper_revision_id != next.workpaper_revision_id));
}

#[test]
fn native_immutable_revision_and_signature_history_rejects_sql_updates_and_deletes() {
    let fixture = NativeRevisionFixture::new();
    let fields = [
        "Original objective\u{0000}",
        "Original procedure",
        "All items",
        "",
        "",
        "Response\u{00a0}",
        "Signed conclusion",
    ];
    let signed_revision = fixture.insert(fields);
    let signed = create_workpaper_signoff(
        &fixture.path,
        NewWorkpaperSignoff {
            workpaper_id: "workpaper-native",
            workpaper_revision_id: &signed_revision.workpaper_revision_id,
            signoff_type: "PREPARED",
            actor_id: "untrusted-fixture",
            actor_role: "PREPARER",
            comment: None,
        },
    )
    .expect("create test-only legacy sign-off");
    let corrected = fixture.insert([
        "Original objective\u{0000}",
        "Original procedure",
        "All items",
        "",
        "",
        "Response\u{00a0}",
        "Signed conclusion!",
    ]);

    // Production migration triggers, not a synthetic mock, prevent writes
    // against already signed immutable rows even from direct SQL.
    let connection = Connection::open(&fixture.path).expect("open native SQLite");
    for (sql, id) in [
        (
            "UPDATE workpaper_revisions SET conclusion='forged' WHERE workpaper_revision_id=?1",
            signed_revision.workpaper_revision_id.as_str(),
        ),
        (
            "DELETE FROM workpaper_revisions WHERE workpaper_revision_id=?1",
            signed_revision.workpaper_revision_id.as_str(),
        ),
        (
            "UPDATE workpaper_signoffs SET actor_id='forged' WHERE signoff_id=?1",
            signed.signoff_id.as_str(),
        ),
        (
            "DELETE FROM workpaper_signoffs WHERE signoff_id=?1",
            signed.signoff_id.as_str(),
        ),
        (
            "UPDATE workpaper_signoff_supersessions SET superseded_by_revision_id='forged' WHERE signoff_id=?1",
            signed.signoff_id.as_str(),
        ),
        (
            "DELETE FROM workpaper_signoff_supersessions WHERE signoff_id=?1",
            signed.signoff_id.as_str(),
        ),
    ] {
        let error = connection
            .execute(sql, [id])
            .expect_err("immutable history SQL must fail");
        assert!(
            error.to_string().contains("immutable"),
            "unexpected direct-SQL denial for {sql}: {error}"
        );
    }
    assert_eq!(fixture.rows().len(), 2);
    assert_eq!(as_fields(&fixture.rows()[1]), fields);
    let signoffs = list_workpaper_signoffs(&fixture.path, "workpaper-native")
        .expect("historical sign-off still visible");
    assert_eq!(signoffs.len(), 1);
    assert_eq!(signoffs[0].actor_id, "untrusted-fixture");
    assert_eq!(
        signoffs[0].superseded_by_revision_id.as_deref(),
        Some(corrected.workpaper_revision_id.as_str())
    );
}

#[test]
fn native_signoff_rejects_stale_exact_revision_without_creating_history() {
    let fixture = NativeRevisionFixture::new();
    let first = fixture.insert(["a", "b", "c", "d", "e", "f", "g"]);
    let latest = fixture.insert(["a", "b", "c", "d", "e", "f", "h"]);
    let error = create_workpaper_signoff(
        &fixture.path,
        NewWorkpaperSignoff {
            workpaper_id: "workpaper-native",
            workpaper_revision_id: &first.workpaper_revision_id,
            signoff_type: "PREPARED",
            actor_id: "untrusted-fixture",
            actor_role: "PREPARER",
            comment: None,
        },
    )
    .expect_err("historical revision cannot receive a fresh legacy sign-off");
    assert!(error.to_string().contains("latest workpaper revision"));
    assert!(list_workpaper_signoffs(&fixture.path, "workpaper-native")
        .expect("check sign-off rows")
        .is_empty());
    assert_eq!(
        fixture.rows()[0].workpaper_revision_id,
        latest.workpaper_revision_id
    );
}

#[test]
fn native_third_revision_preserves_original_signoff_and_immediate_supersession() {
    let fixture = NativeRevisionFixture::new();
    let original = [
        "ø",
        "a\u{0000}b",
        "Population",
        "",
        "Exception",
        "",
        "Original",
    ];
    let first = fixture.insert(original);
    let signoff = create_workpaper_signoff(
        &fixture.path,
        NewWorkpaperSignoff {
            workpaper_id: "workpaper-native",
            workpaper_revision_id: &first.workpaper_revision_id,
            signoff_type: "PREPARED",
            actor_id: "untrusted-fixture",
            actor_role: "PREPARER",
            comment: None,
        },
    )
    .expect("test fixture sign-off");
    let second = fixture.insert([
        "ø",
        "a\u{0000}b",
        "Population",
        "",
        "Exception",
        "",
        "Second",
    ]);
    let third = fixture.insert([
        "ø",
        "a\u{0000}b",
        "Population",
        "",
        "Exception",
        "",
        "Third",
    ]);

    let rows = fixture.rows();
    assert_eq!(rows.len(), 3);
    assert_eq!(
        [
            rows[0].revision_number,
            rows[1].revision_number,
            rows[2].revision_number
        ],
        [3, 2, 1]
    );
    assert_eq!(rows[0].workpaper_revision_id, third.workpaper_revision_id);
    assert_eq!(
        rows[0].supersedes_revision_id.as_deref(),
        Some(second.workpaper_revision_id.as_str())
    );
    assert_eq!(
        rows[1].supersedes_revision_id.as_deref(),
        Some(first.workpaper_revision_id.as_str())
    );
    assert_eq!(as_fields(&rows[2]), original);

    let signoffs = list_workpaper_signoffs(&fixture.path, "workpaper-native")
        .expect("historical signed predecessor still visible");
    assert_eq!(signoffs.len(), 1);
    assert_eq!(signoffs[0].signoff_id, signoff.signoff_id);
    assert_eq!(
        signoffs[0].workpaper_revision_id,
        first.workpaper_revision_id
    );
    assert_eq!(
        signoffs[0].superseded_by_revision_id.as_deref(),
        Some(second.workpaper_revision_id.as_str())
    );
    assert!(signoffs.iter().all(|entry| {
        entry.workpaper_revision_id != second.workpaper_revision_id
            && entry.workpaper_revision_id != third.workpaper_revision_id
    }));
}

/// DOCUMENTED UNSAFE LEGACY BEHAVIOR, NOT A DESIRED ACCEPTANCE CONTRACT.
///
/// The existing API accepts a complete client-supplied seven-field snapshot,
/// but has NO expected_latest_revision_id parameter. Two editors who
/// started from the same predecessor can therefore serially append different
/// successor revisions: the second writes an obsolete whole-document image.
/// The old rows remain immutable, but the last/current revision loses the
/// first editor's changes. Replace this characterization test when native
/// CAS is introduced under reviewed DEC-03/04/06/08 authorization.
#[test]
fn legacy_native_api_currently_accepts_stale_full_snapshot_without_base_cas() {
    let fixture = NativeRevisionFixture::new();
    let original = [
        "Objective",
        "Procedure",
        "Population A",
        "Sample",
        "",
        "",
        "Conclusion A",
    ];
    let base = fixture.insert(original);

    // Two independent caller snapshots conceptually observe the same base.
    let mut first_edit = original;
    first_edit[2] = "Population B: another author updated this";
    let mut stale_edit = original;
    stale_edit[6] = "Conclusion B: stale author's correction";

    let current = fixture.insert(first_edit);
    let stale_accepted = fixture.insert(stale_edit);
    let rows = fixture.rows();

    assert_eq!(rows.len(), 3);
    assert_eq!(
        rows[0].workpaper_revision_id,
        stale_accepted.workpaper_revision_id
    );
    assert_eq!(
        rows[0].supersedes_revision_id.as_deref(),
        Some(current.workpaper_revision_id.as_str())
    );
    assert_eq!(as_fields(&rows[0]), stale_edit);
    assert_eq!(rows[0].population, original[2]);
    assert_ne!(rows[0].population, current.population);
    assert_eq!(rows[1].population, first_edit[2]);
    assert_eq!(rows[2].workpaper_revision_id, base.workpaper_revision_id);
    // This is evidence of a CURRENT GAP, not evidence of safe concurrency.
}

/// DOCUMENTED UNSAFE LEGACY BEHAVIOR, NOT AN AUTHORIZATION TEST.
///
/// Caller-chosen actor identity and role pass through this legacy API into
/// an immutable PREPARED row without establishing a trusted person/session.
/// The historical row is durable but does not authenticate the alleged actor.
/// This characterization MUST change when a native identity issuer is built.
#[test]
fn legacy_native_signoff_currently_records_caller_asserted_identity_and_role() {
    let fixture = NativeRevisionFixture::new();
    let revision = fixture.insert(["a", "b", "c", "d", "e", "f", "g"]);
    let claimed = create_workpaper_signoff(
        &fixture.path,
        NewWorkpaperSignoff {
            workpaper_id: "workpaper-native",
            workpaper_revision_id: &revision.workpaper_revision_id,
            signoff_type: "PREPARED",
            actor_id: "arbitrary-caller-claimed-person",
            actor_role: "ARBITRARY_CALLER_CLAIMED_ROLE",
            comment: Some("Unverified fixture claims are not human approval"),
        },
    )
    .expect("legacy command presently accepts syntactically valid asserted labels");

    let rows = list_workpaper_signoffs(&fixture.path, "workpaper-native")
        .expect("read persisted legacy asserted labels");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].signoff_id, claimed.signoff_id);
    assert_eq!(rows[0].actor_id, "arbitrary-caller-claimed-person");
    assert_eq!(rows[0].actor_role, "ARBITRARY_CALLER_CLAIMED_ROLE");
    assert_eq!(
        rows[0].workpaper_revision_id,
        revision.workpaper_revision_id
    );
    // Existence of the row provides NO evidence of verified signer authority.
}

/// DOCUMENTED UNRESOLVED DEC-08 POLICY DIFFERENCE, NOT DESIRED NO-OP SEMANTICS.
///
/// The actual legacy create function currently appends even byte-identical
/// canonical text and supersedes prior PREPARED history. Draft policy PR #136
/// proposes a no-op on fully unchanged content + evidence + review state.
/// The production choice is an open human decision. Do NOT implement a
/// content-only shortcut here or automatically transfer sign-off.
#[test]
fn legacy_native_api_currently_appends_identical_text_and_supersedes_signoff() {
    let fixture = NativeRevisionFixture::new();
    let fields = [
        "A\u{0000}B",
        "e\u{0301}",
        "Population",
        "",
        "Exception\r\n",
        "Response\u{00a0}",
        "Conclusion",
    ];
    let first = fixture.insert(fields);
    let signed = create_workpaper_signoff(
        &fixture.path,
        NewWorkpaperSignoff {
            workpaper_id: "workpaper-native",
            workpaper_revision_id: &first.workpaper_revision_id,
            signoff_type: "PREPARED",
            actor_id: "untrusted-fixture",
            actor_role: "PREPARER",
            comment: None,
        },
    )
    .expect("fixture signoff for legacy no-op characterization");
    let duplicate = fixture.insert(fields);
    assert_ne!(duplicate.workpaper_revision_id, first.workpaper_revision_id);
    assert_eq!(duplicate.revision_number, 2);
    assert_eq!(duplicate.content_hash, first.content_hash);
    assert_eq!(as_fields(&duplicate), fields);
    assert_eq!(as_fields(&fixture.rows()[1]), fields);

    let history = list_workpaper_signoffs(&fixture.path, "workpaper-native")
        .expect("read immutable legacy signoff after identical revision");
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].signoff_id, signed.signoff_id);
    assert_eq!(
        history[0].superseded_by_revision_id.as_deref(),
        Some(duplicate.workpaper_revision_id.as_str())
    );
    assert!(history
        .iter()
        .all(|entry| entry.workpaper_revision_id != duplicate.workpaper_revision_id));
}
