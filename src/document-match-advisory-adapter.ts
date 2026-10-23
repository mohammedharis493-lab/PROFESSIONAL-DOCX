/**
 * Detached bridge between the existing indexed-file viewer identity and a
 * separately supplied, READ-ONLY extraction-field snapshot.
 *
 * The indexed-file DTO has no structured extracted voucher fields. This module
 * NEVER guesses them from filename, paths, previews, book data or timestamps.
 * No native command, storage access, automatic matching or source write occurs.
 */
import { inspectOptionalDocumentMatch } from "./document-match-advisory.ts";
import type {
  AdvisoryComparisonRule,
  AdvisoryFields,
  OptionalDocumentMatchReport,
  OptionalReference,
} from "./document-match-advisory.ts";

/** Only the identity columns exposed by the current IndexedFile viewer DTO. */
export type IndexedDocumentIdentity = Readonly<{
  documentId: string;
  fileInstanceId: string;
}>;

/**
 * Future read-only extractor input, NOT a claim that such a native reader
 * currently exists. Both identities must match the explicitly opened file.
 * A snapshot ID alone does not establish provenance or content freshness.
 */
export type ReadOnlyExtractionFieldSnapshot = Readonly<{
  documentId: string;
  fileInstanceId: string;
  extractionSnapshotId: string;
  fields: AdvisoryFields;
}>;

export type IndexedDocumentMatchPreparation =
  | Readonly<{
      status: "unavailable";
      reason:
        | "indexed-identity-unavailable"
        | "extraction-not-available"
        | "extraction-identity-mismatch"
        | "extraction-invalid";
      observation: null;
      blocksDocumentUse: false;
      requiresDocumentChange: false;
      requiresReupload: false;
    }>
  | Readonly<{
      status: "ready";
      reason: null;
      observation: OptionalDocumentMatchReport;
      blocksDocumentUse: false;
      requiresDocumentChange: false;
      requiresReupload: false;
    }>;

function validId(value: unknown): value is string {
  return typeof value === "string" && value.trim().length > 0;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function validFields(value: unknown): value is AdvisoryFields {
  return isRecord(value) && Object.values(value).every(field =>
    field === null || field === undefined ||
    typeof field === "string" || typeof field === "number");
}

function unavailable(
  reason: Extract<IndexedDocumentMatchPreparation, { status: "unavailable" }>["reason"],
): IndexedDocumentMatchPreparation {
  return Object.freeze({
    status: "unavailable" as const,
    reason,
    observation: null,
    blocksDocumentUse: false as const,
    requiresDocumentChange: false as const,
    requiresReupload: false as const,
  });
}

/**
 * A viewer may pass an existing file identity; only a future independently
 * sourced, explicitly bound field snapshot makes comparison possible.
 *
 * requested=false is always the default. Missing books/rules/incomplete
 * fields are advisory outcomes, never document-management errors.
 * No candidate association, extraction, refresh or correction is performed.
 */
export function inspectOptionalIndexedDocumentMatch(
  indexedFile: IndexedDocumentIdentity,
  snapshot: ReadOnlyExtractionFieldSnapshot | null,
  reference: OptionalReference | null,
  rules: readonly AdvisoryComparisonRule[],
  requested = false,
): IndexedDocumentMatchPreparation {
  if (!isRecord(indexedFile) ||
      !validId(indexedFile.documentId) ||
      !validId(indexedFile.fileInstanceId)) {
    return unavailable("indexed-identity-unavailable");
  }
  if (snapshot === null) {
    return unavailable("extraction-not-available");
  }
  if (!isRecord(snapshot) ||
      !validId(snapshot.documentId) ||
      !validId(snapshot.fileInstanceId) ||
      !validId(snapshot.extractionSnapshotId) ||
      !validFields(snapshot.fields)) {
    return unavailable("extraction-invalid");
  }
  if (snapshot.documentId !== indexedFile.documentId ||
      snapshot.fileInstanceId !== indexedFile.fileInstanceId) {
    return unavailable("extraction-identity-mismatch");
  }

  const observation = inspectOptionalDocumentMatch({
    documentId: indexedFile.documentId,
    extractionSnapshotId: snapshot.extractionSnapshotId,
    fields: snapshot.fields,
  }, reference, rules, requested === true);

  return Object.freeze({
    status: "ready" as const,
    reason: null,
    observation,
    blocksDocumentUse: false as const,
    requiresDocumentChange: false as const,
    requiresReupload: false as const,
  });
}
