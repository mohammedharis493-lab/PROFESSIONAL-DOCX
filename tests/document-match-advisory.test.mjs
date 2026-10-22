import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import {
  inspectOptionalDocumentMatch,
  noteOptionalMatchFinding,
} from "../src/document-match-advisory.ts";

function inputs() {
  return {
    document: {
      documentId: "doc-original-001",
      extractionSnapshotId: "snapshot-4",
      fields: {
        voucherNumber: "INV-1001",
        amountMinor: 12500,
        currency: "INR",
        note: "Preserve exact  \r\n",
      },
    },
    reference: {
      referenceId: "book-42",
      fields: {
        voucherRef: "INV-1002",
        totalMinor: 12500,
        currency: "INR",
        note: "Preserve exact  \r\n",
      },
    },
    rules: [
      {
        ruleId: "invoice-number",
        label: "Invoice number",
        documentField: "voucherNumber",
        referenceField: "voucherRef",
        comparison: "exact-text",
      },
      {
        ruleId: "amount",
        label: "Amount in minor currency units",
        documentField: "amountMinor",
        referenceField: "totalMinor",
        comparison: "integer-minor-units",
      },
      {
        ruleId: "currency",
        label: "Currency",
        documentField: "currency",
        referenceField: "currency",
        comparison: "exact-text",
      },
    ],
  };
}

test("matching defaults to not requested even with a supplied reference and rules", () => {
  const { document, reference, rules } = inputs();
  const original = JSON.stringify(document);
  const priorBook = JSON.stringify(reference);
  const result = inspectOptionalDocumentMatch(document, reference, rules);
  assert.equal(result.outcome, "not-requested");
  assert.equal(result.findings.length, 0);
  assert.equal(result.requiresDocumentChange, false);
  assert.equal(result.requiresReupload, false);
  assert.equal(result.blocksDocumentUse, false);
  assert.equal(JSON.stringify(document), original);
  assert.equal(JSON.stringify(reference), priorBook);
});

test("explicit opt-in with no book reference is not a document failure", () => {
  const { document, rules } = inputs();
  const result = inspectOptionalDocumentMatch(document, null, rules, true);
  assert.equal(result.outcome, "no-reference");
  assert.equal(result.referenceId, null);
  assert.deepEqual(result.findings, []);
  assert.equal(result.blocksDocumentUse, false);
});

test("explicit opt-in with zero rules is advisory-only and nonblocking", () => {
  const { document, reference } = inputs();
  const result = inspectOptionalDocumentMatch(document, reference, [], true);
  assert.equal(result.outcome, "no-rules");
  assert.deepEqual(result.findings, []);
  assert.equal(result.requiresReupload, false);
});

test("a discrepancy is a suggestion while matching fields remain individually consistent", () => {
  const { document, reference, rules } = inputs();
  const result = inspectOptionalDocumentMatch(document, reference, rules, true);
  assert.equal(result.scope, "detached-optional-matching-observation-only");
  assert.equal(result.outcome, "suggestions");
  assert.deepEqual(result.findings.map(x => x.status),
    ["difference", "consistent", "consistent"]);
  assert.equal(result.findings[0].documentValue, "INV-1001");
  assert.equal(result.findings[0].referenceValue, "INV-1002");
  assert.equal(result.findings[0].reason, null);
  assert.equal(result.documentId, document.documentId);
  assert.equal(result.extractionSnapshotId, document.extractionSnapshotId);
  assert.equal(result.blocksDocumentUse, false);
});

test("missing document value remains not compared, not a claim the source is invalid", () => {
  const { document, reference, rules } = inputs();
  const result = inspectOptionalDocumentMatch(
    { ...document, fields: { amountMinor: 12500, currency: "INR" } },
    reference, rules, true,
  );
  assert.equal(result.outcome, "incomplete");
  assert.equal(result.findings[0].status, "not-compared");
  assert.equal(result.findings[0].reason, "document-field-absent");
  assert.equal(result.requiresDocumentChange, false);
});

test("missing book value remains not compared and preserves the existing extraction", () => {
  const { document, reference, rules } = inputs();
  const docBefore = JSON.stringify(document);
  const result = inspectOptionalDocumentMatch(
    document, { ...reference, fields: { currency: "INR" } }, rules, true,
  );
  assert.equal(result.outcome, "incomplete");
  assert.equal(result.findings[0].reason, "reference-field-absent");
  assert.equal(result.findings[1].reason, "reference-field-absent");
  assert.equal(JSON.stringify(document), docBefore);
});

test("a real discrepancy stays a suggestion even if another reference field is missing", () => {
  const { document, reference, rules } = inputs();
  const compared = inspectOptionalDocumentMatch(
    document,
    { ...reference, fields: { voucherRef: "OTHER-REF", currency: "INR" } },
    rules,
    true,
  );
  assert.equal(compared.outcome, "suggestions");
  assert.equal(compared.findings[0].status, "difference");
  assert.equal(compared.findings[1].status, "not-compared");
  assert.equal(compared.findings[1].reason, "reference-field-absent");
  assert.equal(compared.blocksDocumentUse, false);
});

test("both missing values are explicitly different from a mismatch", () => {
  const { document, reference, rules } = inputs();
  const result = inspectOptionalDocumentMatch(
    { ...document, fields: {} }, { ...reference, fields: {} }, rules, true,
  );
  assert.equal(result.outcome, "incomplete");
  assert.ok(result.findings.every(x => x.status === "not-compared"));
  assert.ok(result.findings.every(x => x.reason === "both-fields-absent"));
});

test("blank strings mean unavailable while zero monetary minor units are real values", () => {
  const { document, reference, rules } = inputs();
  const result = inspectOptionalDocumentMatch(
    { ...document, fields: { voucherNumber: " \r\n", amountMinor: 0, currency: "INR" } },
    { ...reference, fields: { voucherRef: "INV", totalMinor: 0, currency: "INR" } },
    rules, true,
  );
  assert.equal(result.outcome, "incomplete");
  assert.equal(result.findings[0].reason, "document-field-absent");
  assert.equal(result.findings[1].status, "consistent");
});

test("unsafe floats or stringified amounts are not silently treated as reliable matches", () => {
  const { document, reference, rules } = inputs();
  const values = [12.50, 9007199254740992, "12500", Number.NaN, Number.POSITIVE_INFINITY];
  for (const value of values) {
    const result = inspectOptionalDocumentMatch(
      { ...document, fields: { amountMinor: value } },
      reference, rules.slice(1, 2), true,
    );
    assert.equal(result.outcome, "incomplete");
    assert.equal(result.findings[0].reason, "incompatible-field-type");
  }
});

test("exact text comparison preserves case, Unicode, CRLF and trailing whitespace", () => {
  const { document, reference } = inputs();
  const rule = [{
    ruleId: "literal",
    label: "Literal note",
    documentField: "note",
    referenceField: "note",
    comparison: "exact-text",
  }];
  const consistent = inspectOptionalDocumentMatch(document, reference, rule, true);
  assert.equal(consistent.outcome, "consistent");
  const changed = inspectOptionalDocumentMatch(document, {
    ...reference, fields: { ...reference.fields, note: "Preserve exact \n" },
  }, rule, true);
  assert.equal(changed.outcome, "suggestions");
  assert.equal(changed.findings[0].status, "difference");
});

test("output and findings are frozen, input snapshots are not edited or copied into writes", () => {
  const { document, reference, rules } = inputs();
  const before = [JSON.stringify(document), JSON.stringify(reference), JSON.stringify(rules)];
  const result = inspectOptionalDocumentMatch(document, reference, rules, true);
  assert.ok(Object.isFrozen(result));
  assert.ok(Object.isFrozen(result.findings));
  assert.ok(Object.isFrozen(result.findings[0]));
  assert.deepEqual([JSON.stringify(document), JSON.stringify(reference), JSON.stringify(rules)], before);
  assert.deepEqual(
    Object.keys(result).sort(),
    ["scope", "documentId", "extractionSnapshotId", "referenceId", "outcome",
      "findings", "requiresDocumentChange", "requiresReupload", "blocksDocumentUse"].sort(),
  );
});

test("local acknowledgement and dismissal never change source documents or matching results", () => {
  const { document, reference, rules } = inputs();
  const result = inspectOptionalDocumentMatch(document, reference, rules, true);
  const before = JSON.stringify(result);
  const dismissed = noteOptionalMatchFinding(result, "invoice-number", "dismissed");
  const acknowledged = noteOptionalMatchFinding(result, "invoice-number", "acknowledged");
  assert.equal(dismissed.scope, "local-advisory-review-only");
  assert.equal(dismissed.documentId, document.documentId);
  assert.equal(dismissed.extractionSnapshotId, document.extractionSnapshotId);
  assert.equal(dismissed.referenceId, reference.referenceId);
  assert.equal(dismissed.disposition, "dismissed");
  assert.equal(acknowledged.disposition, "acknowledged");
  assert.ok(Object.isFrozen(dismissed));
  assert.equal(JSON.stringify(result), before);
  assert.equal(document.fields.voucherNumber, "INV-1001");
});

test("cannot annotate a consistent field or an unrequested observation", () => {
  const { document, reference, rules } = inputs();
  const compared = inspectOptionalDocumentMatch(document, reference, rules, true);
  assert.throws(() => noteOptionalMatchFinding(compared, "amount", "dismissed"), /Only a difference/);
  assert.throws(() => noteOptionalMatchFinding(compared, "not-a-rule", "acknowledged"), /Only a difference/);
  assert.throws(() => noteOptionalMatchFinding(
    inspectOptionalDocumentMatch(document, reference, rules), "invoice-number", "dismissed",
  ), /Only a difference/);
});

test("invalid document identities, records, duplicate rules and modes fail loudly", () => {
  const { document, reference, rules } = inputs();
  assert.throws(() => inspectOptionalDocumentMatch(
    { ...document, documentId: " " }, reference, rules, true,
  ), /Document ID/);
  assert.throws(() => inspectOptionalDocumentMatch(
    document, { ...reference, referenceId: "" }, rules, true,
  ), /Reference ID/);
  assert.throws(() => inspectOptionalDocumentMatch(
    { ...document, fields: [] }, reference, rules, true,
  ), /field object/);
  assert.throws(() => inspectOptionalDocumentMatch(document, reference, [rules[0], rules[0]], true),
    /Duplicate advisory/);
  assert.throws(() => inspectOptionalDocumentMatch(document, reference, [
    { ...rules[0], comparison: "fuzzy" },
  ], true), /Unsupported advisory/);
});

test("optional advisory module has no Tauri, storage, document mutation, or side-effect imports", () => {
  const source = readFileSync("src/document-match-advisory.ts", "utf8");
  assert.doesNotMatch(source, /(?:^|\n)\s*import\s/m);
  assert.doesNotMatch(source, /invoke\(|fs\.|fetch\(|localStorage|UPDATE documents|DELETE FROM|INSERT INTO/);
  const packageText = readFileSync("package.json", "utf8");
  assert.match(packageText, /test:document-match-advisory/);
});
