/**
 * Volatile, user-triggered optional comparison workflow (DRAFT, unmounted).
 *
 * A selected reference is not a book match or professional determination.
 * A visit key represents a fresh UI opening, NOT source/version provenance.
 * Nothing here opens a file, invokes Tauri, persists state or changes evidence.
 */
import { inspectOptionalIndexedDocumentMatch } from "./document-match-advisory-adapter.ts";
import type { IndexedDocumentIdentity, ReadOnlyExtractionFieldSnapshot } from "./document-match-advisory-adapter.ts";
import type {
  AdvisoryComparisonRule,
  AdvisoryFields,
  LocalAdvisoryDisposition,
  OptionalDocumentMatchReport,
  OptionalReference,
} from "./document-match-advisory.ts";

export type ExplicitAdvisorySelection = Readonly<{
  scope: "volatile-optional-reference-selection";
  visitKey: string;
  documentId: string;
  fileInstanceId: string;
  extractionSnapshotId: string;
  referenceId: string;
  /** UI-only binding to optional claimed version metadata, never trusted provenance. */
  sourceContentVersionId?: string;
  sourceSha256Hex?: string;
}>;

export type VolatileAdvisoryReview = Readonly<{
  scope: "volatile-optional-comparison-review";
  visitKey: string;
  documentId: string;
  fileInstanceId: string;
  extractionSnapshotId: string;
  referenceId: string;
  // Only a local UI invalidation key. Not a cryptographic digest or evidence attestation.
  inputKey: string;
  observation: OptionalDocumentMatchReport;
}>;

export type OptionalMatchAttempt = Readonly<
  | { status: "ready"; review: VolatileAdvisoryReview; reason: null; blocksDocumentUse: false }
  | {
      status: "unavailable";
      review: null;
      reason: "no-explicit-selection" | "stale-selection" | "ambiguous-reference" |
        "reference-unavailable" | "extraction-unavailable" | "invalid-comparison-input";
      blocksDocumentUse: false;
    }
>;

function requiredId(value: unknown): value is string {
  return typeof value === "string" && value.trim().length > 0;
}

function unavailable(reason: Extract<OptionalMatchAttempt, {status: "unavailable"}>["reason"]): OptionalMatchAttempt {
  return Object.freeze({ status: "unavailable" as const, review: null, reason, blocksDocumentUse: false as const });
}

function versionClaims(snapshot: ReadOnlyExtractionFieldSnapshot): Readonly<{
  sourceContentVersionId?: string;
  sourceSha256Hex?: string;
}> {
  const value = snapshot as ReadOnlyExtractionFieldSnapshot & {
    sourceContentVersionId?: unknown;
    sourceSha256Hex?: unknown;
  };
  return {
    ...(typeof value.sourceContentVersionId === "string"
      ? {sourceContentVersionId: value.sourceContentVersionId} : {}),
    ...(typeof value.sourceSha256Hex === "string"
      ? {sourceSha256Hex: value.sourceSha256Hex} : {}),
  };
}

function fieldsKey(fields: AdvisoryFields): readonly (readonly [string, string, string])[] {
  return Object.entries(fields).sort(([a], [b]) => a < b ? -1 : a > b ? 1 : 0)
    .map(([key, value]) => [key, value === null ? "null" : typeof value, String(value)] as const);
}

/**
 * Exact, ordinary serialization for local review invalidation. Deliberately
 * makes NO assertion about underlying bytes, provenance, authorization or
 * matching confidence. All inputs must remain read-only while displayed.
 */
function comparisonInputKey(
  file: IndexedDocumentIdentity,
  snapshot: ReadOnlyExtractionFieldSnapshot,
  reference: OptionalReference,
  rules: readonly AdvisoryComparisonRule[],
): string {
  return JSON.stringify([
    file.documentId, file.fileInstanceId,
    snapshot.documentId, snapshot.fileInstanceId, snapshot.extractionSnapshotId,
    versionClaims(snapshot).sourceContentVersionId ?? null,
    versionClaims(snapshot).sourceSha256Hex ?? null,
    fieldsKey(snapshot.fields),
    reference.referenceId, fieldsKey(reference.fields),
    rules.map(rule => [
      rule.ruleId, rule.label, rule.documentField, rule.referenceField, rule.comparison,
    ]),
  ]);
}

export function chooseExplicitAdvisoryReference(
  visitKey: string,
  file: IndexedDocumentIdentity,
  snapshot: ReadOnlyExtractionFieldSnapshot | null,
  referenceId: string,
): ExplicitAdvisorySelection | null {
  if (!requiredId(visitKey) || !requiredId(referenceId) ||
      !file || !requiredId(file.documentId) || !requiredId(file.fileInstanceId) ||
      !snapshot || !requiredId(snapshot.extractionSnapshotId) ||
      snapshot.documentId !== file.documentId ||
      snapshot.fileInstanceId !== file.fileInstanceId) return null;

  return Object.freeze({
    scope: "volatile-optional-reference-selection" as const,
    visitKey, documentId: file.documentId, fileInstanceId: file.fileInstanceId,
    extractionSnapshotId: snapshot.extractionSnapshotId, referenceId,
    ...versionClaims(snapshot),
  });
}

export function selectionIsCurrent(
  selection: ExplicitAdvisorySelection | null,
  visitKey: string,
  file: IndexedDocumentIdentity,
  snapshot: ReadOnlyExtractionFieldSnapshot | null,
): boolean {
  return !!selection && selection.scope === "volatile-optional-reference-selection" &&
    requiredId(visitKey) && selection.visitKey === visitKey &&
    !!file && selection.documentId === file.documentId &&
    selection.fileInstanceId === file.fileInstanceId &&
    !!snapshot && selection.extractionSnapshotId === snapshot.extractionSnapshotId &&
    selection.sourceContentVersionId === versionClaims(snapshot).sourceContentVersionId &&
    selection.sourceSha256Hex === versionClaims(snapshot).sourceSha256Hex &&
    snapshot.documentId === file.documentId &&
    snapshot.fileInstanceId === file.fileInstanceId;
}

/**
 * This is the ONLY event entry point allowed to run an advisory comparison.
 * Caller invokes it synchronously from an explicit Compare button click.
 * References are never chosen by matching identifiers/amounts/names.
 */
export function requestExplicitOptionalMatch(
  visitKey: string,
  file: IndexedDocumentIdentity,
  snapshot: ReadOnlyExtractionFieldSnapshot | null,
  references: readonly OptionalReference[],
  rules: readonly AdvisoryComparisonRule[],
  selection: ExplicitAdvisorySelection | null,
): OptionalMatchAttempt {
  if (!selection) return unavailable("no-explicit-selection");
  if (!selectionIsCurrent(selection, visitKey, file, snapshot)) {
    return unavailable("stale-selection");
  }
  if (!Array.isArray(references)) return unavailable("reference-unavailable");
  const matching = references.filter(reference => reference &&
    reference.referenceId === selection.referenceId);
  if (matching.length > 1) return unavailable("ambiguous-reference");
  if (matching.length !== 1) return unavailable("reference-unavailable");
  try {
    const reference = matching[0];
    const prepared = inspectOptionalIndexedDocumentMatch(file, snapshot, reference, rules, true);
    if (prepared.status !== "ready") return unavailable("extraction-unavailable");

    // Exactly the selected reference and snapshot appear in this observation.
    const review: VolatileAdvisoryReview = Object.freeze({
      scope: "volatile-optional-comparison-review" as const,
      visitKey,
      documentId: file.documentId,
      fileInstanceId: file.fileInstanceId,
      extractionSnapshotId: snapshot!.extractionSnapshotId,
      referenceId: reference.referenceId,
      inputKey: comparisonInputKey(file, snapshot!, reference, rules),
      observation: prepared.observation,
    });
    return Object.freeze({ status: "ready" as const, reason: null, review, blocksDocumentUse: false as const });
  } catch {
    // Invalid detached comparison input must never become a viewer error.
    return unavailable("invalid-comparison-input");
  }
}

/**
 * Revalidate every displayed claim, not just the cached input key. A future
 * controlled React host can accidentally (or deliberately) pass a substituted
 * observation while preserving the version, document and reference IDs.
 * This deterministic check is run ONLY for an existing user-requested review.
 * It never reads a file, invokes native commands or creates a comparison
 * automatically when no review is present.
 */
function sameObservation(
  observed: OptionalDocumentMatchReport,
  expected: OptionalDocumentMatchReport,
): boolean {
  if (observed.scope !== expected.scope ||
      observed.documentId !== expected.documentId ||
      observed.extractionSnapshotId !== expected.extractionSnapshotId ||
      observed.referenceId !== expected.referenceId ||
      observed.outcome !== expected.outcome ||
      observed.blocksDocumentUse !== expected.blocksDocumentUse ||
      observed.requiresDocumentChange !== expected.requiresDocumentChange ||
      observed.requiresReupload !== expected.requiresReupload ||
      !Array.isArray(observed.findings) ||
      observed.findings.length !== expected.findings.length) return false;

  return expected.findings.every((match, i) => {
    const actual = observed.findings[i];
    return !!actual &&
      actual.ruleId === match.ruleId &&
      actual.label === match.label &&
      actual.status === match.status &&
      actual.documentField === match.documentField &&
      actual.referenceField === match.referenceField &&
      Object.is(actual.documentValue, match.documentValue) &&
      Object.is(actual.referenceValue, match.referenceValue) &&
      actual.reason === match.reason;
  });
}

export function reviewIsCurrent(
  review: VolatileAdvisoryReview | null,
  visitKey: string,
  file: IndexedDocumentIdentity,
  snapshot: ReadOnlyExtractionFieldSnapshot | null,
  references: readonly OptionalReference[],
  rules: readonly AdvisoryComparisonRule[],
  selection: ExplicitAdvisorySelection | null,
): boolean {
  if (!review || review.scope !== "volatile-optional-comparison-review" ||
      !selectionIsCurrent(selection, visitKey, file, snapshot) ||
      review.visitKey !== visitKey ||
      review.documentId !== file.documentId || review.fileInstanceId !== file.fileInstanceId ||
      review.extractionSnapshotId !== snapshot!.extractionSnapshotId ||
      review.referenceId !== selection!.referenceId ||
      review.observation?.scope !== "detached-optional-matching-observation-only" ||
      review.observation?.documentId !== review.documentId ||
      review.observation?.extractionSnapshotId !== review.extractionSnapshotId ||
      review.observation?.referenceId !== review.referenceId ||
      !Array.isArray(review.observation?.findings) ||
      !["consistent", "incomplete", "suggestions", "no-rules"].includes(review.observation?.outcome) ||
      review.observation?.blocksDocumentUse !== false ||
      review.observation?.requiresDocumentChange !== false ||
      review.observation?.requiresReupload !== false ||
      !Array.isArray(references) || !Array.isArray(rules)) return false;
  const matching = references.filter(reference => reference &&
    reference.referenceId === selection!.referenceId);
  if (matching.length !== 1) return false;
  try {
    if (review.inputKey !== comparisonInputKey(file, snapshot!, matching[0], rules)) {
      return false;
    }
    // No user request means no review and no recomputation. This call only
    // verifies a comparison that was already initiated by explicit Compare.
    const fresh = inspectOptionalIndexedDocumentMatch(file, snapshot!, matching[0], rules, true);
    return fresh.status === "ready" && sameObservation(review.observation, fresh.observation);
  } catch {
    return false;
  }
}

/** Local-only annotation, optionally held by a React host during this visit. */
export function keepLocalAdvisoryDisposition(
  review: VolatileAdvisoryReview,
  annotation: LocalAdvisoryDisposition,
): LocalAdvisoryDisposition {
  if (!review || !annotation || review.scope !== "volatile-optional-comparison-review" ||
      annotation.scope !== "local-advisory-review-only" ||
      annotation.documentId !== review.documentId ||
      annotation.extractionSnapshotId !== review.extractionSnapshotId ||
      annotation.referenceId !== review.referenceId ||
      !review.observation.findings.some(finding =>
        finding.ruleId === annotation.ruleId && finding.status !== "consistent")) {
    throw new TypeError("Local annotation does not belong to this optional comparison");
  }
  return annotation;
}
