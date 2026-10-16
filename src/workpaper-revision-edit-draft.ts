/**
 * Draft-only pure UI helper for lossless workpaper revision editing.
 *
 * NOT wired to App.tsx/Tauri. NO authentication, grants, persistence, evidence
 * custody or professional approval occurs here. A future trusted backend MUST
 * revalidate identity, grants, expected latest revision, and evidence versions
 * inside its final write transaction.
 */
export const WORKPAPER_REVISION_CONTENT_FIELDS = [
  "objective",
  "procedurePerformed",
  "population",
  "sample",
  "exceptions",
  "managementExplanation",
  "conclusion",
] as const;

export type WorkpaperRevisionContentField =
  (typeof WORKPAPER_REVISION_CONTENT_FIELDS)[number];

export type WorkpaperRevisionContent = Record<WorkpaperRevisionContentField, string>;

export type WorkpaperRevisionEditBase = Readonly<{
  workpaperId: string;
  workpaperRevisionId: string;
  content: Readonly<WorkpaperRevisionContent>;
}>;

export type WorkpaperRevisionEditDraft = {
  workpaperId: string;
  expectedLatestRevisionId: string;
  content: WorkpaperRevisionContent;
};

/**
 * Content-only comparison. It CANNOT detect changes to linked evidence,
 * review notes, workflow or approval state. It must never authorize a no-op
 * persistence decision, revision creation, or sign-off transfer by itself.
 */
export type WorkpaperRevisionContentPreview =
  | Readonly<{
      scope: "canonical-content-only";
      kind: "content-unchanged";
      changedFields: readonly [];
    }>
  | Readonly<{
      scope: "canonical-content-only";
      kind: "content-changed";
      changedFields: readonly WorkpaperRevisionContentField[];
      proposedWrite: Readonly<{
        workpaperId: string;
        expectedLatestRevisionId: string;
        content: WorkpaperRevisionContent;
      }>;
    }>;

function copyCanonicalContent(
  content: Readonly<WorkpaperRevisionContent>,
): WorkpaperRevisionContent {
  if (content === null || typeof content !== "object" || Array.isArray(content)) {
    throw new TypeError("Canonical workpaper content must be a field object");
  }
  const copy = {} as WorkpaperRevisionContent;
  for (const field of WORKPAPER_REVISION_CONTENT_FIELDS) {
    if (!Object.prototype.hasOwnProperty.call(content, field)) {
      throw new TypeError(`Missing canonical content field: ${field}`);
    }
    const value = content[field];
    if (typeof value !== "string") {
      throw new TypeError(`Invalid canonical content field: ${field}`);
    }
    copy[field] = value;
  }
  // Fail closed on newly introduced canonical fields until the lossless
  // editor is intentionally updated, instead of silently dropping their data.
  for (const field of Object.keys(content)) {
    if (!(WORKPAPER_REVISION_CONTENT_FIELDS as readonly string[]).includes(field)) {
      throw new TypeError(`Unexpected canonical content field: ${field}`);
    }
  }
  return copy;
}

/** Prefill every canonical field, not only the visible form fields. */
export function openWorkpaperRevisionEditDraft(
  base: WorkpaperRevisionEditBase,
): WorkpaperRevisionEditDraft {
  if (!base.workpaperId || !base.workpaperRevisionId) {
    throw new TypeError("A specific workpaper and immutable base revision are required");
  }
  return {
    workpaperId: base.workpaperId,
    expectedLatestRevisionId: base.workpaperRevisionId,
    content: copyCanonicalContent(base.content),
  };
}

/**
 * Pure canonical-content comparison, NOT a complete Save/no-op decision.
 * Evidence links and review state are intentionally outside this function.
 * This client-side base identity check is UX validation, NOT concurrency
 * control: only the trusted backend can recheck
 * expectedLatestRevisionId against the database under a write reservation.
 * Actor/role, evidence IDs and authorization claims intentionally do not enter.
 */
export function previewWorkpaperRevisionContentChange(
  base: WorkpaperRevisionEditBase,
  draft: WorkpaperRevisionEditDraft,
): WorkpaperRevisionContentPreview {
  if (
    !base.workpaperId ||
    !base.workpaperRevisionId ||
    draft.workpaperId !== base.workpaperId ||
    draft.expectedLatestRevisionId !== base.workpaperRevisionId
  ) {
    throw new TypeError("The draft does not match its immutable workpaper/revision base");
  }

  const canonicalBase = copyCanonicalContent(base.content);
  const canonicalNext = copyCanonicalContent(draft.content);
  const changedFields = WORKPAPER_REVISION_CONTENT_FIELDS.filter(
    (field) => canonicalBase[field] !== canonicalNext[field],
  );

  if (changedFields.length === 0) {
    return { scope: "canonical-content-only", kind: "content-unchanged", changedFields: [] };
  }
  return {
    scope: "canonical-content-only",
    kind: "content-changed",
    changedFields,
    proposedWrite: {
      workpaperId: base.workpaperId,
      expectedLatestRevisionId: base.workpaperRevisionId,
      content: canonicalNext,
    },
  };
}


/**
 * Detached, immutable snapshot for a future seven-field editor.
 *
 * This is only UI draft state, never a permission, Save authorization, or
 * persistence API. All snapshots are separate from live query results;
 * a signed historical revision can be opened for drafting without implying
 * the ability to write, carry evidence, or inherit any approval.
 */
export type WorkpaperRevisionEditorSession = Readonly<{
  base: WorkpaperRevisionEditBase;
  draft: Readonly<{
    workpaperId: string;
    expectedLatestRevisionId: string;
    content: Readonly<WorkpaperRevisionContent>;
  }>;
}>;

export type WorkpaperRevisionEditorFreshness = Readonly<{
  scope: "client-preview-only";
  state: "base-current" | "base-stale";
  expectedLatestRevisionId: string;
  observedLatestRevisionId: string;
}>;

function snapshotWorkpaperRevisionEditorSession(
  base: WorkpaperRevisionEditBase,
  draft: WorkpaperRevisionEditDraft,
): WorkpaperRevisionEditorSession {
  // Both calls validate every canonical field and the exact two identities.
  const baseCopy = openWorkpaperRevisionEditDraft(base);
  const comparison = previewWorkpaperRevisionContentChange(base, draft);
  const editedContent =
    comparison.kind === "content-changed" ? comparison.proposedWrite.content : baseCopy.content;

  return Object.freeze({
    base: Object.freeze({
      workpaperId: baseCopy.workpaperId,
      workpaperRevisionId: baseCopy.expectedLatestRevisionId,
      content: Object.freeze({ ...baseCopy.content }),
    }),
    draft: Object.freeze({
      workpaperId: baseCopy.workpaperId,
      expectedLatestRevisionId: baseCopy.expectedLatestRevisionId,
      content: Object.freeze({ ...editedContent }),
    }),
  });
}

/** Open existing text verbatim, including all four less-visible fields. */
export function openWorkpaperRevisionEditorSession(
  base: WorkpaperRevisionEditBase,
): WorkpaperRevisionEditorSession {
  return snapshotWorkpaperRevisionEditorSession(base, openWorkpaperRevisionEditDraft(base));
}

/**
 * A pure per-field edit: no in-place mutation, normalization or trimming.
 * UI code can use the returned session for controlled inputs, including
 * deliberate blank strings. The trusted backend must still authorize Save.
 */
export function editWorkpaperRevisionSessionField(
  session: WorkpaperRevisionEditorSession,
  field: WorkpaperRevisionContentField,
  value: string,
): WorkpaperRevisionEditorSession {
  if (!(WORKPAPER_REVISION_CONTENT_FIELDS as readonly string[]).includes(field)) {
    throw new TypeError("Unrecognized workpaper revision content field");
  }
  if (typeof value !== "string") {
    throw new TypeError("Edited workpaper content must be text");
  }
  // Validate the incoming session before editing it; this rejects malformed
  // or future-schema drafts rather than accidentally dropping other fields.
  previewWorkpaperRevisionEditorSession(session);
  return snapshotWorkpaperRevisionEditorSession(session.base, {
    workpaperId: session.draft.workpaperId,
    expectedLatestRevisionId: session.draft.expectedLatestRevisionId,
    content: { ...session.draft.content, [field]: value },
  });
}

/**
 * Restore ONE edited field from this session's immutable original revision.
 * This is a local draft action only. It never reopens, rebases, signs, saves or
 * changes linked evidence, and it leaves all other pending edits untouched.
 * In particular a stale editor must NOT adopt any newer revision's text.
 */
export function restoreWorkpaperRevisionSessionField(
  session: WorkpaperRevisionEditorSession,
  field: WorkpaperRevisionContentField,
): WorkpaperRevisionEditorSession {
  if (!(WORKPAPER_REVISION_CONTENT_FIELDS as readonly string[]).includes(field)) {
    throw new TypeError("Unrecognized workpaper revision content field");
  }
  // Validates the complete base and current draft, including unexpected
  // future fields and the unchanged exact workpaper/revision identity.
  previewWorkpaperRevisionEditorSession(session);
  return editWorkpaperRevisionSessionField(session, field, session.base.content[field]);
}

/**
 * Untrusted, detached projection of the seven editable fields and frozen base
 * identity. This is available for BOTH unchanged and changed text; a
 * content-only comparison MUST NEVER suppress an intended revision Save.
 *
 * This is NOT a Save request, Tauri payload, permission proof, evidence-set
 * hash or approval. Only a future reviewed backend transaction can decide
 * whether to accept a requested successor revision, by authenticating the
 * human actor and checking scope, expected latest revision, exact evidence
 * and historical sign-off integrity under the same write reservation.
 */
export type WorkpaperRevisionDraftProjection = Readonly<{
  scope: "untrusted-draft-content-only";
  workpaperId: string;
  expectedLatestRevisionId: string;
  content: Readonly<WorkpaperRevisionContent>;
}>;

export function projectWorkpaperRevisionDraftContent(
  session: WorkpaperRevisionEditorSession,
): WorkpaperRevisionDraftProjection {
  // Revalidate both identities and all canonical content, including future
  // schema additions, rather than trusting a cached preview or mutable caller.
  previewWorkpaperRevisionEditorSession(session);
  const workpaperId = session.base.workpaperId;
  const expectedLatestRevisionId = session.base.workpaperRevisionId;
  if (
    typeof workpaperId !== "string" || workpaperId.trim().length === 0 ||
    typeof expectedLatestRevisionId !== "string" ||
    expectedLatestRevisionId.trim().length === 0
  ) {
    throw new TypeError("The immutable workpaper and revision identities must be nonblank");
  }
  return Object.freeze({
    scope: "untrusted-draft-content-only",
    workpaperId,
    expectedLatestRevisionId,
    content: Object.freeze(copyCanonicalContent(session.draft.content)),
  });
}

/** Compare text only; unchanged content does NOT mean no Save is necessary. */
export function previewWorkpaperRevisionEditorSession(
  session: WorkpaperRevisionEditorSession,
): WorkpaperRevisionContentPreview {
  return previewWorkpaperRevisionContentChange(session.base, {
    workpaperId: session.draft.workpaperId,
    expectedLatestRevisionId: session.draft.expectedLatestRevisionId,
    content: { ...session.draft.content },
  });
}

/**
 * Visual warning for concurrent revisions without silently discarding edits.
 * A backend transactional latest-revision check is mandatory even when this
 * preview says "base-current". No client-reported state can authorize a Save.
 */
export function inspectWorkpaperRevisionEditorFreshness(
  session: WorkpaperRevisionEditorSession,
  observedLatest: Readonly<{ workpaperId: string; workpaperRevisionId: string }>,
): WorkpaperRevisionEditorFreshness {
  previewWorkpaperRevisionEditorSession(session);
  if (
    !observedLatest ||
    typeof observedLatest.workpaperId !== "string" ||
    typeof observedLatest.workpaperRevisionId !== "string" ||
    !observedLatest.workpaperRevisionId ||
    observedLatest.workpaperId !== session.base.workpaperId
  ) {
    throw new TypeError("Latest revision observation must match the edited workpaper");
  }
  return {
    scope: "client-preview-only",
    state:
      observedLatest.workpaperRevisionId === session.base.workpaperRevisionId
        ? "base-current"
        : "base-stale",
    expectedLatestRevisionId: session.base.workpaperRevisionId,
    observedLatestRevisionId: observedLatest.workpaperRevisionId,
  };
}
