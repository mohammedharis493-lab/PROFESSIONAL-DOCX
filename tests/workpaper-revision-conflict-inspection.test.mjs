import { test } from "node:test";
import assert from "node:assert/strict";
import {
  WORKPAPER_REVISION_CONTENT_FIELDS,
  openWorkpaperRevisionEditorSession,
  editWorkpaperRevisionSessionField,
  projectWorkpaperRevisionDraftContent,
} from "../src/workpaper-revision-edit-draft.ts";
import {
  inspectWorkpaperRevisionObservedText,
  inspectWorkpaperRevisionReadObservationChanges,
} from "../src/workpaper-revision-conflict-inspection.ts";
import { openStoredWorkpaperRevisionReadSnapshot } from "../src/workpaper-revision-read-snapshot.ts";

function base() {
  return {
    workpaperId: "workpaper-1",
    workpaperRevisionId: "revision-4",
    content: {
      objective: "Review  \r\n",
      procedurePerformed: "Verify approval",
      population: "Whole population",
      sample: "Sample 25",
      exceptions: "None",
      managementExplanation: "No response",
      conclusion: "Prior conclusion",
    },
  };
}
function observed(updates = {}, revisionId = "revision-5") {
  const start = base();
  return {
    ...start,
    workpaperRevisionId: revisionId,
    content: { ...start.content, ...updates },
  };
}
function statusFor(report, name) {
  return report.fields.find(x => x.field === name)?.status;
}
test("same immutable base with identical text has no observed changes", () => {
  const report = inspectWorkpaperRevisionObservedText(
    openWorkpaperRevisionEditorSession(base()), observed({}, "revision-4"),
  );
  assert.equal(report.scope, "client-three-way-text-only");
  assert.equal(report.differentRevisionObserved, false);
  assert.deepEqual(report.fields.map(x => x.field), [...WORKPAPER_REVISION_CONTENT_FIELDS]);
  assert.ok(report.fields.every(x => x.status === "unchanged"));
});
test("same observed base preserves local typing and does not rebase", () => {
  const session = editWorkpaperRevisionSessionField(
    openWorkpaperRevisionEditorSession(base()), "objective", "Local",
  );
  const report = inspectWorkpaperRevisionObservedText(session, observed({}, "revision-4"));
  assert.equal(statusFor(report, "objective"), "local-only");
  assert.equal(report.expectedBaseRevisionId, "revision-4");
  assert.equal(session.draft.expectedLatestRevisionId, "revision-4");
});
test("stale observed revision with no textual differences is still marked different", () => {
  const report = inspectWorkpaperRevisionObservedText(
    openWorkpaperRevisionEditorSession(base()), observed(),
  );
  assert.equal(report.differentRevisionObserved, true);
  assert.ok(report.fields.every(x => x.status === "unchanged"));
});
test("remote-only edit is marked observed-only, not applied to the unsaved draft", () => {
  const session = openWorkpaperRevisionEditorSession(base());
  const report = inspectWorkpaperRevisionObservedText(
    session, observed({ population: "Different population" }),
  );
  assert.equal(statusFor(report, "population"), "observed-only");
  assert.equal(session.draft.content.population, "Whole population");
});
test("local-only edit is not lost when another revision appears", () => {
  const session = editWorkpaperRevisionSessionField(
    openWorkpaperRevisionEditorSession(base()), "exceptions", "Minor typo fix",
  );
  const report = inspectWorkpaperRevisionObservedText(
    session, observed({ conclusion: "Remote conclusion" }),
  );
  assert.equal(statusFor(report, "exceptions"), "local-only");
  assert.equal(statusFor(report, "conclusion"), "observed-only");
  assert.equal(session.draft.content.exceptions, "Minor typo fix");
  assert.equal(session.draft.expectedLatestRevisionId, "revision-4");
});
test("two distinct edits of the same field flag a text conflict", () => {
  const session = editWorkpaperRevisionSessionField(
    openWorkpaperRevisionEditorSession(base()), "conclusion", "Local result",
  );
  const report = inspectWorkpaperRevisionObservedText(
    session, observed({ conclusion: "Observed result" }),
  );
  assert.equal(statusFor(report, "conclusion"), "text-conflict");
  assert.equal(session.draft.content.conclusion, "Local result");
});
test("identical edits on both sides are converged text, NOT permission to Save", () => {
  const session = editWorkpaperRevisionSessionField(
    openWorkpaperRevisionEditorSession(base()), "conclusion", "Agreed text",
  );
  const report = inspectWorkpaperRevisionObservedText(
    session, observed({ conclusion: "Agreed text" }),
  );
  assert.equal(statusFor(report, "conclusion"), "converged-text");
  assert.equal(report.differentRevisionObserved, true);
  assert.equal("authorized" in report, false);
  assert.equal("maySave" in report, false);
});
test("field statuses are fixed-order, immutable, and exclude raw text and evidence", () => {
  const session = editWorkpaperRevisionSessionField(
    openWorkpaperRevisionEditorSession(base()), "sample", "Edited sample",
  );
  const read = observed({ objective: "Changed objective" });
  const before = JSON.stringify(session);
  const beforeObserved = JSON.stringify(read);
  const report = inspectWorkpaperRevisionObservedText(session, read);
  assert.equal(JSON.stringify(session), before);
  assert.equal(JSON.stringify(read), beforeObserved);
  assert.ok(Object.isFrozen(report));
  assert.ok(Object.isFrozen(report.fields));
  assert.ok(Object.isFrozen(report.fields[0]));
  assert.deepEqual(Object.keys(report).sort(),
    ["differentRevisionObserved","expectedBaseRevisionId","fields","observedRevisionId","scope","workpaperId"].sort());
  assert.deepEqual(Object.keys(report.fields[0]).sort(), ["field","status"]);
  assert.equal("evidenceLinks" in report, false);
  assert.equal("actorId" in report, false);
});
test("exact CRLF, Unicode and trailing whitespace determine observed conflict", () => {
  const session = editWorkpaperRevisionSessionField(
    openWorkpaperRevisionEditorSession(base()), "objective", "Review  \r\n🔎",
  );
  const report = inspectWorkpaperRevisionObservedText(
    session, observed({ objective: "Review  \n🔎" }),
  );
  assert.equal(statusFor(report, "objective"), "text-conflict");
});
test("unchanged local content still carries an independent projection of all seven fields", () => {
  const session = openWorkpaperRevisionEditorSession(base());
  const report = inspectWorkpaperRevisionObservedText(
    session, observed({ conclusion: "Updated" }),
  );
  const projected = projectWorkpaperRevisionDraftContent(session);
  assert.equal(report.differentRevisionObserved, true);
  assert.equal(statusFor(report, "conclusion"), "observed-only");
  assert.equal(projected.expectedLatestRevisionId, "revision-4");
  assert.equal(projected.content.conclusion, "Prior conclusion");
});
test("foreign workpaper and missing latest revision are rejected", () => {
  const session = openWorkpaperRevisionEditorSession(base());
  assert.throws(() => inspectWorkpaperRevisionObservedText(
    session, { ...observed(), workpaperId: "another" },
  ), /same workpaper/);
  assert.throws(() => inspectWorkpaperRevisionObservedText(
    session, { ...observed(), workpaperRevisionId: " " },
  ), /complete revision identity/);
  assert.throws(() => inspectWorkpaperRevisionObservedText(session, null),
    /Observed revision/);
});
test("unexpected future field or missing canonical field fails closed", () => {
  const session = openWorkpaperRevisionEditorSession(base());
  assert.throws(() => inspectWorkpaperRevisionObservedText(
    session, observed({ futureField: "keep new data" }),
  ), /Unexpected observed canonical/);
  const broken = observed();
  delete broken.content.sample;
  assert.throws(() => inspectWorkpaperRevisionObservedText(session, broken),
    /Incomplete observed canonical.*sample/);
  const malformed = observed({ exceptions: 123 });
  assert.throws(() => inspectWorkpaperRevisionObservedText(session, malformed),
    /Incomplete observed canonical.*exceptions/);
});
test("same revision ID with divergent content is a consistency error even without local edits", () => {
  const session = openWorkpaperRevisionEditorSession(base());
  assert.throws(() => inspectWorkpaperRevisionObservedText(
    session, observed({ sample: "Inconsistent source" }, "revision-4"),
  ), /Inconsistent text.*same immutable revision/);
});

function evidenceLink(overrides = {}) {
  return {
    evidenceLinkId: "source-a", workpaperRevisionId: "revision-4",
    documentId: "document-a", contentVersionId: "version-a",
    controlledEvidenceVersionId: null, relationshipType: "SUPPORTING",
    description: "Exact observed source", ...overrides,
  };
}
function observedSignoff(overrides = {}) {
  return {
    signoffId: "signoff-a", workpaperId: "workpaper-1",
    workpaperRevisionId: "revision-4", supersededAtMs: null,
    supersededReason: null, supersededByRevisionId: null,
    ...overrides,
  };
}
function readSnapshot({
  revisionId = "revision-4", content = base().content,
  evidence = [evidenceLink({workpaperRevisionId: revisionId})],
  signoffs = [observedSignoff({workpaperRevisionId: revisionId})],
} = {}) {
  const baseline = base();
  return openStoredWorkpaperRevisionReadSnapshot({
    workpaperId: "workpaper-1",
    revisions: [{
      workpaperId: "workpaper-1",
      workpaperRevisionId: revisionId,
      revisionNumber: revisionId === "revision-4" ? 4 : 5,
      ...content,
    }],
    evidenceLinks: evidence,
    signoffs,
  });
}

test("same-base reordered evidence and active sign-offs are not false conflicts", () => {
  const links = [
    evidenceLink({ evidenceLinkId: "z-source" }),
    evidenceLink({ evidenceLinkId: "a-source", documentId: "document-b" }),
  ];
  const signoffs = [
    observedSignoff({ signoffId: "z-sign" }),
    observedSignoff({ signoffId: "a-sign" }),
  ];
  const opening = readSnapshot({ evidence: links, signoffs });
  const later = readSnapshot({ evidence: [...links].reverse(), signoffs: [...signoffs].reverse() });
  const session = editWorkpaperRevisionSessionField(
    opening.session, "conclusion", "Keep my unsaved change",
  );
  const report = inspectWorkpaperRevisionReadObservationChanges(session, opening, later);
  assert.equal(report.scope, "client-read-observation-diff-only");
  assert.equal(report.comparisonScope, "same-revision");
  assert.equal(report.differentRevisionObserved, false);
  assert.deepEqual(report.evidenceChanges, []);
  assert.deepEqual(report.activeSignoffChanges, []);
  assert.equal(session.draft.content.conclusion, "Keep my unsaved change");
  assert.equal(session.draft.expectedLatestRevisionId, "revision-4");
});

test("same-base link metadata drift and added or removed link IDs are visible independently", () => {
  const original = evidenceLink();
  const opening = readSnapshot({ evidence: [original], signoffs: [] });
  const later = readSnapshot({
    evidence: [
      evidenceLink({ relationshipType: "CORROBORATES", description: null }),
      evidenceLink({ evidenceLinkId: "source-new", documentId: "document-new" }),
    ],
    signoffs: [],
  });
  const report = inspectWorkpaperRevisionReadObservationChanges(opening.session, opening, later);
  assert.deepEqual(report.evidenceChanges, [
    { evidenceLinkId: "source-a", change: "metadata-changed" },
    { evidenceLinkId: "source-new", change: "added" },
  ]);
  const removed = inspectWorkpaperRevisionReadObservationChanges(
    opening.session, opening, readSnapshot({ evidence: [], signoffs: [] }),
  );
  assert.deepEqual(removed.evidenceChanges, [{ evidenceLinkId: "source-a", change: "removed" }]);
  assert.ok(Object.isFrozen(report));
  assert.ok(Object.isFrozen(report.evidenceChanges));
  assert.ok(Object.isFrozen(report.evidenceChanges[0]));
  assert.equal(opening.observedSourceEvidence[0].relationshipType, "SUPPORTING");
});

test("same-base late sign-off and supersession become ID changes, not inherited approval", () => {
  const opening = readSnapshot();
  const late = readSnapshot({
    signoffs: [observedSignoff(), observedSignoff({ signoffId: "signoff-new" })],
  });
  const added = inspectWorkpaperRevisionReadObservationChanges(opening.session, opening, late);
  assert.deepEqual(added.activeSignoffChanges, [
    { signoffId: "signoff-new", change: "added" },
  ]);
  assert.deepEqual(added.evidenceChanges, []);
  const superseded = readSnapshot({
    signoffs: [observedSignoff({
      supersededAtMs: 39,
      supersededReason: "History linked to newer revision",
      supersededByRevisionId: "revision-5",
    })],
  });
  const removed = inspectWorkpaperRevisionReadObservationChanges(
    opening.session, opening, superseded,
  );
  assert.deepEqual(removed.activeSignoffChanges, [
    { signoffId: "signoff-a", change: "removed" },
  ]);
  assert.equal("maySave" in added, false);
  assert.equal("approved" in added, false);
  assert.equal("grant" in removed, false);
});

test("successor revision comparisons never claim evidence custody or sign-off carry-forward", () => {
  const opening = readSnapshot();
  const later = readSnapshot({
    revisionId: "revision-5",
    content: { ...base().content, population: "New population" },
    evidence: [evidenceLink({
      evidenceLinkId: "new-source-link", workpaperRevisionId: "revision-5",
    })],
    signoffs: [],
  });
  const session = editWorkpaperRevisionSessionField(
    opening.session, "conclusion", "Unsaved local conclusion",
  );
  const report = inspectWorkpaperRevisionReadObservationChanges(session, opening, later);
  assert.equal(report.comparisonScope, "across-revisions");
  assert.equal(report.differentRevisionObserved, true);
  assert.deepEqual(report.evidenceChanges, [
    { evidenceLinkId: "new-source-link", change: "added" },
    { evidenceLinkId: "source-a", change: "removed" },
  ]);
  assert.deepEqual(report.activeSignoffChanges, [
    { signoffId: "signoff-a", change: "removed" },
  ]);
  assert.equal(statusFor(report.text, "population"), "observed-only");
  assert.equal(statusFor(report.text, "conclusion"), "local-only");
  assert.equal(session.draft.expectedLatestRevisionId, "revision-4");
  assert.equal(session.draft.content.conclusion, "Unsaved local conclusion");
});

test("tampered opening base, foreign or incomplete observation, or divergent immutable text fails closed", () => {
  const opening = readSnapshot();
  const later = readSnapshot();
  const foreign = { ...later, session: {
    ...later.session, base: { ...later.session.base, workpaperId: "foreign" },
  } };
  assert.throws(
    () => inspectWorkpaperRevisionReadObservationChanges(opening.session, opening, foreign),
    /same workpaper/,
  );
  assert.throws(
    () => inspectWorkpaperRevisionReadObservationChanges(opening.session, opening, {
      ...later, observedBaseActiveSignoffIds: undefined,
    }),
    /Incomplete later revision read observation/,
  );
  assert.throws(
    () => inspectWorkpaperRevisionReadObservationChanges(opening.session, opening, {
      ...later, observedBaseActiveSignoffIds: ["signoff-a", "signoff-a"],
    }),
    /Invalid later active sign-off observation/,
  );
  assert.throws(
    () => inspectWorkpaperRevisionReadObservationChanges(
      opening.session, { ...opening, observedSourceEvidence: [evidenceLink(), evidenceLink()] }, later,
    ),
    /Invalid opening evidence observation/,
  );
  const wrongSession = openWorkpaperRevisionEditorSession({
    ...base(), content: { ...base().content, objective: "Different immutable base" },
  });
  assert.throws(
    () => inspectWorkpaperRevisionReadObservationChanges(wrongSession, opening, later),
    /immutable draft base/,
  );
  const sameIdAlteredContent = readSnapshot({
    content: { ...base().content, sample: "Unexpected divergent persisted text" },
  });
  assert.throws(
    () => inspectWorkpaperRevisionReadObservationChanges(opening.session, opening, sameIdAlteredContent),
    /Inconsistent text observed for the same immutable revision/,
  );
});
