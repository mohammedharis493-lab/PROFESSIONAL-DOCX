import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import {
  chooseExplicitAdvisoryReference,
  keepLocalAdvisoryDisposition,
  requestExplicitOptionalMatch,
  reviewIsCurrent,
  selectionIsCurrent,
} from "../src/document-match-advisory-flow.ts";
import { DocumentMatchOptInControl } from "../src/document-match-opt-in-control.ts";
import { noteOptionalMatchFinding } from "../src/document-match-advisory.ts";

const file = Object.freeze({ documentId: "doc-1", fileInstanceId: "instance-1", name: "test.pdf" });
const snapshot = Object.freeze({
  documentId: "doc-1", fileInstanceId: "instance-1",
  extractionSnapshotId: "snap-1",
  fields: Object.freeze({ number: "INV-100", amountMinor: 12500 }),
});
const references = Object.freeze([
  Object.freeze({ referenceId: "book-1", fields: Object.freeze({ number: "INV-999", amountMinor: 12500 }) }),
  Object.freeze({ referenceId: "book-2", fields: Object.freeze({ number: "INV-100", amountMinor: 12500 }) }),
]);
const rules = Object.freeze([
  Object.freeze({ ruleId: "number", label: "Number", documentField: "number",
    referenceField: "number", comparison: "exact-text" }),
  Object.freeze({ ruleId: "amount", label: "Amount", documentField: "amountMinor",
    referenceField: "amountMinor", comparison: "integer-minor-units" }),
]);
const visit = "viewer-open-1";
const selected = () => chooseExplicitAdvisoryReference(visit, file, snapshot, "book-1");
const request = (overrides = {}) => requestExplicitOptionalMatch(
  overrides.visit ?? visit, overrides.file ?? file, overrides.snapshot === undefined ? snapshot : overrides.snapshot,
  overrides.references ?? references, overrides.rules ?? rules,
  overrides.selection === undefined ? selected() : overrides.selection,
);
const current = (review, overrides = {}) => reviewIsCurrent(
  review, overrides.visit ?? visit, overrides.file ?? file,
  overrides.snapshot === undefined ? snapshot : overrides.snapshot,
  overrides.references ?? references, overrides.rules ?? rules,
  overrides.selection === undefined ? selected() : overrides.selection,
);
function ui(overrides = {}) {
  const calls = { selections: [], reviews: [], clears: 0, unavailable: [], annotations: [] };
  const props = {
    visitKey: visit, indexedFile: file, snapshot, references, rules,
    selection: null, review: null,
    onSelectionChange: x => calls.selections.push(x),
    onReview: x => calls.reviews.push(x),
    onClearReview: () => calls.clears++,
    onUnavailable: x => calls.unavailable.push(x),
    onLocalAnnotation: x => calls.annotations.push(x),
    ...overrides,
  };
  return { props, calls, tree: DocumentMatchOptInControl(props),
    html: renderToStaticMarkup(createElement(DocumentMatchOptInControl, props)) };
}
function allNodes(root, kind, results = []) {
  if (Array.isArray(root)) {
    for (const part of root) allNodes(part, kind, results);
  } else if (root && typeof root === "object") {
    if (root.type === kind) results.push(root);
    allNodes(root.props?.children, kind, results);
  }
  return results;
}

test("default has no reference selected, no computed report, and no automatic comparison", () => {
  const { html, tree } = ui();
  assert.match(html, /No reference selected/);
  assert.match(html, /Optional reference comparison/);
  assert.match(html, /original document remains available/);
  assert.doesNotMatch(html, /INV-999|Difference to review|Dismiss suggestion/);
  assert.equal(allNodes(tree, "button")[0].props.disabled, true);
  assert.doesNotMatch(html, /<form|type="submit"/);
});

test("lack of any extraction snapshot cannot require upload or block existing document use", () => {
  const { html, tree } = ui({ snapshot: null });
  assert.match(html, /No bound structured extraction snapshot is available/);
  assert.match(html, /continue to preview and manage it normally/);
  assert.equal(allNodes(tree, "select")[0].props.disabled, true);
  assert.equal(allNodes(tree, "button")[0].props.disabled, true);
  assert.doesNotMatch(html, /re-upload|<input type="file"/);
});

test("no books leaves a simple unavailable reference choice without a document error", () => {
  const { html, tree } = ui({ references: [] });
  assert.equal(allNodes(tree, "select")[0].props.disabled, true);
  assert.equal(allNodes(tree, "button")[0].props.disabled, true);
  assert.match(html, /Your original document remains available/);
});

test("explicit dropdown change produces a visit-scoped selection, clears old local report", () => {
  const { tree, calls } = ui();
  const dropdown = allNodes(tree, "select")[0];
  dropdown.props.onChange({ currentTarget: { value: "book-1" } });
  assert.equal(calls.clears, 1);
  assert.equal(calls.selections.length, 1);
  assert.deepEqual(calls.selections[0], selected());
  assert.ok(Object.isFrozen(calls.selections[0]));
});

test("clearing selection removes choice without triggering a comparison", () => {
  const { tree, calls } = ui({ selection: selected() });
  allNodes(tree, "select")[0].props.onChange({ currentTarget: { value: "" } });
  assert.deepEqual(calls.selections, [null]);
  assert.equal(calls.clears, 1);
  assert.deepEqual(calls.reviews, []);
});

test("invented dropdown option cannot trigger reference matching", () => {
  const { tree, calls } = ui();
  allNodes(tree, "select")[0].props.onChange({ currentTarget: { value: "not-an-option" } });
  assert.deepEqual(calls.selections, [null]);
  assert.equal(calls.reviews.length, 0);
});

test("missing explicit selection cannot call the comparison engine", () => {
  const result = request({ selection: null });
  assert.deepEqual({status: result.status, reason: result.reason, blocks: result.blocksDocumentUse},
    {status: "unavailable", reason: "no-explicit-selection", blocks: false});
  assert.equal(result.review, null);
});

test("Compare is the only UI path that produces a reference observation", () => {
  const { tree, calls, html } = ui({ selection: selected() });
  assert.match(html, /Compare selected reference/);
  assert.doesNotMatch(html, /INV-999/);
  const compare = allNodes(tree, "button")[0];
  assert.equal(compare.props.disabled, false);
  assert.deepEqual(calls.reviews, []);
  compare.props.onClick();
  assert.equal(calls.reviews.length, 1);
  assert.equal(calls.reviews[0].observation.outcome, "suggestions");
  assert.equal(calls.reviews[0].referenceId, "book-1");
  assert.equal(calls.reviews[0].observation.requiresDocumentChange, false);
  assert.equal(calls.reviews[0].observation.requiresReupload, false);
  assert.equal(calls.reviews[0].observation.blocksDocumentUse, false);
});

test("changing the explicit reference requires a fresh comparison", () => {
  const first = request();
  assert.equal(first.status, "ready");
  const other = chooseExplicitAdvisoryReference(visit, file, snapshot, "book-2");
  assert.equal(current(first.review, {selection: other}), false);
  const second = request({selection: other});
  assert.equal(second.review.observation.outcome, "consistent");
  assert.equal(second.review.observation.referenceId, "book-2");
});

test("document identity or file instance changes hide old results", () => {
  const review = request().review;
  assert.equal(current(review), true);
  assert.equal(current(review, {file: {...file, documentId: "doc-2"}}), false);
  assert.equal(current(review, {file: {...file, fileInstanceId: "instance-2"}}), false);
  assert.equal(current(review, {snapshot: {...snapshot, fileInstanceId: "instance-2"}}), false);
});

test("same document reopened under a new visit token cannot resurrect its old comparison", () => {
  const review = request().review;
  assert.equal(current(review, {visit: "viewer-open-2"}), false);
  assert.equal(selectionIsCurrent(selected(), "viewer-open-2", file, snapshot), false);
  assert.equal(request({visit: "viewer-open-2"}).reason, "stale-selection");
});

test("different extraction snapshot invalidates selection and review without altering the file", () => {
  const newer = {...snapshot, extractionSnapshotId: "snap-2"};
  assert.equal(selectionIsCurrent(selected(), visit, file, newer), false);
  assert.equal(current(request().review, {snapshot: newer}), false);
  assert.equal(request({snapshot: newer}).reason, "stale-selection");
});

test("changed values under the same extraction snapshot ID hide cached comparison", () => {
  const prior = request().review;
  assert.equal(current(prior, {snapshot: {...snapshot, fields: {...snapshot.fields, number: "UPDATED"}}}), false);
  assert.equal(current(prior, {references: [
    {...references[0], fields: {...references[0].fields, number: "UPDATED"}}, references[1]]}), false);
  assert.equal(current(prior, {rules: [{...rules[0], label: "New rule label"}, rules[1]]}), false);
});

test("reference removal or ambiguity suppresses stale review and refuses new comparison", () => {
  const prior = request().review;
  assert.equal(current(prior, {references: []}), false);
  assert.equal(request({references: []}).reason, "reference-unavailable");
  const duplicates = [references[0], {...references[0]}];
  assert.equal(current(prior, {references: duplicates}), false);
  assert.equal(request({references: duplicates}).reason, "ambiguous-reference");
  const {tree} = ui({selection: selected(), references: duplicates});
  assert.equal(allNodes(tree, "button")[0].props.disabled, true);
});

test("incomplete fields are advisory-only and continue to permit document use", () => {
  const missing = request({snapshot: {...snapshot, fields: {}}});
  assert.equal(missing.status, "ready");
  assert.equal(missing.review.observation.outcome, "incomplete");
  assert.equal(missing.blocksDocumentUse, false);
  assert.equal(missing.review.observation.blocksDocumentUse, false);
});

test("invalid rule configuration is nonblocking and does not submit or persist anything", () => {
  const result = request({rules: [rules[0], rules[0]]});
  assert.equal(result.status, "unavailable");
  assert.equal(result.reason, "invalid-comparison-input");
  assert.equal(result.blocksDocumentUse, false);
});

test("a precomputed current comparison renders advisory text and optional local Clear button", () => {
  const review = request().review;
  const {html, tree, calls} = ui({selection: selected(), review});
  assert.match(html, /Difference to review/);
  assert.match(html, /INV-999/);
  assert.match(html, /Clear local comparison/);
  assert.match(html, /Matching does not require editing or uploading/);
  const buttons = allNodes(tree, "button");
  assert.equal(buttons.length, 2);
  buttons[1].props.onClick();
  assert.equal(calls.clears, 1);
});

test("stale review is hidden from the UI when the viewer or fields change", () => {
  const review = request().review;
  for (const changes of [
    {visitKey: "different-opening"},
    {snapshot: {...snapshot, fields: {number: "NEW"}}},
    {references: [{...references[0], fields: {number: "NEW"}}, references[1]]},
    {rules: [{...rules[0], ruleId: "renamed"}, rules[1]]},
  ]) {
    const {html} = ui({selection: selected(), review, ...changes});
    assert.doesNotMatch(html, /INV-999|Clear local comparison|Difference to review/);
  }
});

test("invalid, mismatched or unselected references are not auto-associated", () => {
  assert.equal(chooseExplicitAdvisoryReference("", file, snapshot, "book-1"), null);
  assert.equal(chooseExplicitAdvisoryReference(visit, file, snapshot, ""), null);
  assert.equal(chooseExplicitAdvisoryReference(visit, file, null, "book-1"), null);
  assert.equal(chooseExplicitAdvisoryReference(visit, file, {...snapshot, documentId: "other"}, "book-1"), null);
  const picked = chooseExplicitAdvisoryReference(visit, file, snapshot, "book-1");
  assert.equal(picked.referenceId, "book-1");
  assert.equal(picked.scope, "volatile-optional-reference-selection");
});

test("dismissed and acknowledged findings remain local to exact reviewed identifiers", () => {
  const review = request().review;
  const original = JSON.stringify([file, snapshot, references, rules, review]);
  const dismissed = noteOptionalMatchFinding(review.observation, "number", "dismissed");
  const acknowledged = noteOptionalMatchFinding(review.observation, "number", "acknowledged");
  assert.equal(keepLocalAdvisoryDisposition(review, dismissed), dismissed);
  assert.equal(keepLocalAdvisoryDisposition(review, acknowledged), acknowledged);
  assert.throws(() => keepLocalAdvisoryDisposition(review, {...dismissed, referenceId: "other"}), /does not belong/);
  assert.throws(() => keepLocalAdvisoryDisposition(review, {...dismissed, ruleId: "amount"}), /does not belong/);
  assert.equal(JSON.stringify([file, snapshot, references, rules, review]), original);
});

test("rendered reference identifiers are React-escaped rather than injected as HTML", () => {
  const unsafe = [{referenceId: "<img src=x onerror=alert(1)>", fields: {number: "INV-1"}}];
  const {html} = ui({references: unsafe});
  assert.match(html, /&lt;img src=x onerror=alert\(1\)&gt;/);
  assert.doesNotMatch(html, /<img/);
});

test("flow and control have no Tauri, storage, network, file mutation or native approval imports", () => {
  const flow = readFileSync("src/document-match-advisory-flow.ts", "utf8");
  const uiSource = readFileSync("src/document-match-opt-in-control.ts", "utf8");
  const app = readFileSync("src/App.tsx", "utf8");
  for (const text of [flow, uiSource]) {
    assert.doesNotMatch(text, /@tauri-apps|invoke\(|fetch\(|localStorage|sessionStorage|writeFile|fs\.write|create_workpaper_revision|sign_off_workpaper/);
  }
  assert.doesNotMatch(app, /document-match-opt-in-control|document-match-advisory-flow/);
  assert.match(uiSource, /onClick: compare/);
  assert.match(flow, /selectionIsCurrent\(selection, visitKey, file, snapshot\)/);
  assert.match(flow, /volatile-optional-comparison-review/);
});

test("malformed externally supplied review never crashes or blocks the document", () => {
  const original = request().review;
  const invalid = [
    {...original, observation: null},
    {...original, observation: undefined},
    {...original, observation: {...original.observation, blocksDocumentUse: true}},
    {...original, observation: {...original.observation, requiresReupload: true}},
    {...original, observation: {...original.observation, requiresDocumentChange: true}},
  ];
  for (const review of invalid) {
    assert.equal(current(review), false);
    const {html} = ui({selection: selected(), review});
    assert.doesNotMatch(html, /Clear local comparison|Difference to review/);
    assert.match(html, /original document remains available/);
  }
});

test("forged observation identities or statuses never surface stale or unauthorized comparisons", () => {
  const original = request().review;
  const forged = [
    {...original, observation: {...original.observation, documentId: "another-doc"}},
    {...original, observation: {...original.observation, extractionSnapshotId: "another-snapshot"}},
    {...original, observation: {...original.observation, referenceId: "another-book"}},
    {...original, observation: {...original.observation, outcome: "not-requested"}},
    {...original, observation: {...original.observation, findings: null}},
    {...original, observation: {...original.observation, outcome: "malformed"}},
  ];
  for (const review of forged) {
    assert.equal(current(review), false);
    const {html} = ui({selection: selected(), review});
    assert.doesNotMatch(html, /Clear local comparison|Difference to review/);
  }
});
