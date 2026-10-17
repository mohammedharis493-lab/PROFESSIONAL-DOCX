/**
 * Unmounted client-only three-way TEXT observation. No Save, automatic merge,
 * rebasing, evidence authority, signature transfer or permission decision.
 *
 * Compares a frozen local draft to its exact original content and one
 * separately observed revision. Even "no text conflict" cannot establish
 * evidence-set equality, freshness, review completeness or safe overwrite.
 */
import {
  WORKPAPER_REVISION_CONTENT_FIELDS,
  projectWorkpaperRevisionDraftContent,
} from "./workpaper-revision-edit-draft.ts";
import type {
  WorkpaperRevisionContentField,
  WorkpaperRevisionEditBase,
  WorkpaperRevisionEditorSession,
} from "./workpaper-revision-edit-draft.ts";

export type WorkpaperRevisionTextComparisonStatus =
  | "unchanged"
  | "local-only"
  | "observed-only"
  | "converged-text"
  | "text-conflict";

export type WorkpaperRevisionTextObservation = Readonly<{
  scope: "client-three-way-text-only";
  workpaperId: string;
  expectedBaseRevisionId: string;
  observedRevisionId: string;
  differentRevisionObserved: boolean;
  fields: readonly Readonly<{
    field: WorkpaperRevisionContentField;
    status: WorkpaperRevisionTextComparisonStatus;
  }>[];
}>;

export function inspectWorkpaperRevisionObservedText(
  session: WorkpaperRevisionEditorSession,
  observedRevision: WorkpaperRevisionEditBase,
): WorkpaperRevisionTextObservation {
  const projected = projectWorkpaperRevisionDraftContent(session);
  if (
    !observedRevision ||
    typeof observedRevision !== "object" ||
    typeof observedRevision.workpaperId !== "string" ||
    observedRevision.workpaperId !== projected.workpaperId ||
    typeof observedRevision.workpaperRevisionId !== "string" ||
    observedRevision.workpaperRevisionId.trim().length === 0 ||
    !observedRevision.content ||
    typeof observedRevision.content !== "object" ||
    Array.isArray(observedRevision.content)
  ) {
    throw new TypeError("Observed revision must have the same workpaper and a complete revision identity");
  }
  const observedContent = observedRevision.content;
  for (const field of WORKPAPER_REVISION_CONTENT_FIELDS) {
    if (
      !Object.prototype.hasOwnProperty.call(observedContent, field) ||
      typeof observedContent[field] !== "string"
    ) {
      throw new TypeError("Incomplete observed canonical text field: " + field);
    }
  }
  for (const field of Object.keys(observedContent)) {
    if (!(WORKPAPER_REVISION_CONTENT_FIELDS as readonly string[]).includes(field)) {
      throw new TypeError("Unexpected observed canonical text field: " + field);
    }
  }
  const differentRevisionObserved =
    observedRevision.workpaperRevisionId !== projected.expectedLatestRevisionId;

  // The same purported immutable revision must not have two different texts.
  // A cache/source divergence is an error, not a safe rebase opportunity.
  if (!differentRevisionObserved &&
      WORKPAPER_REVISION_CONTENT_FIELDS.some(
        field => session.base.content[field] !== observedContent[field]
      )) {
    throw new TypeError("Inconsistent text observed for the same immutable revision");
  }

  const fields = WORKPAPER_REVISION_CONTENT_FIELDS.map((field) => {
    const base = session.base.content[field];
    const local = projected.content[field];
    const observed = observedContent[field];
    const localChanged = local !== base;
    const observedChanged = observed !== base;
    let status: WorkpaperRevisionTextComparisonStatus;
    if (!localChanged && !observedChanged) status = "unchanged";
    else if (localChanged && !observedChanged) status = "local-only";
    else if (!localChanged && observedChanged) status = "observed-only";
    else if (local === observed) status = "converged-text";
    else status = "text-conflict";
    return Object.freeze({ field, status });
  });

  return Object.freeze({
    scope: "client-three-way-text-only",
    workpaperId: projected.workpaperId,
    expectedBaseRevisionId: projected.expectedLatestRevisionId,
    observedRevisionId: observedRevision.workpaperRevisionId,
    differentRevisionObserved,
    fields: Object.freeze(fields),
  });
}
