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
function observedSignoff(overrides = {}) {
  return {
    signoffId: "sign-1",
    workpaperId: "wp-1",
    workpaperRevisionId: "rev-7",
    supersededAtMs: null,
    supersededReason: null,
    supersededByRevisionId: null,
    ...overrides,
  };
}
function input(overrides = {}) {
  return {
    workpaperId: "wp-1",
    revisions: [revision()],
    evidenceLinks: [contentLink()],
    signoffs: [observedSignoff()],
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
  assert.deepEqual(read.observedBaseActiveSignoffIds, ["sign-1"]);
  assert.ok(Object.isFrozen(read.observedBaseActiveSignoffIds));
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
  assert.deepEqual(read.observedBaseActiveSignoffIds, ["sign-1"]);
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
  assert.deepEqual(read.observedBaseActiveSignoffIds, []);
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
  assert.deepEqual(read.observedBaseActiveSignoffIds, []);
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
      observedSignoff(),
      observedSignoff(),
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
    "observedBaseActiveSignoffIds", "observedBaseSignoffCount",
    "observedSourceEvidence", "scope", "session",
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
  assert.equal("observedBaseActiveSignoffIds" in projected, false);
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
  assert.deepEqual(observation.observedBaseActiveSignoffIds, ["sign-1"]);
  assert.equal("observedBaseActiveSignoffIds" in projected, false);
  assert.equal("evidenceLinkIds" in projected, false);
  assert.equal("signoffs" in projected, false);
});


function completeEvidenceLink(overrides = {}) {
  return contentLink({
    documentName: "Source document",
    contentObservedAtMs: 13,
    controlledVersionNumber: null,
    controlledCapturedAtMs: null,
    createdAtMs: 15,
    ...overrides,
  });
}
function completeSignoff(overrides = {}) {
  return {
    signoffId: "sign-1", workpaperId: "wp-1", workpaperRevisionId: "rev-7",
    revisionNumber: 7, signoffType: "PREPARED", actorId: "fixture",
    actorRole: "PREPARER", signedAtMs: 17, comment: null,
    evidenceLinkIds: ["link-1"], supersededAtMs: null,
    supersededReason: null, supersededByRevisionId: null,
    ...overrides,
  };
}

test("complete current App.tsx evidence and signoff DTOs are observed without authorizing Save", () => {
  const read = openStoredWorkpaperRevisionReadSnapshot(input({
    evidenceLinks: [completeEvidenceLink()],
    signoffs: [completeSignoff()],
  }));
  assert.equal(read.observedBaseSignoffCount, 1);
  assert.deepEqual(read.observedBaseActiveSignoffIds, ["sign-1"]);
  assert.deepEqual(read.observedSourceEvidence, [contentLink()]);
  assert.equal("documentName" in read.observedSourceEvidence[0], false);
  assert.equal("evidenceLinkIds" in read.session, false);
  assert.equal("actorId" in read.session, false);
});

test("future evidence DTO field fails closed instead of silently dropping custody metadata", () => {
  assert.throws(() => openStoredWorkpaperRevisionReadSnapshot(input({
    evidenceLinks: [completeEvidenceLink({
      custodyReclassification: "requires conscious schema update",
    })],
  })), /Unrecognized stored evidence field: custodyReclassification/);
});

test("future signoff DTO field fails closed instead of silently dropping approval metadata", () => {
  assert.throws(() => openStoredWorkpaperRevisionReadSnapshot(input({
    signoffs: [completeSignoff({ futureApprovalScope: "new semantics" })],
  })), /Unrecognized stored sign-off field: futureApprovalScope/);
});

test("optional evidence DTO metadata rejects malformed timestamps and version numbers", () => {
  for (const overrides of [
    { documentName: 5 },
    { contentObservedAtMs: "now" },
    { contentObservedAtMs: -1 },
    { controlledVersionNumber: -2 },
    { controlledVersionNumber: 1.5 },
    { controlledCapturedAtMs: Infinity },
    { createdAtMs: null },
  ]) {
    assert.throws(() => openStoredWorkpaperRevisionReadSnapshot(input({
      evidenceLinks: [completeEvidenceLink(overrides)],
    })), /Invalid stored source evidence metadata/);
  }
});

test("optional signoff DTO metadata rejects invalid actor, timestamps and types", () => {
  for (const overrides of [
    { revisionNumber: 0 }, { revisionNumber: 3.2 },
    { signoffType: "" }, { actorId: " " }, { actorRole: null },
    { signedAtMs: -1 }, { signedAtMs: "today" },
    { comment: 5 }, { supersededAtMs: Infinity },
    { supersededReason: 3 }, { supersededByRevisionId: " " },
  ]) {
    assert.throws(() => openStoredWorkpaperRevisionReadSnapshot(input({
      signoffs: [completeSignoff(overrides)],
    })), /Invalid stored sign-off metadata/);
  }
});

test("signoff DTO revision number must match its immutable referenced revision", () => {
  assert.throws(() => openStoredWorkpaperRevisionReadSnapshot(input({
    signoffs: [completeSignoff({ revisionNumber: 8 })],
  })), /revision-number observation disagrees/);
});

test("base signoff missing a signed link is not presented as an exact source snapshot", () => {
  assert.throws(() => openStoredWorkpaperRevisionReadSnapshot(input({
    evidenceLinks: [contentLink(), contentLink({
      evidenceLinkId: "link-2", contentVersionId: "content-two",
    })],
    signoffs: [completeSignoff({ evidenceLinkIds: ["link-1"] })],
  })), /Observed base sign-off evidence snapshot disagrees/);
});

test("base signoff with foreign or extra link ID is rejected", () => {
  for (const evidenceLinkIds of [
    ["foreign-link"],
    ["link-1", "foreign-link"],
  ]) {
    assert.throws(() => openStoredWorkpaperRevisionReadSnapshot(input({
      signoffs: [completeSignoff({ evidenceLinkIds })],
    })), /Observed base sign-off evidence snapshot disagrees/);
  }
});

test("signoff evidence-link IDs must be a unique nonblank array when present", () => {
  for (const evidenceLinkIds of [
    ["link-1", "link-1"], [" "], [5], "link-1", null,
  ]) {
    assert.throws(() => openStoredWorkpaperRevisionReadSnapshot(input({
      signoffs: [completeSignoff({ evidenceLinkIds })],
    })), /Invalid stored sign-off evidence-link snapshot/);
  }
});

test("exact signoff links compare as sets, not by incidental DTO ordering", () => {
  const read = openStoredWorkpaperRevisionReadSnapshot(input({
    evidenceLinks: [contentLink(), contentLink({
      evidenceLinkId: "link-2", contentVersionId: "content-two",
    })],
    signoffs: [completeSignoff({ evidenceLinkIds: ["link-2", "link-1"] })],
  }));
  assert.equal(read.observedSourceEvidence.length, 2);
  assert.equal(read.observedBaseSignoffCount, 1);
});

test("signoffs on older observed revisions do not need latest revision's evidence IDs", () => {
  const read = openStoredWorkpaperRevisionReadSnapshot(input({
    revisions: [revision({
      revisionNumber: 7, workpaperRevisionId: "rev-7",
    }), revision({
      revisionNumber: 6, workpaperRevisionId: "rev-6",
    })],
    signoffs: [completeSignoff({
      signoffId: "sign-old", workpaperRevisionId: "rev-6",
      revisionNumber: 6, evidenceLinkIds: ["historical-link"],
      supersededAtMs: 19, supersededByRevisionId: "rev-7",
      supersededReason: "revision replaced",
    })],
  }));
  assert.equal(read.observedBaseSignoffCount, 0);
  assert.equal(read.session.base.workpaperRevisionId, "rev-7");
});

test("metadata validation does not mutate complete source DTOs or change editable content", () => {
  const source = input({
    evidenceLinks: [completeEvidenceLink()],
    signoffs: [completeSignoff()],
  });
  const unchanged = JSON.stringify(source);
  const read = openStoredWorkpaperRevisionReadSnapshot(source);
  assert.equal(JSON.stringify(source), unchanged);
  assert.equal(read.session.draft.content.objective, "Old objective  ");
  assert.equal(read.session.draft.expectedLatestRevisionId, "rev-7");
});


test("active base sign-off IDs form a sorted frozen snapshot, independent of DTO order", () => {
  const source = input({
    signoffs: [
      observedSignoff({ signoffId: "sign-z" }),
      observedSignoff({ signoffId: "sign-a" }),
      observedSignoff({ signoffId: "sign-k" }),
    ],
  });
  const read = openStoredWorkpaperRevisionReadSnapshot(source);
  assert.deepEqual(read.observedBaseActiveSignoffIds, ["sign-a", "sign-k", "sign-z"]);
  assert.ok(Object.isFrozen(read.observedBaseActiveSignoffIds));
  source.signoffs[0].signoffId = "changed-after-opening";
  source.signoffs.reverse();
  assert.deepEqual(read.observedBaseActiveSignoffIds, ["sign-a", "sign-k", "sign-z"]);
  assert.throws(() => read.observedBaseActiveSignoffIds.push("forged"));
  assert.equal(read.observedBaseSignoffCount, 3);
  assert.equal("observedBaseActiveSignoffIds" in read.session.draft, false);
});

test("superseded base sign-offs remain counted but are excluded from active ID CAS observation", () => {
  const read = openStoredWorkpaperRevisionReadSnapshot(input({
    signoffs: [
      observedSignoff({ signoffId: "active-z" }),
      observedSignoff({
        signoffId: "historical",
        supersededAtMs: 27,
        supersededReason: "New workpaper revision created",
        supersededByRevisionId: "rev-8",
      }),
      observedSignoff({ signoffId: "active-a" }),
    ],
  }));
  assert.equal(read.observedBaseSignoffCount, 3);
  assert.deepEqual(read.observedBaseActiveSignoffIds, ["active-a", "active-z"]);
  assert.equal("actorRole" in read, false);
});

test("missing and undefined native supersession fields fail closed for base sign-offs", () => {
  for (const key of ["supersededAtMs", "supersededReason", "supersededByRevisionId"]) {
    const missing = observedSignoff();
    delete missing[key];
    assert.throws(
      () => openStoredWorkpaperRevisionReadSnapshot(input({ signoffs: [missing] })),
      /Incomplete base sign-off supersession metadata/,
    );
    assert.throws(
      () => openStoredWorkpaperRevisionReadSnapshot(input({
        signoffs: [observedSignoff({ [key]: undefined })],
      })),
      /Incomplete base sign-off supersession metadata/,
    );
  }
});

test("partial or contradictory native supersession metadata cannot masquerade as active", () => {
  for (const overrides of [
    { supersededAtMs: 17 },
    { supersededReason: "superseded" },
    { supersededByRevisionId: "rev-8" },
    { supersededAtMs: 17, supersededReason: "superseded" },
    { supersededAtMs: 17, supersededByRevisionId: "rev-8" },
    { supersededReason: "superseded", supersededByRevisionId: "rev-8" },
  ]) {
    assert.throws(
      () => openStoredWorkpaperRevisionReadSnapshot(input({
        signoffs: [observedSignoff(overrides)],
      })),
      /Inconsistent sign-off supersession metadata/,
    );
  }
});

test("older revision sign-offs are not classified as active base sign-offs", () => {
  const read = openStoredWorkpaperRevisionReadSnapshot(input({
    revisions: [revision(), revision({ workpaperRevisionId: "rev-6", revisionNumber: 6 })],
    signoffs: [
      observedSignoff({
        signoffId: "historical-signed",
        workpaperRevisionId: "rev-6",
        supersededAtMs: 21,
        supersededReason: "New workpaper revision created",
        supersededByRevisionId: "rev-7",
      }),
      observedSignoff({ signoffId: "current" }),
    ],
  }));
  assert.deepEqual(read.observedBaseActiveSignoffIds, ["current"]);
  assert.equal(read.observedBaseSignoffCount, 1);
});
