import { test } from "node:test";
import assert from "node:assert/strict";
import {
  WORKPAPER_REVISION_CONTENT_FIELDS,
  openWorkpaperRevisionEditorSession,
  editWorkpaperRevisionSessionField,
  projectWorkpaperRevisionDraftContent,
} from "../src/workpaper-revision-edit-draft.ts";
import { inspectWorkpaperRevisionObservedText } from "../src/workpaper-revision-conflict-inspection.ts";

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
