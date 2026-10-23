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
  /** Exact value binding for local opt-in, NOT a source hash or provenance. */
  extractionFieldsKey: string;
  /** Caller-rotated history-response epoch: UI invalidation ONLY, not proof. */
  historyReadKey?: string;
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
  historyReadKey?: string;
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

/** Deterministic UI binding, not a digest/attestation. Bad field data fails closed. */
function extractionFieldsKey(snapshot: ReadOnlyExtractionFieldSnapshot): string | null {
  try {
    const fields = snapshot.fields;
    if (!fields || typeof fields !== "object" || Array.isArray(fields) ||
        Object.values(fields).some(value =>
          value !== null && value !== undefined &&
          typeof value !== "string" && typeof value !== "number")) {
      return null;
    }
    return JSON.stringify(fieldsKey(fields));
  } catch {
    return null;
  }
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
  historyReadKey?: string,
): ExplicitAdvisorySelection | null {
  if (!requiredId(visitKey) || !requiredId(referenceId) ||
      (historyReadKey !== undefined && !requiredId(historyReadKey)) ||
      !file || !requiredId(file.documentId) || !requiredId(file.fileInstanceId) ||
      !snapshot || !requiredId(snapshot.extractionSnapshotId) ||
      snapshot.documentId !== file.documentId ||
      snapshot.fileInstanceId !== file.fileInstanceId) return null;
  const fieldsBinding = extractionFieldsKey(snapshot);
  if (fieldsBinding === null) return null;

  return Object.freeze({
    scope: "volatile-optional-reference-selection" as const,
    visitKey, documentId: file.documentId, fileInstanceId: file.fileInstanceId,
    extractionSnapshotId: snapshot.extractionSnapshotId, referenceId,
    extractionFieldsKey: fieldsBinding,
    ...(historyReadKey === undefined ? {} : {historyReadKey}),
    ...versionClaims(snapshot),
  });
}

export function selectionIsCurrent(
  selection: ExplicitAdvisorySelection | null,
  visitKey: string,
  file: IndexedDocumentIdentity,
  snapshot: ReadOnlyExtractionFieldSnapshot | null,
  historyReadKey?: string,
): boolean {
  return !!selection && selection.scope === "volatile-optional-reference-selection" &&
    requiredId(visitKey) && selection.visitKey === visitKey &&
    selection.historyReadKey === historyReadKey &&
    (historyReadKey === undefined || requiredId(historyReadKey)) &&
    !!file && selection.documentId === file.documentId &&
    selection.fileInstanceId === file.fileInstanceId &&
    !!snapshot && selection.extractionSnapshotId === snapshot.extractionSnapshotId &&
    selection.extractionFieldsKey !== null &&
    selection.extractionFieldsKey === extractionFieldsKey(snapshot) &&
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
  historyReadKey?: string,
): OptionalMatchAttempt {
  if (!selection) return unavailable("no-explicit-selection");
  if (!selectionIsCurrent(selection, visitKey, file, snapshot, historyReadKey)) {
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
      ...(historyReadKey === undefined ? {} : {historyReadKey}),
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
  historyReadKey?: string,
): boolean {
  if (!review || review.scope !== "volatile-optional-comparison-review" ||
      !selectionIsCurrent(selection, visitKey, file, snapshot, historyReadKey) ||
      review.historyReadKey !== historyReadKey ||
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

/**
 * Ephemeral presentation state for a finding in precisely one optional
 * comparison. No authority, native actor, evidence approval, durable audit or
 * source mutation is represented by this structure.
 */
export type VolatileAdvisoryAnnotation = Readonly<{
  scope: "volatile-optional-annotation-only";
  visitKey: string;
  documentId: string;
  fileInstanceId: string;
  extractionSnapshotId: string;
  referenceId: string;
  historyReadKey?: string;
  inputKey: string;
  ruleId: string;
  disposition: "acknowledged" | "dismissed";
}>;

/** Local-only annotation, optionally held by a React host during this visit. */
export function keepLocalAdvisoryDisposition(
  review: VolatileAdvisoryReview,
  annotation: LocalAdvisoryDisposition,
): LocalAdvisoryDisposition {
  if (!review || !annotation || review.scope !== "volatile-optional-comparison-review" ||
      annotation.scope !== "local-advisory-review-only" ||
      (annotation.disposition !== "acknowledged" && annotation.disposition !== "dismissed") ||
      !requiredId(annotation.ruleId) ||
      annotation.documentId !== review.documentId ||
      annotation.extractionSnapshotId !== review.extractionSnapshotId ||
      annotation.referenceId !== review.referenceId ||
      !Array.isArray(review.observation?.findings) ||
      !review.observation.findings.some(finding =>
        finding && finding.ruleId === annotation.ruleId &&
        (finding.status === "difference" || finding.status === "not-compared"))) {
    throw new TypeError("Local annotation does not belong to this optional comparison");
  }
  return annotation;
}

/**
 * Convert a panel's narrow user-action payload to an explicitly visit-local
 * annotation. Only a CURRENT review from the controlled host may invoke this;
 * use volatileAnnotationIsCurrent before retaining/showing the result. A
 * comparison input key and history epoch are UI invalidation data, NOT proof
 * of origin, authority, or actual user identity.
 */
export function stampVolatileAdvisoryAnnotation(
  review: VolatileAdvisoryReview,
  annotation: LocalAdvisoryDisposition,
): VolatileAdvisoryAnnotation {
  const valid = keepLocalAdvisoryDisposition(review, annotation);
  if (!requiredId(review.visitKey) || !requiredId(review.documentId) ||
      !requiredId(review.fileInstanceId) || !requiredId(review.extractionSnapshotId) ||
      !requiredId(review.referenceId) || !requiredId(review.inputKey) ||
      (review.historyReadKey !== undefined && !requiredId(review.historyReadKey))) {
    throw new TypeError("Volatile annotation review identity is unavailable");
  }
  return Object.freeze({
    scope: "volatile-optional-annotation-only" as const,
    visitKey: review.visitKey,
    documentId: review.documentId,
    fileInstanceId: review.fileInstanceId,
    extractionSnapshotId: review.extractionSnapshotId,
    referenceId: review.referenceId,
    ...(review.historyReadKey === undefined ? {} : {historyReadKey: review.historyReadKey}),
    inputKey: review.inputKey,
    ruleId: valid.ruleId,
    disposition: valid.disposition,
  });
}

/**
 * A future visit-local host must revalidate before rendering annotations.
 * Rejects stale history epochs, visit/file/ref changes, changed fields/rules
 * and annotations for findings that are now consistent. Never persists or
 * changes a document. A malicious caller can forge this pure client payload:
 * it MUST NOT be accepted as native authority or an approval record.
 */
export function volatileAnnotationIsCurrent(
  annotation: VolatileAdvisoryAnnotation | null,
  review: VolatileAdvisoryReview | null,
  visitKey: string,
  file: IndexedDocumentIdentity,
  snapshot: ReadOnlyExtractionFieldSnapshot | null,
  references: readonly OptionalReference[],
  rules: readonly AdvisoryComparisonRule[],
  selection: ExplicitAdvisorySelection | null,
  historyReadKey?: string,
): boolean {
  if (!annotation || !review ||
      !reviewIsCurrent(review, visitKey, file, snapshot, references, rules, selection, historyReadKey) ||
      annotation.scope !== "volatile-optional-annotation-only" ||
      annotation.visitKey !== review.visitKey ||
      annotation.documentId !== review.documentId ||
      annotation.fileInstanceId !== review.fileInstanceId ||
      annotation.extractionSnapshotId !== review.extractionSnapshotId ||
      annotation.referenceId !== review.referenceId ||
      annotation.historyReadKey !== review.historyReadKey ||
      annotation.inputKey !== review.inputKey ||
      !requiredId(annotation.ruleId) ||
      (annotation.disposition !== "acknowledged" &&
       annotation.disposition !== "dismissed")) return false;
  return review.observation.findings.some(finding =>
    finding && finding.ruleId === annotation.ruleId &&
    (finding.status === "difference" || finding.status === "not-compared"));
}
