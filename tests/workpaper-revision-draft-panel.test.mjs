import { after, before, test } from "node:test";
import assert from "node:assert/strict";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { createServer } from "vite";
import {
  WORKPAPER_REVISION_CONTENT_FIELDS,
  editWorkpaperRevisionSessionField,
  restoreWorkpaperRevisionSessionField,
  openWorkpaperRevisionEditorSession,
} from "../src/workpaper-revision-edit-draft.ts";
import { openStoredWorkpaperRevisionReadSnapshot } from "../src/workpaper-revision-read-snapshot.ts";

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
  assert.match(html, /aria-label="Restore original Objective text"/);
  assert.match(html, /type="button"/);
  assert.doesNotMatch(html, /<form\b|type="submit"|>Save</);
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
  assert.match(html, /aria-label="Restore original Conclusion text"/);
  assert.doesNotMatch(html, /<form\b|type="submit"|>Save</);
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


test("unchanged seven-field draft contains no restore buttons", () => {
  const html = render(openWorkpaperRevisionEditorSession(base()));
  assert.equal((html.match(/<textarea\b/g) ?? []).length, 7);
  assert.equal((html.match(/aria-label="Restore original /g) ?? []).length, 0);
  assert.doesNotMatch(html, /<button\b|<form\b|type="submit"/);
});

test("one local correction renders exactly one non-submitting restore control", () => {
  const session = editWorkpaperRevisionSessionField(
    openWorkpaperRevisionEditorSession(base()), "conclusion", "Corrected wording",
  );
  const html = render(session);
  assert.equal((html.match(/<textarea\b/g) ?? []).length, 7);
  assert.equal((html.match(/<button\b/g) ?? []).length, 1);
  assert.match(html, /aria-label="Restore original Conclusion text"/);
  assert.match(html, /type="button"/);
  assert.match(html, /Restore original text/);
  assert.doesNotMatch(html, /type="submit"|<form\b|>Save</);
});

test("two edited fields render two independent restore actions without losing other text", () => {
  const first = editWorkpaperRevisionSessionField(
    openWorkpaperRevisionEditorSession(base()), "sample", "Changed count",
  );
  const session = editWorkpaperRevisionSessionField(first, "objective", "Local correction");
  const html = render(session);
  assert.equal((html.match(/<button\b/g) ?? []).length, 2);
  assert.match(html, /aria-label="Restore original Sample text"/);
  assert.match(html, /aria-label="Restore original Objective text"/);
  assert.match(html, /Changed count/);
  assert.match(html, /All vendors/);
  assert.equal(session.draft.content.objective, "Local correction");
});

test("restoring one field removes only that field restore control in a detached render", () => {
  const start = openWorkpaperRevisionEditorSession(base());
  let session = editWorkpaperRevisionSessionField(start, "sample", "Changed count");
  session = editWorkpaperRevisionSessionField(session, "objective", "Local correction");
  const restored = restoreWorkpaperRevisionSessionField(session, "sample");
  const html = render(restored);
  assert.equal((html.match(/<button\b/g) ?? []).length, 1);
  assert.match(html, /aria-label="Restore original Objective text"/);
  assert.doesNotMatch(html, /aria-label="Restore original Sample text"/);
  assert.equal(restored.draft.content.sample, "Twenty vendors.");
  assert.equal(restored.draft.content.objective, "Local correction");
});

test("stale observed revision leaves restore bound to original base, never observed text", () => {
  const changed = editWorkpaperRevisionSessionField(
    openWorkpaperRevisionEditorSession(base()), "conclusion", "My unsaved correction",
  );
  const current = {
    ...base(),
    workpaperRevisionId: "revision-8",
    content: { ...base().content, conclusion: "Other writer's conclusion" },
  };
  const html = render(changed, { observedLatestRevision: current });
  assert.match(html, /role="alert"/);
  assert.match(html, /Restore original Conclusion text/);
  assert.match(html, /original opened revision rather than any newer revision/);
  const restored = restoreWorkpaperRevisionSessionField(changed, "conclusion");
  assert.equal(restored.draft.content.conclusion, base().content.conclusion);
  assert.notEqual(restored.draft.content.conclusion, current.content.conclusion);
  assert.equal(restored.draft.expectedLatestRevisionId, "revision-7");
});

test("signed draft exposes local restore but no review unlock, Save or sign-off button", () => {
  const session = editWorkpaperRevisionSessionField(
    openWorkpaperRevisionEditorSession(base()), "objective", "Minor correction",
  );
  const html = render(session, { hasHistoricalSignoff: true });
  assert.match(html, /does not inherit that sign-off/);
  assert.match(html, /aria-label="Restore original Objective text"/);
  assert.doesNotMatch(html, /<form\b|type="submit"|>Save</);
});

function observedRead(revisionId = "revision-7", {
  content = base().content,
  evidence = [{
    evidenceLinkId: "link-a", workpaperRevisionId: revisionId,
    documentId: "doc-a", contentVersionId: "content-v1",
    controlledEvidenceVersionId: null, relationshipType: "SUPPORTING",
    description: null,
  }],
  signoffs = [{
    signoffId: "sign-a", workpaperId: "synthetic-workpaper",
    workpaperRevisionId: revisionId, supersededAtMs: null,
    supersededReason: null, supersededByRevisionId: null,
  }],
} = {}) {
  return openStoredWorkpaperRevisionReadSnapshot({
    workpaperId: "synthetic-workpaper",
    revisions: [{
      workpaperId: "synthetic-workpaper", workpaperRevisionId: revisionId,
      revisionNumber: revisionId === "revision-7" ? 7 : 8,
      ...content,
    }],
    evidenceLinks: evidence,
    signoffs,
  });
}

test("detached preview reports exact source metadata and late active signoff without Save", () => {
  const opening = observedRead();
  const later = observedRead("revision-7", {
    evidence: [{
      ...opening.observedSourceEvidence[0],
      relationshipType: "CORROBORATING",
    }],
    signoffs: [
      { signoffId: "sign-a", workpaperId: "synthetic-workpaper",
        workpaperRevisionId: "revision-7", supersededAtMs: null,
        supersededReason: null, supersededByRevisionId: null },
      { signoffId: "sign-later", workpaperId: "synthetic-workpaper",
        workpaperRevisionId: "revision-7", supersededAtMs: null,
        supersededReason: null, supersededByRevisionId: null },
    ],
  });
  const session = editWorkpaperRevisionSessionField(
    opening.session, "conclusion", "Keep unsaved wording",
  );
  const html = render(session, {
    openingReadSnapshot: opening, observedReadSnapshot: later,
  });
  assert.match(html, /Observed evidence and sign-off comparison/);
  assert.match(html, /same revision is observed/);
  assert.match(html, /1 evidence-link observation change\(s\)/);
  assert.match(html, /1 active sign-off ID change\(s\)/);
  assert.match(html, /Source link link-a/);
  assert.match(html, /relationship or version metadata differs/);
  assert.match(html, /Sign-off sign-later/);
  assert.match(html, /Keep unsaved wording/);
  assert.doesNotMatch(html, /<form\b|type="submit"|>Save</);
  assert.equal(session.draft.expectedLatestRevisionId, "revision-7");
  assert.equal(opening.observedSourceEvidence[0].relationshipType, "SUPPORTING");
});

test("successor read preview warns of different revision without inheriting sign-offs", () => {
  const opening = observedRead();
  const later = observedRead("revision-8", {
    content: { ...base().content, population: "Remotely revised population" },
    evidence: [{
      evidenceLinkId: "link-b", workpaperRevisionId: "revision-8",
      documentId: "doc-a", contentVersionId: "content-v1",
      controlledEvidenceVersionId: null, relationshipType: "SUPPORTING",
      description: null,
    }],
    signoffs: [],
  });
  const session = editWorkpaperRevisionSessionField(
    opening.session, "conclusion", "Preserve local unsaved correction",
  );
  const html = render(session, {
    openingReadSnapshot: opening, observedReadSnapshot: later,
  });
  assert.match(html, /role="alert"/);
  assert.match(html, /different revision is observed/i);
  assert.match(html, /not inherited approvals/i);
  assert.match(html, /link-b/);
  assert.match(html, /sign-a/);
  assert.match(html, /Preserve local unsaved correction/);
  assert.match(html, /revision-8/);
  assert.doesNotMatch(html, /<form\b|type="submit"|>Save</);
  assert.equal(session.draft.expectedLatestRevisionId, "revision-7");
});

test("missing or mutually contradictory observation pairs fail closed", () => {
  const opening = observedRead();
  // These are distinct immutable revisions: use fresh link IDs so that
  // observation-prop contradictions, not ID reuse, are under test here.
  const later = observedRead("revision-8", {
    evidence: [{
      ...opening.observedSourceEvidence[0], evidenceLinkId: "link-new",
      workpaperRevisionId: "revision-8",
    }],
    signoffs: [],
  });
  assert.throws(() => render(opening.session, {
    openingReadSnapshot: opening,
  }), /needs both complete read observations/);
  assert.throws(() => render(opening.session, {
    observedReadSnapshot: later,
  }), /needs both complete read observations/);
  assert.throws(() => render(opening.session, {
    openingReadSnapshot: opening, observedReadSnapshot: later,
    observedLatestRevisionId: "revision-9",
  }), /Conflicting latest revision observations/);
  assert.throws(() => render(opening.session, {
    openingReadSnapshot: opening, observedReadSnapshot: later,
    observedLatestRevision: {
      ...base(), workpaperRevisionId: "revision-8",
      content: { ...base().content, conclusion: "Contradictory cached text" },
    },
  }), /Conflicting canonical text observations/);
  const other = openWorkpaperRevisionEditorSession({
    ...base(), workpaperRevisionId: "foreign-base",
  });
  assert.throws(() => render(other, {
    openingReadSnapshot: opening, observedReadSnapshot: later,
  }), /immutable draft base/);
});

test("detached panel rejects reused cross-revision evidence or active sign-off identities", () => {
  const opening = observedRead();
  const session = editWorkpaperRevisionSessionField(
    opening.session, "conclusion", "Preserve my unsaved local wording",
  );
  const reusedEvidence = observedRead("revision-8", {
    evidence: [{
      ...opening.observedSourceEvidence[0], workpaperRevisionId: "revision-8",
    }],
    signoffs: [],
  });
  assert.throws(() => render(session, {
    openingReadSnapshot: opening, observedReadSnapshot: reusedEvidence,
  }), /Reused evidence-link ID across different immutable revisions/);

  const reusedSignoff = observedRead("revision-8", {
    evidence: [{
      ...opening.observedSourceEvidence[0], evidenceLinkId: "new-link",
      workpaperRevisionId: "revision-8",
    }],
  });
  assert.throws(() => render(session, {
    openingReadSnapshot: opening, observedReadSnapshot: reusedSignoff,
  }), /Reused active sign-off ID across different immutable revisions/);
  assert.equal(session.draft.content.conclusion, "Preserve my unsaved local wording");
  assert.equal(session.draft.expectedLatestRevisionId, "revision-7");
});

test("detached panel rejects contradictory signed metadata while preserving unsaved correction", () => {
  const signed = {
    signoffId: "sign-a", workpaperId: "synthetic-workpaper",
    workpaperRevisionId: "revision-7", revisionNumber: 7,
    signoffType: "PREPARED", actorId: "caller-supplied",
    actorRole: "Unverified preparer", signedAtMs: 123,
    comment: "Original statement", evidenceLinkIds: ["link-a"],
    supersededAtMs: null, supersededReason: null,
    supersededByRevisionId: null,
  };
  const opening = observedRead("revision-7", { signoffs: [signed] });
  const later = observedRead("revision-7", {
    signoffs: [{ ...signed, actorRole: "Altered role" }],
  });
  const session = editWorkpaperRevisionSessionField(
    opening.session, "conclusion", "Unsaved professional correction — preserve",
  );
  const before = JSON.stringify(session);
  assert.throws(() => render(session, {
    openingReadSnapshot: opening, observedReadSnapshot: later,
  }), /Conflicting immutable sign-off metadata or signed-evidence snapshot/);
  assert.equal(JSON.stringify(session), before);
  assert.equal(session.draft.expectedLatestRevisionId, "revision-7");
});
