//! Standalone, actual-rusqlite candidate transaction for successor workpapers.
//! *** #[cfg(test)] ONLY: never compiled into an installed application. ***
//! No native principal issuer, revocable workpaper grant, controlled-evidence
//! custody policy or reviewed signer exists. This is not a callable Save route.
//! The test-only gate is checked WITHIN BEGIN IMMEDIATE. There is deliberately
//! no production constructor or Tauri registration of the fixture allow gate.
//! Promote only by an independently reviewed replacement after DEC-01..08.

use rusqlite::{Connection, Transaction};
use std::path::Path;
use uuid::Uuid;

/// Candidate authorization must be rechecked under the final SQLite writer
/// lock. No caller-provided role, claimed actor or frontend preflight suffices.
pub(crate) trait CandidateTransactionAuthority {
    fn check_under_write_lock(
        &self,
        tx: &Transaction<'_>,
        workpaper_id: &str,
    ) -> Result<(), String>;
}

pub(crate) struct DenyAllCandidateAuthority;
impl CandidateTransactionAuthority for DenyAllCandidateAuthority {
    fn check_under_write_lock(
        &self,
        _tx: &Transaction<'_>,
        _workpaper_id: &str,
    ) -> Result<(), String> {
        Err("CANDIDATE_NATIVE_AUTHORITY_UNAVAILABLE".to_string())
    }
}

/// Fixture-only bypass of the candidate gate, compiled only by cargo test.
/// It is NOT a trusted native session, permission or grant.
pub(crate) struct FixtureOnlyCandidateAuthority;
impl CandidateTransactionAuthority for FixtureOnlyCandidateAuthority {
    fn check_under_write_lock(
        &self,
        _tx: &Transaction<'_>,
        _workpaper_id: &str,
    ) -> Result<(), String> {
        Ok(())
    }
}

// The following logic was extracted from the prior tested inline reference.
// It is parameterized by exact workpaper and enforces the candidate gate.

/// Exact editor-observed tuple. IDs alone cannot detect altered metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CandidateObservedEvidenceLink {
    pub(crate) id: String,
    pub(crate) document_id: String,
    pub(crate) content_version_id: Option<String>,
    pub(crate) controlled_version_id: Option<String>,
    pub(crate) relationship_type: String,
    pub(crate) description: Option<String>,
}

/// Single, immutable input shape for the editor's opening observations.
/// This is untrusted metadata, never native authorization or evidence custody.
pub(crate) struct CandidateObservedSnapshot<'a> {
    pub(crate) evidence: &'a [CandidateObservedEvidenceLink],
    pub(crate) active_signoff_ids: &'a [String],
}

/// Test-only validation of the ENTIRE workpaper revision ancestry and all
/// non-null SHA-256 content hashes, including historical signed revisions.
/// Legacy NULL hashes are accepted for compatibility but cannot prove prior
/// content identity. This is NOT a production native trust/custody check.
fn candidate_validate_complete_ancestry_and_hashes(
    tx: &rusqlite::Transaction<'_>,
    workpaper_id: &str,
) -> Result<(), String> {
    use sha2::{Digest, Sha256};

    let mut statement = tx
        .prepare(
            "SELECT workpaper_revision_id, revision_number, supersedes_revision_id,
                    objective, procedure_performed, population, sample, exceptions,
                    management_explanation, conclusion, content_hash
             FROM workpaper_revisions
             WHERE workpaper_id=?1
             ORDER BY revision_number ASC",
        )
        .map_err(|error| error.to_string())?;
    let mut rows = statement
        .query([workpaper_id])
        .map_err(|error| error.to_string())?;
    let mut expected_number = 1_i64;
    let mut expected_parent: Option<String> = None;
    while let Some(row) = rows.next().map_err(|error| error.to_string())? {
        let revision_id: String = row.get(0).map_err(|error| error.to_string())?;
        let revision_number: i64 = row.get(1).map_err(|error| error.to_string())?;
        let parent: Option<String> = row.get(2).map_err(|error| error.to_string())?;
        if revision_number != expected_number || parent.as_deref() != expected_parent.as_deref() {
            return Err("INVALID_NATIVE_REVISION_ANCESTRY".to_string());
        }
        let fields: [String; 7] = [
            row.get(3).map_err(|error| error.to_string())?,
            row.get(4).map_err(|error| error.to_string())?,
            row.get(5).map_err(|error| error.to_string())?,
            row.get(6).map_err(|error| error.to_string())?,
            row.get(7).map_err(|error| error.to_string())?,
            row.get(8).map_err(|error| error.to_string())?,
            row.get(9).map_err(|error| error.to_string())?,
        ];
        let recorded_hash: Option<Vec<u8>> = row.get(10).map_err(|error| error.to_string())?;
        if let Some(recorded) = recorded_hash {
            let mut digest = Sha256::new();
            for field in &fields {
                digest.update((field.len() as u64).to_le_bytes());
                digest.update(field.as_bytes());
            }
            if recorded.as_slice() != digest.finalize().as_slice() {
                return Err("INVALID_NATIVE_IMMUTABLE_CONTENT_HASH".to_string());
            }
        }
        expected_parent = Some(revision_id);
        expected_number += 1;
    }
    Ok(())
}

/// A sign-off has two separately stored workpaper references: its claimed
/// workpaper_id and the workpaper_id of the immutable revision it names.
/// Current legacy SQLite foreign keys check each separately, but do NOT
/// enforce equality. Inspect BOTH sides of the relationship, so a row with
/// a foreign claimed workpaper cannot hide on this workpaper's ancestry.
///
/// This is a test-only consistency check, not identity, grants or approval.
fn candidate_validate_signoff_revision_scope(
    tx: &Transaction<'_>,
    workpaper_id: &str,
) -> Result<(), String> {
    let invalid: i64 = tx
        .query_row(
            "SELECT EXISTS(
               SELECT 1 FROM workpaper_signoffs s
               LEFT JOIN workpaper_revisions r
                 ON r.workpaper_revision_id = s.workpaper_revision_id
               WHERE (s.workpaper_id = ?1 OR r.workpaper_id = ?1)
                 AND (r.workpaper_revision_id IS NULL
                      OR r.workpaper_id <> s.workpaper_id)
             )",
            [workpaper_id],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    if invalid != 0 {
        return Err("INVALID_NATIVE_SIGNOFF_REVISION_SCOPE".to_string());
    }
    Ok(())
}

/// Check every recorded sign-off (including superseded historical rows)
/// against the exact evidence links of its own immutable revision. The
/// existing candidate checked only ACTIVE signatures on the latest base;
/// a malformed older signed snapshot could otherwise be carried forward
/// without notice. This checks metadata consistency only, never provenance,
/// authentic signer identity, byte custody or human approval.
fn candidate_validate_historical_signed_evidence(
    tx: &Transaction<'_>,
    workpaper_id: &str,
) -> Result<(), String> {
    // Symmetric set check: both omitted source links and extra/cross-revision
    // signed links are inconsistent. A truly empty source/snapshot pair is
    // valid. LEFT JOINs reject orphaned or wrong-workpaper revision metadata.
    let invalid: i64 = tx
        .query_row(
            "SELECT EXISTS(
               SELECT 1 FROM workpaper_signoffs s
               LEFT JOIN workpaper_revisions r
                 ON r.workpaper_revision_id = s.workpaper_revision_id
               WHERE s.workpaper_id = ?1 AND (
                 r.workpaper_id IS NULL OR r.workpaper_id <> s.workpaper_id
                 OR EXISTS(
                   SELECT 1 FROM workpaper_evidence_links e
                   WHERE e.workpaper_revision_id = s.workpaper_revision_id
                     AND NOT EXISTS(
                       SELECT 1 FROM workpaper_signoff_evidence se
                       WHERE se.signoff_id = s.signoff_id
                         AND se.evidence_link_id = e.evidence_link_id
                     )
                 )
                 OR EXISTS(
                   SELECT 1 FROM workpaper_signoff_evidence se
                   LEFT JOIN workpaper_evidence_links e
                     ON e.evidence_link_id = se.evidence_link_id
                   WHERE se.signoff_id = s.signoff_id
                     AND (e.evidence_link_id IS NULL
                          OR e.workpaper_revision_id <> s.workpaper_revision_id)
                 )
               )
             )",
            [workpaper_id],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    if invalid != 0 {
        return Err("INVALID_NATIVE_HISTORICAL_SIGNED_EVIDENCE".to_string());
    }
    Ok(())
}

/// Fail closed when a signed revision's supersession record disagrees
/// with the exact immutable one-step ancestry. For a historical signed
/// revision, every sign-off must be superseded by its IMMEDIATE successor;
/// sign-offs on the currently latest revision must remain unsuperseded
/// until the candidate's own transaction supersedes them atomically.
///
/// This is a consistency gate over stored metadata, not verified human
/// authority, custody approval or an authenticated professional signature.
fn candidate_validate_historical_signoff_supersessions(
    tx: &Transaction<'_>,
    workpaper_id: &str,
    latest_revision_id: &str,
) -> Result<(), String> {
    let invalid: i64 = tx
        .query_row(
            "SELECT EXISTS(
               SELECT 1 FROM workpaper_signoffs s
               LEFT JOIN workpaper_revisions signed
                 ON signed.workpaper_revision_id = s.workpaper_revision_id
               LEFT JOIN workpaper_signoff_supersessions ss
                 ON ss.signoff_id = s.signoff_id
               LEFT JOIN workpaper_revisions successor
                 ON successor.workpaper_revision_id = ss.superseded_by_revision_id
               WHERE s.workpaper_id = ?1 AND (
                 signed.workpaper_id IS NULL
                 OR signed.workpaper_id <> ?1
                 OR (s.workpaper_revision_id = ?2 AND ss.signoff_id IS NOT NULL)
                 OR (s.workpaper_revision_id <> ?2 AND (
                   ss.signoff_id IS NULL
                   OR successor.workpaper_id IS NULL
                   OR successor.workpaper_id <> ?1
                   OR successor.revision_number <> signed.revision_number + 1
                   OR successor.supersedes_revision_id IS NULL
                   OR successor.supersedes_revision_id <> signed.workpaper_revision_id
                 ))
               )
             )",
            rusqlite::params![workpaper_id, latest_revision_id],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    if invalid != 0 {
        return Err("INVALID_NATIVE_HISTORICAL_SIGNOFF_SUPERSESSION".to_string());
    }
    Ok(())
}

/// Inspect ONLY existing legacy workpaper audit references; the early
/// migration gives audit_events no foreign keys for entity/related IDs.
/// Scope each check by EITHER side (stored entity owner or related revision)
/// to avoid hiding a cross-workpaper audit row behind its incorrect label.
///
/// This does not require old audit events to exist: legacy rows and test-only
/// transactions may not have a complete native event series. No source-byte
/// custody, tamper-evident audit storage or signer authentication is proven.
fn candidate_validate_existing_workpaper_audit_references(
    tx: &Transaction<'_>,
    workpaper_id: &str,
) -> Result<(), String> {
    let invalid: i64 = tx
        .query_row(
            "SELECT
               EXISTS(
                 SELECT 1 FROM audit_events a
                 LEFT JOIN workpaper_revisions r
                   ON r.workpaper_revision_id = a.entity_id
                 WHERE a.event_type = 'WORKPAPER_REVISION_CREATED'
                   AND (r.workpaper_id = ?1 OR a.related_entity_id = ?1)
                   AND (a.entity_type IS NOT 'WORKPAPER_REVISION'
                     OR a.related_entity_type IS NOT 'WORKPAPER'
                     OR r.workpaper_revision_id IS NULL
                     OR r.workpaper_id IS NOT a.related_entity_id)
               )
               OR EXISTS(
                 SELECT 1 FROM audit_events a
                 LEFT JOIN workpaper_evidence_links e
                   ON e.evidence_link_id = a.entity_id
                 LEFT JOIN workpaper_revisions owner
                   ON owner.workpaper_revision_id = e.workpaper_revision_id
                 LEFT JOIN workpaper_revisions claimed
                   ON claimed.workpaper_revision_id = a.related_entity_id
                 WHERE a.event_type = 'WORKPAPER_EVIDENCE_LINK_ADDED'
                   AND (owner.workpaper_id = ?1 OR claimed.workpaper_id = ?1)
                   AND (a.entity_type IS NOT 'WORKPAPER_EVIDENCE_LINK'
                     OR a.related_entity_type IS NOT 'WORKPAPER_REVISION'
                     OR e.evidence_link_id IS NULL
                     OR e.workpaper_revision_id IS NOT a.related_entity_id)
               )
               OR EXISTS(
                 SELECT 1 FROM audit_events a
                 LEFT JOIN workpaper_signoffs s
                   ON s.signoff_id = a.entity_id
                 LEFT JOIN workpaper_revisions owner
                   ON owner.workpaper_revision_id = s.workpaper_revision_id
                 LEFT JOIN workpaper_revisions claimed
                   ON claimed.workpaper_revision_id = a.related_entity_id
                 WHERE a.event_type = 'WORKPAPER_SIGNOFF_CREATED'
                   AND (s.workpaper_id = ?1 OR owner.workpaper_id = ?1
                        OR claimed.workpaper_id = ?1)
                   AND (a.entity_type IS NOT 'WORKPAPER_SIGNOFF'
                     OR a.related_entity_type IS NOT 'WORKPAPER_REVISION'
                     OR s.signoff_id IS NULL
                     OR s.workpaper_revision_id IS NOT a.related_entity_id)
               )
               OR EXISTS(
                 SELECT 1 FROM audit_events a
                 LEFT JOIN workpaper_signoffs s
                   ON s.signoff_id = a.entity_id
                 LEFT JOIN workpaper_revisions owner
                   ON owner.workpaper_revision_id = s.workpaper_revision_id
                 LEFT JOIN workpaper_revisions claimed
                   ON claimed.workpaper_revision_id = a.related_entity_id
                 LEFT JOIN workpaper_signoff_supersessions ss
                   ON ss.signoff_id = a.entity_id
                 WHERE a.event_type = 'WORKPAPER_SIGNOFF_SUPERSEDED'
                   AND (s.workpaper_id = ?1 OR owner.workpaper_id = ?1
                        OR claimed.workpaper_id = ?1)
                   AND (a.entity_type IS NOT 'WORKPAPER_SIGNOFF'
                     OR a.related_entity_type IS NOT 'WORKPAPER_REVISION'
                     OR ss.signoff_id IS NULL
                     OR ss.superseded_by_revision_id IS NOT a.related_entity_id)
               )",
            [workpaper_id],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    if invalid != 0 {
        return Err("INVALID_NATIVE_HISTORICAL_AUDIT_REFERENCE".to_string());
    }
    Ok(())
}

pub(crate) fn candidate_successor_transaction(
    path: &Path,
    workpaper_id: &str,
    expected_base: &str,
    observed: CandidateObservedSnapshot<'_>,
    fields: [&str; 7],
    fail_after_first_copied_link: bool,
    authority: &dyn CandidateTransactionAuthority,
) -> Result<String, String> {
    use rusqlite::{params, OptionalExtension, TransactionBehavior};
    use sha2::{Digest, Sha256};

    let mut connection = Connection::open(path).map_err(|error| error.to_string())?;
    connection
        .busy_timeout(std::time::Duration::from_secs(5))
        .map_err(|error| error.to_string())?;
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(|error| error.to_string())?;
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| error.to_string())?;
    // This gate is deliberately inside the same IMMEDIATE transaction
    // that controls exact-base checks and all successor/evidence/audit writes.
    authority.check_under_write_lock(&tx, workpaper_id)?;
    let latest: Option<(String, i64)> = tx
        .query_row(
            "SELECT workpaper_revision_id, revision_number
             FROM workpaper_revisions WHERE workpaper_id=?1
             ORDER BY revision_number DESC LIMIT 1",
            [workpaper_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let Some((latest_id, latest_number)) = latest else {
        return Err("NO_NATIVE_BASE".to_string());
    };
    if latest_id != expected_base {
        return Err("STALE_NATIVE_BASE".to_string());
    }
    candidate_validate_complete_ancestry_and_hashes(&tx, workpaper_id)?;
    // Never infer the sign-off's scope solely from its claimed workpaper ID.
    // Catch mis-scoped and orphaned rows before inspecting signed snapshots.
    candidate_validate_signoff_revision_scope(&tx, workpaper_id)?;
    // Reject inconsistent historical signed evidence under the same writer
    // lock, before any successor, link, supersession or audit insert.
    candidate_validate_historical_signed_evidence(&tx, workpaper_id)?;
    candidate_validate_historical_signoff_supersessions(&tx, workpaper_id, &latest_id)?;
    candidate_validate_existing_workpaper_audit_references(&tx, workpaper_id)?;

    let links: Vec<CandidateObservedEvidenceLink> = {
        let mut statement = tx
            .prepare(
                "SELECT evidence_link_id, document_id, content_version_id,
                        controlled_evidence_version_id, relationship_type, description
                 FROM workpaper_evidence_links
                 WHERE workpaper_revision_id=?1 ORDER BY evidence_link_id",
            )
            .map_err(|error| error.to_string())?;
        let collected = statement
            .query_map([expected_base], |row| {
                Ok(CandidateObservedEvidenceLink {
                    id: row.get(0)?,
                    document_id: row.get(1)?,
                    content_version_id: row.get(2)?,
                    controlled_version_id: row.get(3)?,
                    relationship_type: row.get(4)?,
                    description: row.get(5)?,
                })
            })
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        collected
    };
    let mut expected_links = observed.evidence.to_vec();
    expected_links.sort_by(|a, b| a.id.cmp(&b.id));
    if expected_links != links {
        return Err("STALE_NATIVE_EVIDENCE".to_string());
    }
    let actual_ids: Vec<String> = links.iter().map(|link| link.id.clone()).collect();
    // Actual document-to-version checks in this test-only schema oracle.
    // These are NOT source-byte retention or formal controlled-custody checks.
    for link in &links {
        if link.content_version_id.is_some() == link.controlled_version_id.is_some() {
            return Err("INVALID_NATIVE_EVIDENCE_VERSION".to_string());
        }
        let valid: i64 = tx
            .query_row(
                "SELECT CASE
                   WHEN ?2 IS NOT NULL THEN
                     EXISTS(SELECT 1 FROM content_versions
                            WHERE content_version_id=?2 AND document_id=?1)
                   WHEN ?3 IS NOT NULL THEN
                     EXISTS(SELECT 1 FROM controlled_evidence_versions
                            WHERE controlled_evidence_version_id=?3 AND document_id=?1)
                   ELSE 0 END",
                params![
                    &link.document_id,
                    link.content_version_id.as_deref(),
                    link.controlled_version_id.as_deref()
                ],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        if valid != 1 {
            return Err("INVALID_NATIVE_EVIDENCE_VERSION".to_string());
        }
    }

    let active_signoff_ids: Vec<String> = {
        let mut statement = tx
            .prepare(
                "SELECT s.signoff_id FROM workpaper_signoffs s
                 LEFT JOIN workpaper_signoff_supersessions ss ON ss.signoff_id=s.signoff_id
                 WHERE s.workpaper_revision_id=?1 AND ss.signoff_id IS NULL
                 ORDER BY s.signoff_id",
            )
            .map_err(|error| error.to_string())?;
        let collected = statement
            .query_map([expected_base], |row| row.get(0))
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        collected
    };
    let mut expected_signoff_ids = observed.active_signoff_ids.to_vec();
    expected_signoff_ids.sort();
    if active_signoff_ids != expected_signoff_ids {
        return Err("STALE_NATIVE_SIGNOFFS".to_string());
    }
    for signoff_id in &active_signoff_ids {
        let signed_ids: Vec<String> = {
            let mut statement = tx
                .prepare(
                    "SELECT evidence_link_id FROM workpaper_signoff_evidence
                     WHERE signoff_id=?1 ORDER BY evidence_link_id",
                )
                .map_err(|error| error.to_string())?;
            let collected = statement
                .query_map([signoff_id], |row| row.get(0))
                .map_err(|error| error.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| error.to_string())?;
            collected
        };
        if signed_ids != actual_ids {
            return Err("INVALID_NATIVE_SIGNED_EVIDENCE_SNAPSHOT".to_string());
        }
    }

    let mut digest = Sha256::new();
    for field in fields {
        digest.update((field.len() as u64).to_le_bytes());
        digest.update(field.as_bytes());
    }
    let hash = digest.finalize();
    let successor_id = Uuid::new_v4().to_string();
    tx.execute(
        "INSERT INTO workpaper_revisions
         (workpaper_revision_id, workpaper_id, revision_number, created_at_ms,
          revision_reason, supersedes_revision_id, objective, procedure_performed,
          population, sample, exceptions, management_explanation, conclusion, content_hash)
         VALUES (?1, ?2, ?3, 2, 'test-only native CAS candidate',
                 ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            &successor_id,
            workpaper_id,
            latest_number + 1,
            expected_base,
            fields[0],
            fields[1],
            fields[2],
            fields[3],
            fields[4],
            fields[5],
            fields[6],
            hash.as_slice()
        ],
    )
    .map_err(|error| error.to_string())?;
    for (index, link) in links.iter().enumerate() {
        tx.execute(
            "INSERT INTO workpaper_evidence_links
             (evidence_link_id, workpaper_revision_id, document_id, content_version_id,
              controlled_evidence_version_id, relationship_type, description, created_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 3)",
            params![
                Uuid::new_v4().to_string(),
                &successor_id,
                &link.document_id,
                link.content_version_id.as_deref(),
                link.controlled_version_id.as_deref(),
                &link.relationship_type,
                link.description.as_deref()
            ],
        )
        .map_err(|error| error.to_string())?;
        if index == 0 && fail_after_first_copied_link {
            return Err("INJECTED_NATIVE_COPY_FAILURE".to_string());
        }
    }
    for signoff_id in &active_signoff_ids {
        tx.execute(
            "INSERT INTO workpaper_signoff_supersessions
             (signoff_supersession_id, signoff_id, superseded_by_revision_id,
              superseded_at_ms, superseded_reason)
             VALUES (?1, ?2, ?3, 4, 'test-only native successor')",
            params![Uuid::new_v4().to_string(), signoff_id, &successor_id],
        )
        .map_err(|error| error.to_string())?;
    }
    tx.execute(
        "INSERT INTO audit_events
         (audit_event_id, event_type, entity_type, entity_id, occurred_at_ms, details_json)
         VALUES (?1, 'TEST_ONLY_REFERENCE_SUCCESSOR', 'WORKPAPER_REVISION', ?2, 4, '{}')",
        params![Uuid::new_v4().to_string(), &successor_id],
    )
    .map_err(|error| error.to_string())?;
    tx.commit().map_err(|error| error.to_string())?;
    Ok(successor_id)
}
