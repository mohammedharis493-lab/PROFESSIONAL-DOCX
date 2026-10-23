/**
 * DRAFT / UNMOUNTED: correlate a future extraction's claimed content version
 * with existing read-only document history DTO fields.
 *
 * This DOES NOT prove the extraction came from those bytes, authenticate the
 * extractor, authorize history reads or verify the current on-disk file.
 * The native row's stored file-instance ID is now checked, but a matching
 * identity is NOT proof that extraction was derived from that source version.
 *
 * No Tauri, filesystem, database, network, hash calculation or write imports.
 */
import { inspectOptionalIndexedDocumentMatch } from "./document-match-advisory-adapter.ts";
import type {
  IndexedDocumentIdentity,
  IndexedDocumentMatchPreparation,
  ReadOnlyExtractionFieldSnapshot,
} from "./document-match-advisory-adapter.ts";
import type {
  AdvisoryComparisonRule,
  OptionalReference,
} from "./document-match-advisory.ts";

/**
 * Read-only fields already exposed by list_document_version_history in lib.rs.
 * Each native row now includes the stored fileInstanceId; the DTO still
 * lacks per-row documentId, so the caller must provide the document ID used
 * for the history request. No native extraction fields are exposed.
 */
export type ReadOnlyDocumentVersionHistory = Readonly<{
  requestedDocumentId: string;
  entries: readonly Readonly<{
    contentVersionId: string;
    fileInstanceId: string;
    verificationState: string;
    sourceStableDuringRead: boolean | null;
    sha256Hex: string | null;
  }>[];
}>;

export type VersionClaimedExtractionSnapshot = ReadOnlyExtractionFieldSnapshot & Readonly<{
  sourceContentVersionId: string;
  sourceSha256Hex: string;
}>;

export type VersionCorrelationResult =
  | Readonly<{
      status: "unavailable";
      reason: "source-unavailable" | "extraction-unavailable" | "history-unavailable" |
        "history-identity-mismatch" | "invalid-version-claim" | "version-not-found" |
        "version-ambiguous" | "history-not-verified" | "source-hash-mismatch" |
        "snapshot-identity-mismatch" | "comparison-input-invalid" |
        "history-instance-unavailable" | "history-instance-mismatch";
      preparation: null;
      blocksDocumentUse: false;
      requiresDocumentChange: false;
      requiresReupload: false;
    }>
  | Readonly<{
      status: "correlated";
      reason: null;
      /** Pure advisory only, never a verified extraction or signed approval. */
      preparation: IndexedDocumentMatchPreparation;
      blocksDocumentUse: false;
      requiresDocumentChange: false;
      requiresReupload: false;
    }>;

function validId(value: unknown): value is string {
  return typeof value === "string" && value.trim().length > 0;
}
function isObject(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}
function sha256(value: unknown): value is string {
  return typeof value === "string" && /^[0-9a-f]{64}$/.test(value);
}
function unavailable(
  reason: Extract<VersionCorrelationResult, {status: "unavailable"}>["reason"],
): VersionCorrelationResult {
  return Object.freeze({
    status: "unavailable" as const, reason, preparation: null,
    blocksDocumentUse: false as const, requiresDocumentChange: false as const,
    requiresReupload: false as const,
  });
}

/**
 * The selection of a matching version ID must be EXPLICIT. No fallback to
 * latest, filename, size, timestamps or previous document history is allowed.
 * Only native history rows whose SHA-256 and stability state are verified can
 * corroborate a future extractor's claim. The matching native row MUST
 * identify the exact indexed file instance. This still does not prove that
 * an extractor consumed those bytes or that current disk bytes are unchanged.
 */
export function correlateOptionalExtractionWithDocumentHistory(
  file: IndexedDocumentIdentity & Readonly<{availabilityState: string}>,
  extraction: VersionClaimedExtractionSnapshot | null,
  history: ReadOnlyDocumentVersionHistory | null,
  reference: OptionalReference | null,
  rules: readonly AdvisoryComparisonRule[],
  requested = false,
): VersionCorrelationResult {
  // Future read-only DTO inputs are not a trusted authority boundary. A
  // malformed provider/object getter/proxy must disable this OPTIONAL gate,
  // never propagate an exception into the normal document viewer.
  try {
  if (!isObject(file) || !validId(file.documentId) ||
      !validId(file.fileInstanceId) || file.availabilityState !== "AVAILABLE") {
    return unavailable("source-unavailable");
  }
  if (extraction === null) return unavailable("extraction-unavailable");
  if (!isObject(extraction) ||
      extraction.documentId !== file.documentId ||
      extraction.fileInstanceId !== file.fileInstanceId) {
    return unavailable("snapshot-identity-mismatch");
  }
  if (!validId(extraction.extractionSnapshotId) ||
      !validId(extraction.sourceContentVersionId) ||
      !sha256(extraction.sourceSha256Hex)) {
    return unavailable("invalid-version-claim");
  }
  if (history === null || !isObject(history) || !Array.isArray(history.entries)) {
    return unavailable("history-unavailable");
  }
  if (history.requestedDocumentId !== file.documentId) {
    return unavailable("history-identity-mismatch");
  }
  const found = history.entries.filter(row => isObject(row) &&
    row.contentVersionId === extraction.sourceContentVersionId);
  if (found.length === 0) return unavailable("version-not-found");

  // The native history joins controlled evidence records. Duplicate rows for
  // one content version are permitted only when every relevant field agrees.
  const first = found[0];
  if (found.some(row => row.sha256Hex !== first.sha256Hex ||
       row.fileInstanceId !== first.fileInstanceId ||
       row.verificationState !== first.verificationState ||
       row.sourceStableDuringRead !== first.sourceStableDuringRead)) {
    return unavailable("version-ambiguous");
  }
  if (!validId(first.fileInstanceId)) {
    return unavailable("history-instance-unavailable");
  }
  if (first.fileInstanceId !== file.fileInstanceId) {
    return unavailable("history-instance-mismatch");
  }
  if (first.verificationState !== "HASH_VERIFIED" ||
      first.sourceStableDuringRead !== true || !sha256(first.sha256Hex)) {
    return unavailable("history-not-verified");
  }
  if (first.sha256Hex !== extraction.sourceSha256Hex) {
    return unavailable("source-hash-mismatch");
  }

  // Still requires a separately permission-checked and provenance-bound
  // extraction reader before any live UI integration.
  try {
    const preparation = inspectOptionalIndexedDocumentMatch(
      file, extraction, reference, rules, requested === true,
    );
    if (preparation.status !== "ready") return unavailable("extraction-unavailable");
    return Object.freeze({
      status: "correlated" as const, reason: null, preparation,
      blocksDocumentUse: false as const, requiresDocumentChange: false as const,
      requiresReupload: false as const,
    });
  } catch {
    // Malformed optional comparison data must not interrupt document usage.
    return unavailable("comparison-input-invalid");
  }
  } catch {
    // Even preflight history/file/snapshot getters and array operations can
    // throw. The user can still preview and manage the original document.
    return unavailable("comparison-input-invalid");
  }
}
