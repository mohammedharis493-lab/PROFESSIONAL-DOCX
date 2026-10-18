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
    create_initial_workpaper_revision, create_workpaper_evidence_link, create_workpaper_revision,
    create_workpaper_signoff, initialize_database, list_workpaper_evidence_links,
    list_workpaper_revisions, list_workpaper_signoffs, NewWorkpaperRevision, NewWorkpaperSignoff,
    WorkpaperRevisionRecord,
};
use crate::workpaper_revision_successor_candidate::{
    candidate_successor_transaction, CandidateObservedEvidenceLink, CandidateObservedSnapshot,
    DenyAllCandidateAuthority, FixtureOnlyCandidateAuthority,
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

    /// Only test data: linked metadata/version, NOT a captured immutable
    /// controlled evidence file. No source filesystem bytes are trusted.
    fn seed_unverified_linked_content_version(&self) {
        let connection = Connection::open(&self.path).expect("open isolated database");
        connection
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                 INSERT INTO documents
                   (document_id, storage_state, display_name, created_at_ms)
                   VALUES ('doc-native', 'LINKED', 'Observation.txt', 1);
                 INSERT INTO storage_roots
                   (storage_root_id, kind, native_locator, native_locator_encoding,
                    display_locator, availability_state, approved_at_ms, created_at_ms,
                    updated_at_ms)
                   VALUES ('root-native', 'LOCAL', X'2F', 'UNIX', '/fixture-only',
                           'AVAILABLE', 1, 1, 1);
                 INSERT INTO file_instances
                   (file_instance_id, document_id, storage_root_id,
                    relative_path_native, path_native_encoding,
                    relative_path_display, relative_path_search, size_bytes,
                    first_seen_at_ms, last_seen_at_ms, availability_state)
                   VALUES ('instance-native', 'doc-native', 'root-native',
                           X'61', 'UNIX', 'Observation.txt', 'observation.txt', 5,
                           1, 1, 'AVAILABLE');
                 INSERT INTO content_versions
                   (content_version_id, document_id, file_instance_id,
                    observed_at_ms, size_bytes, verification_state)
                   VALUES ('version-native', 'doc-native', 'instance-native',
                           1, 5, 'METADATA_ONLY');",
            )
            .expect("seed test-only linked version with actual migration tables");
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

/// DOCUMENTED LEGACY GAP: the real create revision path does NOT inherit or
/// carry forward a previously signed revision's exact evidence-link set.
/// The prior signed link and immutable snapshot survive, but the successor
/// has zero links even for a one-character change. A future trusted Save
/// requires separately approved atomic unchanged-version carry-forward.
#[test]
fn legacy_native_revision_does_not_carry_signed_exact_evidence_to_successor() {
    let fixture = NativeRevisionFixture::new();
    fixture.seed_unverified_linked_content_version();
    let original = fixture.insert([
        "Objective",
        "Procedure",
        "Population",
        "Sample",
        "",
        "",
        "Signed conclusion",
    ]);
    let link = create_workpaper_evidence_link(
        &fixture.path,
        &original.workpaper_revision_id,
        "doc-native",
        Some("version-native"),
        None,
        "SUPPORTING",
        Some("Exact observed content version"),
    )
    .expect("attach test-only exact linked-content version before signing");
    let signed = create_workpaper_signoff(
        &fixture.path,
        NewWorkpaperSignoff {
            workpaper_id: "workpaper-native",
            workpaper_revision_id: &original.workpaper_revision_id,
            signoff_type: "PREPARED",
            actor_id: "untrusted-fixture",
            actor_role: "PREPARER",
            comment: None,
        },
    )
    .expect("legacy PREPARED fixture sign-off; not an authenticated reviewer");

    assert_eq!(
        signed.evidence_link_ids,
        vec![link.evidence_link_id.clone()]
    );

    let updated = fixture.insert([
        "Objective",
        "Procedure",
        "Population",
        "Sample",
        "",
        "",
        "Signed conclusion.",
    ]);
    let historical = list_workpaper_evidence_links(&fixture.path, &original.workpaper_revision_id)
        .expect("original historical evidence remains visible");
    let successor = list_workpaper_evidence_links(&fixture.path, &updated.workpaper_revision_id)
        .expect("new revision's independent evidence links");
    assert_eq!(historical.len(), 1);
    assert_eq!(historical[0].evidence_link_id, link.evidence_link_id);
    assert_eq!(
        historical[0].content_version_id.as_deref(),
        Some("version-native")
    );
    assert_eq!(
        historical[0].description.as_deref(),
        Some("Exact observed content version")
    );
    assert!(
        successor.is_empty(),
        "legacy revision creates no new evidence links"
    );

    let signoffs = list_workpaper_signoffs(&fixture.path, "workpaper-native")
        .expect("read historical signoff and exact source snapshot");
    assert_eq!(signoffs.len(), 1);
    assert_eq!(signoffs[0].signoff_id, signed.signoff_id);
    assert_eq!(
        signoffs[0].evidence_link_ids,
        vec![link.evidence_link_id.clone()]
    );
    assert_eq!(
        signoffs[0].superseded_by_revision_id.as_deref(),
        Some(updated.workpaper_revision_id.as_str())
    );
    assert_eq!(
        signoffs[0].workpaper_revision_id,
        original.workpaper_revision_id
    );
    // This is a real missing carry-forward characteristic, NOT safe
    // immutable evidence custody or authorization to copy on the new Save.
}

/// Assert that the production SQL triggers freeze the exact source evidence
/// link and sign-off snapshot, and that the API refuses attaching evidence
/// to either an old signed revision or a newer actively signed revision.
#[test]
fn native_signed_evidence_snapshot_is_immutable_and_late_attachment_is_rejected() {
    let fixture = NativeRevisionFixture::new();
    fixture.seed_unverified_linked_content_version();
    let original = fixture.insert(["A", "B", "C", "D", "E", "F", "G"]);
    let link = create_workpaper_evidence_link(
        &fixture.path,
        &original.workpaper_revision_id,
        "doc-native",
        Some("version-native"),
        None,
        "SUPPORTING",
        Some("Before PREPARED sign-off"),
    )
    .expect("fixture exact observed-version link");
    let signed = create_workpaper_signoff(
        &fixture.path,
        NewWorkpaperSignoff {
            workpaper_id: "workpaper-native",
            workpaper_revision_id: &original.workpaper_revision_id,
            signoff_type: "PREPARED",
            actor_id: "untrusted-fixture",
            actor_role: "PREPARER",
            comment: None,
        },
    )
    .expect("sign latest revision for fixture only");

    let too_late = create_workpaper_evidence_link(
        &fixture.path,
        &original.workpaper_revision_id,
        "doc-native",
        Some("version-native"),
        None,
        "OTHER",
        None,
    )
    .expect_err("actively signed latest revision may not acquire another link");
    assert!(too_late.to_string().contains("signed workpaper revision"));

    let connection = Connection::open(&fixture.path).expect("inspect migrated triggers");
    let snapshot_id: String = connection
        .query_row(
            "SELECT signoff_evidence_id
             FROM workpaper_signoff_evidence WHERE signoff_id=?1",
            [&signed.signoff_id],
            |row| row.get(0),
        )
        .expect("sign-off includes exact link snapshot");
    for (sql, id) in [
        (
            "UPDATE workpaper_evidence_links SET description='tampered' WHERE evidence_link_id=?1",
            link.evidence_link_id.as_str(),
        ),
        (
            "DELETE FROM workpaper_evidence_links WHERE evidence_link_id=?1",
            link.evidence_link_id.as_str(),
        ),
        (
            "UPDATE workpaper_signoff_evidence SET evidence_link_id='tampered' WHERE signoff_evidence_id=?1",
            snapshot_id.as_str(),
        ),
        (
            "DELETE FROM workpaper_signoff_evidence WHERE signoff_evidence_id=?1",
            snapshot_id.as_str(),
        ),
    ] {
        let failure = connection
            .execute(sql, [id])
            .expect_err("native immutable SQL evidence history must reject updates");
        assert!(
            failure.to_string().contains("immutable"),
            "unexpected SQL evidence mutation result {sql}: {failure}"
        );
    }
    let successor = fixture.insert(["A", "B", "C", "D", "E", "F", "H"]);
    let historical_add = create_workpaper_evidence_link(
        &fixture.path,
        &original.workpaper_revision_id,
        "doc-native",
        Some("version-native"),
        None,
        "OTHER",
        None,
    )
    .expect_err("superseded predecessor may not acquire evidence");
    assert!(historical_add
        .to_string()
        .contains("latest workpaper revision"));
    assert_eq!(
        list_workpaper_evidence_links(&fixture.path, &original.workpaper_revision_id)
            .expect("history still visible")
            .len(),
        1
    );
    assert!(
        list_workpaper_evidence_links(&fixture.path, &successor.workpaper_revision_id)
            .expect("successor is initially independent")
            .is_empty()
    );
    assert_eq!(
        list_workpaper_signoffs(&fixture.path, "workpaper-native")
            .expect("frozen sign-off link snapshot remains")[0]
            .evidence_link_ids,
        vec![link.evidence_link_id]
    );
}

/// Test-only reference, never a Tauri command, production grant or custody
/// verifier. This demonstrates SQLite atomicity with actual migrations, NOT
/// that arbitrary metadata-only links may be copied in a real engagement.
/// Production must supply DEC-03/04 authenticated authority, DEC-05 custody
/// and DEC-08 carry-forward approval before any implementation.
// Thin adapter: existing real-migration regression tests now exercise the
// parameterized candidate, instead of a copied transaction in this test file.
// Test-only opening state maps native DTO metadata to an exact comparison
// tuple; this is not verification of physical evidence bytes or authority.
fn fixture_observed_links(
    path: &std::path::Path,
    revision_id: &str,
    observed_ids: &[String],
) -> Result<Vec<CandidateObservedEvidenceLink>, String> {
    let stored =
        list_workpaper_evidence_links(path, revision_id).map_err(|error| error.to_string())?;
    Ok(observed_ids
        .iter()
        .filter_map(|id| stored.iter().find(|row| row.evidence_link_id == *id))
        .map(|link| CandidateObservedEvidenceLink {
            id: link.evidence_link_id.clone(),
            document_id: link.document_id.clone(),
            content_version_id: link.content_version_id.clone(),
            controlled_version_id: link.controlled_evidence_version_id.clone(),
            relationship_type: link.relationship_type.clone(),
            description: link.description.clone(),
        })
        .collect())
}

fn fixture_observed_active_signoffs(
    path: &std::path::Path,
    revision_id: &str,
) -> Result<Vec<String>, String> {
    let mut ids: Vec<String> = list_workpaper_signoffs(path, "workpaper-native")
        .map_err(|error| error.to_string())?
        .into_iter()
        .filter(|signoff| {
            signoff.workpaper_revision_id == revision_id
                && signoff.superseded_by_revision_id.is_none()
        })
        .map(|signoff| signoff.signoff_id)
        .collect();
    ids.sort();
    Ok(ids)
}

fn test_only_checked_successor(
    path: &std::path::Path,
    expected_base: &str,
    observed_link_ids: &[String],
    fields: [&str; 7],
    fail_after_first_copied_link: bool,
) -> Result<String, String> {
    let observed_links = fixture_observed_links(path, expected_base, observed_link_ids)?;
    let observed_signoffs = fixture_observed_active_signoffs(path, expected_base)?;
    candidate_successor_transaction(
        path,
        "workpaper-native",
        expected_base,
        CandidateObservedSnapshot {
            evidence: &observed_links,
            active_signoff_ids: &observed_signoffs,
        },
        fields,
        fail_after_first_copied_link,
        &FixtureOnlyCandidateAuthority,
    )
}

fn test_only_signed_two_link_base() -> (
    NativeRevisionFixture,
    WorkpaperRevisionRecord,
    String,
    Vec<String>,
) {
    let fixture = NativeRevisionFixture::new();
    fixture.seed_unverified_linked_content_version();
    let base = fixture.insert([
        "Original\u{0000} objective",
        "Trace",
        "Full population",
        "Sample",
        "",
        "",
        "Signed conclusion",
    ]);
    let first = create_workpaper_evidence_link(
        &fixture.path,
        &base.workpaper_revision_id,
        "doc-native",
        Some("version-native"),
        None,
        "SUPPORTING",
        Some("Exact version \u{00a0}"),
    )
    .expect("first fixture exact version");
    let second = create_workpaper_evidence_link(
        &fixture.path,
        &base.workpaper_revision_id,
        "doc-native",
        Some("version-native"),
        None,
        "CORROBORATING",
        Some("A different relationship"),
    )
    .expect("second fixture exact version");
    let signed = create_workpaper_signoff(
        &fixture.path,
        NewWorkpaperSignoff {
            workpaper_id: "workpaper-native",
            workpaper_revision_id: &base.workpaper_revision_id,
            signoff_type: "PREPARED",
            actor_id: "unverified-test-label",
            actor_role: "PREPARER",
            comment: None,
        },
    )
    .expect("test-only PREPARED label");
    let mut ids = vec![first.evidence_link_id, second.evidence_link_id];
    ids.sort();
    (fixture, base, signed.signoff_id, ids)
}

#[test]
fn reference_native_transaction_copies_exact_link_tuples_and_keeps_signoff_historical() {
    let (fixture, base, signoff_id, opened_links) = test_only_signed_two_link_base();
    let fields = [
        "Original\u{0000} objective",
        "Trace",
        "Full population",
        "Sample",
        "",
        "",
        "Signed conclusion.",
    ];
    let successor_id = test_only_checked_successor(
        &fixture.path,
        &base.workpaper_revision_id,
        &opened_links,
        fields,
        false,
    )
    .expect("test-only SQLite reference transaction");

    let previous = list_workpaper_evidence_links(&fixture.path, &base.workpaper_revision_id)
        .expect("load original link tuples");
    let current = list_workpaper_evidence_links(&fixture.path, &successor_id)
        .expect("load successor link tuples");
    assert_eq!(current.len(), 2);
    for new_link in &current {
        let old = previous
            .iter()
            .find(|old| old.relationship_type == new_link.relationship_type)
            .expect("same relationship must exist in predecessor");
        assert_ne!(old.evidence_link_id, new_link.evidence_link_id);
        assert_eq!(old.document_id, new_link.document_id);
        assert_eq!(old.content_version_id, new_link.content_version_id);
        assert_eq!(
            old.controlled_evidence_version_id,
            new_link.controlled_evidence_version_id
        );
        assert_eq!(old.description, new_link.description);
    }
    let revisions = fixture.rows();
    assert_eq!(revisions.len(), 2);
    assert_eq!(as_fields(&revisions[0]), fields);
    assert_eq!(
        revisions[0].supersedes_revision_id.as_deref(),
        Some(base.workpaper_revision_id.as_str())
    );
    let signoffs = list_workpaper_signoffs(&fixture.path, "workpaper-native")
        .expect("load historical sign-off");
    assert_eq!(signoffs.len(), 1);
    assert_eq!(signoffs[0].signoff_id, signoff_id);
    assert_eq!(signoffs[0].evidence_link_ids.len(), 2);
    assert_eq!(
        signoffs[0].superseded_by_revision_id.as_deref(),
        Some(successor_id.as_str())
    );
    assert!(signoffs
        .iter()
        .all(|entry| entry.workpaper_revision_id != successor_id));
}

#[test]
fn reference_native_transaction_refuses_stale_base_without_mutation() {
    let (fixture, base, _signoff_id, opened_links) = test_only_signed_two_link_base();
    let fields = [
        "Obj",
        "Procedure",
        "Population",
        "Sample",
        "",
        "",
        "New conclusion",
    ];
    let first = test_only_checked_successor(
        &fixture.path,
        &base.workpaper_revision_id,
        &opened_links,
        fields,
        false,
    )
    .expect("first test-only successor commits");
    let result = test_only_checked_successor(
        &fixture.path,
        &base.workpaper_revision_id,
        &opened_links,
        fields,
        false,
    );
    assert_eq!(result, Err("STALE_NATIVE_BASE".to_string()));
    assert_eq!(fixture.rows().len(), 2);
    assert_eq!(fixture.rows()[0].workpaper_revision_id, first);
    assert_eq!(
        list_workpaper_evidence_links(&fixture.path, &first)
            .expect("only the committed successor has links")
            .len(),
        2
    );
}

#[test]
fn reference_native_transaction_refuses_evidence_only_race_without_mutation() {
    let (fixture, base, _signoff_id, mut opened_links) = test_only_signed_two_link_base();
    // Simulate an incomplete/stale editor observation; no review or source
    // mutation needed. The actual source has two links, the opening state one.
    opened_links.pop();
    let result = test_only_checked_successor(
        &fixture.path,
        &base.workpaper_revision_id,
        &opened_links,
        ["O", "P", "Q", "R", "", "", "S"],
        false,
    );
    assert_eq!(result, Err("STALE_NATIVE_EVIDENCE".to_string()));
    assert_eq!(fixture.rows().len(), 1);
    assert_eq!(
        list_workpaper_signoffs(&fixture.path, "workpaper-native").expect("sign-off unchanged")[0]
            .superseded_by_revision_id,
        None
    );
}

#[test]
fn reference_native_transaction_rolls_back_partial_link_copy_and_supersession() {
    let (fixture, base, _signoff_id, opened_links) = test_only_signed_two_link_base();
    let result = test_only_checked_successor(
        &fixture.path,
        &base.workpaper_revision_id,
        &opened_links,
        ["O", "P", "Q", "R", "", "", "S"],
        true,
    );
    assert_eq!(result, Err("INJECTED_NATIVE_COPY_FAILURE".to_string()));
    assert_eq!(fixture.rows().len(), 1);
    let connection = Connection::open(&fixture.path).expect("inspect actual DB after rollback");
    let revision_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM workpaper_revisions", [], |row| {
            row.get(0)
        })
        .expect("count revisions");
    let link_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM workpaper_evidence_links", [], |row| {
            row.get(0)
        })
        .expect("count link rows");
    let supersessions: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM workpaper_signoff_supersessions",
            [],
            |row| row.get(0),
        )
        .expect("count supersessions");
    let fake_audits: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM audit_events WHERE event_type='TEST_ONLY_REFERENCE_SUCCESSOR'",
            [],
            |row| row.get(0),
        )
        .expect("count committed test-only successor audit events");
    assert_eq!(
        (revision_count, link_count, supersessions, fake_audits),
        (1, 2, 0, 0)
    );
}

/// Two native Rust threads use TWO INDEPENDENT rusqlite connections to the
/// same actual migrated SQLite database. Both start from one exact immutable
/// predecessor; BEGIN IMMEDIATE plus in-transaction latest-CAS must allow
/// ONE append and reject the other as stale, without inventing approval.
/// This test-only model is not the callable legacy production revision API.
#[test]
fn reference_native_two_independent_writers_only_one_can_commit() {
    use std::sync::{Arc, Barrier};

    let (fixture, base, signoff_id, opened_links) = test_only_signed_two_link_base();
    let start_together = Arc::new(Barrier::new(3));
    let mut threads = Vec::new();
    for new_conclusion in [
        "First independent correction",
        "Second independent correction",
    ] {
        let path = fixture.path.clone();
        let expected_base = base.workpaper_revision_id.clone();
        let opened = opened_links.clone();
        let ready = Arc::clone(&start_together);
        threads.push(std::thread::spawn(move || {
            ready.wait();
            test_only_checked_successor(
                &path,
                &expected_base,
                &opened,
                [
                    "Original\u{0000} objective",
                    "Trace",
                    "Full population",
                    "Sample",
                    "",
                    "",
                    new_conclusion,
                ],
                false,
            )
        }));
    }
    start_together.wait();

    let mut successful = Vec::new();
    let mut rejected = Vec::new();
    for thread in threads {
        match thread
            .join()
            .expect("native SQLite writer thread did not panic")
        {
            Ok(successor_id) => successful.push(successor_id),
            Err(error) => rejected.push(error),
        }
    }
    assert_eq!(
        successful.len(),
        1,
        "only one independent writer may commit"
    );
    assert_eq!(rejected, vec!["STALE_NATIVE_BASE".to_string()]);
    let saved = &successful[0];
    let revisions = fixture.rows();
    assert_eq!(revisions.len(), 2);
    assert_eq!(revisions[0].workpaper_revision_id, *saved);
    assert_eq!(revisions[0].revision_number, 2);
    assert_eq!(
        revisions[0].supersedes_revision_id.as_deref(),
        Some(base.workpaper_revision_id.as_str())
    );
    assert!(
        revisions[0].conclusion == "First independent correction"
            || revisions[0].conclusion == "Second independent correction"
    );
    assert_eq!(
        revisions[1].workpaper_revision_id,
        base.workpaper_revision_id
    );
    assert_eq!(
        list_workpaper_evidence_links(&fixture.path, saved)
            .expect("committed successor owns only its two fresh copied links")
            .len(),
        2
    );
    assert_eq!(
        list_workpaper_evidence_links(&fixture.path, &base.workpaper_revision_id)
            .expect("signed source evidence retained")
            .len(),
        2
    );
    let signoffs = list_workpaper_signoffs(&fixture.path, "workpaper-native")
        .expect("sign-off remains historical");
    assert_eq!(signoffs.len(), 1);
    assert_eq!(signoffs[0].signoff_id, signoff_id);
    assert_eq!(
        signoffs[0].superseded_by_revision_id.as_deref(),
        Some(saved.as_str())
    );
    let db = Connection::open(&fixture.path).expect("read commit-only audit");
    let accepted_audits: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM audit_events
             WHERE event_type='TEST_ONLY_REFERENCE_SUCCESSOR'",
            [],
            |row| row.get(0),
        )
        .expect("count committed successor events");
    assert_eq!(accepted_audits, 1);
}

/// A mid-copy fault must not leave a partially linked revision, consume a
/// unique supersession, or hold the SQLite reservation so that a clean retry
/// from the SAME base can be committed after the rollback.
#[test]
fn reference_native_partial_copy_failure_can_retry_same_base_without_leaking_rows() {
    let (fixture, base, signoff_id, opened_links) = test_only_signed_two_link_base();
    let fields = [
        "Original\u{0000} objective",
        "Trace",
        "Full population",
        "Sample",
        "",
        "",
        "Post-rollback clean correction",
    ];
    let error = test_only_checked_successor(
        &fixture.path,
        &base.workpaper_revision_id,
        &opened_links,
        fields,
        true,
    );
    assert_eq!(error, Err("INJECTED_NATIVE_COPY_FAILURE".to_string()));
    let successor_id = test_only_checked_successor(
        &fixture.path,
        &base.workpaper_revision_id,
        &opened_links,
        fields,
        false,
    )
    .expect("rolled-back write reservation and predecessor should permit a fresh retry");
    assert_eq!(fixture.rows().len(), 2);
    assert_eq!(fixture.rows()[0].workpaper_revision_id, successor_id);
    assert_eq!(as_fields(&fixture.rows()[0]), fields);

    let signoffs = list_workpaper_signoffs(&fixture.path, "workpaper-native")
        .expect("read single committed supersession");
    assert_eq!(signoffs.len(), 1);
    assert_eq!(signoffs[0].signoff_id, signoff_id);
    assert_eq!(
        signoffs[0].superseded_by_revision_id.as_deref(),
        Some(successor_id.as_str())
    );
    let db = Connection::open(&fixture.path).expect("inspect committed write after retry");
    let counts: (i64, i64, i64, i64) = db
        .query_row(
            "SELECT
                (SELECT COUNT(*) FROM workpaper_revisions),
                (SELECT COUNT(*) FROM workpaper_evidence_links),
                (SELECT COUNT(*) FROM workpaper_signoff_supersessions),
                (SELECT COUNT(*) FROM audit_events
                 WHERE event_type='TEST_ONLY_REFERENCE_SUCCESSOR')",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("count all related writes after retry");
    assert_eq!(counts, (2, 4, 1, 1));
}

/// Direct, isolated SQL fixture injection is intentional: the production
/// immutable-row triggers forbid editing a row after creation, but do not
/// validate every newly inserted digest or revision-parent relationship.
fn test_only_insert_direct_revision(
    fixture: &NativeRevisionFixture,
    revision_id: &str,
    revision_number: i64,
    parent: Option<&str>,
    hash: Option<&[u8]>,
) {
    use rusqlite::params;

    let connection = Connection::open(&fixture.path).expect("open isolated native fixture");
    connection
        .execute(
            "INSERT INTO workpaper_revisions
             (workpaper_revision_id, workpaper_id, revision_number, created_at_ms,
              supersedes_revision_id, objective, procedure_performed, population, sample,
              exceptions, management_explanation, conclusion, content_hash)
             VALUES (?1, 'workpaper-native', ?2, 1, ?3, ?4, 'B', '', '\r\n',
                     'Exception', 'Response', 'Conclusion', ?5)",
            params![revision_id, revision_number, parent, "A\u{0000}Ω", hash],
        )
        .expect("direct SQL test fixture records actual migration-allowed row");
}

/// Current legacy PREPARED sign-off accepts this syntactically valid fixture
/// row despite its incorrect stored hash. The new test-only successor
/// reference MUST reject it without altering a prior sign-off or adding rows.
/// This is an intentionally malformed source, NOT a production authorization.
#[test]
fn reference_native_rejects_corrupted_signed_source_content_hash() {
    let fixture = NativeRevisionFixture::new();
    let corrupt = [0_u8; 32];
    test_only_insert_direct_revision(&fixture, "corrupt-native-1", 1, None, Some(&corrupt));
    let signed = create_workpaper_signoff(
        &fixture.path,
        NewWorkpaperSignoff {
            workpaper_id: "workpaper-native",
            workpaper_revision_id: "corrupt-native-1",
            signoff_type: "PREPARED",
            actor_id: "fixture-unverified",
            actor_role: "PREPARER",
            comment: None,
        },
    )
    .expect("legacy API does not authenticate or validate a stored content hash");

    let result = test_only_checked_successor(
        &fixture.path,
        "corrupt-native-1",
        &[],
        [
            "Edited",
            "B",
            "",
            "\r\n",
            "Exception",
            "Response",
            "Conclusion",
        ],
        false,
    );
    assert_eq!(
        result,
        Err("INVALID_NATIVE_IMMUTABLE_CONTENT_HASH".to_string())
    );
    assert_eq!(fixture.rows().len(), 1);
    let history = list_workpaper_signoffs(&fixture.path, "workpaper-native")
        .expect("original sign-off remains historical");
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].signoff_id, signed.signoff_id);
    assert_eq!(history[0].superseded_by_revision_id, None);
}

/// Even when the CURRENT unsigned revision is valid, a historical signed
/// row's mismatched digest must block the test-only copy-forward path.
#[test]
fn reference_native_rejects_corrupted_older_signed_revision_hash() {
    let fixture = NativeRevisionFixture::new();
    let corrupt = [255_u8; 32];
    test_only_insert_direct_revision(&fixture, "corrupt-older-1", 1, None, Some(&corrupt));
    create_workpaper_signoff(
        &fixture.path,
        NewWorkpaperSignoff {
            workpaper_id: "workpaper-native",
            workpaper_revision_id: "corrupt-older-1",
            signoff_type: "PREPARED",
            actor_id: "fixture-unverified",
            actor_role: "PREPARER",
            comment: None,
        },
    )
    .expect("fixture legacy sign-off");
    let latest = fixture.insert([
        "Revised",
        "B",
        "",
        "\r\n",
        "Exception",
        "Response",
        "Conclusion",
    ]);
    let result = test_only_checked_successor(
        &fixture.path,
        &latest.workpaper_revision_id,
        &[],
        [
            "Again",
            "B",
            "",
            "\r\n",
            "Exception",
            "Response",
            "Conclusion",
        ],
        false,
    );
    assert_eq!(
        result,
        Err("INVALID_NATIVE_IMMUTABLE_CONTENT_HASH".to_string())
    );
    assert_eq!(fixture.rows().len(), 2);
    assert_eq!(
        fixture.rows()[0].workpaper_revision_id,
        latest.workpaper_revision_id
    );
}

/// The real migration permits legacy null hashes. The test-only guard does
/// not pretend to verify missing historical digests; a successful successor
/// gets a complete native seven-field hash of its own.
#[test]
fn reference_native_allows_legacy_null_hash_and_hashes_successor() {
    let fixture = NativeRevisionFixture::new();
    test_only_insert_direct_revision(&fixture, "legacy-null-1", 1, None, None);
    let successor_id = test_only_checked_successor(
        &fixture.path,
        "legacy-null-1",
        &[],
        [
            "A\u{0000}Ω!",
            "B",
            "",
            "\r\n",
            "Exception",
            "Response",
            "Conclusion",
        ],
        false,
    )
    .expect("nullable historical hash compatibility should allow append");
    let rows = fixture.rows();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[1].content_hash, None);
    assert_eq!(rows[0].workpaper_revision_id, successor_id);
    assert_eq!(rows[0].content_hash.as_ref().map(Vec::len), Some(32));
    assert_eq!(rows[0].revision_number, 2);
}

/// The actual migration permits a directly inserted revision #2 with no
/// predecessor. The test-only transaction must fail closed on that ancestry
/// instead of silently treating the malformed row as an authorized parent.
#[test]
fn reference_native_rejects_malformed_contiguous_revision_ancestry() {
    let fixture = NativeRevisionFixture::new();
    let first = fixture.insert(["A", "B", "C", "D", "E", "F", "G"]);
    test_only_insert_direct_revision(&fixture, "broken-parent-2", 2, None, None);
    let result = test_only_checked_successor(
        &fixture.path,
        "broken-parent-2",
        &[],
        [
            "Edited",
            "B",
            "",
            "\r\n",
            "Exception",
            "Response",
            "Conclusion",
        ],
        false,
    );
    assert_eq!(result, Err("INVALID_NATIVE_REVISION_ANCESTRY".to_string()));
    assert_eq!(fixture.rows().len(), 2);
    assert_eq!(
        fixture.rows()[1].workpaper_revision_id,
        first.workpaper_revision_id
    );
}

#[test]
fn native_initial_only_entry_rejects_existing_revision_without_mutation() {
    let fixture = NativeRevisionFixture::new();
    let first = create_initial_workpaper_revision(
        &fixture.path,
        "workpaper-native",
        NewWorkpaperRevision {
            revision_reason: Some("Unverified initial legacy draft"),
            objective: "First",
            procedure_performed: "Procedure",
            population: "",
            sample: "",
            exceptions: "",
            management_explanation: "",
            conclusion: "Initial conclusion",
        },
    )
    .expect("initial-only native entry should accept an empty workpaper");
    let refused = create_initial_workpaper_revision(
        &fixture.path,
        "workpaper-native",
        NewWorkpaperRevision {
            revision_reason: Some("Must be refused"),
            objective: "Would erase existing content",
            procedure_performed: "",
            population: "",
            sample: "",
            exceptions: "",
            management_explanation: "",
            conclusion: "",
        },
    )
    .expect_err("initial-only native entry must reject a successor");
    assert!(refused.to_string().contains("LEGACY_INITIAL_REVISION_ONLY"));
    let saved = fixture.rows();
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].workpaper_revision_id, first.workpaper_revision_id);
    assert_eq!(
        as_fields(&saved[0]),
        ["First", "Procedure", "", "", "", "", "Initial conclusion"]
    );

    // Existing callable legacy API is intentionally backward-compatible;
    // this fixture outcome is a known untrusted bypass, NOT a security proof.
    let legacy_successor = fixture.insert([
        "Legacy successor",
        "Procedure",
        "Historic",
        "Sample",
        "Exception",
        "Explanation",
        "Conclusion",
    ]);
    assert_eq!(fixture.rows().len(), 2);
    assert_eq!(legacy_successor.revision_number, 2);
}

#[test]
fn native_initial_only_two_independent_writers_accept_only_one() {
    use std::sync::{Arc, Barrier};
    let fixture = NativeRevisionFixture::new();
    let ready = Arc::new(Barrier::new(3));
    let mut workers = Vec::new();
    for objective in ["First writer", "Second writer"] {
        let path = fixture.path.clone();
        let barrier = Arc::clone(&ready);
        workers.push(std::thread::spawn(move || {
            barrier.wait();
            create_initial_workpaper_revision(
                &path,
                "workpaper-native",
                NewWorkpaperRevision {
                    revision_reason: Some("Fixture initial-only race"),
                    objective,
                    procedure_performed: "Trace",
                    population: "",
                    sample: "",
                    exceptions: "",
                    management_explanation: "",
                    conclusion: "Unverified local draft",
                },
            )
        }));
    }
    ready.wait();
    let mut accepted = 0;
    let mut refused = 0;
    for worker in workers {
        match worker
            .join()
            .expect("native first-create worker must not panic")
        {
            Ok(revision) => {
                accepted += 1;
                assert_eq!(revision.revision_number, 1);
            }
            Err(error) => {
                assert!(error.to_string().contains("LEGACY_INITIAL_REVISION_ONLY"));
                refused += 1;
            }
        }
    }
    assert_eq!((accepted, refused), (1, 1));
    assert_eq!(fixture.rows().len(), 1);
}

#[test]
fn native_legacy_initial_only_dto_defaults_off_for_installed_callers() {
    use serde_json::json;
    let payload = json!({
        "objective": "Objective",
        "procedurePerformed": "Procedure",
        "population": "",
        "sample": "",
        "exceptions": "",
        "managementExplanation": "",
        "conclusion": ""
    });
    let installed: crate::WorkpaperRevisionInputDto =
        serde_json::from_value(payload.clone()).expect("old caller DTO must parse");
    assert!(!installed.initial_only);
    let mut guarded_payload = payload;
    guarded_payload["initialOnly"] = json!(true);
    let guarded: crate::WorkpaperRevisionInputDto =
        serde_json::from_value(guarded_payload).expect("new frontend DTO must parse");
    assert!(guarded.initial_only);
}

#[test]
fn candidate_transaction_default_deny_preserves_real_native_history() {
    let (fixture, base, _signoff, _observed) = test_only_signed_two_link_base();
    let result = candidate_successor_transaction(
        &fixture.path,
        "workpaper-native",
        &base.workpaper_revision_id,
        CandidateObservedSnapshot {
            evidence: &[],
            active_signoff_ids: &[],
        },
        as_fields(&base),
        false,
        &DenyAllCandidateAuthority,
    );
    assert_eq!(
        result,
        Err("CANDIDATE_NATIVE_AUTHORITY_UNAVAILABLE".to_string())
    );
    assert_eq!(fixture.rows().len(), 1);
    assert_eq!(
        list_workpaper_evidence_links(&fixture.path, &base.workpaper_revision_id)
            .unwrap()
            .len(),
        2
    );
    let signoffs = list_workpaper_signoffs(&fixture.path, "workpaper-native").unwrap();
    assert_eq!(signoffs.len(), 1);
    assert_eq!(signoffs[0].superseded_by_revision_id, None);
}

#[test]
fn candidate_transaction_cannot_cross_workpaper_scope() {
    let (fixture, base, _signoff, _observed) = test_only_signed_two_link_base();
    let connection = Connection::open(&fixture.path).unwrap();
    connection
        .execute(
            "INSERT INTO workpapers
        (workpaper_id, engagement_id, reference, title, workflow_state, created_at_ms)
        VALUES ('other-workpaper', 'engagement-native', 'WP-02', 'Other', 'IN_PROGRESS', 1)",
            [],
        )
        .unwrap();
    let result = candidate_successor_transaction(
        &fixture.path,
        "other-workpaper",
        &base.workpaper_revision_id,
        CandidateObservedSnapshot {
            evidence: &[],
            active_signoff_ids: &[],
        },
        as_fields(&base),
        false,
        &FixtureOnlyCandidateAuthority,
    );
    assert_eq!(result, Err("NO_NATIVE_BASE".to_string()));
    assert_eq!(fixture.rows().len(), 1);
    assert_eq!(
        list_workpaper_revisions(&fixture.path, "other-workpaper")
            .unwrap()
            .len(),
        0
    );
}

#[test]
fn candidate_rejects_same_evidence_ids_with_changed_tuple_without_writes() {
    let (fixture, base, signoff_id, ids) = test_only_signed_two_link_base();
    let mut stale = fixture_observed_links(&fixture.path, &base.workpaper_revision_id, &ids)
        .expect("capture full exact evidence snapshot");
    // Only relationship text differs; merely comparing IDs would accept it.
    stale[0]
        .relationship_type
        .push_str("-old-client-observation");
    let signoffs =
        fixture_observed_active_signoffs(&fixture.path, &base.workpaper_revision_id).unwrap();
    let refused = candidate_successor_transaction(
        &fixture.path,
        "workpaper-native",
        &base.workpaper_revision_id,
        CandidateObservedSnapshot {
            evidence: &stale,
            active_signoff_ids: &signoffs,
        },
        as_fields(&base),
        false,
        &FixtureOnlyCandidateAuthority,
    );
    assert_eq!(refused, Err("STALE_NATIVE_EVIDENCE".to_string()));
    assert_eq!(fixture.rows().len(), 1);
    let history = list_workpaper_signoffs(&fixture.path, "workpaper-native").unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].signoff_id, signoff_id);
    assert_eq!(history[0].superseded_by_revision_id, None);
    assert_eq!(
        list_workpaper_evidence_links(&fixture.path, &base.workpaper_revision_id)
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn candidate_rejects_signoff_added_after_opening_without_supersession() {
    let fixture = NativeRevisionFixture::new();
    fixture.seed_unverified_linked_content_version();
    let base = fixture.insert(["O", "P", "Population", "Sample", "E", "M", "C"]);
    let link = create_workpaper_evidence_link(
        &fixture.path,
        &base.workpaper_revision_id,
        "doc-native",
        Some("version-native"),
        None,
        "SUPPORTING",
        Some("Fixture evidence"),
    )
    .expect("add unsigned source evidence");
    let opening_links = fixture_observed_links(
        &fixture.path,
        &base.workpaper_revision_id,
        &[link.evidence_link_id],
    )
    .unwrap();
    let opening_signoffs =
        fixture_observed_active_signoffs(&fixture.path, &base.workpaper_revision_id).unwrap();
    assert!(opening_signoffs.is_empty());
    // Another native caller creates a PREPARED record after the editor read.
    let late_signoff = create_workpaper_signoff(
        &fixture.path,
        NewWorkpaperSignoff {
            workpaper_id: "workpaper-native",
            workpaper_revision_id: &base.workpaper_revision_id,
            signoff_type: "PREPARED",
            actor_id: "fixture-unverified-signer",
            actor_role: "PREPARER",
            comment: None,
        },
    )
    .expect("legacy signer added after snapshot");
    let refused = candidate_successor_transaction(
        &fixture.path,
        "workpaper-native",
        &base.workpaper_revision_id,
        CandidateObservedSnapshot {
            evidence: &opening_links,
            active_signoff_ids: &opening_signoffs,
        },
        ["O", "P", "Population", "Sample", "E", "M", "C."],
        false,
        &FixtureOnlyCandidateAuthority,
    );
    assert_eq!(refused, Err("STALE_NATIVE_SIGNOFFS".to_string()));
    assert_eq!(fixture.rows().len(), 1);
    let history = list_workpaper_signoffs(&fixture.path, "workpaper-native").unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].signoff_id, late_signoff.signoff_id);
    assert_eq!(history[0].superseded_by_revision_id, None);
    assert_eq!(
        list_workpaper_evidence_links(&fixture.path, &base.workpaper_revision_id)
            .unwrap()
            .len(),
        1
    );
}

/// Count all observed transaction artifacts for negative tests; the test-only
/// candidate must not create a new revision, evidence link, supersession or
/// audit event when any historical signature is inconsistent.
fn fixture_candidate_write_counts(path: &std::path::Path) -> (i64, i64, i64, i64) {
    let connection = Connection::open(path).expect("open actual SQLite fixture for counts");
    connection
        .query_row(
            "SELECT
               (SELECT COUNT(*) FROM workpaper_revisions),
               (SELECT COUNT(*) FROM workpaper_evidence_links),
               (SELECT COUNT(*) FROM workpaper_signoff_supersessions),
               (SELECT COUNT(*) FROM audit_events
                  WHERE event_type='TEST_ONLY_REFERENCE_SUCCESSOR')",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("count revision/evidence/supersession/candidate audit rows")
}

#[test]
fn candidate_accepts_consistent_superseded_historical_signed_evidence_without_inheritance() {
    let (fixture, original, old_signoff_id, source_links) = test_only_signed_two_link_base();
    let current = fixture.insert([
        "Next objective",
        "Trace",
        "Full population",
        "Sample",
        "",
        "",
        "Next",
    ]);
    // The callable legacy append does NOT carry forward source evidence.
    assert_eq!(
        list_workpaper_evidence_links(&fixture.path, &current.workpaper_revision_id)
            .unwrap()
            .len(),
        0
    );
    let next = test_only_checked_successor(
        &fixture.path,
        &current.workpaper_revision_id,
        &[],
        [
            "Small fix",
            "Trace",
            "Full population",
            "Sample",
            "",
            "",
            "Next",
        ],
        false,
    )
    .expect("valid older signed snapshots are checked, not inherited");
    assert_eq!(fixture.rows().len(), 3);
    assert_eq!(fixture.rows()[0].workpaper_revision_id, next);
    let signed = list_workpaper_signoffs(&fixture.path, "workpaper-native").unwrap();
    assert_eq!(signed.len(), 1);
    assert_eq!(signed[0].signoff_id, old_signoff_id);
    assert_eq!(
        signed[0].workpaper_revision_id,
        original.workpaper_revision_id
    );
    assert_eq!(signed[0].evidence_link_ids, source_links);
    assert_eq!(
        signed[0].superseded_by_revision_id.as_deref(),
        Some(current.workpaper_revision_id.as_str())
    );
    assert!(list_workpaper_signoffs(&fixture.path, "workpaper-native")
        .unwrap()
        .iter()
        .all(|entry| entry.workpaper_revision_id != next));
}

#[test]
fn candidate_rejects_missing_historical_signed_link_snapshot_before_any_write() {
    let fixture = NativeRevisionFixture::new();
    fixture.seed_unverified_linked_content_version();
    let original = fixture.insert(["O", "P", "Population", "Sample", "E", "M", "C"]);
    let link = create_workpaper_evidence_link(
        &fixture.path,
        &original.workpaper_revision_id,
        "doc-native",
        Some("version-native"),
        None,
        "SUPPORTING",
        Some("Exact fixture source"),
    )
    .expect("source link on first revision");
    // The actual migration allows a directly inserted sign-off without
    // snapshot rows. This is fixture corruption, NOT an authenticated sign-off.
    let db = Connection::open(&fixture.path).unwrap();
    db.execute(
        "INSERT INTO workpaper_signoffs
          (signoff_id, workpaper_id, workpaper_revision_id, signoff_type,
           actor_id, actor_role, signed_at_ms, comment)
         VALUES ('historical-missing-snapshot', 'workpaper-native', ?1,
                 'PREPARED', 'fixture-unverified', 'UNVERIFIED', 2, NULL)",
        [&original.workpaper_revision_id],
    )
    .expect("seed migration-permitted malformed sign-off without signed links");
    drop(db);
    let current = fixture.insert(["O", "P", "Population", "Sample", "E", "M", "C2"]);
    let counts = fixture_candidate_write_counts(&fixture.path);
    let result = test_only_checked_successor(
        &fixture.path,
        &current.workpaper_revision_id,
        &[],
        ["O", "P", "Population", "Sample", "E", "M", "C3"],
        false,
    );
    assert_eq!(
        result,
        Err("INVALID_NATIVE_HISTORICAL_SIGNED_EVIDENCE".to_string())
    );
    assert_eq!(fixture_candidate_write_counts(&fixture.path), counts);
    assert_eq!(
        fixture.rows()[0].workpaper_revision_id,
        current.workpaper_revision_id
    );
    let signed = list_workpaper_signoffs(&fixture.path, "workpaper-native").unwrap();
    assert_eq!(signed.len(), 1);
    assert_eq!(signed[0].signoff_id, "historical-missing-snapshot");
    assert!(signed[0].evidence_link_ids.is_empty());
    assert_eq!(
        signed[0].superseded_by_revision_id.as_deref(),
        Some(current.workpaper_revision_id.as_str())
    );
    assert_eq!(
        list_workpaper_evidence_links(&fixture.path, &original.workpaper_revision_id).unwrap()[0]
            .evidence_link_id,
        link.evidence_link_id
    );
}

#[test]
fn candidate_rejects_historical_signoff_with_extra_foreign_revision_link_atomically() {
    let (fixture, original, old_signoff_id, original_link_ids) = test_only_signed_two_link_base();
    let current = fixture.insert([
        "Second objective",
        "Trace",
        "Full population",
        "Sample",
        "",
        "",
        "Second",
    ]);
    let foreign_link = create_workpaper_evidence_link(
        &fixture.path,
        &current.workpaper_revision_id,
        "doc-native",
        Some("version-native"),
        None,
        "SUPPORTING",
        Some("Belongs to the second revision, not old signature"),
    )
    .expect("add legitimate independently identified link to unsigned latest");
    // The immutable UPDATE/DELETE triggers do not forbid an extra INSERT
    // into workpaper_signoff_evidence. Directly model damaged history.
    let db = Connection::open(&fixture.path).unwrap();
    db.execute(
        "INSERT INTO workpaper_signoff_evidence
          (signoff_evidence_id, signoff_id, evidence_link_id, created_at_ms)
         VALUES ('malformed-extra-historical-snapshot', ?1, ?2, 6)",
        rusqlite::params![old_signoff_id, foreign_link.evidence_link_id],
    )
    .expect("inject migration-allowed cross-revision signed-link corruption");
    drop(db);
    let counts = fixture_candidate_write_counts(&fixture.path);
    let opened = vec![foreign_link.evidence_link_id.clone()];
    let result = test_only_checked_successor(
        &fixture.path,
        &current.workpaper_revision_id,
        &opened,
        [
            "Small correction",
            "Trace",
            "Full population",
            "Sample",
            "",
            "",
            "Second",
        ],
        false,
    );
    assert_eq!(
        result,
        Err("INVALID_NATIVE_HISTORICAL_SIGNED_EVIDENCE".to_string())
    );
    assert_eq!(fixture_candidate_write_counts(&fixture.path), counts);
    assert_eq!(fixture.rows().len(), 2);
    assert_eq!(
        fixture.rows()[0].workpaper_revision_id,
        current.workpaper_revision_id
    );
    let historical = list_workpaper_signoffs(&fixture.path, "workpaper-native").unwrap();
    assert_eq!(historical.len(), 1);
    assert_eq!(
        historical[0].workpaper_revision_id,
        original.workpaper_revision_id
    );
    assert_eq!(
        historical[0].superseded_by_revision_id.as_deref(),
        Some(current.workpaper_revision_id.as_str())
    );
    for id in &original_link_ids {
        assert!(historical[0].evidence_link_ids.contains(id));
    }
    assert!(historical[0]
        .evidence_link_ids
        .contains(&foreign_link.evidence_link_id));
}
