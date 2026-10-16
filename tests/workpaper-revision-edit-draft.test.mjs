import { test } from "node:test";
import assert from "node:assert/strict";
import {
  WORKPAPER_REVISION_CONTENT_FIELDS,
  openWorkpaperRevisionEditDraft,
  previewWorkpaperRevisionContentChange,
  projectWorkpaperRevisionDraftContent,
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
  const preview = previewWorkpaperRevisionContentChange(base, draft);
  assert.equal(preview.kind, "content-changed");
  assert.deepEqual(preview.changedFields, ["objective"]);
  assert.deepEqual(preview.proposedWrite.content, {
    ...base.content,
    objective: "Check control operation.",
  });
  assert.equal(preview.proposedWrite.expectedLatestRevisionId, "revision-7");
});

test("unchanged canonical fields produce content-only no-change, not a full Save decision", () => {
  const base = baseline();
  const draft = openWorkpaperRevisionEditDraft(base);
  assert.deepEqual(previewWorkpaperRevisionContentChange(base, draft), {
    scope: "canonical-content-only",
    kind: "content-unchanged",
    changedFields: [],
  });
});

test("explicitly clearing one field is recorded, not confused with omission", () => {
  const base = baseline();
  const draft = openWorkpaperRevisionEditDraft(base);
  draft.content.exceptions = "";
  const preview = previewWorkpaperRevisionContentChange(base, draft);
  assert.equal(preview.kind, "content-changed");
  assert.deepEqual(preview.changedFields, ["exceptions"]);
  assert.equal(preview.proposedWrite.content.exceptions, "");
  assert.equal(preview.proposedWrite.content.sample, base.content.sample);
});

test("missing canonical field fails instead of silently becoming blank", () => {
  const base = baseline();
  const draft = openWorkpaperRevisionEditDraft(base);
  delete draft.content.population;
  assert.throws(() => previewWorkpaperRevisionContentChange(base, draft), /population/);
});

test("draft identity must match exact workpaper and immutable base", () => {
  const base = baseline();
  const wrongWorkpaper = openWorkpaperRevisionEditDraft(base);
  wrongWorkpaper.workpaperId = "workpaper-b";
  assert.throws(() => previewWorkpaperRevisionContentChange(base, wrongWorkpaper), /immutable.*base/);
  const wrongRevision = openWorkpaperRevisionEditDraft(base);
  wrongRevision.expectedLatestRevisionId = "revision-8";
  assert.throws(() => previewWorkpaperRevisionContentChange(base, wrongRevision), /immutable.*base/);
});

test("Save preview snapshots content without aliasing a mutable draft", () => {
  const base = baseline();
  const draft = openWorkpaperRevisionEditDraft(base);
  draft.content.conclusion = "Edited conclusion.";
  const preview = previewWorkpaperRevisionContentChange(base, draft);
  assert.equal(preview.kind, "content-changed");
  draft.content.conclusion = "Later unsaved change";
  assert.equal(preview.proposedWrite.content.conclusion, "Edited conclusion.");
  assert.equal(base.content.conclusion, "Control is effective subject to the exception.");
});

test("preview contains no actor, role, evidence or sign-off claims", () => {
  const base = baseline();
  const draft = openWorkpaperRevisionEditDraft(base);
  draft.content.objective += " Typo corrected.";
  const preview = previewWorkpaperRevisionContentChange(base, draft);
  assert.equal(preview.kind, "content-changed");
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
  const a = previewWorkpaperRevisionContentChange(base, first);
  const b = previewWorkpaperRevisionContentChange(base, second);
  assert.equal(a.kind, "content-changed");
  assert.equal(b.kind, "content-changed");
  assert.equal(a.proposedWrite.expectedLatestRevisionId, "revision-7");
  assert.equal(b.proposedWrite.expectedLatestRevisionId, "revision-7");
  // NOT a real concurrency test: the backend must enforce compare-and-swap.
});

test("unchanged text cannot establish an unchanged evidence set or approval", () => {
  const base = baseline();
  const draft = openWorkpaperRevisionEditDraft(base);
  // There could be a newly linked evidence version or reopened review note,
  // which this deliberately content-only helper has no authority to inspect.
  const comparison = previewWorkpaperRevisionContentChange(base, draft);
  assert.equal(comparison.kind, "content-unchanged");
  assert.equal(comparison.scope, "canonical-content-only");
  assert.equal("proposedWrite" in comparison, false);
  assert.equal("evidenceLinkIds" in comparison, false);
  assert.equal("approvalStatus" in comparison, false);
});

test("new canonical fields fail closed instead of disappearing in a successor", () => {
  const base = baseline();
  base.content.followUpFinding = "A new field introduced by later schema migration.";
  assert.throws(() => openWorkpaperRevisionEditDraft(base), /Unexpected canonical content field/);
});

test("invalid canonical runtime objects and inherited fields fail closed", () => {
  const base = baseline();
  base.content = null;
  assert.throws(() => openWorkpaperRevisionEditDraft(base), /field object/);
  const incomplete = baseline();
  const inherited = Object.create({ population: "inherited but not canonical" });
  Object.assign(inherited, incomplete.content);
  delete inherited.population;
  incomplete.content = inherited;
  assert.throws(() => openWorkpaperRevisionEditDraft(incomplete), /population/);
});

test("Unicode and whitespace edits are faithfully represented without classification", () => {
  const base = baseline();
  const draft = openWorkpaperRevisionEditDraft(base);
  draft.content.managementExplanation += "🔎 é\u0301\u00a0";
  const comparison = previewWorkpaperRevisionContentChange(base, draft);
  assert.equal(comparison.scope, "canonical-content-only");
  assert.deepEqual(comparison.changedFields, ["managementExplanation"]);
  assert.equal(comparison.kind, "content-changed");
  assert.equal(comparison.proposedWrite.content.managementExplanation,
    base.content.managementExplanation + "🔎 é\u0301\u00a0");
  assert.equal("classification" in comparison, false);
});


import {
  openWorkpaperRevisionEditorSession,
  editWorkpaperRevisionSessionField,
  previewWorkpaperRevisionEditorSession,
  inspectWorkpaperRevisionEditorFreshness,
} from "../src/workpaper-revision-edit-draft.ts";

test("immutable editor session snapshots all seven fields and the exact base identity", () => {
  const base = baseline();
  const session = openWorkpaperRevisionEditorSession(base);
  assert.deepEqual(session.draft.content, base.content);
  assert.deepEqual(Object.keys(session.draft.content), [...WORKPAPER_REVISION_CONTENT_FIELDS]);
  assert.equal(session.base.workpaperRevisionId, "revision-7");
  assert.equal(session.draft.expectedLatestRevisionId, "revision-7");
  assert.ok(Object.isFrozen(session));
  assert.ok(Object.isFrozen(session.base));
  assert.ok(Object.isFrozen(session.base.content));
  assert.ok(Object.isFrozen(session.draft));
  assert.ok(Object.isFrozen(session.draft.content));
  base.content.population = "external change after opening";
  assert.equal(session.draft.content.population, "All vendor setup records.");
  assert.equal(session.base.content.population, "All vendor setup records.");
});

test("one-field edit returns another detached, frozen session without dropping any field", () => {
  const original = openWorkpaperRevisionEditorSession(baseline());
  const updated = editWorkpaperRevisionSessionField(original, "objective", "Corrected typo");
  assert.notEqual(updated, original);
  assert.notEqual(updated.draft.content, original.draft.content);
  assert.equal(original.draft.content.objective, "Check control design.");
  assert.equal(updated.draft.content.objective, "Corrected typo");
  assert.equal(updated.draft.content.population, original.draft.content.population);
  assert.equal(updated.draft.content.managementExplanation, original.draft.content.managementExplanation);
  assert.deepEqual(previewWorkpaperRevisionEditorSession(updated).changedFields, ["objective"]);
  assert.ok(Object.isFrozen(updated.draft.content));
});

test("successive edits of all seven fields preserve intermediate edits", () => {
  const original = openWorkpaperRevisionEditorSession(baseline());
  let session = original;
  for (const [index, field] of WORKPAPER_REVISION_CONTENT_FIELDS.entries()) {
    session = editWorkpaperRevisionSessionField(session, field, "edited field " + index);
  }
  const preview = previewWorkpaperRevisionEditorSession(session);
  assert.equal(preview.kind, "content-changed");
  assert.deepEqual(preview.changedFields, [...WORKPAPER_REVISION_CONTENT_FIELDS]);
  for (const [index, field] of WORKPAPER_REVISION_CONTENT_FIELDS.entries()) {
    assert.equal(preview.proposedWrite.content[field], "edited field " + index);
  }
  assert.deepEqual(original.draft.content, baseline().content);
  assert.equal(preview.proposedWrite.expectedLatestRevisionId, "revision-7");
});

test("unchanged snapshot is content-only, not a permission or full no-op decision", () => {
  const session = openWorkpaperRevisionEditorSession(baseline());
  const comparison = previewWorkpaperRevisionEditorSession(session);
  assert.deepEqual(comparison, {
    scope: "canonical-content-only",
    kind: "content-unchanged",
    changedFields: [],
  });
  assert.equal("proposedWrite" in comparison, false);
  assert.equal("approval" in comparison, false);
  assert.equal("evidence" in comparison, false);
});

test("clearing a field and whitespace/Unicode are preserved verbatim", () => {
  const original = openWorkpaperRevisionEditorSession(baseline());
  const cleared = editWorkpaperRevisionSessionField(original, "exceptions", "");
  const next = editWorkpaperRevisionSessionField(
    cleared, "managementExplanation", "  \r\n🔎 e\u0301\u00a0 ",
  );
  const comparison = previewWorkpaperRevisionEditorSession(next);
  assert.deepEqual(comparison.changedFields, ["exceptions", "managementExplanation"]);
  assert.equal(comparison.proposedWrite.content.exceptions, "");
  assert.equal(comparison.proposedWrite.content.managementExplanation, "  \r\n🔎 e\u0301\u00a0 ");
  assert.equal(comparison.proposedWrite.content.population, "All vendor setup records.");
});

test("editing and then restoring the exact value remains a content-only unchanged preview", () => {
  const original = openWorkpaperRevisionEditorSession(baseline());
  const changed = editWorkpaperRevisionSessionField(original, "objective", "Temporary");
  const restored = editWorkpaperRevisionSessionField(
    changed, "objective", original.base.content.objective,
  );
  assert.equal(previewWorkpaperRevisionEditorSession(restored).kind, "content-unchanged");
  assert.equal(restored.draft.expectedLatestRevisionId, "revision-7");
});

test("signed historical revision is editable in draft without approval inheritance", () => {
  const signed = {
    ...baseline(),
    workflowState: "FINALISED",
    signoffs: [{ signoffId: "historical-signoff", workpaperRevisionId: "revision-7" }],
  };
  const session = editWorkpaperRevisionSessionField(
    openWorkpaperRevisionEditorSession(signed), "conclusion", "Editorial correction",
  );
  assert.equal(previewWorkpaperRevisionEditorSession(session).kind, "content-changed");
  assert.equal("workflowState" in session, false);
  assert.equal("signoffs" in session, false);
  assert.deepEqual(Object.keys(previewWorkpaperRevisionEditorSession(session).proposedWrite).sort(),
    ["content", "expectedLatestRevisionId", "workpaperId"]);
});

test("freshness preview detects a newer revision but leaves the draft unchanged", () => {
  const session = editWorkpaperRevisionSessionField(
    openWorkpaperRevisionEditorSession(baseline()), "sample", "Updated sample",
  );
  const current = inspectWorkpaperRevisionEditorFreshness(
    session, { workpaperId: "workpaper-a", workpaperRevisionId: "revision-7" },
  );
  assert.deepEqual(current, {
    scope: "client-preview-only",
    state: "base-current",
    expectedLatestRevisionId: "revision-7",
    observedLatestRevisionId: "revision-7",
  });
  const stale = inspectWorkpaperRevisionEditorFreshness(
    session, { workpaperId: "workpaper-a", workpaperRevisionId: "revision-8" },
  );
  assert.deepEqual(stale, {
    scope: "client-preview-only",
    state: "base-stale",
    expectedLatestRevisionId: "revision-7",
    observedLatestRevisionId: "revision-8",
  });
  assert.equal(session.draft.content.sample, "Updated sample");
  assert.equal(previewWorkpaperRevisionEditorSession(session).proposedWrite.expectedLatestRevisionId,
    "revision-7");
});

test("freshness cannot use a different workpaper or absent latest revision", () => {
  const session = openWorkpaperRevisionEditorSession(baseline());
  assert.throws(() => inspectWorkpaperRevisionEditorFreshness(
    session, { workpaperId: "workpaper-b", workpaperRevisionId: "revision-7" },
  ), /must match/);
  assert.throws(() => inspectWorkpaperRevisionEditorFreshness(
    session, { workpaperId: "workpaper-a", workpaperRevisionId: "" },
  ), /must match/);
  assert.throws(() => inspectWorkpaperRevisionEditorFreshness(session, null), /must match/);
});

test("unrecognized field and non-text runtime values are rejected", () => {
  const session = openWorkpaperRevisionEditorSession(baseline());
  assert.throws(() =>
    editWorkpaperRevisionSessionField(session, "newField", "would otherwise lose data"),
    /Unrecognized/,
  );
  assert.throws(() => editWorkpaperRevisionSessionField(session, "objective", null), /must be text/);
  assert.equal(session.draft.content.objective, "Check control design.");
});

test("invalid or future-schema content cannot be opened as a lossy session", () => {
  const broken = baseline();
  delete broken.content.sample;
  assert.throws(() => openWorkpaperRevisionEditorSession(broken), /sample/);
  const evolved = baseline();
  evolved.content.followUpAction = "must not be discarded";
  assert.throws(() => openWorkpaperRevisionEditorSession(evolved), /Unexpected canonical/);
});

test("proposed text content is snapshotted, never an alias of an input object", () => {
  const external = baseline();
  const session = editWorkpaperRevisionSessionField(
    openWorkpaperRevisionEditorSession(external), "conclusion", "Revised",
  );
  const proposal = previewWorkpaperRevisionEditorSession(session);
  assert.equal(proposal.kind, "content-changed");
  external.content.conclusion = "Late input mutation";
  assert.equal(proposal.proposedWrite.content.conclusion, "Revised");
  assert.equal(session.draft.content.conclusion, "Revised");
  assert.equal("actorId" in proposal.proposedWrite, false);
});


test("unchanged text still projects all seven fields and the expected immutable base", () => {
  const session = openWorkpaperRevisionEditorSession(baseline());
  assert.equal(previewWorkpaperRevisionEditorSession(session).kind, "content-unchanged");
  const projected = projectWorkpaperRevisionDraftContent(session);
  assert.equal(projected.scope, "untrusted-draft-content-only");
  assert.equal(projected.workpaperId, "workpaper-a");
  assert.equal(projected.expectedLatestRevisionId, "revision-7");
  assert.deepEqual(projected.content, baseline().content);
  assert.deepEqual(Object.keys(projected.content), [...WORKPAPER_REVISION_CONTENT_FIELDS]);
});

test("one-word edit projects changed text plus every untouched canonical field", () => {
  const session = editWorkpaperRevisionSessionField(
    openWorkpaperRevisionEditorSession(baseline()), "objective", "Typo corrected",
  );
  const projected = projectWorkpaperRevisionDraftContent(session);
  assert.equal(projected.content.objective, "Typo corrected");
  assert.equal(projected.content.population, baseline().content.population);
  assert.equal(projected.content.managementExplanation, baseline().content.managementExplanation);
  assert.equal(projected.expectedLatestRevisionId, "revision-7");
});

test("projection keeps explicit empty strings rather than substituting omitted fields", () => {
  const session = editWorkpaperRevisionSessionField(
    openWorkpaperRevisionEditorSession(baseline()), "exceptions", "",
  );
  const projected = projectWorkpaperRevisionDraftContent(session);
  assert.equal(projected.content.exceptions, "");
  assert.equal(Object.keys(projected.content).length, 7);
  assert.equal(projected.content.sample, baseline().content.sample);
});

test("projection preserves CRLF, trailing whitespace and Unicode byte-for-byte", () => {
  const base = baseline();
  base.content.objective = "A\r\nB \t \u00a0🔍é\u0301";
  const session = openWorkpaperRevisionEditorSession(base);
  const projected = projectWorkpaperRevisionDraftContent(session);
  assert.equal(projected.content.objective, "A\r\nB \t \u00a0🔍é\u0301");
  assert.equal(projected.content.managementExplanation, base.content.managementExplanation);
});

test("projection is deeply frozen and remains independent of subsequent session edits", () => {
  const session = openWorkpaperRevisionEditorSession(baseline());
  const first = projectWorkpaperRevisionDraftContent(session);
  const second = editWorkpaperRevisionSessionField(session, "conclusion", "Edited later");
  const later = projectWorkpaperRevisionDraftContent(second);
  assert.ok(Object.isFrozen(first));
  assert.ok(Object.isFrozen(first.content));
  assert.notEqual(first.content, session.draft.content);
  assert.equal(first.content.conclusion, baseline().content.conclusion);
  assert.equal(later.content.conclusion, "Edited later");
});

test("projection excludes actor, grant, source evidence, copied links and all signoffs", () => {
  const projected = projectWorkpaperRevisionDraftContent(
    openWorkpaperRevisionEditorSession(baseline()),
  );
  assert.deepEqual(Object.keys(projected).sort(), [
    "content", "expectedLatestRevisionId", "scope", "workpaperId",
  ]);
  for (const forbidden of [
    "actorId", "actorRole", "permission", "grant", "evidenceLinkIds",
    "contentVersionId", "controlledEvidenceVersionId", "signoffId",
    "approval", "authorized", "save",
  ]) {
    assert.equal(forbidden in projected, false);
  }
});

test("projection fails closed when edited session targets another revision or workpaper", () => {
  const session = openWorkpaperRevisionEditorSession(baseline());
  assert.throws(() => projectWorkpaperRevisionDraftContent({
    ...session,
    draft: { ...session.draft, expectedLatestRevisionId: "revision-8" },
  }), /immutable.*base/);
  assert.throws(() => projectWorkpaperRevisionDraftContent({
    ...session,
    draft: { ...session.draft, workpaperId: "workpaper-other" },
  }), /immutable.*base/);
});

test("projection refuses missing or future canonical fields instead of losing text", () => {
  const session = openWorkpaperRevisionEditorSession(baseline());
  assert.throws(() => projectWorkpaperRevisionDraftContent({
    ...session,
    draft: { ...session.draft, content: {
      ...session.draft.content, sample: undefined,
    } },
  }), /sample/);
  assert.throws(() => projectWorkpaperRevisionDraftContent({
    ...session,
    draft: { ...session.draft, content: {
      ...session.draft.content, futureCanonicalField: "Keep this",
    } },
  }), /futureCanonicalField/);
});

test("projection refuses missing or whitespace-only base identifiers", () => {
  const base = baseline();
  base.workpaperId = " ";
  assert.throws(() => projectWorkpaperRevisionDraftContent(
    openWorkpaperRevisionEditorSession(base),
  ), /nonblank/);
  const another = baseline();
  another.workpaperRevisionId = " ";
  assert.throws(() => projectWorkpaperRevisionDraftContent(
    openWorkpaperRevisionEditorSession(another),
  ), /nonblank/);
});
