/**
 * Legacy workpaper form tests. Existing successors are temporarily blocked
 * in the UI; the native commands remain callable, without CAS, trusted author
 * identity or evidence carry-forward. Not production security authorization.
 */
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import ts from "typescript";
import {
  assertLegacyInitialRevisionOnly,
  LEGACY_REVISION_SUCCESSOR_BLOCKED,
} from "../src/workpaper-legacy-revision-guard.ts";

const source = readFileSync("src/App.tsx", "utf8");
const tree = ts.createSourceFile(
  "src/App.tsx", source, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX,
);
assert.equal(tree.parseDiagnostics.length, 0, "legacy form source must parse");

function legacyForm(submitHandler, text = source) {
  const file = ts.createSourceFile(
    "src/App.tsx", text, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX,
  );
  let found = null;
  function visit(node) {
    if (ts.isJsxElement(node) && node.openingElement.tagName.getText(file) === "form") {
      const submit = node.openingElement.attributes.properties.find(attr =>
        ts.isJsxAttribute(attr) &&
        attr.name.getText(file) === "onSubmit" &&
        attr.initializer && ts.isJsxExpression(attr.initializer) &&
        attr.initializer.expression?.getText(file) === submitHandler
      );
      if (submit) {
        assert.equal(found, null, "unexpected duplicate legacy form");
        found = node.getText(file);
      }
    }
    ts.forEachChild(node, visit);
  }
  visit(file);
  assert.ok(found, "missing legacy form: " + submitHandler);
  return found;
}

test("legacy revision form prominently warns before existing three-field submit", () => {
  const form = legacyForm("submitWorkpaperRevision");
  assert.match(form, /Legacy local-draft entry only/);
  assert.match(form, /role="note"/);
  assert.match(form, /workpaperRevisions\.length > 0/);
  assert.match(form, /role="alert"/);
  assert.match(form, /Legacy revision warning:/);
  assert.match(form, /successor creation is blocked/);
  const button = form.slice(form.indexOf("<button"), form.indexOf("</button>") + 9);
  assert.match(button, /workpaperRevisions\.length > 0/);
  assert.ok(
    form.indexOf("Legacy revision warning:") < form.indexOf("Create first revision"),
    "successor risk disclosure must be before submit control",
  );
});

test("legacy warning names all four vulnerable hidden canonical fields", () => {
  // JSX line-wrapping is not a semantic difference in the visible warning.
  const form = legacyForm("submitWorkpaperRevision").replace(/\s+/g, " ");
  for (const field of [
    "Population", "Sample", "Exceptions", "Management explanation",
  ]) {
    assert.ok(form.includes(field), "omitted hidden field risk: " + field);
  }
  assert.match(form, /Earlier revisions remain immutable/);
  assert.match(form, /exact evidence\s+links are not carried forward/);
  assert.match(form, /previous sign-offs become historical/);
  assert.match(form, /does not detect a stale editor/);
  assert.match(form, /not use this form as a\s+lossless correction/);
});

test("legacy first-create payload remains unchanged behind successor UI guard", () => {
  const start = source.indexOf("async function submitWorkpaperRevision(");
  const end = source.indexOf("\n  function openClientEngagements", start);
  assert.ok(start >= 0 && end > start, "legacy revision handler disappeared");
  const handler = source.slice(start, end);
  assert.match(handler, /invoke<WorkpaperRevision>\("create_workpaper_revision"/);
  assert.match(handler, /initialOnly: true/);
  const native = readFileSync("src-tauri/src/lib.rs", "utf8");
  const persistence = readFileSync("src-tauri/src/persistence.rs", "utf8");
  assert.match(native, /initial_only: bool/);
  assert.match(native, /if revision\.initial_only/);
  assert.match(native, /persistence::create_initial_workpaper_revision/);
  assert.match(persistence, /if initial_only && previous\.is_some\(\)/);
  assert.match(persistence, /transaction_with_behavior\(TransactionBehavior::Immediate\)/);
  for (const field of [
    "population", "sample", "exceptions", "managementExplanation",
  ]) {
    assert.match(handler, new RegExp("\\b" + field + ': ""'));
  }
  assert.doesNotMatch(handler, /expectedLatestRevisionId/);
  assert.doesNotMatch(handler, /workpaper_revision_authorization_contract/);
});

test("legacy signer form conspicuously rejects verified-signature inference", () => {
  const form = legacyForm("submitWorkpaperSignoff");
  assert.match(form, /role="alert"/);
  assert.match(form, /Legacy unverified sign-off record:/);
  assert.match(form, /entered by the caller/);
  assert.match(form, /not established by an authenticated native/);
  assert.match(form, /must not be treated as independently verified professional approval/);
  assert.match(form, /exact evidence snapshots remain historical/);
  assert.ok(
    form.indexOf("Legacy unverified sign-off record:") < form.indexOf("Record sign-off"),
    "identity disclaimer must appear before the legacy submit control",
  );
});

test("risk disclosure regression detects missing successor notice", () => {
  const altered = source.replace("Legacy revision warning:", "Missing revision warning:");
  const form = legacyForm("submitWorkpaperRevision", altered);
  assert.doesNotMatch(form, /Legacy revision warning:/);
});

test("risk disclosure regression detects removed signer authenticity warning", () => {
  const altered = source.replace("Legacy unverified sign-off record:", "Signing record:");
  const form = legacyForm("submitWorkpaperSignoff", altered);
  assert.doesNotMatch(form, /Legacy unverified sign-off record:/);
});

test("legacy guard permits only first revision and fails closed for malformed native data", () => {
  assert.doesNotThrow(() => assertLegacyInitialRevisionOnly([]));
  for (const current of [[{}], [{}, {}], null, undefined, {}, "bad"]) {
    assert.throws(
      () => assertLegacyInitialRevisionOnly(current),
      error => error instanceof Error &&
        error.message === LEGACY_REVISION_SUCCESSOR_BLOCKED,
    );
  }
});

test("legacy submit re-reads native revisions and checks the guard before atomic initial-only create", () => {
  const start = source.indexOf("async function submitWorkpaperRevision(");
  const end = source.indexOf("\n  function openClientEngagements", start);
  assert.ok(start >= 0 && end > start, "missing legacy create handler");
  const handler = source.slice(start, end);
  const uiGuard = handler.indexOf("if (workpaperRevisions.length > 0)");
  const freshRead = handler.indexOf('invoke<WorkpaperRevision[]>("list_workpaper_revisions"');
  // The call is intentionally multiline in the App for readability.
  const freshReadMulti = handler.indexOf('"list_workpaper_revisions"');
  const verify = handler.indexOf("assertLegacyInitialRevisionOnly(currentRevisions)");
  const write = handler.indexOf('invoke<WorkpaperRevision>("create_workpaper_revision"');
  assert.ok(uiGuard >= 0 && freshReadMulti > uiGuard && verify > freshReadMulti && write > verify,
    "both loaded-state guard and fresh native read must precede legacy write");
  assert.equal(freshRead, -1, "expected multiline fresh-read invocation in App");
  assert.match(handler, /setError\(LEGACY_REVISION_SUCCESSOR_BLOCKED\);\s*return;/);
});
