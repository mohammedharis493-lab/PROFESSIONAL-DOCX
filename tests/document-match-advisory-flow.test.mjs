import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import {
  chooseExplicitAdvisoryReference,
  keepLocalAdvisoryDisposition,
  stampVolatileAdvisoryAnnotation,
  volatileAnnotationIsCurrent,
  requestExplicitOptionalMatch,
  reviewIsCurrent,
  selectionIsCurrent,
} from "../src/document-match-advisory-flow.ts";
import { DocumentMatchOptInControl } from "../src/document-match-opt-in-control.ts";
import { DocumentMatchAdvisoryPanel } from "../src/document-match-advisory-panel.ts";
import { noteOptionalMatchFinding } from "../src/document-match-advisory.ts";

const file = Object.freeze({ documentId: "doc-1", fileInstanceId: "instance-1", name: "test.pdf", availabilityState: "AVAILABLE" });
const SOURCE_HASH = "a".repeat(64);
const snapshot = Object.freeze({
  documentId: "doc-1", fileInstanceId: "instance-1",
  extractionSnapshotId: "snap-1",
  sourceContentVersionId: "observed-version-1",
  sourceSha256Hex: SOURCE_HASH,
  fields: Object.freeze({ number: "INV-100", amountMinor: 12500 }),
});
const history = Object.freeze({
  requestedDocumentId: file.documentId,
  entries: Object.freeze([Object.freeze({
    contentVersionId: snapshot.sourceContentVersionId,
    fileInstanceId: file.fileInstanceId,
    sha256Hex: SOURCE_HASH,
    sourceStableDuringRead: true,
    verificationState: "HASH_VERIFIED",
  })]),
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
const HISTORY_READ_KEY = "fresh-history-read-1";
const selected = () => chooseExplicitAdvisoryReference(
  visit, file, snapshot, "book-1", HISTORY_READ_KEY,
);
const request = (overrides = {}) => requestExplicitOptionalMatch(
  overrides.visit ?? visit, overrides.file ?? file, overrides.snapshot === undefined ? snapshot : overrides.snapshot,
  overrides.references ?? references, overrides.rules ?? rules,
  overrides.selection === undefined ? selected() : overrides.selection,
  overrides.historyReadKey === undefined ? HISTORY_READ_KEY : overrides.historyReadKey,
);
const current = (review, overrides = {}) => reviewIsCurrent(
  review, overrides.visit ?? visit, overrides.file ?? file,
  overrides.snapshot === undefined ? snapshot : overrides.snapshot,
  overrides.references ?? references, overrides.rules ?? rules,
  overrides.selection === undefined ? selected() : overrides.selection,
  overrides.historyReadKey === undefined ? HISTORY_READ_KEY : overrides.historyReadKey,
);
function ui(overrides = {}) {
  const calls = { selections: [], reviews: [], clears: 0, unavailable: [], annotations: [] };
  const props = {
    visitKey: visit, indexedFile: file, snapshot, history,
    historyReadKey: HISTORY_READ_KEY, references, rules,
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
  const other = chooseExplicitAdvisoryReference(
    visit, file, snapshot, "book-2", HISTORY_READ_KEY,
  );
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

test("changed incomplete fields require renewed opt-in and never block document use", () => {
  const missingSnapshot = {...snapshot, fields: {}};
  const stale = request({snapshot: missingSnapshot});
  assert.equal(stale.status, "unavailable");
  assert.equal(stale.reason, "stale-selection");
  assert.equal(stale.blocksDocumentUse, false);
  const selection = chooseExplicitAdvisoryReference(
    visit, file, missingSnapshot, "book-1", HISTORY_READ_KEY,
  );
  assert.ok(selection);
  const missing = request({snapshot: missingSnapshot, selection});
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
  assert.match(flow, /selectionIsCurrent\(selection, visitKey, file, snapshot, historyReadKey\)/);
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

test("detached UI refuses comparison without independently supplied matching version history", () => {
  const review = request().review;
  for (const mismatched of [
    null,
    {requestedDocumentId: "different-document", entries: history.entries},
    {...history, entries: []},
    {...history, entries: [{...history.entries[0], fileInstanceId: "other-instance"}]},
    {...history, entries: [{...history.entries[0], sha256Hex: "b".repeat(64)}]},
    {...history, entries: [{...history.entries[0], verificationState: "METADATA_ONLY"}]},
    {...history, entries: [{...history.entries[0], sourceStableDuringRead: false}]},
  ]) {
    const {tree, html, calls} = ui({history: mismatched, selection: selected(), review});
    assert.equal(allNodes(tree, "button")[0].props.disabled, true);
    assert.equal(allNodes(tree, "select")[0].props.disabled, true);
    assert.match(html, /Optional comparison unavailable/);
    assert.doesNotMatch(html, /Difference to review|Clear local comparison/);
    allNodes(tree, "button")[0].props.onClick(); // bypass visual disabled state
    assert.deepEqual(calls.reviews, []);
    assert.equal(calls.unavailable.length, 1);
  }
});

test("detached UI refuses changed or unavailable source without blocking file operations", () => {
  const previously = request().review;
  const {html,tree,calls} = ui({
    indexedFile: {...file, availabilityState: "CHANGED"},
    selection: selected(),
    review: previously,
  });
  assert.equal(allNodes(tree, "button")[0].props.disabled, true);
  assert.doesNotMatch(html, /Difference to review/);
  assert.match(html, /Document access is unaffected/);
  allNodes(tree, "button")[0].props.onClick();
  assert.deepEqual(calls.reviews, []);
  assert.deepEqual(calls.unavailable, ["source-unavailable"]);
});

test("extraction claims changed under same snapshot ID cannot retain opt-in selection or stale review", () => {
  const review = request().review;
  const changingVersion = {...snapshot, sourceContentVersionId: "new-content-version"};
  const changingHash = {...snapshot, sourceSha256Hex: "b".repeat(64)};
  for (const newer of [changingVersion, changingHash]) {
    assert.equal(selectionIsCurrent(selected(), visit, file, newer), false);
    assert.equal(current(review, {snapshot: newer}), false);
    const {tree,html,calls} = ui({snapshot: newer, selection: selected(), review});
    assert.equal(allNodes(tree, "button")[0].props.disabled, true);
    assert.doesNotMatch(html, /Difference to review/);
    allNodes(tree, "button")[0].props.onClick();
    assert.deepEqual(calls.reviews, []);
  }
});

test("an old explicitly selected book cannot silently retarget an otherwise matching new version", () => {
  const newer = {
    ...snapshot, extractionSnapshotId: snapshot.extractionSnapshotId,
    sourceContentVersionId: "later-version",
    sourceSha256Hex: "b".repeat(64),
  };
  const laterHistory = {
    ...history, entries: [{
      ...history.entries[0],
      contentVersionId: newer.sourceContentVersionId,
      sha256Hex: newer.sourceSha256Hex,
    }],
  };
  const {tree,html,calls} = ui({
    snapshot: newer, history: laterHistory, selection: selected(), review: request().review,
  });
  assert.equal(allNodes(tree, "select")[0].props.disabled, false);
  assert.equal(allNodes(tree, "button")[0].props.disabled, true);
  assert.doesNotMatch(html, /Difference to review/);
  allNodes(tree, "button")[0].props.onClick();
  assert.deepEqual(calls.reviews, []);
  assert.deepEqual(calls.unavailable, ["no-explicit-selection"]);
});

test("detached UI requires source history on both render and Compare click paths", () => {
  const source = readFileSync("src/document-match-opt-in-control.ts", "utf8");
  assert.match(source, /correlateOptionalExtractionWithDocumentHistory\(/);
  assert.match(source, /indexedFile, snapshot, history, selectedReference \?\? null, rules, true/);
  assert.match(source, /disabled: !versionReady/);
  assert.doesNotMatch(source, /@tauri-apps|invoke\(|fetch\(|writeFile|localStorage|sessionStorage/);
});

test("fabricated advisory outcomes and finding details never surface with otherwise valid IDs", () => {
  const original = request().review;
  assert.equal(current(original), true);
  const originalFindings = original.observation.findings;
  const forged = [
    {outcome: "consistent"},
    {outcome: "incomplete"},
    {findings: []},
    {findings: [...originalFindings, originalFindings[0]]},
    {findings: [...originalFindings].reverse()},
    {findings: [{...originalFindings[0], status: "consistent"}, originalFindings[1]]},
    {findings: [{...originalFindings[0], label: "Unjustified verified match"}, originalFindings[1]]},
    {findings: [{...originalFindings[0], ruleId: "renamed"}, originalFindings[1]]},
    {findings: [{...originalFindings[0], documentField: "another-field"}, originalFindings[1]]},
    {findings: [{...originalFindings[0], referenceField: "another-field"}, originalFindings[1]]},
    {findings: [{...originalFindings[0], documentValue: "UNTRUSTED"}, originalFindings[1]]},
    {findings: [{...originalFindings[0], referenceValue: "UNTRUSTED"}, originalFindings[1]]},
    {findings: [{...originalFindings[0], reason: "both-fields-absent"}, originalFindings[1]]},
    {findings: [null, originalFindings[1]]},
  ];
  for (const tamper of forged) {
    const review = {
      ...original,
      observation: {...original.observation, ...tamper},
    };
    assert.equal(current(review), false);
    const {html,tree} = ui({selection: selected(), review});
    assert.doesNotMatch(html, /Difference to review|Clear local comparison|Unjustified verified match/);
    assert.equal(allNodes(tree, "button").length, 1);
  }
});

test("faithful copied advisory observations remain displayable without automatic selection", () => {
  const original = request().review;
  const faithful = {
    ...original,
    observation: {
      ...original.observation,
      findings: original.observation.findings.map(f => ({...f})),
    },
  };
  assert.equal(current(faithful), true);
  const {html} = ui({selection: selected(), review: faithful});
  assert.match(html, /Difference to review/);
  assert.match(html, /Clear local comparison/);
  assert.doesNotMatch(ui().html, /Difference to review|Clear local comparison/);
});

test("revalidating cached findings never reads source bytes or mutates the comparison", () => {
  const flow = readFileSync("src/document-match-advisory-flow.ts", "utf8");
  const initial = request().review;
  const before = JSON.stringify([initial, file, snapshot, references, rules]);
  assert.equal(current(initial), true);
  assert.equal(JSON.stringify([initial, file, snapshot, references, rules]), before);
  assert.match(flow, /sameObservation\(review\.observation, fresh\.observation\)/);
  assert.match(flow, /inspectOptionalIndexedDocumentMatch\(file, snapshot!, matching\[0\], rules, true\)/);
  assert.doesNotMatch(flow, /@tauri-apps|invoke\(|fetch\(|localStorage|sessionStorage|fs\.write|writeFile/);
});

test("history refresh requires a new explicit book choice, even for identical metadata", () => {
  const review = request().review;
  assert.equal(current(review), true);
  const newHistoryEpoch = "fresh-history-read-2";
  // The native metadata is byte-for-byte identical; the host read epoch changed.
  assert.equal(selectionIsCurrent(selected(), visit, file, snapshot, newHistoryEpoch), false);
  assert.equal(current(review, {historyReadKey: newHistoryEpoch}), false);
  assert.equal(request({historyReadKey: newHistoryEpoch}).reason, "stale-selection");
  const {tree, html, calls} = ui({
    historyReadKey: newHistoryEpoch, selection: selected(), review,
  });
  assert.equal(allNodes(tree, "select")[0].props.disabled, false);
  assert.equal(allNodes(tree, "button")[0].props.disabled, true);
  assert.doesNotMatch(html, /Difference to review|Clear local comparison/);
  allNodes(tree, "button")[0].props.onClick();
  assert.deepEqual(calls.reviews, []);
  assert.deepEqual(calls.unavailable, ["no-explicit-selection"]);
  allNodes(tree, "select")[0].props.onChange({
    currentTarget: {value: "book-1"},
  });
  assert.equal(calls.selections.length, 1);
  assert.equal(calls.selections[0].historyReadKey, newHistoryEpoch);
  const fresh = request({
    selection: calls.selections[0], historyReadKey: newHistoryEpoch,
  });
  assert.equal(fresh.status, "ready");
  assert.equal(fresh.review.historyReadKey, newHistoryEpoch);
  assert.equal(current(fresh.review, {
    selection: calls.selections[0], historyReadKey: newHistoryEpoch,
  }), true);
});

test("missing, whitespace, or wrong epoch disables comparison and hides stale review", () => {
  const review = request().review;
  for (const historyReadKey of ["", " ", null, undefined]) {
    const {tree,html,calls} = ui({historyReadKey, selection: selected(), review});
    assert.equal(allNodes(tree, "select")[0].props.disabled, true);
    assert.equal(allNodes(tree, "button")[0].props.disabled, true);
    assert.doesNotMatch(html, /Clear local comparison|Difference to review/);
    assert.match(html, /Document access is unaffected/);
    allNodes(tree, "button")[0].props.onClick();
    assert.deepEqual(calls.reviews, []);
    assert.deepEqual(calls.unavailable, ["history-epoch-unavailable"]);
  }
  assert.equal(request({historyReadKey: "other-read"}).reason, "stale-selection");
  assert.equal(current(review, {historyReadKey: "other-read"}), false);
});

test("a changed history response cannot silently reuse the prior epoch under the host contract", () => {
  const priorReview = request().review;
  const changed = {...history, entries: [{
    ...history.entries[0], verificationState: "UNVERIFIED",
  }]};
  const blocked = ui({history: changed, selection: selected(), review: priorReview});
  assert.doesNotMatch(blocked.html, /Difference to review|Clear local comparison/);
  assert.equal(allNodes(blocked.tree, "button")[0].props.disabled, true);
  // On recovery, a new read token is required even if all original row
  // metadata is exactly restored by the native layer.
  const recovered = ui({
    history, historyReadKey: "history-read-after-recovery",
    selection: selected(), review: priorReview,
  });
  assert.equal(allNodes(recovered.tree, "button")[0].props.disabled, true);
  assert.doesNotMatch(recovered.html, /Difference to review|Clear local comparison/);
});

test("the history token is UI-only and cannot become a native permission grant", () => {
  const flow = readFileSync("src/document-match-advisory-flow.ts", "utf8");
  const control = readFileSync("src/document-match-opt-in-control.ts", "utf8");
  const app = readFileSync("src/App.tsx", "utf8");
  assert.match(control, /historyReadKey: string/);
  assert.match(control, /historyReadKey, references, rules/);
  assert.match(flow, /selection\.historyReadKey === historyReadKey/);
  assert.match(flow, /review\.historyReadKey !== historyReadKey/);
  assert.doesNotMatch(app, /document-match-opt-in-control/);
  assert.doesNotMatch(control + flow, /@tauri-apps|invoke\(|fetch\(|localStorage|sessionStorage|fs\.write|writeFile/);
});

function annotationCurrent(note, review, overrides = {}) {
  return volatileAnnotationIsCurrent(
    note, review,
    overrides.visit ?? visit,
    overrides.file ?? file,
    overrides.snapshot === undefined ? snapshot : overrides.snapshot,
    overrides.references ?? references,
    overrides.rules ?? rules,
    overrides.selection === undefined ? selected() : overrides.selection,
    overrides.historyReadKey === undefined ? HISTORY_READ_KEY : overrides.historyReadKey,
  );
}

test("a user acknowledgement is stamped to the exact volatile review inputs", () => {
  const review = request().review;
  const initial = JSON.stringify([review, file, snapshot, history, rules]);
  const plain = noteOptionalMatchFinding(review.observation, "number", "acknowledged");
  const stamped = stampVolatileAdvisoryAnnotation(review, plain);
  assert.equal(stamped.scope, "volatile-optional-annotation-only");
  assert.equal(stamped.visitKey, visit);
  assert.equal(stamped.fileInstanceId, file.fileInstanceId);
  assert.equal(stamped.historyReadKey, HISTORY_READ_KEY);
  assert.equal(stamped.inputKey, review.inputKey);
  assert.equal(stamped.disposition, "acknowledged");
  assert.equal(annotationCurrent(stamped, review), true);
  assert.ok(Object.isFrozen(stamped));
  assert.equal(JSON.stringify([review, file, snapshot, history, rules]), initial);
  assert.doesNotMatch(JSON.stringify(stamped), /sourceSha256Hex|sourceContentVersionId|professionalApproval|actorId|sourcePath/);
});

test("volatile annotation is hidden after visit, source, history or reference changes", () => {
  const review = request().review;
  const stamped = stampVolatileAdvisoryAnnotation(
    review, noteOptionalMatchFinding(review.observation, "number", "dismissed"),
  );
  const scenarios = [
    {visit: "another-visit"},
    {file: {...file, documentId: "other-document"}},
    {file: {...file, fileInstanceId: "other-instance"}},
    {snapshot: {...snapshot, extractionSnapshotId: "different-snapshot"}},
    {snapshot: {...snapshot, sourceContentVersionId: "different-version"}},
    {snapshot: {...snapshot, sourceSha256Hex: "b".repeat(64)}},
    {historyReadKey: "fresh-read-next"},
    {references: [references[1]]},
    {selection: chooseExplicitAdvisoryReference(
      visit, file, snapshot, "book-2", HISTORY_READ_KEY,
    )},
    {rules: [{...rules[0], label: "Changed mapping"}, rules[1]]},
  ];
  for (const scenario of scenarios) {
    assert.equal(annotationCurrent(stamped, review, scenario), false);
  }
});

test("even same snapshot ID and rule IDs cannot transfer an annotation to changed values", () => {
  const older = request().review;
  const stamped = stampVolatileAdvisoryAnnotation(
    older, noteOptionalMatchFinding(older.observation, "number", "acknowledged"),
  );
  const changedSnapshot = {
    ...snapshot, fields: {...snapshot.fields, number: "INV-200"},
  };
  const renewedSelection = chooseExplicitAdvisoryReference(
    visit, file, changedSnapshot, "book-1", HISTORY_READ_KEY,
  );
  const newer = request({snapshot: changedSnapshot, selection: renewedSelection}).review;
  assert.equal(newer.status, undefined);
  assert.notEqual(newer.inputKey, older.inputKey);
  assert.equal(annotationCurrent(stamped, newer, {
    snapshot: changedSnapshot, selection: renewedSelection,
  }), false);
  assert.equal(annotationCurrent(stamped, older, {snapshot: changedSnapshot}), false);
});

test("consistent or forged annotations cannot be stamped or accepted", () => {
  const review = request().review;
  const note = noteOptionalMatchFinding(review.observation, "number", "acknowledged");
  assert.throws(() => stampVolatileAdvisoryAnnotation(
    review, {...note, disposition: "approved"},
  ), /does not belong/);
  assert.throws(() => stampVolatileAdvisoryAnnotation(
    review, {...note, ruleId: "amount"},
  ), /does not belong/);
  assert.throws(() => stampVolatileAdvisoryAnnotation(
    {...review, inputKey: ""}, note,
  ), /identity is unavailable/);
  const stamped = stampVolatileAdvisoryAnnotation(review, note);
  const changes = [
    {scope: "formal-approval"}, {disposition: "approved"},
    {ruleId: "amount"}, {ruleId: "missing"},
    {historyReadKey: "other-history"}, {visitKey: "other-visit"},
    {inputKey: "different-input"}, {fileInstanceId: "other-file"},
    {referenceId: "other-reference"}, {extractionSnapshotId: "other-snapshot"},
  ];
  for (const change of changes) {
    assert.equal(annotationCurrent({...stamped, ...change}, review), false);
  }
  assert.equal(annotationCurrent(null, review), false);
  assert.equal(annotationCurrent(stamped, null), false);
});

test("detached control emits only visit-local guarded annotations", () => {
  const review = request().review;
  const {tree,calls,html} = ui({selection: selected(), review});
  assert.match(html, /Difference to review/);
  const embedded = allNodes(tree, DocumentMatchAdvisoryPanel)[0];
  assert.ok(embedded);
  const panel = DocumentMatchAdvisoryPanel(embedded.props);
  const annotationButtons = allNodes(panel, "button");
  assert.equal(annotationButtons.length, 2);
  annotationButtons[0].props.onClick();
  annotationButtons[1].props.onClick();
  assert.deepEqual(calls.annotations.map(x => x.disposition), ["acknowledged", "dismissed"]);
  assert.ok(calls.annotations.every(x => annotationCurrent(x, review)));
  assert.ok(calls.annotations.every(x => x.scope === "volatile-optional-annotation-only"));
  assert.ok(calls.annotations.every(x => Object.isFrozen(x)));
  const noCallback = ui({selection: selected(), review, onLocalAnnotation: undefined});
  const noCallbackPanel = allNodes(noCallback.tree, DocumentMatchAdvisoryPanel)[0];
  assert.equal(allNodes(DocumentMatchAdvisoryPanel(noCallbackPanel.props), "button").length, 0);
  assert.deepEqual(ui({review: null, selection: selected()}).calls.annotations, []);
});

test("annotation delivery remains detached, nonpersistent and without sign-off authority", () => {
  const flow = readFileSync("src/document-match-advisory-flow.ts", "utf8");
  const control = readFileSync("src/document-match-opt-in-control.ts", "utf8");
  const app = readFileSync("src/App.tsx", "utf8");
  assert.match(control, /onLocalAnnotation\?: \(annotation: VolatileAdvisoryAnnotation\)/);
  assert.match(control, /volatileAnnotationIsCurrent\(/);
  assert.match(flow, /annotation\.inputKey !== review\.inputKey/);
  assert.doesNotMatch(app, /document-match-opt-in-control|volatileAnnotationIsCurrent/);
  assert.doesNotMatch(flow + control,
    /@tauri-apps|invoke\(|fetch\(|localStorage|sessionStorage|fs\.write|writeFile|create_workpaper_signoff/);
});

test("rendered reference selector drops a stale choice after history changes in place", () => {
  const mutableHistory = {
    requestedDocumentId: history.requestedDocumentId,
    entries: history.entries.map(entry => ({...entry})),
  };
  const {tree,calls} = ui({history: mutableHistory});
  const selector = allNodes(tree, "select")[0];
  assert.equal(selector.props.disabled, false);
  mutableHistory.entries[0].verificationState = "UNVERIFIED";
  selector.props.onChange({currentTarget: {value: "book-1"}});
  assert.deepEqual(calls.selections, [null]);
  assert.equal(calls.clears, 1);
  assert.deepEqual(calls.reviews, []);
});

test("rendered reference selector rejects reference ambiguity introduced after render", () => {
  const mutableReferences = references.map(reference => ({...reference}));
  const {tree,calls} = ui({references: mutableReferences});
  assert.equal(allNodes(tree, "select")[0].props.disabled, false);
  mutableReferences.push({...mutableReferences[0]});
  allNodes(tree, "select")[0].props.onChange({
    currentTarget: {value: "book-1"},
  });
  assert.deepEqual(calls.selections, [null]);
  assert.deepEqual(calls.reviews, []);
});

test("stale annotation handler rechecks changed history before delivering any note", () => {
  const mutableHistory = {
    requestedDocumentId: history.requestedDocumentId,
    entries: history.entries.map(entry => ({...entry})),
  };
  const review = request().review;
  const {tree,calls} = ui({
    history: mutableHistory, selection: selected(), review,
  });
  const embedded = allNodes(tree, DocumentMatchAdvisoryPanel)[0];
  assert.ok(embedded);
  assert.equal(typeof embedded.props.onLocalAnnotation, "function");
  const note = noteOptionalMatchFinding(review.observation, "number", "dismissed");
  // Row can change in place without a React re-render or new historyReadKey.
  mutableHistory.entries[0].fileInstanceId = "other-file-instance";
  embedded.props.onLocalAnnotation(note);
  assert.deepEqual(calls.annotations, []);
  assert.deepEqual(calls.reviews, []);
});

test("malformed annotation delivery never leaks into the viewer host callback", () => {
  const review = request().review;
  const {tree,calls} = ui({selection: selected(), review});
  const embedded = allNodes(tree, DocumentMatchAdvisoryPanel)[0];
  const forged = [
    null,
    {},
    {scope: "formal-approval", ruleId: "number", disposition: "approved"},
    {...noteOptionalMatchFinding(review.observation, "number", "acknowledged"),
      disposition: "approved"},
  ];
  for (const annotation of forged) {
    assert.doesNotThrow(() => embedded.props.onLocalAnnotation(annotation));
  }
  assert.deepEqual(calls.annotations, []);
  const legitimate = noteOptionalMatchFinding(review.observation, "number", "dismissed");
  embedded.props.onLocalAnnotation(legitimate);
  assert.equal(calls.annotations.length, 1);
  assert.equal(calls.annotations[0].disposition, "dismissed");
});

test("history and annotation freshness are checked at events, not solely at render", () => {
  const source = readFileSync("src/document-match-opt-in-control.ts", "utf8");
  assert.match(source, /function historyStillCurrent\(\)/);
  assert.match(source, /const nextSelection = historyStillCurrent\(\)/);
  assert.match(source, /if \(!historyStillCurrent\(\)\) return;/);
  assert.doesNotMatch(source, /@tauri-apps|invoke\(|fetch\(|localStorage|sessionStorage|writeFile/);
  const app = readFileSync("src/App.tsx", "utf8");
  assert.doesNotMatch(app, /document-match-opt-in-control/);
});

test("extraction field edits under reused snapshot/version ID expire reference consent", () => {
  const priorSelection = selected();
  const priorReview = request().review;
  assert.equal(typeof priorSelection.extractionFieldsKey, "string");
  const changed = {...snapshot, fields: {...snapshot.fields, number: "INV-200"}};
  assert.equal(selectionIsCurrent(priorSelection, visit, file, changed, HISTORY_READ_KEY), false);
  assert.equal(request({snapshot: changed}).reason, "stale-selection");
  assert.equal(current(priorReview, {snapshot: changed}), false);
  const {tree,html,calls} = ui({snapshot: changed, selection: priorSelection, review: priorReview});
  assert.equal(allNodes(tree, "select")[0].props.disabled, false);
  assert.equal(allNodes(tree, "button")[0].props.disabled, true);
  assert.doesNotMatch(html, /Clear local comparison|Difference to review/);
  allNodes(tree, "button")[0].props.onClick();
  assert.deepEqual(calls.unavailable, ["no-explicit-selection"]);
  assert.deepEqual(calls.reviews, []);
  const renewed = chooseExplicitAdvisoryReference(
    visit, file, changed, "book-1", HISTORY_READ_KEY,
  );
  assert.notEqual(renewed.extractionFieldsKey, priorSelection.extractionFieldsKey);
  assert.equal(selectionIsCurrent(renewed, visit, file, changed, HISTORY_READ_KEY), true);
  const compared = request({snapshot: changed, selection: renewed});
  assert.equal(compared.status, "ready");
  assert.equal(compared.review.observation.outcome, "suggestions");
});

test("canonical opt-in binding ignores field order but detects changes in type or value", () => {
  const selectedNow = selected();
  const reordered = {
    ...snapshot, fields: {amountMinor: 12500, number: "INV-100"},
  };
  assert.equal(selectionIsCurrent(selectedNow, visit, file, reordered, HISTORY_READ_KEY), true);
  for (const fields of [
    {number: "INV-100", amountMinor: "12500"},
    {number: "INV-100", amountMinor: 12501},
    {number: "INV-100"},
    {number: "INV-100", amountMinor: 12500, ignoredNewField: 0},
  ]) {
    assert.equal(selectionIsCurrent(
      selectedNow, visit, file, {...snapshot, fields}, HISTORY_READ_KEY,
    ), false);
    assert.equal(request({snapshot: {...snapshot, fields}}).reason, "stale-selection");
  }
});

test("malformed extraction fields invalidate consent instead of causing optional UI errors", () => {
  const throwingFields = Object.defineProperty({}, "number", {
    enumerable: true,
    get() { throw new Error("bad optional field accessor"); },
  });
  for (const fields of [null, [], {number: {nested: true}}, throwingFields]) {
    const invalidSnapshot = {...snapshot, fields};
    assert.equal(
      chooseExplicitAdvisoryReference(visit, file, invalidSnapshot, "book-1", HISTORY_READ_KEY),
      null,
    );
    assert.equal(selectionIsCurrent(selected(), visit, file, invalidSnapshot, HISTORY_READ_KEY), false);
    assert.equal(request({snapshot: invalidSnapshot}).reason, "stale-selection");
  }
});

test("a selection with missing or substituted field binding cannot enable Compare", () => {
  const withoutBinding = {...selected()};
  delete withoutBinding.extractionFieldsKey;
  const forged = [
    withoutBinding,
    {...selected(), extractionFieldsKey: ""},
    {...selected(), extractionFieldsKey: "not-the-current-fields"},
  ];
  for (const selection of forged) {
    assert.equal(selectionIsCurrent(selection, visit, file, snapshot, HISTORY_READ_KEY), false);
    assert.equal(request({selection}).reason, "stale-selection");
    const {tree,html,calls} = ui({selection, review: request().review});
    assert.equal(allNodes(tree, "button")[0].props.disabled, true);
    assert.doesNotMatch(html, /Difference to review|Clear local comparison/);
    allNodes(tree, "button")[0].props.onClick();
    assert.deepEqual(calls.reviews, []);
  }
});

test("field binding stays an unmounted local invalidation string, not provenance", () => {
  const source = readFileSync("src/document-match-advisory-flow.ts", "utf8");
  const app = readFileSync("src/App.tsx", "utf8");
  assert.match(source, /extractionFieldsKey: fieldsBinding/);
  assert.match(source, /selection\.extractionFieldsKey === extractionFieldsKey\(snapshot\)/);
  assert.doesNotMatch(source, /@tauri-apps|invoke\(|fetch\(|writeFile|localStorage|sessionStorage/);
  assert.doesNotMatch(app, /document-match-opt-in-control|extractionFieldsKey/);
});

test("malformed snapshot identity getters cannot crash detached optional matching UI", () => {
  for (const property of [
    "documentId", "fileInstanceId", "extractionSnapshotId", "sourceContentVersionId",
  ]) {
    const malformed = {...snapshot};
    Object.defineProperty(malformed, property, {
      enumerable: true,
      get() { throw new Error("broken optional extraction metadata"); },
    });
    const {html,tree,calls} = ui({snapshot: malformed, selection: selected(), review: request().review});
    assert.match(html, /Your original document remains available/);
    assert.doesNotMatch(html, /Clear local comparison|Difference to review/);
    assert.equal(allNodes(tree, "select")[0].props.disabled, true);
    assert.equal(allNodes(tree, "button")[0].props.disabled, true);
    allNodes(tree, "button")[0].props.onClick();
    assert.deepEqual(calls.reviews, []);
    assert.deepEqual(calls.unavailable, ["comparison-input-invalid"]);
  }
});

test("malformed selected file or history getters disable only optional UI", () => {
  const badFile = {...file};
  Object.defineProperty(badFile, "documentId", {
    enumerable: true, get() { throw new Error("inaccessible file metadata"); },
  });
  const badHistory = {...history};
  Object.defineProperty(badHistory, "entries", {
    enumerable: true, get() { throw new Error("unreadable history"); },
  });
  for (const overrides of [{indexedFile: badFile}, {history: badHistory}]) {
    const {html,tree,calls} = ui({
      ...overrides, selection: selected(), review: request().review,
    });
    assert.match(html, /original document remains available/);
    assert.doesNotMatch(html, /Clear local comparison|Difference to review/);
    assert.equal(allNodes(tree, "select")[0].props.disabled, true);
    assert.equal(allNodes(tree, "button")[0].props.disabled, true);
    allNodes(tree, "button")[0].props.onClick();
    assert.deepEqual(calls.reviews, []);
    assert.deepEqual(calls.unavailable, ["comparison-input-invalid"]);
  }
});

test("malformed history on stale handlers cannot create a new selection or annotation", () => {
  const mutableHistory = {...history, entries: history.entries.map(row => ({...row}))};
  const review = request().review;
  const {tree,calls} = ui({history: mutableHistory, selection: selected(), review});
  const selector = allNodes(tree, "select")[0];
  const panel = allNodes(tree, DocumentMatchAdvisoryPanel)[0];
  assert.ok(panel);
  Object.defineProperty(mutableHistory.entries[0], "fileInstanceId", {
    get() { throw new Error("history row failed after render"); },
  });
  assert.doesNotThrow(() => selector.props.onChange({currentTarget: {value: "book-1"}}));
  assert.deepEqual(calls.selections, [null]);
  const note = noteOptionalMatchFinding(review.observation, "number", "dismissed");
  assert.doesNotThrow(() => panel.props.onLocalAnnotation(note));
  assert.deepEqual(calls.annotations, []);
  assert.deepEqual(calls.reviews, []);
});

test("malformed metadata is a presentation-only failure, not a permission or source edit", () => {
  const gate = readFileSync("src/document-match-source-version-gate.ts", "utf8");
  const control = readFileSync("src/document-match-opt-in-control.ts", "utf8");
  const app = readFileSync("src/App.tsx", "utf8");
  assert.match(gate, /Even preflight history\/file\/snapshot getters/);
  assert.match(control, /snapshotAvailable = false;/);
  assert.doesNotMatch(app, /document-match-opt-in-control/);
  assert.doesNotMatch(gate + control,
    /@tauri-apps|invoke\(|fetch\(|localStorage|sessionStorage|fs\.write|writeFile/);
});
