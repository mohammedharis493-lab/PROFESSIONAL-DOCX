/**
 * TEST ONLY. Successor-write reference transaction using actual production
 * SQLite migration DDL. Not imported by src/ or src-tauri/src/, not a Save API.
 * __fixture_grants and subject IDs are SYNTHETIC: no authentication, native
 * principal issuer, real grant authority or evidence custody is provided.
 */
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { randomUUID } from "node:crypto";
import { DatabaseSync } from "node:sqlite";

const fields = ["objective", "procedure_performed", "population", "sample",
  "exceptions", "management_explanation", "conclusion"];
const original = () => ({
  objective: "Objective\r\noriginal ",
  procedure_performed: "Trace approvals",
  population: "All entries",
  sample: "Sample twenty",
  exceptions: "One exception",
  management_explanation: "Reply\u00a0 ",
  conclusion: "Prior signed conclusion",
});
const rows = (db, rev = "rev-1") => db.prepare(
  "SELECT evidence_link_id,document_id,content_version_id,controlled_evidence_version_id," +
  "relationship_type,description FROM workpaper_evidence_links " +
  "WHERE workpaper_revision_id = ? ORDER BY evidence_link_id"
).all(rev);
const observed = (db, rev = "rev-1") => JSON.stringify(rows(db, rev));
const count = (db, name) => db.prepare("SELECT count(*) n FROM " + name).get().n;

function fixture(t) {
  const db = new DatabaseSync(":memory:");
  t.after(() => db.close());
  db.exec("PRAGMA foreign_keys=ON");
  for (const file of ["0001_initial.sql", "0004_controlled_evidence.sql",
    "0006_engagement_workpapers.sql", "0009_workpaper_signoffs.sql"]) {
    db.exec(readFileSync(new URL("../src-tauri/migrations/" + file, import.meta.url), "utf8"));
  }
  db.exec([
    "CREATE TABLE __fixture_grants (subject_id TEXT,workpaper_id TEXT,permission TEXT,revoked INTEGER NOT NULL,PRIMARY KEY(subject_id,workpaper_id,permission));",
    "INSERT INTO clients VALUES('c','Fixture client',1,NULL);",
    "INSERT INTO service_types VALUES('s','Fixture service','fixture',1,NULL);",
    "INSERT INTO engagements VALUES('e','c','s','Fixture engagement',NULL,NULL,'ACTIVE',1,NULL);",
    "INSERT INTO workpapers VALUES('wp-1','e',NULL,NULL,'W-1','Fixture workpaper','IN_PROGRESS',1,NULL),('wp-2','e',NULL,NULL,'W-2','Other workpaper','IN_PROGRESS',1,NULL);",
    "INSERT INTO documents VALUES('doc-a','LINKED','Source A',1,NULL,NULL),('doc-b','CONTROLLED_EVIDENCE','Source B',1,NULL,NULL);",
    "INSERT INTO storage_roots (storage_root_id,kind,native_locator,native_locator_encoding,display_locator,availability_state,approved_at_ms,created_at_ms,updated_at_ms) VALUES ('root','LOCAL',X'2F','UTF-8','/','AVAILABLE',1,1,1);",
    "INSERT INTO file_instances (file_instance_id,document_id,storage_root_id,relative_path_native,path_native_encoding,relative_path_display,relative_path_search,size_bytes,first_seen_at_ms,last_seen_at_ms,availability_state) VALUES ('fa','doc-a','root',X'61','UTF-8','a','a',2,1,1,'AVAILABLE'),('fb','doc-b','root',X'62','UTF-8','b','b',2,1,1,'AVAILABLE');",
    "INSERT INTO content_versions (content_version_id,document_id,file_instance_id,observed_at_ms,size_bytes,verification_state) VALUES ('va','doc-a','fa',1,2,'UNVERIFIED'),('vb','doc-b','fb',1,2,'HASH_VERIFIED');",
    "INSERT INTO evidence_capture_jobs (evidence_capture_job_id,file_instance_id,document_id,status,requested_at_ms,started_at_ms,completed_at_ms,capture_reason,capture_policy) VALUES ('job-b','fb','doc-b','COMPLETE',1,1,1,'FIXTURE','FIXTURE');",
    "INSERT INTO controlled_evidence_versions (controlled_evidence_version_id,document_id,source_file_instance_id,source_content_version_id,evidence_capture_job_id,version_number,controlled_storage_locator,sha256,size_bytes,captured_at_ms,capture_reason,capture_policy,retention_state,verification_state,source_stable_during_read) VALUES ('cb','doc-b','fb','vb','job-b',1,'test-only://b',zeroblob(32),2,1,'FIXTURE','FIXTURE','RETAINED','HASH_VERIFIED',1);",
    "INSERT INTO workpaper_revisions (workpaper_revision_id,workpaper_id,revision_number,created_at_ms,objective,procedure_performed,population,sample,exceptions,management_explanation,conclusion) VALUES ('rev-1','wp-1',1,1,'Objective'||char(13)||char(10)||'original ','Trace approvals','All entries','Sample twenty','One exception','Reply'||char(160)||' ','Prior signed conclusion');",
    "INSERT INTO workpaper_evidence_links (evidence_link_id,workpaper_revision_id,document_id,content_version_id,controlled_evidence_version_id,relationship_type,description,created_at_ms) VALUES ('link-a','rev-1','doc-a','va',NULL,'SUPPORTS','Exact original note',1),('link-b','rev-1','doc-b',NULL,'cb','REVIEWED_WITH',NULL,1);",
    "INSERT INTO workpaper_signoffs (signoff_id,workpaper_id,workpaper_revision_id,signoff_type,actor_id,actor_role,signed_at_ms) VALUES ('sign-1','wp-1','rev-1','PREPARED','fixture-preparer','PREPARER',1);",
    "INSERT INTO workpaper_signoff_evidence VALUES ('snapshot-1','sign-1','link-a',1);",
    "INSERT INTO __fixture_grants VALUES ('author-1','wp-1','REVISE_WORKPAPER',0);",
  ].join("\n"));
  return db;
}

/** Test oracle. The mock grant registry is not production authentication. */
function successor(db, options = {}) {
  const {
    subjectId = "author-1", workpaperId = "wp-1",
    expectedRevisionId = "rev-1", expectedEvidence = observed(db, expectedRevisionId),
    content = { ...original(), conclusion: "Corrected conclusion" },
    failAt = "",
  } = options;
  if (!content || typeof content !== "object" || Array.isArray(content) ||
    Object.keys(content).length !== fields.length ||
    fields.some(k => !Object.hasOwn(content, k) || typeof content[k] !== "string")) {
    throw new Error("INVALID_SEVEN_FIELDS");
  }
  db.exec("BEGIN IMMEDIATE");
  try {
    const grant = db.prepare("SELECT 1 FROM __fixture_grants WHERE subject_id=? AND workpaper_id=? AND permission='REVISE_WORKPAPER' AND revoked=0").get(subjectId, workpaperId);
    if (!grant) throw new Error("DENIED_NO_REVISION_GRANT");
    const latest = db.prepare("SELECT r.workpaper_revision_id id,r.revision_number num FROM workpaper_revisions r JOIN workpapers w ON w.workpaper_id=r.workpaper_id WHERE w.workpaper_id=? AND w.archived_at_ms IS NULL ORDER BY r.revision_number DESC LIMIT 1").get(workpaperId);
    if (!latest || latest.id !== expectedRevisionId) throw new Error("STALE_REVISION");
    if (observed(db, latest.id) !== expectedEvidence) throw new Error("STALE_EVIDENCE");
    const links = rows(db, latest.id);
    for (const link of links) {
      const controlled = link.controlled_evidence_version_id !== null;
      const version = controlled
        ? db.prepare("SELECT document_id,verification_state FROM controlled_evidence_versions WHERE controlled_evidence_version_id=?").get(link.controlled_evidence_version_id)
        : db.prepare("SELECT document_id FROM content_versions WHERE content_version_id=?").get(link.content_version_id);
      if (!version || version.document_id !== link.document_id ||
        (controlled && version.verification_state !== "HASH_VERIFIED")) {
        throw new Error("INVALID_EVIDENCE_VERSION");
      }
    }
    const next = randomUUID();
    db.prepare("INSERT INTO workpaper_revisions (workpaper_revision_id,workpaper_id,revision_number,created_at_ms,revision_reason,supersedes_revision_id,objective,procedure_performed,population,sample,exceptions,management_explanation,conclusion) VALUES (?,?,?,2,'Fixture correction',?,?,?,?,?,?,?,?)")
      .run(next, workpaperId, latest.num + 1, latest.id, ...fields.map(k => content[k]));
    if (failAt === "revision") throw new Error("INJECTED_ROLLBACK");
    const insertLink = db.prepare("INSERT INTO workpaper_evidence_links (evidence_link_id,workpaper_revision_id,document_id,content_version_id,controlled_evidence_version_id,relationship_type,description,created_at_ms) VALUES (?,?,?,?,?,?,?,2)");
    for (const link of links) {
      insertLink.run(randomUUID(), next, link.document_id, link.content_version_id,
        link.controlled_evidence_version_id, link.relationship_type, link.description);
    }
    if (failAt === "evidence") throw new Error("INJECTED_ROLLBACK");
    const signoffs = db.prepare("SELECT s.signoff_id id FROM workpaper_signoffs s LEFT JOIN workpaper_signoff_supersessions ss ON ss.signoff_id=s.signoff_id WHERE s.workpaper_revision_id=? AND ss.signoff_id IS NULL").all(latest.id);
    const supersede = db.prepare("INSERT INTO workpaper_signoff_supersessions (signoff_supersession_id,signoff_id,superseded_by_revision_id,superseded_at_ms,superseded_reason,actor_id) VALUES (?,?,?,2,'New immutable revision',?)");
    for (const s of signoffs) supersede.run(randomUUID(), s.id, next, subjectId);
    if (failAt === "signoff") throw new Error("INJECTED_ROLLBACK");
    db.prepare("INSERT INTO audit_events (audit_event_id,event_type,entity_type,entity_id,related_entity_type,related_entity_id,occurred_at_ms,actor_id,details_json) VALUES (?,'FIXTURE_SUCCESSOR','WORKPAPER_REVISION',?,'WORKPAPER',?,2,?,?)")
      .run(randomUUID(), next, workpaperId, subjectId, JSON.stringify({ supersedes: latest.id }));
    db.exec("COMMIT");
    return next;
  } catch (error) {
    db.exec("ROLLBACK");
    throw error;
  }
}

test("signed base admits a fixture-authorized correction without reviewer unlock", t => {
  const db = fixture(t);
  const next = successor(db);
  const newRow = db.prepare("SELECT * FROM workpaper_revisions WHERE workpaper_revision_id=?").get(next);
  assert.equal(newRow.revision_number, 2);
  assert.equal(newRow.supersedes_revision_id, "rev-1");
  assert.equal(newRow.objective, original().objective);
  assert.equal(newRow.population, "All entries");
  assert.equal(newRow.conclusion, "Corrected conclusion");
  assert.equal(count(db, "workpaper_revisions"), 2);
  assert.equal(count(db, "workpaper_signoffs"), 1);
});

test("exact version, relation and description carry into new link IDs, not old IDs", t => {
  const db = fixture(t);
  const before = rows(db);
  const next = successor(db);
  const after = rows(db, next);
  assert.equal(after.length, before.length);
  for (let i = 0; i < before.length; i++) {
    assert.notEqual(after[i].evidence_link_id, before[i].evidence_link_id);
    for (const k of ["document_id", "content_version_id", "controlled_evidence_version_id",
      "relationship_type", "description"]) assert.equal(after[i][k], before[i][k]);
  }
  assert.deepEqual(rows(db), before);
});

test("historical signoff and its exact evidence snapshot stay old, successor unsigned", t => {
  const db = fixture(t);
  const next = successor(db);
  assert.equal(db.prepare("SELECT workpaper_revision_id FROM workpaper_signoffs WHERE signoff_id='sign-1'").get().workpaper_revision_id, "rev-1");
  assert.equal(db.prepare("SELECT evidence_link_id FROM workpaper_signoff_evidence WHERE signoff_id='sign-1'").get().evidence_link_id, "link-a");
  assert.equal(db.prepare("SELECT superseded_by_revision_id FROM workpaper_signoff_supersessions WHERE signoff_id='sign-1'").get().superseded_by_revision_id, next);
  assert.equal(db.prepare("SELECT count(*) n FROM workpaper_signoffs WHERE workpaper_revision_id=?").get(next).n, 0);
});

test("no workpaper-scoped fixture grant, wrong actor and revocation all deny atomically", t => {
  const db = fixture(t);
  assert.throws(() => successor(db, { subjectId: "someone-else" }), /DENIED/);
  assert.throws(() => successor(db, { workpaperId: "wp-2" }), /DENIED/);
  db.exec("UPDATE __fixture_grants SET revoked=1 WHERE subject_id='author-1'");
  assert.throws(() => successor(db), /DENIED/);
  assert.equal(count(db, "workpaper_revisions"), 1);
});

test("second writer must detect stale revision, without overwriting or rebasing", t => {
  const db = fixture(t);
  const atOpen = observed(db);
  successor(db, { expectedEvidence: atOpen });
  assert.throws(() => successor(db, {
    expectedRevisionId: "rev-1", expectedEvidence: atOpen,
    content: { ...original(), objective: "Different writer" },
  }), /STALE_REVISION/);
  assert.equal(count(db, "workpaper_revisions"), 2);
});

test("evidence-only addition causes conflict even with unchanged canonical text", t => {
  const db = fixture(t);
  const atOpen = observed(db);
  db.exec("INSERT INTO workpaper_evidence_links (evidence_link_id,workpaper_revision_id,document_id,content_version_id,controlled_evidence_version_id,relationship_type,description,created_at_ms) VALUES ('later','rev-1','doc-a','va',NULL,'CORROBORATES',NULL,2)");
  assert.throws(() => successor(db, { expectedEvidence: atOpen, content: original() }), /STALE_EVIDENCE/);
  assert.equal(count(db, "workpaper_revisions"), 1);
});

test("wrong expected base and archived workpaper fail closed", t => {
  const db = fixture(t);
  assert.throws(() => successor(db, { expectedRevisionId: "wrong", expectedEvidence: "[]" }), /STALE_REVISION/);
  db.exec("UPDATE workpapers SET archived_at_ms=2 WHERE workpaper_id='wp-1'");
  assert.throws(() => successor(db), /STALE_REVISION/);
  assert.equal(count(db, "workpaper_revisions"), 1);
});

test("missing fields, non-text fields and unknown future fields reject data loss", t => {
  const db = fixture(t);
  for (const invalid of [
    { ...original(), sample: null },
    { ...original(), extra: "new schema field" },
    Object.fromEntries(Object.entries(original()).filter(([k]) => k !== "population")),
  ]) assert.throws(() => successor(db, { content: invalid }), /INVALID_SEVEN_FIELDS/);
  assert.equal(count(db, "workpaper_revisions"), 1);
});

test("deliberate empty string remains empty and every accepted Save makes a revision", t => {
  const db = fixture(t);
  const next = successor(db, { content: { ...original(), exceptions: "" } });
  const result = db.prepare("SELECT exceptions,conclusion,management_explanation FROM workpaper_revisions WHERE workpaper_revision_id=?").get(next);
  assert.equal(result.exceptions, "");
  assert.equal(result.conclusion, original().conclusion);
  assert.equal(result.management_explanation, original().management_explanation);
  assert.equal(count(db, "workpaper_revisions"), 2);
});

test("immutable production DDL rejects mutation of historical revision, links, and signoff", t => {
  const db = fixture(t);
  successor(db);
  assert.throws(() => db.exec("UPDATE workpaper_revisions SET conclusion='hijacked' WHERE workpaper_revision_id='rev-1'"), /immutable/);
  assert.throws(() => db.exec("DELETE FROM workpaper_evidence_links WHERE evidence_link_id='link-a'"), /immutable/);
  assert.throws(() => db.exec("UPDATE workpaper_signoffs SET actor_id='hijacked' WHERE signoff_id='sign-1'"), /immutable/);
});

test("cross-document evidence version cannot be transferred despite valid foreign keys", t => {
  const db = fixture(t);
  db.exec("INSERT INTO workpaper_evidence_links (evidence_link_id,workpaper_revision_id,document_id,content_version_id,controlled_evidence_version_id,relationship_type,created_at_ms) VALUES ('bad','rev-1','doc-a','vb',NULL,'CROSS_DOCUMENT',2)");
  assert.throws(() => successor(db), /INVALID_EVIDENCE_VERSION/);
  assert.equal(count(db, "workpaper_revisions"), 1);
});

test("quarantined controlled evidence cannot be silently carried into successor", t => {
  const db = fixture(t);
  db.exec("UPDATE controlled_evidence_versions SET verification_state='QUARANTINED' WHERE controlled_evidence_version_id='cb'");
  assert.throws(() => successor(db), /INVALID_EVIDENCE_VERSION/);
  assert.equal(count(db, "workpaper_revisions"), 1);
});

for (const stage of ["revision", "evidence", "signoff"]) {
  test("injected failure after " + stage + " rolls back revision, evidence, signoff and audit", t => {
    const db = fixture(t);
    assert.throws(() => successor(db, { failAt: stage }), /INJECTED_ROLLBACK/);
    assert.equal(count(db, "workpaper_revisions"), 1);
    assert.equal(count(db, "workpaper_evidence_links"), 2);
    assert.equal(count(db, "workpaper_signoff_supersessions"), 0);
    assert.equal(count(db, "audit_events"), 0);
  });
}

test("fixture audit records claimed synthetic actor; this is NOT real human identity proof", t => {
  const db = fixture(t);
  const next = successor(db);
  const event = db.prepare("SELECT entity_id,actor_id,details_json FROM audit_events WHERE event_type='FIXTURE_SUCCESSOR'").get();
  assert.equal(event.entity_id, next);
  assert.equal(event.actor_id, "author-1");
  assert.equal(JSON.parse(event.details_json).supersedes, "rev-1");
});
