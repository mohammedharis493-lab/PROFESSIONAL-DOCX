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
import type {
  WorkpaperRevisionReadSnapshot,
} from "./workpaper-revision-read-snapshot.ts";

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

/**
 * Read-only diagnostics for two independently observed, validated snapshots.
 * Same-revision metadata can change as other writers attach evidence/sign-offs.
 * Across revisions, link IDs and active sign-offs refer to DIFFERENT bases:
 * reported differences are informational and do not prove transfer/custody.
 * None of the outcomes authorize Save, approval, automatic merge or rebase.
 */
export type WorkpaperObservedEvidenceChange = Readonly<{
  evidenceLinkId: string;
  change: "added" | "removed" | "metadata-changed";
}>;
export type WorkpaperObservedSignoffChange = Readonly<{
  signoffId: string;
  change: "added" | "removed";
}>;
export type WorkpaperRevisionReadObservationComparison = Readonly<{
  scope: "client-read-observation-diff-only";
  comparisonScope: "same-revision" | "across-revisions";
  workpaperId: string;
  expectedBaseRevisionId: string;
  observedRevisionId: string;
  differentRevisionObserved: boolean;
  text: WorkpaperRevisionTextObservation;
  evidenceChanges: readonly WorkpaperObservedEvidenceChange[];
  activeSignoffChanges: readonly WorkpaperObservedSignoffChange[];
}>;

function checkReadObservation(
  candidate: WorkpaperRevisionReadSnapshot,
  label: string,
): void {
  if (
    !candidate || candidate.scope !== "client-read-snapshot-only" ||
    !candidate.session?.base ||
    !Array.isArray(candidate.observedSourceEvidence) ||
    !Array.isArray(candidate.observedBaseActiveSignoffIds)
  ) {
    throw new TypeError("Incomplete " + label + " revision read observation");
  }
  const base = candidate.session.base;
  const linkIds = new Set<string>();
  for (const link of candidate.observedSourceEvidence) {
    if (
      !link || typeof link.evidenceLinkId !== "string" ||
      link.evidenceLinkId.trim() === "" || linkIds.has(link.evidenceLinkId) ||
      link.workpaperRevisionId !== base.workpaperRevisionId ||
      typeof link.documentId !== "string" || link.documentId.trim() === "" ||
      typeof link.relationshipType !== "string" || link.relationshipType.trim() === "" ||
      (link.description !== null && typeof link.description !== "string") ||
      ((typeof link.contentVersionId === "string" &&
        link.contentVersionId.trim().length > 0) ===
       (typeof link.controlledEvidenceVersionId === "string" &&
        link.controlledEvidenceVersionId.trim().length > 0)) ||
      (link.contentVersionId !== null && (
        typeof link.contentVersionId !== "string" ||
        link.contentVersionId.trim() === ""
      )) ||
      (link.controlledEvidenceVersionId !== null && (
        typeof link.controlledEvidenceVersionId !== "string" ||
        link.controlledEvidenceVersionId.trim() === ""
      ))
    ) {
      throw new TypeError("Invalid " + label + " evidence observation");
    }
    linkIds.add(link.evidenceLinkId);
  }
  const ids = new Set<string>();
  for (const id of candidate.observedBaseActiveSignoffIds) {
    if (typeof id !== "string" || id.trim() === "" || ids.has(id)) {
      throw new TypeError("Invalid " + label + " active sign-off observation");
    }
    ids.add(id);
  }
}

const EVIDENCE_OBSERVED_FIELDS = [
  "documentId", "contentVersionId", "controlledEvidenceVersionId",
  "relationshipType", "description",
] as const;

/** A pure inspection: neither snapshot nor the caller's unsaved draft changes. */
export function inspectWorkpaperRevisionReadObservationChanges(
  session: WorkpaperRevisionEditorSession,
  opening: WorkpaperRevisionReadSnapshot,
  later: WorkpaperRevisionReadSnapshot,
): WorkpaperRevisionReadObservationComparison {
  checkReadObservation(opening, "opening");
  checkReadObservation(later, "later");
  const original = opening.session.base;
  const current = later.session.base;
  if (
    original.workpaperId !== session.base.workpaperId ||
    original.workpaperRevisionId !== session.base.workpaperRevisionId ||
    current.workpaperId !== original.workpaperId ||
    WORKPAPER_REVISION_CONTENT_FIELDS.some(
      field => original.content[field] !== session.base.content[field]
    )
  ) {
    throw new TypeError("Read observations must match the immutable draft base and workpaper");
  }
  const text = inspectWorkpaperRevisionObservedText(session, current);
  const first = new Map(opening.observedSourceEvidence.map(link => [link.evidenceLinkId, link]));
  const second = new Map(later.observedSourceEvidence.map(link => [link.evidenceLinkId, link]));
  const evidenceChanges: WorkpaperObservedEvidenceChange[] = [];
  for (const [id, link] of first) {
    const now = second.get(id);
    if (!now) {
      evidenceChanges.push(Object.freeze({ evidenceLinkId: id, change: "removed" }));
    } else if (EVIDENCE_OBSERVED_FIELDS.some(field => link[field] !== now[field])) {
      evidenceChanges.push(Object.freeze({ evidenceLinkId: id, change: "metadata-changed" }));
    }
  }
  for (const id of second.keys()) {
    if (!first.has(id)) {
      evidenceChanges.push(Object.freeze({ evidenceLinkId: id, change: "added" }));
    }
  }
  evidenceChanges.sort((a, b) =>
    a.evidenceLinkId.localeCompare(b.evidenceLinkId) || a.change.localeCompare(b.change)
  );
  const oldSignoffs = new Set(opening.observedBaseActiveSignoffIds);
  const nextSignoffs = new Set(later.observedBaseActiveSignoffIds);
  const activeSignoffChanges: WorkpaperObservedSignoffChange[] = [
    ...[...oldSignoffs].filter(id => !nextSignoffs.has(id))
      .map(signoffId => Object.freeze({ signoffId, change: "removed" as const })),
    ...[...nextSignoffs].filter(id => !oldSignoffs.has(id))
      .map(signoffId => Object.freeze({ signoffId, change: "added" as const })),
  ];
  activeSignoffChanges.sort((a, b) =>
    a.signoffId.localeCompare(b.signoffId) || a.change.localeCompare(b.change)
  );
  const differentRevisionObserved = text.differentRevisionObserved;
  return Object.freeze({
    scope: "client-read-observation-diff-only",
    comparisonScope: differentRevisionObserved ? "across-revisions" : "same-revision",
    workpaperId: original.workpaperId,
    expectedBaseRevisionId: original.workpaperRevisionId,
    observedRevisionId: current.workpaperRevisionId,
    differentRevisionObserved,
    text,
    evidenceChanges: Object.freeze(evidenceChanges),
    activeSignoffChanges: Object.freeze(activeSignoffChanges),
  });
}
