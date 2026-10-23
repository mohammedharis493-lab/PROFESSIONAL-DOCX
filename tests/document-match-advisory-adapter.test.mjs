import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import {
  inspectOptionalIndexedDocumentMatch,
} from "../src/document-match-advisory-adapter.ts";
import { noteOptionalMatchFinding } from "../src/document-match-advisory.ts";

const file = Object.freeze({
  documentId: "doc-from-index",
  fileInstanceId: "instance-from-index",
  name: "file.pdf",
  path: "C:/immutable/original/file.pdf",
  availabilityState: "AVAILABLE",
});
const snapshot = Object.freeze({
  documentId: file.documentId,
  fileInstanceId: file.fileInstanceId,
  extractionSnapshotId: "explicit-snapshot-9",
  fields: Object.freeze({ voucher: "INV-001", amountMinor: 12500, currency: "INR" }),
});
const reference = Object.freeze({
  referenceId: "explicitly-selected-reference",
  fields: Object.freeze({ voucher: "INV-002", amountMinor: 12500, currency: "INR" }),
});
const rules = Object.freeze([
  Object.freeze({ ruleId: "voucher", label: "Voucher reference", documentField: "voucher",
    referenceField: "voucher", comparison: "exact-text" }),
  Object.freeze({ ruleId: "amount", label: "Amount in minor units", documentField: "amountMinor",
    referenceField: "amountMinor", comparison: "integer-minor-units" }),
  Object.freeze({ ruleId: "currency", label: "Currency", documentField: "currency",
    referenceField: "currency", comparison: "exact-text" }),
]);

function inspect(extraction = snapshot, book = reference, mappings = rules, requested = false) {
  return inspectOptionalIndexedDocumentMatch(file, extraction, book, mappings, requested);
}

function assertNonblocking(prepared) {
  assert.equal(prepared.blocksDocumentUse, false);
  assert.equal(prepared.requiresDocumentChange, false);
  assert.equal(prepared.requiresReupload, false);
  assert.ok(Object.isFrozen(prepared));
  if (prepared.observation) {
    assert.equal(prepared.observation.blocksDocumentUse, false);
    assert.equal(prepared.observation.requiresDocumentChange, false);
    assert.equal(prepared.observation.requiresReupload, false);
  }
}

test("indexed file alone is not an extracted voucher snapshot and cannot be compared", () => {
  const result = inspect(null, reference, rules, true);
  assert.equal(result.status, "unavailable");
  assert.equal(result.reason, "extraction-not-available");
  assert.equal(result.observation, null);
  assertNonblocking(result);
});

test("missing snapshot prevents comparison even when an explicit reference and rules exist", () => {
  const result = inspect(null);
  assert.equal(result.status, "unavailable");
  assert.equal(result.observation, null);
  assertNonblocking(result);
});

test("matching document ID with a different file instance is refused", () => {
  const result = inspect({ ...snapshot, fileInstanceId: "other-file-instance" }, reference, rules, true);
  assert.equal(result.reason, "extraction-identity-mismatch");
  assert.equal(result.observation, null);
  assertNonblocking(result);
});

test("matching instance with a different document ID is refused", () => {
  const result = inspect({ ...snapshot, documentId: "other-document" }, reference, rules, true);
  assert.equal(result.reason, "extraction-identity-mismatch");
  assertNonblocking(result);
});

test("a nonempty snapshot identity is required; no ID is synthesized from metadata", () => {
  for (const value of [undefined, "", "  ", 17]) {
    const result = inspect({ ...snapshot, extractionSnapshotId: value }, reference, rules, true);
    assert.equal(result.reason, "extraction-invalid");
    assert.equal(result.observation, null);
    assertNonblocking(result);
  }
});

test("malformed and missing indexed identities do not force document editing", () => {
  for (const indexedFile of [null, {}, { documentId: "", fileInstanceId: file.fileInstanceId },
    { documentId: file.documentId, fileInstanceId: "  " }]) {
    const result = inspectOptionalIndexedDocumentMatch(indexedFile, snapshot, reference, rules, true);
    assert.equal(result.reason, "indexed-identity-unavailable");
    assertNonblocking(result);
  }
});

test("malformed extraction field structures cannot be passed off as legitimate snapshots", () => {
  for (const fields of [undefined, null, [], { voucher: { value: "INV-001" } }]) {
    const result = inspect({ ...snapshot, fields }, reference, rules, true);
    assert.equal(result.reason, "extraction-invalid");
    assertNonblocking(result);
  }
});

test("ready snapshot does not auto-run comparison: explicit opt-in is required", () => {
  const result = inspect();
  assert.equal(result.status, "ready");
  assert.equal(result.reason, null);
  assert.equal(result.observation.outcome, "not-requested");
  assert.deepEqual(result.observation.findings, []);
  assert.equal(result.observation.documentId, file.documentId);
  assert.equal(result.observation.extractionSnapshotId, snapshot.extractionSnapshotId);
  assertNonblocking(result);
});

test("a user opting in without a selected book receives no-reference and keeps normal use", () => {
  const result = inspect(snapshot, null, rules, true);
  assert.equal(result.status, "ready");
  assert.equal(result.observation.outcome, "no-reference");
  assert.equal(result.observation.referenceId, null);
  assertNonblocking(result);
});

test("explicitly requested comparison with no rules is nonblocking", () => {
  const result = inspect(snapshot, reference, [], true);
  assert.equal(result.observation.outcome, "no-rules");
  assertNonblocking(result);
});

test("different voucher values produce suggestions, not changes, and equal fields remain consistent", () => {
  const result = inspect(snapshot, reference, rules, true);
  assert.equal(result.observation.outcome, "suggestions");
  assert.deepEqual(result.observation.findings.map(x => x.status),
    ["difference", "consistent", "consistent"]);
  assert.equal(result.observation.referenceId, reference.referenceId);
  assertNonblocking(result);
});

test("incomplete document or book fields never make the source invalid", () => {
  const missingDocument = inspect({ ...snapshot, fields: {} }, reference, rules, true);
  const missingReference = inspect(snapshot, { ...reference, fields: {} }, rules, true);
  assert.equal(missingDocument.observation.outcome, "incomplete");
  assert.equal(missingReference.observation.outcome, "incomplete");
  assertNonblocking(missingDocument);
  assertNonblocking(missingReference);
});

test("dismissed and acknowledged findings change neither extracted values nor report", () => {
  const before = [JSON.stringify(file), JSON.stringify(snapshot),
    JSON.stringify(reference), JSON.stringify(rules)];
  const prepared = inspect(snapshot, reference, rules, true);
  const reportBefore = JSON.stringify(prepared);
  const dismissed = noteOptionalMatchFinding(prepared.observation, "voucher", "dismissed");
  const acknowledged = noteOptionalMatchFinding(prepared.observation, "voucher", "acknowledged");
  assert.equal(dismissed.disposition, "dismissed");
  assert.equal(acknowledged.disposition, "acknowledged");
  assert.equal(JSON.stringify(prepared), reportBefore);
  assert.deepEqual([JSON.stringify(file), JSON.stringify(snapshot),
    JSON.stringify(reference), JSON.stringify(rules)], before);
  assertNonblocking(prepared);
});

test("adapter does not emit path, filename or source metadata as extraction fields", () => {
  const result = inspect(snapshot, reference, rules, true);
  const json = JSON.stringify(result);
  assert.doesNotMatch(json, /C:|immutable\/original|file\.pdf|availabilityState|modifiedUnixMs/);
  assert.deepEqual(Object.keys(result.observation.findings[0]).sort(),
    ["ruleId", "label", "status", "documentField", "referenceField",
      "documentValue", "referenceValue", "reason"].sort());
});

test("adapter remains entirely detached from Tauri, persistence, indexed-file writes and live App", () => {
  const adapter = readFileSync("src/document-match-advisory-adapter.ts", "utf8");
  const app = readFileSync("src/App.tsx", "utf8");
  assert.doesNotMatch(app, /document-match-advisory-adapter/);
  assert.doesNotMatch(adapter,
    /from ["']@tauri-apps|invoke\(|fetch\(|localStorage|writeFile\(|readFile\(|fs\.write|create_workpaper_revision/);
  assert.match(adapter, /requested = false/);
  assert.match(adapter, /snapshot\.fileInstanceId !== indexedFile\.fileInstanceId/);
});

test("only boolean true counts as an explicit opt-in; truthy external values cannot trigger comparison", () => {
  for (const requested of ["true", "false", 1, {}, null, undefined]) {
    const result = inspectOptionalIndexedDocumentMatch(file, snapshot, reference, rules, requested);
    assert.equal(result.observation.outcome, "not-requested");
    assert.deepEqual(result.observation.findings, []);
    assertNonblocking(result);
  }
  assert.equal(inspectOptionalIndexedDocumentMatch(file, snapshot, reference, rules, true)
    .observation.outcome, "suggestions");
});
