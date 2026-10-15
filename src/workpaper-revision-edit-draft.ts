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

/** Pure UI proposal, NOT an authorized or persisted mutation request. */
export type WorkpaperRevisionSavePreview =
  | Readonly<{ kind: "no-change"; changedFields: readonly [] }>
  | Readonly<{
      kind: "changed";
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
  const copy = {} as WorkpaperRevisionContent;
  for (const field of WORKPAPER_REVISION_CONTENT_FIELDS) {
    const value = content[field];
    if (typeof value !== "string") {
      throw new TypeError(`Missing or invalid canonical content field: ${field}`);
    }
    copy[field] = value;
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
 * Pure content-only Save preview. This client-side base identity check is UX
 * validation, NOT concurrency control: only the trusted backend can recheck
 * expectedLatestRevisionId against the database under a write reservation.
 * Actor/role, evidence IDs and authorization claims intentionally do not enter.
 */
export function previewWorkpaperRevisionSave(
  base: WorkpaperRevisionEditBase,
  draft: WorkpaperRevisionEditDraft,
): WorkpaperRevisionSavePreview {
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
    return { kind: "no-change", changedFields: [] };
  }
  return {
    kind: "changed",
    changedFields,
    proposedWrite: {
      workpaperId: base.workpaperId,
      expectedLatestRevisionId: base.workpaperRevisionId,
      content: canonicalNext,
    },
  };
}
