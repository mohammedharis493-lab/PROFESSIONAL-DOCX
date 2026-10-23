import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import {
  correlateOptionalExtractionWithDocumentHistory,
} from "../src/document-match-source-version-gate.ts";

const HASH_A = "a".repeat(64);
const HASH_B = "b".repeat(64);
const file = Object.freeze({
  documentId: "document-1",
  fileInstanceId: "indexed-instance-1",
  availabilityState: "AVAILABLE",
  name: "preserved.pdf",
  path: "/readonly/original/preserved.pdf",
});
const snapshot = Object.freeze({
  documentId: file.documentId,
  fileInstanceId: file.fileInstanceId,
  extractionSnapshotId: "explicit-extraction-snapshot",
  sourceContentVersionId: "observed-version-1",
  sourceSha256Hex: HASH_A,
  fields: Object.freeze({ number: "INV-100", amountMinor: 2500 }),
});
const history = Object.freeze({
  requestedDocumentId: file.documentId,
  entries: Object.freeze([
    Object.freeze({
      contentVersionId: snapshot.sourceContentVersionId,
      fileInstanceId: file.fileInstanceId,
      verificationState: "HASH_VERIFIED",
      sourceStableDuringRead: true,
      sha256Hex: HASH_A,
    }),
  ]),
});
const reference = Object.freeze({
  referenceId: "manually-picked-reference",
  fields: Object.freeze({ number: "INV-999", amountMinor: 2500 }),
});
const rules = Object.freeze([
  Object.freeze({ ruleId: "number", label: "Number",
    documentField: "number", referenceField: "number", comparison: "exact-text" }),
  Object.freeze({ ruleId: "amount", label: "Minor unit amount",
    documentField: "amountMinor", referenceField: "amountMinor", comparison: "integer-minor-units" }),
]);
function check(overrides = {}) {
  return correlateOptionalExtractionWithDocumentHistory(
    overrides.file === undefined ? file : overrides.file,
    overrides.snapshot === undefined ? snapshot : overrides.snapshot,
    overrides.history === undefined ? history : overrides.history,
    overrides.reference === undefined ? reference : overrides.reference,
    overrides.rules === undefined ? rules : overrides.rules,
    overrides.requested === undefined ? false : overrides.requested,
  );
}
function nonblocking(outcome) {
  assert.equal(outcome.blocksDocumentUse, false);
  assert.equal(outcome.requiresDocumentChange, false);
  assert.equal(outcome.requiresReupload, false);
  assert.ok(Object.isFrozen(outcome));
  if (outcome.preparation) {
    assert.equal(outcome.preparation.blocksDocumentUse, false);
    assert.equal(outcome.preparation.requiresDocumentChange, false);
    assert.equal(outcome.preparation.requiresReupload, false);
  }
}
function denied(reason, options) {
  const outcome = check(options);
  assert.equal(outcome.status, "unavailable");
  assert.equal(outcome.reason, reason);
  assert.equal(outcome.preparation, null);
  nonblocking(outcome);
}
test("matching version metadata never automatically runs optional comparison", () => {
  const result = check();
  assert.equal(result.status, "correlated");
  assert.equal(result.preparation.status, "ready");
  assert.equal(result.preparation.observation.outcome, "not-requested");
  assert.deepEqual(result.preparation.observation.findings, []);
  nonblocking(result);
});
test("user-triggered comparison is a suggestion, not a document error", () => {
  const result = check({requested: true});
  assert.equal(result.status, "correlated");
  assert.equal(result.preparation.observation.outcome, "suggestions");
  assert.deepEqual(result.preparation.observation.findings.map(x => x.status),
    ["difference", "consistent"]);
  nonblocking(result);
});
test("no selected accounting reference remains nonblocking", () => {
  const result = check({reference: null, requested: true});
  assert.equal(result.status, "correlated");
  assert.equal(result.preparation.observation.outcome, "no-reference");
  nonblocking(result);
});
test("no comparison rules is a nonblocking advisory state", () => {
  const result = check({rules: [], requested: true});
  assert.equal(result.preparation.observation.outcome, "no-rules");
  nonblocking(result);
});
test("incomplete extraction or book data cannot invalidate original document", () => {
  const missingDocument = check({snapshot: {...snapshot, fields: {}}, requested: true});
  const missingBook = check({reference: {...reference, fields: {}}, requested: true});
  assert.equal(missingDocument.preparation.observation.outcome, "incomplete");
  assert.equal(missingBook.preparation.observation.outcome, "incomplete");
  nonblocking(missingDocument);
  nonblocking(missingBook);
});
test("missing, changed or unavailable source always disables the optional correlation", () => {
  for (const availabilityState of ["CHANGED", "MISSING", "UNKNOWN", "UNAVAILABLE", ""]) {
    denied("source-unavailable", {file: {...file, availabilityState}, requested: true});
  }
  denied("source-unavailable", {file: null, requested: true});
});
test("extraction must be explicit; no snapshot is inferred from filename or history", () => {
  denied("extraction-unavailable", {snapshot: null, requested: true});
  denied("snapshot-identity-mismatch", {
    snapshot: {...snapshot, fileInstanceId: "wrong-instance"}, requested: true,
  });
  denied("snapshot-identity-mismatch", {
    snapshot: {...snapshot, documentId: "wrong-document"}, requested: true,
  });
});
test("content version and hash claims must be explicit and well formed", () => {
  for (const sourceContentVersionId of ["", " ", null, undefined]) {
    denied("invalid-version-claim", {snapshot: {...snapshot, sourceContentVersionId}, requested: true});
  }
  for (const sourceSha256Hex of ["", null, undefined, HASH_A.toUpperCase(), HASH_A.slice(0, 63), "x".repeat(64)]) {
    denied("invalid-version-claim", {snapshot: {...snapshot, sourceSha256Hex}, requested: true});
  }
  denied("invalid-version-claim", {
    snapshot: {...snapshot, extractionSnapshotId: ""}, requested: true,
  });
});
test("history cannot be absent, malformed, or reused from another document", () => {
  denied("history-unavailable", {history: null, requested: true});
  denied("history-unavailable", {history: {requestedDocumentId: file.documentId, entries: null}});
  denied("history-identity-mismatch", {
    history: {...history, requestedDocumentId: "another-document"}, requested: true,
  });
});
test("explicit content version ID is required; never choose latest or similar hash", () => {
  denied("version-not-found", {
    snapshot: {...snapshot, sourceContentVersionId: "version-absent"}, requested: true,
  });
  denied("version-not-found", {
    history: {...history, entries: [{...history.entries[0], contentVersionId: "different-version"}]},
    requested: true,
  });
});
test("unverified or unstable historic observations cannot substantiate a version claim", () => {
  for (const verificationState of ["HASH_PENDING", "RECONCILIATION_REQUIRED", "UNKNOWN"]) {
    denied("history-not-verified", {
      history: {...history, entries: [{...history.entries[0], verificationState}]}, requested: true,
    });
  }
  for (const sourceStableDuringRead of [false, null, undefined]) {
    denied("history-not-verified", {
      history: {...history, entries: [{...history.entries[0], sourceStableDuringRead}]}, requested: true,
    });
  }
  for (const sha256Hex of [null, "", HASH_A.toUpperCase(), "invalid"]) {
    denied("history-not-verified", {
      history: {...history, entries: [{...history.entries[0], sha256Hex}]}, requested: true,
    });
  }
});
test("wrong hash is a missing correlation, not a document defect", () => {
  denied("source-hash-mismatch", {
    snapshot: {...snapshot, sourceSha256Hex: HASH_B}, requested: true,
  });
  denied("source-hash-mismatch", {
    history: {...history, entries: [{...history.entries[0], sha256Hex: HASH_B}]},
    requested: true,
  });
});
test("duplicate version-history rows with identical metadata are permitted", () => {
  const duplicated = check({
    history: {...history, entries: [history.entries[0], {...history.entries[0]}]},
    requested: true,
  });
  assert.equal(duplicated.status, "correlated");
  assert.equal(duplicated.preparation.observation.outcome, "suggestions");
  nonblocking(duplicated);
});
test("conflicting duplicate version-history rows fail closed", () => {
  for (const variation of [
    {sha256Hex: HASH_B},
    {fileInstanceId: "different-file-instance"},
    {verificationState: "UNKNOWN"},
    {sourceStableDuringRead: false},
  ]) {
    denied("version-ambiguous", {
      history: {...history, entries: [history.entries[0], {...history.entries[0], ...variation}]},
      requested: true,
    });
  }
});
test("unrelated histories and controlled evidence metadata cannot be used as extraction fields", () => {
  const result = check({
    history: {...history, entries: [
      {...history.entries[0], controlledEvidenceVersionId: "captured-evidence"},
      {contentVersionId: "unrelated", verificationState: "HASH_VERIFIED",
        sourceStableDuringRead: true, sha256Hex: HASH_B},
    ]},
    requested: true,
  });
  assert.equal(result.status, "correlated");
  const text = JSON.stringify(result);
  assert.doesNotMatch(text, /captured-evidence|unrelated|preserved\.pdf|\/readonly\/original/);
  nonblocking(result);
});
test("truthy nonboolean compare input is never treated as consent", () => {
  for (const requested of [1, "true", "false", {}, null, undefined]) {
    const result = check({requested});
    assert.equal(result.preparation.observation.outcome, "not-requested");
    nonblocking(result);
  }
});
test("inputs are not mutated or rewritten; no persistent annotations are generated", () => {
  const before = JSON.stringify([file, snapshot, history, reference, rules]);
  const result = check({requested: true});
  assert.equal(JSON.stringify([file, snapshot, history, reference, rules]), before);
  assert.ok(Object.isFrozen(result));
  assert.ok(Object.isFrozen(result.preparation.observation));
  assert.equal(result.preparation.observation.requiresReupload, false);
});
test("gate is pure and disconnected from Tauri, filesystem, native authority and live viewer", () => {
  const src = readFileSync("src/document-match-source-version-gate.ts", "utf8");
  const app = readFileSync("src/App.tsx", "utf8");
  const pkg = readFileSync("package.json", "utf8");
  assert.doesNotMatch(src, /@tauri-apps|invoke\(|fetch\(|writeFile|readFile|localStorage|sessionStorage|fs\.write|crypto\.subtle/);
  assert.doesNotMatch(app, /document-match-source-version-gate/);
  assert.match(pkg, /document-match-source-version-gate\.test\.mjs/);
  assert.match(src, /requested === true/);
});

test("malformed optional reference or rules cannot bubble up as document-use errors", () => {
  denied("comparison-input-invalid", {rules: [rules[0], rules[0]], requested: true});
  denied("comparison-input-invalid", {rules: [{...rules[0], comparison: "unknown"}], requested: true});
  denied("comparison-input-invalid", {
    reference: {referenceId: "book", fields: {amountMinor: {nested: true}}},
    requested: true,
  });
});

test("identical bytes do not authorize reusing a content version across indexed file instances", () => {
  const otherInstance = {
    ...file, fileInstanceId: "copy-with-identical-bytes",
  };
  const claimedCopy = {
    ...snapshot, fileInstanceId: otherInstance.fileInstanceId,
  };
  denied("history-instance-mismatch", {
    file: otherInstance,
    snapshot: claimedCopy,
    requested: true,
  });
  // The same SHA-256 on the wrong file instance is not a sufficient match.
  denied("history-instance-mismatch", {
    history: {...history, entries: [
      {...history.entries[0], fileInstanceId: "copy-with-identical-bytes"},
    ]},
    requested: true,
  });
});

test("history missing a real native file instance field never passes correlation", () => {
  for (const fileInstanceId of [undefined, null, "", " "]) {
    denied("history-instance-unavailable", {
      history: {...history, entries: [{...history.entries[0], fileInstanceId}]},
      requested: true,
    });
  }
});

test("a version from the exact selected file instance remains metadata-only and nonblocking", () => {
  const result = check({requested: true});
  assert.equal(result.status, "correlated");
  assert.equal(result.preparation.observation.outcome, "suggestions");
  assert.equal(result.requiresDocumentChange, false);
  assert.equal(result.requiresReupload, false);
});

test("existing native history DTO and SQLite query expose the same stored file instance", () => {
  const native = readFileSync("src-tauri/src/lib.rs", "utf8");
  const persistence = readFileSync("src-tauri/src/persistence.rs", "utf8");
  const app = readFileSync("src/App.tsx", "utf8");
  assert.match(native, /struct DocumentVersionHistoryDto\s*\{\s*content_version_id: String,\s*file_instance_id: String,/);
  assert.match(native, /file_instance_id: value\.file_instance_id/);
  assert.match(persistence, /pub struct DocumentVersionHistoryRecord\s*\{\s*pub content_version_id: String,\s*pub file_instance_id: String,/);
  assert.match(persistence, /cev\.capture_policy,\s*cv\.file_instance_id/);
  assert.match(app, /type DocumentVersionHistoryEntry = \{\s*contentVersionId: string;\s*fileInstanceId: string;/);
  // No new Tauri command or native mutation is necessary to get instance identity.
  assert.doesNotMatch(native, /fn (?:inspect|run|request)_optional_document_match/);
});

function throwingField(name) {
  return Object.defineProperty({}, name, {
    enumerable: true,
    get() { throw new Error("Malformed read-only metadata accessor"); },
  });
}

test("throwing file identity and availability accessors never disrupt document use", () => {
  for (const property of ["documentId", "fileInstanceId", "availabilityState"]) {
    const malformedFile = {...file};
    Object.defineProperty(malformedFile, property, {
      enumerable: true, get() { throw new Error("malformed indexed file"); },
    });
    for (const requested of [false, true]) {
      denied("comparison-input-invalid", {file: malformedFile, requested});
    }
  }
});

test("throwing extraction identity/version accessors fail closed before correlation", () => {
  for (const property of [
    "documentId", "fileInstanceId", "extractionSnapshotId",
    "sourceContentVersionId", "sourceSha256Hex",
  ]) {
    const malformedSnapshot = {...snapshot};
    Object.defineProperty(malformedSnapshot, property, {
      enumerable: true, get() { throw new Error("malformed extraction claim"); },
    });
    for (const requested of [false, true]) {
      denied("comparison-input-invalid", {snapshot: malformedSnapshot, requested});
    }
  }
});

test("throwing history request, entries, version and verification fields stay nonblocking", () => {
  for (const property of ["requestedDocumentId", "entries"]) {
    const malformedHistory = {...history};
    Object.defineProperty(malformedHistory, property, {
      enumerable: true, get() { throw new Error("history request unreadable"); },
    });
    denied("comparison-input-invalid", {history: malformedHistory, requested: true});
  }
  for (const property of [
    "contentVersionId", "fileInstanceId", "sha256Hex",
    "verificationState", "sourceStableDuringRead",
  ]) {
    const malformedRow = {...history.entries[0]};
    Object.defineProperty(malformedRow, property, {
      enumerable: true, get() { throw new Error("history row unreadable"); },
    });
    denied("comparison-input-invalid", {
      history: {...history, entries: [malformedRow]}, requested: true,
    });
  }
});

test("revoked metadata proxies cannot throw through the detached source gate", () => {
  const revokedFile = Proxy.revocable({}, {});
  revokedFile.revoke();
  denied("comparison-input-invalid", {file: revokedFile.proxy});
  const revokedSnapshot = Proxy.revocable({}, {});
  revokedSnapshot.revoke();
  denied("comparison-input-invalid", {snapshot: revokedSnapshot.proxy});
  const revokedHistory = Proxy.revocable({}, {});
  revokedHistory.revoke();
  denied("comparison-input-invalid", {history: revokedHistory.proxy});
  const revokedEntry = Proxy.revocable({}, {});
  revokedEntry.revoke();
  denied("comparison-input-invalid", {
    history: {...history, entries: [revokedEntry.proxy]},
  });
});

test("erroring unrelated version rows never authorize a guessed fallback version", () => {
  const badRow = throwingField("contentVersionId");
  denied("comparison-input-invalid", {
    history: {...history, entries: [badRow, history.entries[0]]},
    requested: true,
  });
  const normal = check({requested: true});
  assert.equal(normal.status, "correlated");
  nonblocking(normal);
});

test("source gate protects complete metadata preflight and never touches native authority", () => {
  const source = readFileSync("src/document-match-source-version-gate.ts", "utf8");
  assert.match(source, /Even preflight history\/file\/snapshot getters and array operations can/);
  assert.doesNotMatch(source,
    /@tauri-apps|invoke\(|fetch\(|fs\.write|writeFile|localStorage|sessionStorage/);
});
