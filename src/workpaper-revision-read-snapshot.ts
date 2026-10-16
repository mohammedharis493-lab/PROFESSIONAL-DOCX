/**
 * Detached READ adapter for current Tauri workpaper revision DTOs.
 *
 * This is a local snapshot, not an authenticated read, completeness proof,
 * evidence transfer plan or Save request. In particular, observed evidence
 * link IDs belong to their SOURCE revision and must never be reused as new
 * link IDs. No production UI, Tauri command or persistence call imports this.
 */
import { openWorkpaperRevisionEditorSession } from "./workpaper-revision-edit-draft.ts";
import type {
  WorkpaperRevisionContent,
  WorkpaperRevisionEditorSession,
} from "./workpaper-revision-edit-draft.ts";

export type StoredWorkpaperRevisionForDraft = Readonly<{
  workpaperId: string;
  workpaperRevisionId: string;
  revisionNumber: number;
} & WorkpaperRevisionContent>;

export type StoredWorkpaperEvidenceLinkForDraft = Readonly<{
  evidenceLinkId: string;
  workpaperRevisionId: string;
  documentId: string;
  contentVersionId: string | null;
  controlledEvidenceVersionId: string | null;
  relationshipType: string;
  description: string | null;
}>;

export type StoredWorkpaperSignoffForDraft = Readonly<{
  signoffId: string;
  workpaperId: string;
  workpaperRevisionId: string;
}>;

export type ObservedSourceEvidenceReference = Readonly<StoredWorkpaperEvidenceLinkForDraft>;

export type WorkpaperRevisionReadSnapshot = Readonly<{
  scope: "client-read-snapshot-only";
  session: WorkpaperRevisionEditorSession;
  observedSourceEvidence: readonly ObservedSourceEvidenceReference[];
  observedBaseSignoffCount: number;
}>;

export type WorkpaperRevisionReadInput = Readonly<{
  workpaperId: string;
  revisions: readonly StoredWorkpaperRevisionForDraft[];
  /** Result of list_workpaper_evidence_links for the selected latest revision. */
  evidenceLinks: readonly StoredWorkpaperEvidenceLinkForDraft[];
  /** Result of list_workpaper_signoffs for this workpaper, including history. */
  signoffs: readonly StoredWorkpaperSignoffForDraft[];
}>;

function nonBlank(value: unknown): value is string {
  return typeof value === "string" && value.trim().length > 0;
}

/**
 * Open a detached edit session based on the highest observed revision number,
 * rather than trusting array position. This NEVER proves the revision is still
 * latest in durable storage. The backend must check that under its final write
 * transaction, alongside scoped authorization and exact evidence custody.
 *
 * All three read arrays are required: an unfetched array is not the same thing
 * as an observed empty result. Even supplied empty arrays are observations,
 * not an authoritative proof of evidence completeness or current sign-off.
 */
export function openStoredWorkpaperRevisionReadSnapshot(
  input: WorkpaperRevisionReadInput,
): WorkpaperRevisionReadSnapshot {
  if (
    !input ||
    !nonBlank(input.workpaperId) ||
    !Array.isArray(input.revisions) ||
    !Array.isArray(input.evidenceLinks) ||
    !Array.isArray(input.signoffs)
  ) {
    throw new TypeError("Workpaper identity and all three read results are required");
  }
  if (input.revisions.length === 0) {
    throw new TypeError("Cannot edit a workpaper with no observed base revision");
  }

  const revisionIds = new Set<string>();
  const revisionNumbers = new Set<number>();
  let latest: StoredWorkpaperRevisionForDraft | undefined;
  for (const revision of input.revisions) {
    if (
      !revision ||
      revision.workpaperId !== input.workpaperId ||
      !nonBlank(revision.workpaperRevisionId) ||
      !Number.isSafeInteger(revision.revisionNumber) ||
      revision.revisionNumber < 1
    ) {
      throw new TypeError("Invalid or foreign workpaper revision in read result");
    }
    if (
      revisionIds.has(revision.workpaperRevisionId) ||
      revisionNumbers.has(revision.revisionNumber)
    ) {
      throw new TypeError("Ambiguous duplicate workpaper revision in read result");
    }
    revisionIds.add(revision.workpaperRevisionId);
    revisionNumbers.add(revision.revisionNumber);
    if (!latest || revision.revisionNumber > latest.revisionNumber) {
      latest = revision;
    }
  }
  // The nonempty-array validation guarantees a latest revision.
  if (!latest) throw new TypeError("Missing latest workpaper revision");

  // Copy exactly the seven known canonical text fields; the core editor
  // validates their presence and string types, without trimming their bytes.
  const session = openWorkpaperRevisionEditorSession({
    workpaperId: input.workpaperId,
    workpaperRevisionId: latest.workpaperRevisionId,
    content: {
      objective: latest.objective,
      procedurePerformed: latest.procedurePerformed,
      population: latest.population,
      sample: latest.sample,
      exceptions: latest.exceptions,
      managementExplanation: latest.managementExplanation,
      conclusion: latest.conclusion,
    },
  });

  const linkIds = new Set<string>();
  const evidence = input.evidenceLinks.map((link) => {
    if (
      !link ||
      !nonBlank(link.evidenceLinkId) ||
      linkIds.has(link.evidenceLinkId) ||
      link.workpaperRevisionId !== latest.workpaperRevisionId ||
      !nonBlank(link.documentId) ||
      !nonBlank(link.relationshipType) ||
      (link.description !== null && typeof link.description !== "string")
    ) {
      throw new TypeError("Invalid, duplicate or wrong-revision source evidence link");
    }
    const hasContentVersion = nonBlank(link.contentVersionId);
    const hasControlledVersion = nonBlank(link.controlledEvidenceVersionId);
    if (
      hasContentVersion === hasControlledVersion ||
      (link.contentVersionId !== null && !hasContentVersion) ||
      (link.controlledEvidenceVersionId !== null && !hasControlledVersion)
    ) {
      throw new TypeError("Source evidence must identify exactly one exact version");
    }
    linkIds.add(link.evidenceLinkId);
    return Object.freeze({
      evidenceLinkId: link.evidenceLinkId,
      workpaperRevisionId: link.workpaperRevisionId,
      documentId: link.documentId,
      contentVersionId: link.contentVersionId,
      controlledEvidenceVersionId: link.controlledEvidenceVersionId,
      relationshipType: link.relationshipType,
      description: link.description,
    });
  });

  const signoffIds = new Set<string>();
  let observedBaseSignoffCount = 0;
  for (const signoff of input.signoffs) {
    if (
      !signoff ||
      !nonBlank(signoff.signoffId) ||
      signoffIds.has(signoff.signoffId) ||
      signoff.workpaperId !== input.workpaperId ||
      !revisionIds.has(signoff.workpaperRevisionId)
    ) {
      throw new TypeError("Invalid, duplicate or foreign workpaper sign-off observation");
    }
    signoffIds.add(signoff.signoffId);
    if (signoff.workpaperRevisionId === latest.workpaperRevisionId) {
      observedBaseSignoffCount++;
    }
  }

  return Object.freeze({
    scope: "client-read-snapshot-only",
    session,
    observedSourceEvidence: Object.freeze(evidence),
    observedBaseSignoffCount,
  });
}
