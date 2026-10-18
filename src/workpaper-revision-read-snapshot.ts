/**
 * Detached READ adapter for current Tauri workpaper revision DTOs.
 *
 * This is a local snapshot, not an authenticated read, completeness proof,
 * evidence transfer plan or Save request. In particular, observed evidence
 * link IDs belong to their SOURCE revision and must never be reused as new
 * link IDs. No production UI, Tauri command or persistence call imports this.
 */
import {
  WORKPAPER_REVISION_CONTENT_FIELDS,
  openWorkpaperRevisionEditorSession,
} from "./workpaper-revision-edit-draft.ts";
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
  // Other fields in the current App.tsx/Tauri DTO may be present but are
  // observation-only: they never become new successor evidence links.
  documentName?: string;
  contentObservedAtMs?: number | null;
  controlledVersionNumber?: number | null;
  controlledCapturedAtMs?: number | null;
  createdAtMs?: number;
}>;

export type StoredWorkpaperSignoffForDraft = Readonly<{
  signoffId: string;
  workpaperId: string;
  workpaperRevisionId: string;
  // Current App.tsx/Tauri metadata is read for shape validation only; none
  // grants approval to a new revision or changes the historical row.
  revisionNumber?: number;
  signoffType?: string;
  actorId?: string;
  actorRole?: string;
  signedAtMs?: number;
  comment?: string | null;
  evidenceLinkIds?: readonly string[];
  supersededAtMs?: number | null;
  supersededReason?: string | null;
  supersededByRevisionId?: string | null;
}>;

export type ObservedSourceEvidenceReference = Readonly<StoredWorkpaperEvidenceLinkForDraft>;

export type WorkpaperRevisionReadSnapshot = Readonly<{
  scope: "client-read-snapshot-only";
  session: WorkpaperRevisionEditorSession;
  observedSourceEvidence: readonly ObservedSourceEvidenceReference[];
  /** The precise IDs of ACTIVE sign-offs on the observed latest revision.
   * Pure, untrusted opening observation, not trusted authority or a Save token.
   * IDs can become stale immediately; native must recheck under write lock.
   */
  observedBaseActiveSignoffIds: readonly string[];
  /** Legacy display-only count of ALL sign-offs on the observed latest revision,
   * including those with recorded supersessions. Never use as active CAS.
   */
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

// Do not silently omit a newly introduced persisted canonical field.
// The accepted DTO currently has these seven text fields and seven metadata
// fields. An updated backend shape must be consciously reviewed and mapped.
const KNOWN_REVISION_DTO_FIELDS = new Set<string>([
  ...WORKPAPER_REVISION_CONTENT_FIELDS,
  "workpaperId", "workpaperRevisionId", "revisionNumber", "createdAtMs",
  "revisionReason", "supersedesRevisionId", "contentHashHex",
]);

function nonBlank(value: unknown): value is string {
  return typeof value === "string" && value.trim().length > 0;
}


// Explicitly based on the existing App.tsx WorkpaperEvidenceLink and
// WorkpaperSignoff DTOs. Unknown fields may represent future custody or
// sign-off semantics; silently dropping them is not acceptable for a
// proposed lossless editor. Metadata below is never Save authorization.
const KNOWN_EVIDENCE_DTO_FIELDS = new Set([
  "evidenceLinkId", "workpaperRevisionId", "documentId", "documentName",
  "contentVersionId", "contentObservedAtMs", "controlledEvidenceVersionId",
  "controlledVersionNumber", "controlledCapturedAtMs", "relationshipType",
  "description", "createdAtMs",
]);
const KNOWN_SIGNOFF_DTO_FIELDS = new Set([
  "signoffId", "workpaperId", "workpaperRevisionId", "revisionNumber",
  "signoffType", "actorId", "actorRole", "signedAtMs", "comment",
  "evidenceLinkIds", "supersededAtMs", "supersededReason",
  "supersededByRevisionId",
]);
function checkKnownFields(row: object, allowed: ReadonlySet<string>, label: string): void {
  for (const key of Object.keys(row)) {
    if (!allowed.has(key)) {
      throw new TypeError("Unrecognized stored " + label + " field: " + key);
    }
  }
}
function validOptionalEpoch(value: number | null | undefined): boolean {
  return value == null || (Number.isSafeInteger(value) && value >= 0);
}
function validOptionalPositiveNumber(value: number | null | undefined): boolean {
  return value == null || (Number.isSafeInteger(value) && value >= 1);
}
function checkEvidenceMetadata(link: StoredWorkpaperEvidenceLinkForDraft): void {
  checkKnownFields(link, KNOWN_EVIDENCE_DTO_FIELDS, "evidence");
  if (
    (link.documentName !== undefined && typeof link.documentName !== "string") ||
    !validOptionalEpoch(link.contentObservedAtMs) ||
    !validOptionalEpoch(link.controlledCapturedAtMs) ||
    !validOptionalPositiveNumber(link.controlledVersionNumber) ||
    (link.createdAtMs !== undefined &&
      (!Number.isSafeInteger(link.createdAtMs) || link.createdAtMs < 0))
  ) {
    throw new TypeError("Invalid stored source evidence metadata");
  }
}
function checkSignoffMetadata(signoff: StoredWorkpaperSignoffForDraft): void {
  checkKnownFields(signoff, KNOWN_SIGNOFF_DTO_FIELDS, "sign-off");
  if (
    (signoff.revisionNumber !== undefined &&
      (!Number.isSafeInteger(signoff.revisionNumber) || signoff.revisionNumber < 1)) ||
    (signoff.signedAtMs !== undefined &&
      (!Number.isSafeInteger(signoff.signedAtMs) || signoff.signedAtMs < 0)) ||
    (signoff.signoffType !== undefined && !nonBlank(signoff.signoffType)) ||
    (signoff.actorId !== undefined && !nonBlank(signoff.actorId)) ||
    (signoff.actorRole !== undefined && !nonBlank(signoff.actorRole)) ||
    (signoff.comment !== undefined && signoff.comment !== null &&
      typeof signoff.comment !== "string") ||
    !validOptionalEpoch(signoff.supersededAtMs) ||
    (signoff.supersededReason !== undefined &&
      signoff.supersededReason !== null &&
      typeof signoff.supersededReason !== "string") ||
    (signoff.supersededByRevisionId !== undefined &&
      signoff.supersededByRevisionId !== null &&
      !nonBlank(signoff.supersededByRevisionId))
  ) {
    throw new TypeError("Invalid stored sign-off metadata");
  }
  if (signoff.evidenceLinkIds !== undefined) {
    if (!Array.isArray(signoff.evidenceLinkIds) ||
      signoff.evidenceLinkIds.some(id => !nonBlank(id)) ||
      new Set(signoff.evidenceLinkIds).size !== signoff.evidenceLinkIds.length) {
      throw new TypeError("Invalid stored sign-off evidence-link snapshot");
    }
  }
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
    for (const field of Object.keys(revision)) {
      if (!KNOWN_REVISION_DTO_FIELDS.has(field)) {
        throw new TypeError("Unrecognized stored revision field: " + field);
      }
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
    checkEvidenceMetadata(link);
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
  const observedBaseActiveSignoffIds: string[] = [];
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
    checkSignoffMetadata(signoff);
    // The current native DTO always returns these three nullable fields.
    // An omitted one does NOT mean an active sign-off. For any sign-off
    // recorded on the base, fail closed instead of inventing an active set.
    const onBase = signoff.workpaperRevisionId === latest.workpaperRevisionId;
    if (onBase && (
      !Object.prototype.hasOwnProperty.call(signoff, "supersededAtMs") ||
      !Object.prototype.hasOwnProperty.call(signoff, "supersededReason") ||
      !Object.prototype.hasOwnProperty.call(signoff, "supersededByRevisionId")
    )) {
      throw new TypeError("Incomplete base sign-off supersession metadata");
    }
    const supersededAt = signoff.supersededAtMs != null;
    const supersededReason = signoff.supersededReason != null;
    const supersededBy = signoff.supersededByRevisionId != null;
    if ((supersededAt || supersededReason || supersededBy) &&
        !(supersededAt && supersededReason && supersededBy)) {
      throw new TypeError("Inconsistent sign-off supersession metadata");
    }
    const signedRevision = input.revisions.find(
      revision => revision.workpaperRevisionId === signoff.workpaperRevisionId,
    );
    if (signoff.revisionNumber !== undefined &&
        signoff.revisionNumber !== signedRevision?.revisionNumber) {
      throw new TypeError("Stored sign-off revision-number observation disagrees with its revision");
    }
    // Current Tauri sign-off DTOs include their exact signed link IDs. For a
    // sign-off on the observed base, reject a partial, foreign or altered
    // snapshot instead of presenting it as consistent history. Historical
    // sign-offs may reference older revisions whose evidence was not fetched.
    if (signoff.workpaperRevisionId === latest.workpaperRevisionId &&
        signoff.evidenceLinkIds !== undefined) {
      const snapshotIds = [...signoff.evidenceLinkIds].sort();
      const observedIds = [...linkIds].sort();
      if (snapshotIds.length !== observedIds.length ||
        snapshotIds.some((id, index) => id !== observedIds[index])) {
        throw new TypeError("Observed base sign-off evidence snapshot disagrees with exact source links");
      }
    }
    signoffIds.add(signoff.signoffId);
    if (onBase) {
      observedBaseSignoffCount++;
      if (!supersededAt) {
        observedBaseActiveSignoffIds.push(signoff.signoffId);
      }
    }
  }

  // Match candidate's sorted active ID set, never array ordering from
  // list_workpaper_signoffs. Clone/freeze prevents later caller mutations.
  observedBaseActiveSignoffIds.sort();
  return Object.freeze({
    scope: "client-read-snapshot-only",
    session,
    observedSourceEvidence: Object.freeze(evidence),
    observedBaseActiveSignoffIds: Object.freeze(observedBaseActiveSignoffIds),
    observedBaseSignoffCount,
  });
}
