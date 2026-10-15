import { test } from "node:test";
import assert from "node:assert/strict";
import {
  WORKPAPER_REVISION_CONTENT_FIELDS,
  openWorkpaperRevisionEditDraft,
  previewWorkpaperRevisionSave,
} from "../src/workpaper-revision-edit-draft.ts";

function baseline() {
  return {
    workpaperId: "workpaper-a",
    workpaperRevisionId: "revision-7",
    content: {
      objective: "Check control design.",
      procedurePerformed: "Interview management and trace approvals.",
      population: "All vendor setup records.",
      sample: "Twenty sampled vendors.",
      exceptions: "One exception remained open.",
      managementExplanation: "Reviewed corrective action  \n",
      conclusion: "Control is effective subject to the exception.",
    },
  };
}

test("Edit prefills all seven canonical fields, including hidden fields", () => {
  const base = baseline();
  const draft = openWorkpaperRevisionEditDraft(base);
  assert.deepEqual(Object.keys(draft.content), [...WORKPAPER_REVISION_CONTENT_FIELDS]);
  assert.deepEqual(draft.content, base.content);
  assert.equal(draft.expectedLatestRevisionId, base.workpaperRevisionId);
  assert.notEqual(draft.content, base.content);
  draft.content.objective = "Updated wording.";
  assert.equal(base.content.objective, "Check control design.");
});

test("one-word correction retains untouched fields and exact whitespace", () => {
  const base = baseline();
  const draft = openWorkpaperRevisionEditDraft(base);
  draft.content.objective = "Check control operation.";
  const preview = previewWorkpaperRevisionSave(base, draft);
  assert.equal(preview.kind, "changed");
  assert.deepEqual(preview.changedFields, ["objective"]);
  assert.deepEqual(preview.proposedWrite.content, {
    ...base.content,
    objective: "Check control operation.",
  });
  assert.equal(preview.proposedWrite.expectedLatestRevisionId, "revision-7");
});

test("no-op Save does not propose a successor", () => {
  const base = baseline();
  const draft = openWorkpaperRevisionEditDraft(base);
  assert.deepEqual(previewWorkpaperRevisionSave(base, draft), {
    kind: "no-change",
    changedFields: [],
  });
});

test("explicitly clearing one field is recorded, not confused with omission", () => {
  const base = baseline();
  const draft = openWorkpaperRevisionEditDraft(base);
  draft.content.exceptions = "";
  const preview = previewWorkpaperRevisionSave(base, draft);
  assert.equal(preview.kind, "changed");
  assert.deepEqual(preview.changedFields, ["exceptions"]);
  assert.equal(preview.proposedWrite.content.exceptions, "");
  assert.equal(preview.proposedWrite.content.sample, base.content.sample);
});

test("missing canonical field fails instead of silently becoming blank", () => {
  const base = baseline();
  const draft = openWorkpaperRevisionEditDraft(base);
  delete draft.content.population;
  assert.throws(() => previewWorkpaperRevisionSave(base, draft), /population/);
});

test("draft identity must match exact workpaper and immutable base", () => {
  const base = baseline();
  const wrongWorkpaper = openWorkpaperRevisionEditDraft(base);
  wrongWorkpaper.workpaperId = "workpaper-b";
  assert.throws(() => previewWorkpaperRevisionSave(base, wrongWorkpaper), /immutable.*base/);
  const wrongRevision = openWorkpaperRevisionEditDraft(base);
  wrongRevision.expectedLatestRevisionId = "revision-8";
  assert.throws(() => previewWorkpaperRevisionSave(base, wrongRevision), /immutable.*base/);
});

test("Save preview snapshots content without aliasing a mutable draft", () => {
  const base = baseline();
  const draft = openWorkpaperRevisionEditDraft(base);
  draft.content.conclusion = "Edited conclusion.";
  const preview = previewWorkpaperRevisionSave(base, draft);
  assert.equal(preview.kind, "changed");
  draft.content.conclusion = "Later unsaved change";
  assert.equal(preview.proposedWrite.content.conclusion, "Edited conclusion.");
  assert.equal(base.content.conclusion, "Control is effective subject to the exception.");
});

test("preview contains no actor, role, evidence or sign-off claims", () => {
  const base = baseline();
  const draft = openWorkpaperRevisionEditDraft(base);
  draft.content.objective += " Typo corrected.";
  const preview = previewWorkpaperRevisionSave(base, draft);
  assert.equal(preview.kind, "changed");
  assert.deepEqual(Object.keys(preview.proposedWrite).sort(),
    ["content", "expectedLatestRevisionId", "workpaperId"]);
  assert.deepEqual(Object.keys(preview.proposedWrite.content),
    [...WORKPAPER_REVISION_CONTENT_FIELDS]);
});

test("two drafts carry same expected base for future backend conflict rejection", () => {
  const base = baseline();
  const first = openWorkpaperRevisionEditDraft(base);
  const second = openWorkpaperRevisionEditDraft(base);
  first.content.objective = "First edit";
  second.content.objective = "Second edit";
  const a = previewWorkpaperRevisionSave(base, first);
  const b = previewWorkpaperRevisionSave(base, second);
  assert.equal(a.kind, "changed");
  assert.equal(b.kind, "changed");
  assert.equal(a.proposedWrite.expectedLatestRevisionId, "revision-7");
  assert.equal(b.proposedWrite.expectedLatestRevisionId, "revision-7");
  // NOT a real concurrency test: the backend must enforce compare-and-swap.
});
