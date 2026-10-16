import { test } from "node:test";
import assert from "node:assert/strict";
import {
  openStoredWorkpaperRevisionReadSnapshot,
} from "../src/workpaper-revision-read-snapshot.ts";
import {
  editWorkpaperRevisionSessionField,
  previewWorkpaperRevisionEditorSession,
  projectWorkpaperRevisionDraftContent,
} from "../src/workpaper-revision-edit-draft.ts";

function revision(overrides = {}) {
  return {
    workpaperId: "wp-1",
    workpaperRevisionId: "rev-7",
    revisionNumber: 7,
    objective: "Old objective  ",
    procedurePerformed: "Procedure \r\n",
    population: "Population Ⅰ",
    sample: "20 samples",
    exceptions: "Exception A",
    managementExplanation: "Manager's explanation",
    conclusion: "Signed conclusion",
    ...overrides,
  };
}
function contentLink(overrides = {}) {
  return {
    evidenceLinkId: "link-1",
    workpaperRevisionId: "rev-7",
    documentId: "doc-1",
    contentVersionId: "file-ver-3",
    controlledEvidenceVersionId: null,
    relationshipType: "SUPPORTS",
    description: "Source note \r\n",
    ...overrides,
  };
}
function input(overrides = {}) {
  return {
    workpaperId: "wp-1",
    revisions: [revision()],
    evidenceLinks: [contentLink()],
    signoffs: [{ signoffId: "sign-1", workpaperId: "wp-1", workpaperRevisionId: "rev-7" }],
    ...overrides,
  };
}

test("opens all seven existing fields exactly rather than starting a three-field blank form", () => {
  const read = openStoredWorkpaperRevisionReadSnapshot(input());
  const session = read.session;
  assert.equal(read.scope, "client-read-snapshot-only");
  assert.equal(session.base.workpaperRevisionId, "rev-7");
  assert.equal(session.draft.expectedLatestRevisionId, "rev-7");
  assert.deepEqual(session.draft.content, {
    objective: "Old objective  ",
    procedurePerformed: "Procedure \r\n",
    population: "Population Ⅰ",
    sample: "20 samples",
    exceptions: "Exception A",
    managementExplanation: "Manager's explanation",
    conclusion: "Signed conclusion",
  });
  assert.equal(read.observedBaseSignoffCount, 1);
  assert.equal("signoffs" in session, false);
});

test("a signed historical base can be edited without inheriting approval", () => {
  const read = openStoredWorkpaperRevisionReadSnapshot(input());
  const next = editWorkpaperRevisionSessionField(read.session, "conclusion", "Typo fix");
  const preview = previewWorkpaperRevisionEditorSession(next);
  assert.deepEqual(preview.changedFields, ["conclusion"]);
  assert.equal(preview.proposedWrite.content.objective, "Old objective  ");
  assert.equal(preview.proposedWrite.content.population, "Population Ⅰ");
  assert.equal(preview.proposedWrite.expectedLatestRevisionId, "rev-7");
  assert.deepEqual(Object.keys(preview.proposedWrite).sort(), [
    "content", "expectedLatestRevisionId", "workpaperId",
  ]);
  assert.equal("observedSourceEvidence" in preview.proposedWrite, false);
});

test("read snapshot retains exact source evidence version and relationship metadata", () => {
  const read = openStoredWorkpaperRevisionReadSnapshot(input({
    evidenceLinks: [
      contentLink(),
      contentLink({
        evidenceLinkId: "link-2",
        documentId: "doc-2",
        contentVersionId: null,
        controlledEvidenceVersionId: "controlled-ver-11",
        relationshipType: "REVIEWED_WITH",
        description: null,
      }),
    ],
  }));
  assert.deepEqual(read.observedSourceEvidence, [
    contentLink(),
    contentLink({
      evidenceLinkId: "link-2",
      documentId: "doc-2",
      contentVersionId: null,
      controlledEvidenceVersionId: "controlled-ver-11",
      relationshipType: "REVIEWED_WITH",
      description: null,
    }),
  ]);
  assert.ok(Object.isFrozen(read));
  assert.ok(Object.isFrozen(read.observedSourceEvidence));
  assert.ok(Object.isFrozen(read.observedSourceEvidence[0]));
  assert.equal("evidenceLinkIds" in read.session.draft, false);
});

test("caller mutations after opening cannot rewrite detached text or evidence observations", () => {
  const source = input();
  const read = openStoredWorkpaperRevisionReadSnapshot(source);
  source.revisions[0].population = "mutated";
  source.evidenceLinks[0].contentVersionId = "different";
  source.signoffs[0].signoffId = "other";
  assert.equal(read.session.base.content.population, "Population Ⅰ");
  assert.equal(read.observedSourceEvidence[0].contentVersionId, "file-ver-3");
  assert.equal(read.observedBaseSignoffCount, 1);
});

test("selects maximal revision number even when backend read results are reordered", () => {
  const read = openStoredWorkpaperRevisionReadSnapshot(input({
    revisions: [revision(), revision({
      workpaperRevisionId: "rev-3", revisionNumber: 3, objective: "OLD",
    })].reverse(),
    signoffs: [
      { signoffId: "old-sign", workpaperId: "wp-1", workpaperRevisionId: "rev-3" },
    ],
  }));
  assert.equal(read.session.base.workpaperRevisionId, "rev-7");
  assert.equal(read.observedBaseSignoffCount, 0);
  assert.equal(read.session.draft.content.objective, "Old objective  ");
});

test("missing base revision fails rather than fabricating empty working paper", () => {
  assert.throws(() => openStoredWorkpaperRevisionReadSnapshot(input({ revisions: [] })),
    /no observed base revision/);
});

test("all three read result arrays must be present, even when observations are empty", () => {
  for (const key of ["revisions", "evidenceLinks", "signoffs"]) {
    const source = input();
    delete source[key];
    assert.throws(() => openStoredWorkpaperRevisionReadSnapshot(source), /three read results/);
  }
  const read = openStoredWorkpaperRevisionReadSnapshot(input({
    evidenceLinks: [], signoffs: [],
  }));
  assert.equal(read.observedSourceEvidence.length, 0);
  assert.equal(read.observedBaseSignoffCount, 0);
});

test("foreign workpaper revisions fail closed", () => {
  assert.throws(() => openStoredWorkpaperRevisionReadSnapshot(input({
    revisions: [revision({ workpaperId: "wp-2" })],
  })), /Invalid or foreign/);
});

test("duplicate revision numbers and duplicate IDs are ambiguous", () => {
  assert.throws(() => openStoredWorkpaperRevisionReadSnapshot(input({
    revisions: [revision(), revision({ workpaperRevisionId: "rev-else" })],
  })), /duplicate/);
  assert.throws(() => openStoredWorkpaperRevisionReadSnapshot(input({
    revisions: [revision(), revision({ revisionNumber: 8 })],
  })), /duplicate/);
});

test("rejects invalid revision numbering and malformed canonical stored content", () => {
  for (const number of [0, -1, 1.5, NaN, Infinity, Number.MAX_SAFE_INTEGER + 1]) {
    assert.throws(() => openStoredWorkpaperRevisionReadSnapshot(input({
      revisions: [revision({ revisionNumber: number })],
    })), /Invalid or foreign/);
  }
  assert.throws(() => openStoredWorkpaperRevisionReadSnapshot(input({
    revisions: [revision({ managementExplanation: undefined })],
  })), /managementExplanation/);
});

test("unknown future persisted content or metadata cannot silently disappear", () => {
  assert.throws(() => openStoredWorkpaperRevisionReadSnapshot(input({
    revisions: [revision({ newCanonicalField: "Keep this text!" })],
  })), /Unrecognized stored revision field: newCanonicalField/);
  const currentDto = revision({
    createdAtMs: 17, revisionReason: null, supersedesRevisionId: "rev-6",
    contentHashHex: "a1",
  });
  assert.equal(openStoredWorkpaperRevisionReadSnapshot(input({
    revisions: [currentDto],
  })).session.base.workpaperRevisionId, "rev-7");
});

test("a source evidence record cannot belong to another revision", () => {
  assert.throws(() => openStoredWorkpaperRevisionReadSnapshot(input({
    evidenceLinks: [contentLink({ workpaperRevisionId: "rev-3" })],
  })), /wrong-revision/);
});

test("evidence records must reference exactly one unchanged explicit version ID", () => {
  for (const [contentVersionId, controlledEvidenceVersionId] of [
    [null, null], ["file", "controlled"], [" ", null], [null, ""],
    [undefined, null],
  ]) {
    assert.throws(() => openStoredWorkpaperRevisionReadSnapshot(input({
      evidenceLinks: [contentLink({ contentVersionId, controlledEvidenceVersionId })],
    })), /exact version/);
  }
});

test("duplicate source link IDs or missing identities are rejected", () => {
  assert.throws(() => openStoredWorkpaperRevisionReadSnapshot(input({
    evidenceLinks: [contentLink(), contentLink()],
  })), /duplicate/);
  assert.throws(() => openStoredWorkpaperRevisionReadSnapshot(input({
    evidenceLinks: [contentLink({ documentId: " " })],
  })), /source evidence link/);
  assert.throws(() => openStoredWorkpaperRevisionReadSnapshot(input({
    evidenceLinks: [contentLink({ description: undefined })],
  })), /source evidence link/);
});

test("invalid, duplicate or cross-workpaper sign-off observations are rejected", () => {
  assert.throws(() => openStoredWorkpaperRevisionReadSnapshot(input({
    signoffs: [
      { signoffId: "sign-1", workpaperId: "wp-1", workpaperRevisionId: "rev-7" },
      { signoffId: "sign-1", workpaperId: "wp-1", workpaperRevisionId: "rev-7" },
    ],
  })), /sign-off observation/);
  assert.throws(() => openStoredWorkpaperRevisionReadSnapshot(input({
    signoffs: [{ signoffId: "sign-1", workpaperId: "wp-2", workpaperRevisionId: "rev-7" }],
  })), /sign-off observation/);
  assert.throws(() => openStoredWorkpaperRevisionReadSnapshot(input({
    signoffs: [{ signoffId: "sign-1", workpaperId: "wp-1", workpaperRevisionId: "missing" }],
  })), /sign-off observation/);
});

test("snapshot observations never produce a persistence command or approval claim", () => {
  const read = openStoredWorkpaperRevisionReadSnapshot(input());
  assert.deepEqual(Object.keys(read).sort(), [
    "observedBaseSignoffCount", "observedSourceEvidence", "scope", "session",
  ]);
  assert.equal("save" in read, false);
  assert.equal("grant" in read, false);
  assert.equal("approval" in read, false);
  assert.equal(previewWorkpaperRevisionEditorSession(read.session).kind, "content-unchanged");
});

test("persisted DTO opens a signed unchanged draft that still projects all seven fields", () => {
  const observation = openStoredWorkpaperRevisionReadSnapshot(input());
  assert.equal(previewWorkpaperRevisionEditorSession(observation.session).kind, "content-unchanged");
  const projected = projectWorkpaperRevisionDraftContent(observation.session);
  assert.equal(projected.expectedLatestRevisionId, "rev-7");
  assert.equal(projected.content.objective, "Old objective  ");
  assert.equal(projected.content.population, "Population Ⅰ");
  assert.equal(projected.content.conclusion, "Signed conclusion");
  assert.equal("observedSourceEvidence" in projected, false);
  assert.equal("observedBaseSignoffCount" in projected, false);
});

test("persisted DTO projection keeps signed history and exact evidence only as read observations", () => {
  const observation = openStoredWorkpaperRevisionReadSnapshot(input());
  const updated = editWorkpaperRevisionSessionField(
    observation.session, "conclusion", "Minor fix",
  );
  const projected = projectWorkpaperRevisionDraftContent(updated);
  assert.equal(projected.content.conclusion, "Minor fix");
  assert.equal(projected.content.managementExplanation, "Manager's explanation");
  assert.equal(projected.expectedLatestRevisionId, "rev-7");
  assert.equal(observation.observedSourceEvidence[0].contentVersionId, "file-ver-3");
  assert.equal(observation.observedBaseSignoffCount, 1);
  assert.equal("evidenceLinkIds" in projected, false);
  assert.equal("signoffs" in projected, false);
});
