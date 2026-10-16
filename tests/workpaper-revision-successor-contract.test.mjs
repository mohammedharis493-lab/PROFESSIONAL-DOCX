/**
 * TEST ONLY. Successor-write reference transaction using actual production
 * SQLite migration DDL. Not imported by src/ or src-tauri/src/, not a Save API.
 * __fixture_grants and subject IDs are SYNTHETIC: no authentication, native
 * principal issuer, real grant authority or evidence custody is provided.
 */
import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { randomUUID } from "node:crypto";
import { DatabaseSync } from "node:sqlite";
import { openStoredWorkpaperRevisionReadSnapshot } from "../src/workpaper-revision-read-snapshot.ts";
import {
  editWorkpaperRevisionSessionField,
  projectWorkpaperRevisionDraftContent,
} from "../src/workpaper-revision-edit-draft.ts";
import { inspectWorkpaperRevisionObservedText } from "../src/workpaper-revision-conflict-inspection.ts";

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
// Expected evidence is frozen at synthetic draft opening, never refreshed by Save.
const openedEvidence = new WeakMap();
// Only the test harness knows these temporary SQLite file paths.
const fixtureFiles = new WeakMap();
const fixtureSiblingConnections = new WeakMap();
const count = (db, name) => db.prepare("SELECT count(*) n FROM " + name).get().n;

function fixture(t, { fileBacked = false } = {}) {
  const directory = fileBacked ? mkdtempSync(join(tmpdir(), "docx-revision-fixture-")) : null;
  const path = directory ? join(directory, "workpapers.sqlite") : ":memory:";
  const db = new DatabaseSync(path);
  const siblings = [];
  // Close every independent connection before removing the file. This makes
  // cleanup safe on Windows, where deleting an open SQLite file can fail.
  t.after(() => {
    for (const sibling of siblings.reverse()) sibling.close();
    db.close();
    if (directory) rmSync(directory, { recursive: true, force: true });
  });
  if (directory) {
    fixtureFiles.set(db, path);
    fixtureSiblingConnections.set(db, siblings);
  }
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
    "INSERT INTO workpaper_signoff_evidence VALUES ('snapshot-1','sign-1','link-a',1),('snapshot-2','sign-1','link-b',1);",
    "INSERT INTO __fixture_grants VALUES ('author-1','wp-1','REVISE_WORKPAPER',0);",
  ].join("\n"));
  openedEvidence.set(db, observed(db));
  return db;
}

/**
 * Independent connection to a real, file-backed fixture database.
 * The connecting party cannot import the in-memory observation captured by
 * fixture(); each test must explicitly pass its frozen at-open evidence set.
 */
function secondConnection(t, sourceDb) {
  const file = fixtureFiles.get(sourceDb);
  if (!file) throw new Error("FILE_BACKED_FIXTURE_REQUIRED");
  const connection = new DatabaseSync(file);
  fixtureSiblingConnections.get(sourceDb).push(connection);
  connection.exec("PRAGMA foreign_keys=ON; PRAGMA busy_timeout=20");
  return connection;
}

/** Test oracle. The mock grant registry is not production authentication. */
function successor(db, options = {}) {
  const {
    subjectId = "author-1", workpaperId = "wp-1",
    expectedRevisionId = "rev-1", expectedEvidence = openedEvidence.get(db),
    content = { ...original(), conclusion: "Corrected conclusion" },
    failAt = "",
  } = options;
  if (!content || typeof content !== "object" || Array.isArray(content) ||
    Object.keys(content).length !== fields.length ||
    fields.some(k => !Object.hasOwn(content, k) || typeof content[k] !== "string")) {
    throw new Error("INVALID_SEVEN_FIELDS");
  }
  if (typeof expectedEvidence !== "string") throw new Error("MISSING_OPEN_EVIDENCE_SNAPSHOT");
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
      if (controlled) {
        // A HASH_VERIFIED label alone is insufficient. The retained version,
        // capture job, source version and file instance must all resolve to
        // the same exact document and source identity before reuse.
        const chain = db.prepare(
          "SELECT cev.document_id,cev.verification_state,cev.retention_state," +
          "cev.source_stable_during_read,cev.source_file_instance_id controlled_source_file_id," +
          "cev.source_content_version_id,cj.status capture_status," +
          "cj.document_id capture_document_id,cj.file_instance_id capture_file_instance_id," +
          "cv.document_id source_document_id,cv.file_instance_id version_source_file_id," +
          "fi.document_id instance_document_id " +
          "FROM controlled_evidence_versions cev " +
          "LEFT JOIN evidence_capture_jobs cj ON cj.evidence_capture_job_id=cev.evidence_capture_job_id " +
          "LEFT JOIN content_versions cv ON cv.content_version_id=cev.source_content_version_id " +
          "LEFT JOIN file_instances fi ON fi.file_instance_id=cev.source_file_instance_id " +
          "WHERE cev.controlled_evidence_version_id=?"
        ).get(link.controlled_evidence_version_id);
        if (!chain ||
          chain.document_id !== link.document_id ||
          chain.verification_state !== "HASH_VERIFIED" ||
          chain.retention_state !== "RETAINED" ||
          chain.source_stable_during_read !== 1 ||
          chain.capture_status !== "COMPLETE" ||
          chain.capture_document_id !== link.document_id ||
          chain.capture_file_instance_id !== chain.controlled_source_file_id ||
          chain.source_document_id !== link.document_id ||
          chain.version_source_file_id !== chain.controlled_source_file_id ||
          chain.instance_document_id !== link.document_id) {
          throw new Error("INVALID_EVIDENCE_VERSION");
        }
      } else {
        const version = db.prepare(
          "SELECT cv.document_id,fi.document_id instance_document_id " +
          "FROM content_versions cv LEFT JOIN file_instances fi " +
          "ON fi.file_instance_id=cv.file_instance_id WHERE cv.content_version_id=?"
        ).get(link.content_version_id);
        if (!version || version.document_id !== link.document_id ||
          version.instance_document_id !== link.document_id) {
          throw new Error("INVALID_EVIDENCE_VERSION");
        }
      }
    }
    // The production sign-off path snapshots EVERY link of its exact signed
    // revision. Before superseding any sign-off, verify historical sign-off
    // scope, exact link-set equality and supersession history. Production
    // migration DDL protects historical rows from update/delete but permits
    // malformed new rows to be inserted by lower-level SQL.
    const signoffBindings = db.prepare(
      "SELECT s.signoff_id id,s.workpaper_id signed_wp,s.workpaper_revision_id signed_revision," +
      "wr.workpaper_id revision_wp,wr.revision_number signed_number," +
      "ss.superseded_by_revision_id superseded_by,next.workpaper_id next_wp," +
      "next.revision_number next_number " +
      "FROM workpaper_signoffs s " +
      "LEFT JOIN workpaper_revisions wr ON wr.workpaper_revision_id=s.workpaper_revision_id " +
      "LEFT JOIN workpaper_signoff_supersessions ss ON ss.signoff_id=s.signoff_id " +
      "LEFT JOIN workpaper_revisions next ON next.workpaper_revision_id=ss.superseded_by_revision_id " +
      "WHERE s.workpaper_id=? OR wr.workpaper_id=?"
    ).all(workpaperId, workpaperId);
    const expectedSignedLinks = db.prepare(
      "SELECT evidence_link_id id FROM workpaper_evidence_links " +
      "WHERE workpaper_revision_id=? ORDER BY evidence_link_id"
    );
    const snapshottedLinks = db.prepare(
      "SELECT se.evidence_link_id id,wel.workpaper_revision_id link_revision " +
      "FROM workpaper_signoff_evidence se " +
      "LEFT JOIN workpaper_evidence_links wel ON wel.evidence_link_id=se.evidence_link_id " +
      "WHERE se.signoff_id=? ORDER BY se.evidence_link_id"
    );
    for (const signed of signoffBindings) {
      if (signed.signed_wp !== workpaperId ||
        signed.revision_wp !== workpaperId || !signed.signed_number) {
        throw new Error("INVALID_HISTORICAL_SIGNOFF_BINDING");
      }
      const exactIds = expectedSignedLinks.all(signed.signed_revision).map(x => x.id);
      const snapshot = snapshottedLinks.all(signed.id);
      if (snapshot.length !== exactIds.length ||
        snapshot.some((x, i) =>
          x.id !== exactIds[i] || x.link_revision !== signed.signed_revision)) {
        throw new Error("INVALID_HISTORICAL_SIGNOFF_EVIDENCE_SET");
      }
      if (signed.superseded_by !== null) {
        if (signed.next_wp !== workpaperId ||
          signed.next_number <= signed.signed_number) {
          throw new Error("INVALID_HISTORICAL_SIGNOFF_SUPERSESSION");
        }
      } else if (signed.signed_number < latest.num) {
        throw new Error("MISSING_HISTORICAL_SIGNOFF_SUPERSESSION");
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
  // Newly generated link IDs are random; order-by-ID is deliberately unstable.
  // Compare complete evidence tuples as multisets and distinct link identities.
  const exactTuples = links => links.map(({ document_id, content_version_id,
    controlled_evidence_version_id, relationship_type, description }) =>
    JSON.stringify([document_id, content_version_id,
      controlled_evidence_version_id, relationship_type, description])).sort();
  assert.deepEqual(exactTuples(after), exactTuples(before));
  const originalLinkIds = new Set(before.map(x => x.evidence_link_id));
  for (const link of after) assert.equal(originalLinkIds.has(link.evidence_link_id), false);
  assert.equal(new Set(after.map(x => x.evidence_link_id)).size, after.length);
  assert.deepEqual(rows(db), before);
});

test("historical signoff and its exact evidence snapshot stay old, successor unsigned", t => {
  const db = fixture(t);
  const next = successor(db);
  assert.equal(db.prepare("SELECT workpaper_revision_id FROM workpaper_signoffs WHERE signoff_id='sign-1'").get().workpaper_revision_id, "rev-1");
  assert.deepEqual(db.prepare("SELECT evidence_link_id FROM workpaper_signoff_evidence WHERE signoff_id='sign-1' ORDER BY evidence_link_id").all().map(x => x.evidence_link_id), ["link-a", "link-b"]);
  assert.deepEqual(rows(db).map(x => x.evidence_link_id), ["link-a", "link-b"]);
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
  assert.throws(() => successor(db, { content: original() }), /STALE_EVIDENCE/);
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
  assert.throws(() => successor(db, { expectedEvidence: observed(db) }), /INVALID_EVIDENCE_VERSION/);
  assert.equal(count(db, "workpaper_revisions"), 1);
});

test("quarantined controlled evidence cannot be silently carried into successor", t => {
  const db = fixture(t);
  db.exec("UPDATE controlled_evidence_versions SET verification_state='QUARANTINED' WHERE controlled_evidence_version_id='cb'");
  assert.throws(() => successor(db), /INVALID_EVIDENCE_VERSION/);
  assert.equal(count(db, "workpaper_revisions"), 1);
});


test("HASH_VERIFIED label does not substitute for successful completed capture job", t => {
  const db = fixture(t);
  db.exec("UPDATE evidence_capture_jobs SET status='FAILED' WHERE evidence_capture_job_id='job-b'");
  assert.throws(() => successor(db), /INVALID_EVIDENCE_VERSION/);
  assert.equal(count(db, "workpaper_revisions"), 1);
  assert.equal(count(db, "workpaper_signoff_supersessions"), 0);
});

test("controlled evidence must be tied to the same captured document", t => {
  const db = fixture(t);
  db.exec("UPDATE evidence_capture_jobs SET document_id='doc-a' WHERE evidence_capture_job_id='job-b'");
  assert.throws(() => successor(db), /INVALID_EVIDENCE_VERSION/);
  assert.equal(count(db, "workpaper_revisions"), 1);
});

test("captured file instance must equal the controlled evidence source instance", t => {
  const db = fixture(t);
  db.exec("UPDATE evidence_capture_jobs SET file_instance_id='fa' WHERE evidence_capture_job_id='job-b'");
  assert.throws(() => successor(db), /INVALID_EVIDENCE_VERSION/);
  assert.equal(count(db, "workpaper_revisions"), 1);
});

test("controlled source content version cannot silently change to another document", t => {
  const db = fixture(t);
  db.exec("UPDATE controlled_evidence_versions SET source_content_version_id='va' WHERE controlled_evidence_version_id='cb'");
  assert.throws(() => successor(db), /INVALID_EVIDENCE_VERSION/);
  assert.equal(count(db, "workpaper_revisions"), 1);
});

test("controlled source file cannot silently change to another file instance", t => {
  const db = fixture(t);
  db.exec("UPDATE controlled_evidence_versions SET source_file_instance_id='fa' WHERE controlled_evidence_version_id='cb'");
  assert.throws(() => successor(db), /INVALID_EVIDENCE_VERSION/);
  assert.equal(count(db, "workpaper_revisions"), 1);
});

test("ordinary content-version file instance must belong to that exact document", t => {
  const db = fixture(t);
  db.exec("UPDATE content_versions SET file_instance_id='fb' WHERE content_version_id='va'");
  assert.throws(() => successor(db), /INVALID_EVIDENCE_VERSION/);
  assert.equal(count(db, "workpaper_revisions"), 1);
});

test("controlled-version document must match the signed source document", t => {
  const db = fixture(t);
  db.exec("UPDATE controlled_evidence_versions SET document_id='doc-a' WHERE controlled_evidence_version_id='cb'");
  assert.throws(() => successor(db), /INVALID_EVIDENCE_VERSION/);
  assert.equal(count(db, "workpaper_revisions"), 1);
});


test("the signed base fixture represents the full exact evidence-link set", t => {
  const db = fixture(t);
  const signedIds = db.prepare("SELECT evidence_link_id id FROM workpaper_signoff_evidence WHERE signoff_id='sign-1' ORDER BY evidence_link_id")
    .all().map(x => x.id);
  assert.deepEqual(signedIds, rows(db).map(x => x.evidence_link_id));
  assert.equal(signedIds.length, 2);
});

test("a sign-off missing one source evidence snapshot blocks successor creation", t => {
  const db = fixture(t);
  db.exec("INSERT INTO workpaper_signoffs (signoff_id,workpaper_id,workpaper_revision_id,signoff_type,actor_id,actor_role,signed_at_ms) VALUES ('incomplete','wp-1','rev-1','PREPARED','fixture','PREPARER',1)");
  db.exec("INSERT INTO workpaper_signoff_evidence VALUES ('incomplete-a','incomplete','link-a',1)");
  assert.throws(() => successor(db), /INVALID_HISTORICAL_SIGNOFF_EVIDENCE_SET/);
  assert.equal(count(db, "workpaper_revisions"), 1);
  assert.equal(count(db, "workpaper_signoff_supersessions"), 0);
});

test("a sign-off with zero captured links cannot claim two-link signed revision", t => {
  const db = fixture(t);
  db.exec("INSERT INTO workpaper_signoffs (signoff_id,workpaper_id,workpaper_revision_id,signoff_type,actor_id,actor_role,signed_at_ms) VALUES ('empty','wp-1','rev-1','PREPARED','fixture','PREPARER',1)");
  assert.throws(() => successor(db), /INVALID_HISTORICAL_SIGNOFF_EVIDENCE_SET/);
  assert.equal(count(db, "workpaper_revisions"), 1);
});

test("an extra link appended to signed evidence history blocks silent inheritance", t => {
  const db = fixture(t);
  db.exec("INSERT INTO workpaper_revisions (workpaper_revision_id,workpaper_id,revision_number,created_at_ms) VALUES ('other-rev','wp-2',1,1)");
  db.exec("INSERT INTO workpaper_evidence_links (evidence_link_id,workpaper_revision_id,document_id,content_version_id,relationship_type,created_at_ms) VALUES ('foreign-link','other-rev','doc-a','va','SUPPORTS',1)");
  db.exec("INSERT INTO workpaper_signoff_evidence VALUES ('malicious-extra','sign-1','foreign-link',2)");
  assert.throws(() => successor(db), /INVALID_HISTORICAL_SIGNOFF_EVIDENCE_SET/);
  assert.equal(count(db, "workpaper_revisions"), 2);
  assert.equal(count(db, "workpaper_signoff_supersessions"), 0);
});

test("sign-off claiming a different workpaper with a foreign revision fails closed", t => {
  const db = fixture(t);
  db.exec("INSERT INTO workpaper_signoffs (signoff_id,workpaper_id,workpaper_revision_id,signoff_type,actor_id,actor_role,signed_at_ms) VALUES ('foreign-sign','wp-2','rev-1','PREPARED','fixture','PREPARER',1)");
  assert.throws(() => successor(db), /INVALID_HISTORICAL_SIGNOFF_BINDING/);
  assert.equal(count(db, "workpaper_revisions"), 1);
});

test("sign-off snapshot referencing only a different revision fails closed", t => {
  const db = fixture(t);
  db.exec("INSERT INTO workpaper_revisions (workpaper_revision_id,workpaper_id,revision_number,created_at_ms) VALUES ('other-rev','wp-2',1,1)");
  db.exec("INSERT INTO workpaper_evidence_links (evidence_link_id,workpaper_revision_id,document_id,content_version_id,relationship_type,created_at_ms) VALUES ('foreign-link','other-rev','doc-a','va','SUPPORTS',1)");
  db.exec("INSERT INTO workpaper_signoffs (signoff_id,workpaper_id,workpaper_revision_id,signoff_type,actor_id,actor_role,signed_at_ms) VALUES ('malformed-sign','wp-1','rev-1','PREPARED','fixture','PREPARER',1)");
  db.exec("INSERT INTO workpaper_signoff_evidence VALUES ('malformed-snapshot','malformed-sign','foreign-link',1)");
  assert.throws(() => successor(db), /INVALID_HISTORICAL_SIGNOFF_EVIDENCE_SET/);
  assert.equal(count(db, "workpaper_signoff_supersessions"), 0);
});

test("a prior supersession pointed at another workpaper cannot be trusted", t => {
  const db = fixture(t);
  db.exec("INSERT INTO workpaper_revisions (workpaper_revision_id,workpaper_id,revision_number,created_at_ms) VALUES ('other-rev','wp-2',1,2)");
  db.exec("INSERT INTO workpaper_signoff_supersessions (signoff_supersession_id,signoff_id,superseded_by_revision_id,superseded_at_ms,superseded_reason) VALUES ('bad-sup','sign-1','other-rev',2,'Malformed fixture row')");
  assert.throws(() => successor(db), /INVALID_HISTORICAL_SIGNOFF_SUPERSESSION/);
  assert.equal(count(db, "workpaper_revisions"), 2);
});

test("appended historical signed-evidence rows cannot be changed or deleted", t => {
  const db = fixture(t);
  assert.throws(() => db.exec("DELETE FROM workpaper_signoff_evidence WHERE signoff_evidence_id='snapshot-2'"), /immutable/);
  assert.throws(() => db.exec("UPDATE workpaper_signoff_evidence SET evidence_link_id='link-a' WHERE signoff_evidence_id='snapshot-2'"), /immutable/);
  assert.equal(count(db, "workpaper_signoff_evidence"), 2);
});

test("second successor retains the first supersession without renewing approval", t => {
  const db = fixture(t);
  const second = successor(db);
  const next = successor(db, {
    expectedRevisionId: second,
    expectedEvidence: observed(db, second),
    content: { ...original(), conclusion: "Third unsigned revision" },
  });
  const historical = db.prepare("SELECT superseded_by_revision_id FROM workpaper_signoff_supersessions WHERE signoff_id='sign-1'").get();
  assert.equal(historical.superseded_by_revision_id, second);
  assert.equal(db.prepare("SELECT supersedes_revision_id FROM workpaper_revisions WHERE workpaper_revision_id=?").get(next).supersedes_revision_id, second);
  assert.equal(db.prepare("SELECT count(*) n FROM workpaper_signoffs WHERE workpaper_revision_id=?").get(next).n, 0);
  assert.equal(count(db, "workpaper_signoff_supersessions"), 1);
  assert.deepEqual(rows(db, second).map(x => x.evidence_link_id).length, 2);
});

test("missing supersession of an older signed revision cannot be silently ignored", t => {
  const db = fixture(t);
  db.exec("INSERT INTO workpaper_revisions (workpaper_revision_id,workpaper_id,revision_number,created_at_ms) VALUES ('unexpected-next','wp-1',2,2)");
  assert.throws(() => successor(db, {
    expectedRevisionId: "unexpected-next",
    expectedEvidence: observed(db, "unexpected-next"),
    content: original(),
  }), /MISSING_HISTORICAL_SIGNOFF_SUPERSESSION/);
  assert.equal(count(db, "workpaper_revisions"), 2);
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


test("two independent SQLite connections cannot accept the same expected base twice", t => {
  const first = fixture(t, { fileBacked: true });
  const second = secondConnection(t, first);
  const atOpen = observed(second);
  const firstResult = successor(first);
  assert.throws(() => successor(second, {
    expectedRevisionId: "rev-1",
    expectedEvidence: atOpen,
    content: { ...original(), conclusion: "Second stale user's edit" },
  }), /STALE_REVISION/);
  assert.equal(count(second, "workpaper_revisions"), 2);
  assert.equal(count(first, "workpaper_revisions"), 2);
  assert.equal(count(second, "workpaper_signoff_supersessions"), 1);
  assert.equal(second.prepare("SELECT superseded_by_revision_id id FROM workpaper_signoff_supersessions WHERE signoff_id='sign-1'").get().id, firstResult);
});

test("file-backed BEGIN IMMEDIATE reservation prevents another connection's concurrent Save", t => {
  const first = fixture(t, { fileBacked: true });
  const second = secondConnection(t, first);
  const atOpen = observed(second);
  first.exec("BEGIN IMMEDIATE");
  try {
    assert.throws(() => successor(second, { expectedEvidence: atOpen }), /SQLITE_BUSY|database is locked/i);
    assert.equal(count(first, "workpaper_revisions"), 1);
  } finally {
    first.exec("ROLLBACK");
  }
  const next = successor(second, { expectedEvidence: atOpen });
  assert.equal(second.prepare("SELECT revision_number n FROM workpaper_revisions WHERE workpaper_revision_id=?").get(next).n, 2);
  assert.equal(count(first, "workpaper_revisions"), 2);
});

test("another SQLite connection revoking a mock grant before Save causes denial", t => {
  const first = fixture(t, { fileBacked: true });
  const second = secondConnection(t, first);
  const atOpen = observed(second);
  first.exec("UPDATE __fixture_grants SET revoked=1 WHERE subject_id='author-1'");
  assert.throws(() => successor(second, { expectedEvidence: atOpen }), /DENIED_NO_REVISION_GRANT/);
  assert.equal(count(first, "workpaper_revisions"), 1);
  assert.equal(count(first, "audit_events"), 0);
});

test("another connection appending evidence after editor open triggers stale-evidence rejection", t => {
  const first = fixture(t, { fileBacked: true });
  const second = secondConnection(t, first);
  const atOpen = observed(second);
  first.exec("INSERT INTO workpaper_evidence_links (evidence_link_id,workpaper_revision_id,document_id,content_version_id,relationship_type,created_at_ms) VALUES ('later-link','rev-1','doc-a','va','ADDED_AFTER_OPEN',2)");
  assert.throws(() => successor(second, {
    expectedEvidence: atOpen, content: original(),
  }), /STALE_EVIDENCE/);
  assert.equal(count(first, "workpaper_revisions"), 1);
  assert.equal(count(first, "workpaper_signoff_supersessions"), 0);
});

test("failed file-backed Save rolls back for a different SQLite connection", t => {
  const first = fixture(t, { fileBacked: true });
  const second = secondConnection(t, first);
  const atOpen = observed(second);
  assert.throws(() => successor(first, { failAt: "signoff" }), /INJECTED_ROLLBACK/);
  assert.equal(count(second, "workpaper_revisions"), 1);
  assert.equal(count(second, "workpaper_evidence_links"), 2);
  assert.equal(count(second, "workpaper_signoff_supersessions"), 0);
  assert.equal(count(second, "audit_events"), 0);
  const next = successor(second, { expectedEvidence: atOpen });
  assert.equal(second.prepare("SELECT revision_number n FROM workpaper_revisions WHERE workpaper_revision_id=?").get(next).n, 2);
  assert.equal(count(first, "workpaper_revisions"), 2);
});

test("a committed successor is visible to a new independent SQLite connection", t => {
  const first = fixture(t, { fileBacked: true });
  const second = secondConnection(t, first);
  const next = successor(first);
  const third = secondConnection(t, first);
  assert.equal(third.prepare("SELECT supersedes_revision_id base FROM workpaper_revisions WHERE workpaper_revision_id=?").get(next).base, "rev-1");
  assert.deepEqual(
    rows(third, next).map(x => JSON.stringify([
      x.document_id, x.content_version_id, x.controlled_evidence_version_id,
      x.relationship_type, x.description,
    ])).sort(),
    rows(second).map(x => JSON.stringify([
      x.document_id, x.content_version_id, x.controlled_evidence_version_id,
      x.relationship_type, x.description,
    ])).sort(),
  );
  assert.equal(third.prepare("SELECT superseded_by_revision_id id FROM workpaper_signoff_supersessions WHERE signoff_id='sign-1'").get().id, next);
  assert.equal(third.prepare("SELECT count(*) n FROM workpaper_signoffs WHERE workpaper_revision_id=?").get(next).n, 0);
});

test("fixture audit records claimed synthetic actor; this is NOT real human identity proof", t => {
  const db = fixture(t);
  const next = successor(db);
  const event = db.prepare("SELECT entity_id,actor_id,details_json FROM audit_events WHERE event_type='FIXTURE_SUCCESSOR'").get();
  assert.equal(event.entity_id, next);
  assert.equal(event.actor_id, "author-1");
  assert.equal(JSON.parse(event.details_json).supersedes, "rev-1");
});


/**
 * CROSS-LAYER TEST FIXTURE ONLY. The explicit DTO mapping below joins the
 * production SQLite DDL to the unmounted read/editor helper and to the
 * synthetic successor transaction. This is NOT a production adapter, Tauri
 * command, authenticated data service, permission or Save implementation.
 *
 * Crucially the exact source evidence expectation is detached at OPEN time,
 * never rebuilt just before Save to mask evidence-only races.
 */
const canonicalSqlFields = Object.freeze([
  ["objective", "objective"],
  ["procedurePerformed", "procedure_performed"],
  ["population", "population"],
  ["sample", "sample"],
  ["exceptions", "exceptions"],
  ["managementExplanation", "management_explanation"],
  ["conclusion", "conclusion"],
]);
function readFixtureIntoEditor(db, workpaperId = "wp-1") {
  const sqlRevisions = db.prepare(
    "SELECT * FROM workpaper_revisions WHERE workpaper_id=? ORDER BY revision_number"
  ).all(workpaperId);
  const revisions = sqlRevisions.map(row => ({
    workpaperId: row.workpaper_id,
    workpaperRevisionId: row.workpaper_revision_id,
    revisionNumber: row.revision_number,
    ...Object.fromEntries(canonicalSqlFields.map(([ui, sql]) => [ui, row[sql]])),
  }));
  const latest = revisions.at(-1);
  const sourceLinks = latest ? rows(db, latest.workpaperRevisionId) : [];
  const evidenceLinks = sourceLinks.map(link => ({
    evidenceLinkId: link.evidence_link_id,
    workpaperRevisionId: latest.workpaperRevisionId,
    documentId: link.document_id,
    contentVersionId: link.content_version_id,
    controlledEvidenceVersionId: link.controlled_evidence_version_id,
    relationshipType: link.relationship_type,
    description: link.description,
  }));
  const signoffs = db.prepare(
    "SELECT signoff_id,workpaper_id,workpaper_revision_id " +
    "FROM workpaper_signoffs WHERE workpaper_id=?"
  ).all(workpaperId).map(row => ({
    signoffId: row.signoff_id,
    workpaperId: row.workpaper_id,
    workpaperRevisionId: row.workpaper_revision_id,
  }));
  const read = openStoredWorkpaperRevisionReadSnapshot({
    workpaperId, revisions, evidenceLinks, signoffs,
  });
  return Object.freeze({
    read,
    // Immutable string detached while the draft is opened, not by Save.
    expectedEvidenceAtOpen: JSON.stringify(sourceLinks),
  });
}
function submitToSyntheticSuccessor(db, opened, session, { subjectId = "author-1" } = {}) {
  const projection = projectWorkpaperRevisionDraftContent(session);
  assert.equal(projection.workpaperId, opened.read.session.base.workpaperId);
  assert.equal(projection.expectedLatestRevisionId,
    opened.read.session.base.workpaperRevisionId);
  const content = Object.fromEntries(
    canonicalSqlFields.map(([ui, sql]) => [sql, projection.content[ui]])
  );
  // In real code the server MUST independently establish the actor and
  // verify grants, evidence custody, CAS and sign-off state in one transaction.
  return successor(db, {
    subjectId,
    workpaperId: projection.workpaperId,
    expectedRevisionId: projection.expectedLatestRevisionId,
    expectedEvidence: opened.expectedEvidenceAtOpen,
    content,
  });
}
function evidenceTuples(db, revision) {
  return rows(db, revision).map(link => JSON.stringify([
    link.document_id,
    link.content_version_id,
    link.controlled_evidence_version_id,
    link.relationship_type,
    link.description,
  ])).sort();
}

test("cross-layer signed typo correction preserves all seven stored fields and exact source evidence", t => {
  const db = fixture(t);
  const opened = readFixtureIntoEditor(db);
  assert.equal(opened.read.observedBaseSignoffCount, 1);
  assert.deepEqual(opened.read.session.draft.content, {
    objective: "Objective\r\noriginal ",
    procedurePerformed: "Trace approvals",
    population: "All entries",
    sample: "Sample twenty",
    exceptions: "One exception",
    managementExplanation: "Reply\u00a0 ",
    conclusion: "Prior signed conclusion",
  });
  const changed = editWorkpaperRevisionSessionField(
    opened.read.session, "conclusion", "Prior signed conclusion.",
  );
  const next = submitToSyntheticSuccessor(db, opened, changed);
  const stored = db.prepare("SELECT * FROM workpaper_revisions WHERE workpaper_revision_id=?").get(next);
  assert.equal(stored.revision_number, 2);
  assert.equal(stored.supersedes_revision_id, "rev-1");
  for (const [ui, sql] of canonicalSqlFields) {
    assert.equal(stored[sql], changed.draft.content[ui], "mismatched canonical field " + ui);
  }
  assert.deepEqual(evidenceTuples(db, next), evidenceTuples(db, "rev-1"));
  assert.equal(new Set(rows(db, next).map(x => x.evidence_link_id)).size, 2);
  for (const link of rows(db, next)) {
    assert.equal(["link-a", "link-b"].includes(link.evidence_link_id), false);
  }
  assert.equal(db.prepare(
    "SELECT superseded_by_revision_id id FROM workpaper_signoff_supersessions WHERE signoff_id='sign-1'"
  ).get().id, next);
  assert.equal(db.prepare(
    "SELECT count(*) n FROM workpaper_signoffs WHERE workpaper_revision_id=?"
  ).get(next).n, 0);
  assert.equal(db.prepare(
    "SELECT workpaper_revision_id id FROM workpaper_signoffs WHERE signoff_id='sign-1'"
  ).get().id, "rev-1");
});

test("cross-layer unchanged-text deliberate Save still makes a new unsigned successor", t => {
  const db = fixture(t);
  const opened = readFixtureIntoEditor(db);
  const next = submitToSyntheticSuccessor(db, opened, opened.read.session);
  const current = db.prepare("SELECT * FROM workpaper_revisions WHERE workpaper_revision_id=?").get(next);
  for (const [ui, sql] of canonicalSqlFields) {
    assert.equal(current[sql], opened.read.session.base.content[ui]);
  }
  assert.equal(current.revision_number, 2);
  assert.equal(count(db, "workpaper_revisions"), 2);
  assert.equal(count(db, "workpaper_signoffs"), 1);
  assert.equal(count(db, "workpaper_signoff_supersessions"), 1);
  assert.deepEqual(evidenceTuples(db, next), evidenceTuples(db, "rev-1"));
});

test("cross-layer all-seven-field edits preserve exact Unicode, CRLF, spaces and deliberate blanks", t => {
  const db = fixture(t);
  const opened = readFixtureIntoEditor(db);
  const edited = {
    objective: "X\r\nY  ",
    procedurePerformed: "🧪\u00a0Audit\t",
    population: "",
    sample: "ⅠⅢ samples",
    exceptions: "False\n",
    managementExplanation: "Response e\u0301 / é",
    conclusion: "\u0000End ",
  };
  let session = opened.read.session;
  for (const [field, value] of Object.entries(edited)) {
    session = editWorkpaperRevisionSessionField(session, field, value);
  }
  const next = submitToSyntheticSuccessor(db, opened, session);
  const row = db.prepare("SELECT * FROM workpaper_revisions WHERE workpaper_revision_id=?").get(next);
  for (const [ui, sql] of canonicalSqlFields) {
    assert.equal(row[sql], edited[ui]);
  }
  assert.equal(row.objective, "X\r\nY  ");
  assert.equal(row.conclusion, "\u0000End ");
  assert.deepEqual(evidenceTuples(db, next), evidenceTuples(db, "rev-1"));
});

test("cross-layer exact version observations remain separate from projected editable text", t => {
  const db = fixture(t);
  const opened = readFixtureIntoEditor(db);
  const projection = projectWorkpaperRevisionDraftContent(opened.read.session);
  assert.equal(opened.read.observedSourceEvidence.length, 2);
  assert.deepEqual(opened.read.observedSourceEvidence.map(x => x.evidenceLinkId).sort(),
    ["link-a", "link-b"]);
  assert.deepEqual(Object.keys(projection).sort(),
    ["content","expectedLatestRevisionId","scope","workpaperId"].sort());
  assert.equal("evidenceLinkIds" in projection, false);
  assert.equal("signoffIds" in projection, false);
  assert.ok(Object.isFrozen(projection.content));
  assert.ok(Object.isFrozen(opened.read.observedSourceEvidence));
});

test("cross-layer evidence-only race rejects an unchanged-text Save rather than refreshing at Save time", t => {
  const db = fixture(t);
  const opened = readFixtureIntoEditor(db);
  db.exec(
    "INSERT INTO workpaper_evidence_links " +
    "(evidence_link_id,workpaper_revision_id,document_id,content_version_id,relationship_type,created_at_ms) " +
    "VALUES ('new-evidence-after-open','rev-1','doc-a','va','CORROBORATES',3)"
  );
  assert.throws(() => submitToSyntheticSuccessor(db, opened, opened.read.session),
    /STALE_EVIDENCE/);
  assert.equal(count(db, "workpaper_revisions"), 1);
  assert.equal(count(db, "workpaper_signoff_supersessions"), 0);
  assert.equal(count(db, "audit_events"), 0);
});

test("cross-layer latest revision conflict never silently reopens or overwrites stale base", t => {
  const db = fixture(t);
  const old = readFixtureIntoEditor(db);
  const local = editWorkpaperRevisionSessionField(old.read.session,
    "objective", "Local correction");
  const first = submitToSyntheticSuccessor(db, old, old.read.session);
  const now = readFixtureIntoEditor(db);
  assert.equal(now.read.session.base.workpaperRevisionId, first);
  assert.equal(now.read.observedBaseSignoffCount, 0);
  const comparison = inspectWorkpaperRevisionObservedText(local, now.read.session.base);
  assert.equal(comparison.differentRevisionObserved, true);
  assert.equal(comparison.fields.find(x => x.field === "objective").status, "local-only");
  assert.throws(() => submitToSyntheticSuccessor(db, old, local),
    /STALE_REVISION/);
  assert.equal(local.draft.expectedLatestRevisionId, "rev-1");
  assert.equal(local.draft.content.objective, "Local correction");
  assert.equal(count(db, "workpaper_revisions"), 2);
});

test("cross-layer in-flight grant revocation is rechecked at synthetic SQLite write boundary", t => {
  const db = fixture(t);
  const opened = readFixtureIntoEditor(db);
  const changed = editWorkpaperRevisionSessionField(
    opened.read.session, "exceptions", "One exception, corrected.",
  );
  db.exec("UPDATE __fixture_grants SET revoked=1 WHERE subject_id='author-1'");
  assert.throws(() => submitToSyntheticSuccessor(db, opened, changed),
    /DENIED_NO_REVISION_GRANT/);
  assert.equal(count(db, "workpaper_revisions"), 1);
  assert.equal(count(db, "workpaper_signoff_supersessions"), 0);
});

test("cross-layer signoff made after draft opened remains historical and is superseded, not inherited", t => {
  const db = fixture(t);
  const opened = readFixtureIntoEditor(db);
  db.exec(
    "INSERT INTO workpaper_signoffs " +
    "(signoff_id,workpaper_id,workpaper_revision_id,signoff_type,actor_id,actor_role,signed_at_ms) " +
    "VALUES ('sign-later','wp-1','rev-1','PREPARED','reviewer-fixture','PREPARER',3)"
  );
  db.exec(
    "INSERT INTO workpaper_signoff_evidence VALUES " +
    "('snapshot-later-a','sign-later','link-a',3)," +
    "('snapshot-later-b','sign-later','link-b',3)"
  );
  const edited = editWorkpaperRevisionSessionField(
    opened.read.session, "sample", "Sample twenty-one",
  );
  const next = submitToSyntheticSuccessor(db, opened, edited);
  assert.equal(db.prepare(
    "SELECT count(*) n FROM workpaper_signoff_supersessions " +
    "WHERE superseded_by_revision_id=?"
  ).get(next).n, 2);
  assert.equal(db.prepare(
    "SELECT count(*) n FROM workpaper_signoffs WHERE workpaper_revision_id=?"
  ).get(next).n, 0);
  assert.equal(opened.read.observedBaseSignoffCount, 1);
  assert.equal(db.prepare(
    "SELECT workpaper_revision_id id FROM workpaper_signoffs WHERE signoff_id='sign-later'"
  ).get().id, "rev-1");
});

test("cross-layer malformed future canonical text is refused before entering the transaction", t => {
  const db = fixture(t);
  const opened = readFixtureIntoEditor(db);
  const corrupt = {
    ...opened.read.session,
    draft: {
      ...opened.read.session.draft,
      content: { ...opened.read.session.draft.content, futureField: "new data" },
    },
  };
  assert.throws(() => submitToSyntheticSuccessor(db, opened, corrupt),
    /Unexpected canonical content field: futureField/);
  assert.equal(count(db, "workpaper_revisions"), 1);
  assert.equal(count(db, "audit_events"), 0);
});

test("cross-layer denied synthetic actor cannot use a valid detached text projection", t => {
  const db = fixture(t);
  const opened = readFixtureIntoEditor(db);
  assert.throws(
    () => submitToSyntheticSuccessor(db, opened, opened.read.session, {
      subjectId: "not-granted",
    }),
    /DENIED_NO_REVISION_GRANT/
  );
  assert.equal(count(db, "workpaper_revisions"), 1);
});

test("cross-layer second open after successor targets latest and retains exact evidence across chains", t => {
  const db = fixture(t);
  const old = readFixtureIntoEditor(db);
  const second = submitToSyntheticSuccessor(db, old,
    editWorkpaperRevisionSessionField(old.read.session,
      "conclusion", "Second conclusion"));
  const current = readFixtureIntoEditor(db);
  assert.equal(current.read.session.base.workpaperRevisionId, second);
  assert.equal(current.read.observedBaseSignoffCount, 0);
  const third = submitToSyntheticSuccessor(db, current,
    editWorkpaperRevisionSessionField(current.read.session,
      "conclusion", "Third conclusion"));
  const thirdRow = db.prepare(
    "SELECT revision_number,conclusion,supersedes_revision_id " +
    "FROM workpaper_revisions WHERE workpaper_revision_id=?"
  ).get(third);
  assert.equal(thirdRow.revision_number, 3);
  assert.equal(thirdRow.conclusion, "Third conclusion");
  assert.equal(thirdRow.supersedes_revision_id, second);
  assert.deepEqual(evidenceTuples(db, third), evidenceTuples(db, second));
  assert.deepEqual(evidenceTuples(db, second), evidenceTuples(db, "rev-1"));
  assert.equal(count(db, "workpaper_signoff_supersessions"), 1);
  assert.equal(count(db, "workpaper_signoffs"), 1);
});

