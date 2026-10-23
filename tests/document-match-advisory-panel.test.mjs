import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { inspectOptionalDocumentMatch } from "../src/document-match-advisory.ts";
import { DocumentMatchAdvisoryPanel } from "../src/document-match-advisory-panel.ts";

function observation({
  requested = true,
  reference = { referenceId: "book-1", fields: { number: "PUR-999", amount: 1575 } },
  documentFields = { number: "PUR-001", amount: 1575 },
  rules = [
    { ruleId: "num", label: "Voucher number", documentField: "number",
      referenceField: "number", comparison: "exact-text" },
    { ruleId: "amt", label: "Minor-unit amount", documentField: "amount",
      referenceField: "amount", comparison: "integer-minor-units" },
  ],
} = {}) {
  return inspectOptionalDocumentMatch({
    documentId: "document-1",
    extractionSnapshotId: "extraction-7",
    fields: documentFields,
  }, reference, rules, requested);
}
function render(report, callback) {
  return renderToStaticMarkup(createElement(DocumentMatchAdvisoryPanel, {
    observation: report,
    ...(callback ? { onLocalAnnotation: callback } : {}),
  }));
}

test("unrequested matching is visibly optional, without a submit or edit interaction", () => {
  const html = render(observation({ requested: false }));
  assert.match(html, /Optional matching/);
  assert.match(html, /Not compared/);
  assert.match(html, /Matching is optional/);
  assert.match(html, /original document/);
  assert.doesNotMatch(html, /<form|<input|<button|type="submit"/);
  assert.doesNotMatch(html, /PUR-999/);
});

test("reference missing does not block normal document use", () => {
  const html = render(observation({ reference: null }));
  assert.match(html, /No reference selected/);
  assert.match(html, /continue working with the existing document/);
  assert.match(html, /does not require editing or uploading/);
  assert.doesNotMatch(html, /<button/);
});

test("no comparison rules is not a document validation error", () => {
  const html = render(observation({ rules: [] }));
  assert.match(html, /No comparison fields selected/);
  assert.doesNotMatch(html, /<ul/);
});

test("mixed differences and equal amounts show separate advisory statuses", () => {
  const html = render(observation());
  assert.match(html, /Difference to review/);
  assert.match(html, /Consistent/);
  assert.match(html, /PUR-001/);
  assert.match(html, /PUR-999/);
  assert.match(html, /data-advisory-status="difference"/);
  assert.match(html, /data-advisory-status="consistent"/);
  assert.match(html, /Differences are suggestions for review/);
  assert.doesNotMatch(html, /<button/);
});

test("absent or incompatible data is not portrayed as a mismatch or invalid source", () => {
  const html = render(observation({
    reference: { referenceId: "book-1", fields: { number: null, amount: "15.75" } },
    documentFields: { number: "INV-3", amount: 1575 },
  }));
  assert.match(html, /Not compared/);
  assert.match(html, /Reference field unavailable/);
  assert.match(html, /Fields could not be compared safely/);
  assert.match(html, /Some fields could not be compared/);
  assert.match(html, /not a document validation failure/);
});

test("dangerous source and reference text is HTML-escaped, never trusted markup", () => {
  const html = render(observation({
    reference: { referenceId: "book-1", fields: { number: "<script>bad()</script>", amount: 1575 } },
    documentFields: { number: "<img src=x onerror=alert(1)>", amount: 1575 },
  }));
  assert.match(html, /&lt;script&gt;bad\(\)&lt;\/script&gt;/);
  assert.match(html, /&lt;img src=x onerror=alert\(1\)&gt;/);
  assert.doesNotMatch(html, /<script>|<img/);
});

function buttonsIn(node, accumulator = []) {
  if (Array.isArray(node)) {
    for (const item of node) buttonsIn(item, accumulator);
  } else if (node && typeof node === "object") {
    if (node.type === "button") accumulator.push(node);
    buttonsIn(node.props?.children, accumulator);
  }
  return accumulator;
}

test("acknowledge and dismiss produce only scoped local annotations when callback supplied", () => {
  const report = observation();
  const initial = JSON.stringify(report);
  const notes = [];
  const element = DocumentMatchAdvisoryPanel({
    observation: report, onLocalAnnotation: value => notes.push(value),
  });
  const buttons = buttonsIn(element);
  assert.equal(buttons.length, 2);
  assert.ok(buttons.every(button => button.props.type === "button"));
  const html = render(report, value => notes.push(value));
  assert.match(html, /Acknowledge suggestion/);
  assert.match(html, /Dismiss suggestion/);
  buttons[0].props.onClick();
  buttons[1].props.onClick();
  assert.deepEqual(notes.map(x => x.disposition), ["acknowledged", "dismissed"]);
  assert.ok(notes.every(x => x.documentId === report.documentId));
  assert.ok(notes.every(x => x.extractionSnapshotId === report.extractionSnapshotId));
  assert.ok(notes.every(x => Object.isFrozen(x)));
  assert.equal(JSON.stringify(report), initial);
});

test("annotations also possible for unavailable fields but never auto-create fixes", () => {
  const report = observation({
    reference: { referenceId: "book-1", fields: {} },
  });
  const notes = [];
  const tree = DocumentMatchAdvisoryPanel({
    observation: report, onLocalAnnotation: x => notes.push(x),
  });
  const buttons = buttonsIn(tree);
  assert.equal(buttons.length, 4);
  buttons[0].props.onClick();
  assert.equal(notes[0].ruleId, "num");
  assert.equal(report.outcome, "incomplete");
  assert.equal(report.requiresDocumentChange, false);
});

test("malformed or false nonblocking promises are rejected instead of rendered as a valid advisory", () => {
  const report = observation();
  assert.throws(() => DocumentMatchAdvisoryPanel({
    observation: { ...report, blocksDocumentUse: true },
  }), /nonblocking advisory/);
  assert.throws(() => DocumentMatchAdvisoryPanel({
    observation: { ...report, requiresReupload: true },
  }), /nonblocking advisory/);
  assert.throws(() => DocumentMatchAdvisoryPanel({
    observation: { ...report, scope: "formal-approval" },
  }), /nonblocking advisory/);
  assert.throws(() => DocumentMatchAdvisoryPanel({ observation: null }), /nonblocking advisory/);
});

test("panel stays detached from application, Tauri, Save, and stored-source writes", () => {
  const panel = readFileSync("src/document-match-advisory-panel.ts", "utf8");
  const app = readFileSync("src/App.tsx", "utf8");
  assert.doesNotMatch(app, /document-match-advisory-panel|document-match-advisory/);
  assert.doesNotMatch(panel, /@tauri-apps|invoke\(|fetch\(|localStorage|writeFile|readFile|create_workpaper_revision/);
  assert.doesNotMatch(panel, /<form|type: "submit"|type="submit"/);
  assert.match(panel, /onLocalAnnotation\?:/);
  const pkg = readFileSync("package.json", "utf8");
  assert.match(pkg, /document-match-advisory-panel\.test\.mjs/);
});
