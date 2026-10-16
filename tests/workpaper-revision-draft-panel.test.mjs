import { after, before, test } from "node:test";
import assert from "node:assert/strict";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { createServer } from "vite";
import {
  WORKPAPER_REVISION_CONTENT_FIELDS,
  editWorkpaperRevisionSessionField,
  openWorkpaperRevisionEditorSession,
} from "../src/workpaper-revision-edit-draft.ts";

function base() {
  return {
    workpaperId: "synthetic-workpaper",
    workpaperRevisionId: "revision-7",
    content: {
      objective: "Inspect <transactions> & approvals.",
      procedurePerformed: "Trace each approval to a source.",
      population: "All vendors.",
      sample: "Twenty vendors.",
      exceptions: "One issue.",
      managementExplanation: "Accepted  \r\n",
      conclusion: "Control design appears appropriate.",
    },
  };
}

let vite;
let DraftPanel;
before(async () => {
  vite = await createServer({
    appType: "custom",
    logLevel: "error",
    server: { middlewareMode: true },
  });
  ({ default: DraftPanel } = await vite.ssrLoadModule("/src/WorkpaperRevisionDraftPanel.tsx"));
});

after(async () => {
  await vite?.close();
});

function render(session, overrides = {}) {
  return renderToStaticMarkup(createElement(DraftPanel, {
    session,
    onSessionChange: () => {
      throw new Error("SSR must not try to persist draft content");
    },
    ...overrides,
  }));
}

test("seven-field controlled preview renders every textarea with original text", () => {
  const session = openWorkpaperRevisionEditorSession(base());
  const html = render(session);
  assert.equal((html.match(/<textarea\b/g) ?? []).length, 7);
  for (const name of WORKPAPER_REVISION_CONTENT_FIELDS) {
    assert.match(html, new RegExp('name="' + name + '"'));
  }
  for (const label of [
    "Objective", "Procedure performed", "Population", "Sample",
    "Exceptions", "Management explanation", "Conclusion",
  ]) {
    assert.ok(html.includes(label), "missing canonical label: " + label);
  }
  assert.ok(html.includes("Inspect &lt;transactions&gt; &amp; approvals."));
  assert.match(html, /revision-7/);
  assert.match(html, /Latest-revision status has not been checked/);
  assert.match(html, /No Save or sign-off action is connected/);
  assert.doesNotMatch(html, /<form\b|<button\b|type="submit"/);
  assert.deepEqual(session.base.content, base().content);
});

test("changed-field preview marks the right field without blanking untouched data", () => {
  const session = editWorkpaperRevisionSessionField(
    openWorkpaperRevisionEditorSession(base()),
    "objective",
    "Edited objective.",
  );
  const html = render(session, { observedLatestRevisionId: "revision-7" });
  assert.match(html, /Objective · Edited/);
  assert.match(html, /1 canonical text field\(s\) edited/);
  assert.match(html, /Edited objective/);
  assert.match(html, /All vendors/);
  assert.match(html, /client currently sees the same base revision/);
  assert.doesNotMatch(html, /role="alert"/);
});

test("stale observation warns without discarding the existing unsaved draft", () => {
  const session = editWorkpaperRevisionSessionField(
    openWorkpaperRevisionEditorSession(base()),
    "conclusion",
    "Unsaved conclusion — preserve me",
  );
  const html = render(session, { observedLatestRevisionId: "revision-8" });
  assert.match(html, /role="alert"/);
  assert.match(html, /revision-8/);
  assert.match(html, /do not overwrite or/i);
  assert.match(html, /Unsaved conclusion — preserve me/);
  assert.equal(session.draft.expectedLatestRevisionId, "revision-7");
});

test("historical sign-off is described without approval inheritance or role claims", () => {
  const session = openWorkpaperRevisionEditorSession(base());
  const html = render(session, { hasHistoricalSignoff: true });
  assert.match(html, /historical sign-off remains linked to its exact previous revision/i);
  assert.match(html, /does not inherit that sign-off/i);
  assert.doesNotMatch(html, /<button\b|<form\b/);
});

test("missing freshness never silently claims the base is current", () => {
  const session = openWorkpaperRevisionEditorSession(base());
  assert.match(render(session), /Latest-revision status has not been checked/);
  assert.doesNotMatch(render(session), /client currently sees the same base revision/);
  assert.throws(
    () => render(session, { observedLatestRevisionId: "" }),
    /Latest revision observation must match/,
  );
});

test("rendering keeps the original draft and canonical base unchanged", () => {
  const session = editWorkpaperRevisionSessionField(
    openWorkpaperRevisionEditorSession(base()),
    "exceptions",
    "",
  );
  const before = JSON.stringify(session);
  const html = render(session, { hasHistoricalSignoff: true, observedLatestRevisionId: "revision-9" });
  assert.equal(JSON.stringify(session), before);
  assert.match(html, /Sample/);
  assert.match(html, /Twenty vendors/);
  assert.match(html, /Exceptions · Edited/);
  assert.match(html, /role="alert"/);
});

test("full observed revision explains remote-only and local-only fields without rebasing", () => {
  const session = editWorkpaperRevisionSessionField(
    openWorkpaperRevisionEditorSession(base()), "objective", "My unsaved correction",
  );
  const latest = {
    ...base(), workpaperRevisionId: "revision-8",
    content: { ...base().content, population: "Observed updated population" },
  };
  const html = render(session, { observedLatestRevision: latest });
  assert.match(html, /role="alert"/);
  assert.match(html, /Observed revision text comparison/);
  assert.match(html, /Objective: Edited in this draft only/);
  assert.match(html, /Population: Changed in observed revision only/);
  assert.match(html, /My unsaved correction/);
  assert.doesNotMatch(html, /<button\b|<form\b/);
  assert.equal(session.draft.expectedLatestRevisionId, "revision-7");
});

test("full observed revision warns about conflicting same-field changes", () => {
  const session = editWorkpaperRevisionSessionField(
    openWorkpaperRevisionEditorSession(base()), "conclusion", "My conclusion",
  );
  const latest = {
    ...base(), workpaperRevisionId: "revision-8",
    content: { ...base().content, conclusion: "Their conclusion" },
  };
  const html = render(session, { observedLatestRevision: latest });
  assert.match(html, /Conclusion: Different edits in both revisions/);
  assert.match(html, /manual review needed/);
  assert.match(html, /My conclusion/);
  assert.doesNotMatch(html, /Their conclusion/);
  assert.doesNotMatch(html, /<button\b/);
});

test("observed same text on different revision never implies save authority", () => {
  const session = editWorkpaperRevisionSessionField(
    openWorkpaperRevisionEditorSession(base()), "sample", "Chosen sample",
  );
  const latest = {
    ...base(), workpaperRevisionId: "revision-8",
    content: { ...base().content, sample: "Chosen sample" },
  };
  const html = render(session, { observedLatestRevision: latest });
  assert.match(html, /Sample: Same text entered in both revisions/);
  assert.match(html, /cannot rebase/);
  assert.match(html, /No Save or sign-off action is connected/);
});

test("mismatched latest ID props and divergent purported immutable text fail closed", () => {
  const session = openWorkpaperRevisionEditorSession(base());
  const latest = { ...base(), workpaperRevisionId: "revision-8" };
  assert.throws(() => render(session, {
    observedLatestRevisionId: "revision-9",
    observedLatestRevision: latest,
  }), /Conflicting latest revision observations/);
  const divergent = {
    ...base(), content: { ...base().content, objective: "Tampered persisted text" },
  };
  assert.throws(() => render(session, { observedLatestRevision: divergent }),
    /Inconsistent text observed for the same immutable revision/);
});
