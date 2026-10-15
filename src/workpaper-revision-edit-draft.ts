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
