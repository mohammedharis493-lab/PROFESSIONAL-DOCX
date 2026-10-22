/**
 * Optional, detached comparison observations for existing document extractions.
 *
 * NOT reconciliation authority, voucher validation, a document mutation, a
 * forced re-upload, a matching database write, or a professional approval.
 * This module never imports Tauri, persistence, filesystem, or network APIs.
 * Linking it to an application screen requires a separate reviewed integration.
 */
export type AdvisoryScalar = string | number | null | undefined;
export type AdvisoryFields = Readonly<Record<string, AdvisoryScalar>>;

export type ExistingDocumentExtraction = Readonly<{
  documentId: string;
  extractionSnapshotId: string;
  fields: AdvisoryFields;
}>;

export type OptionalReference = Readonly<{
  referenceId: string;
  fields: AdvisoryFields;
}>;

export type AdvisoryComparisonRule = Readonly<{
  ruleId: string;
  label: string;
  documentField: string;
  referenceField: string;
  comparison: "exact-text" | "integer-minor-units";
}>;

export type AdvisoryFindingStatus = "consistent" | "difference" | "not-compared";
export type AdvisoryUnavailableReason =
  | "document-field-absent"
  | "reference-field-absent"
  | "both-fields-absent"
  | "incompatible-field-type";

export type AdvisoryFinding = Readonly<{
  ruleId: string;
  label: string;
  status: AdvisoryFindingStatus;
  documentField: string;
  referenceField: string;
  documentValue: AdvisoryScalar;
  referenceValue: AdvisoryScalar;
  reason: AdvisoryUnavailableReason | null;
}>;

export type OptionalDocumentMatchReport = Readonly<{
  scope: "detached-optional-matching-observation-only";
  documentId: string;
  extractionSnapshotId: string;
  referenceId: string | null;
  outcome: "not-requested" | "no-reference" | "no-rules" | "consistent" | "incomplete" | "suggestions";
  findings: readonly AdvisoryFinding[];
  requiresDocumentChange: false;
  requiresReupload: false;
  blocksDocumentUse: false;
}>;

export type LocalAdvisoryDisposition = Readonly<{
  scope: "local-advisory-review-only";
  documentId: string;
  extractionSnapshotId: string;
  referenceId: string;
  ruleId: string;
  disposition: "acknowledged" | "dismissed";
}>;

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function requiredString(value: unknown, name: string): string {
  if (typeof value !== "string" || value.trim().length === 0) {
    throw new TypeError(name + " must be a nonempty string");
  }
  return value;
}

function fieldsOf(value: unknown, name: string): AdvisoryFields {
  if (!isRecord(value)) throw new TypeError(name + " must be a field object");
  for (const [key, field] of Object.entries(value)) {
    if (typeof field !== "string" && typeof field !== "number" && field !== null &&
        field !== undefined) {
      throw new TypeError(name + " contains an unsupported field type: " + key);
    }
  }
  return value as AdvisoryFields;
}

function isMissing(value: AdvisoryScalar): boolean {
  return value === null || value === undefined ||
    (typeof value === "string" && value.trim() === "");
}

function isComparable(value: AdvisoryScalar, kind: AdvisoryComparisonRule["comparison"]): boolean {
  return kind === "exact-text"
    ? typeof value === "string"
    : typeof value === "number" && Number.isSafeInteger(value);
}

function validateRules(rules: readonly AdvisoryComparisonRule[]): void {
  if (!Array.isArray(rules)) throw new TypeError("Advisory rules must be an array");
  const ids = new Set<string>();
  for (const rule of rules) {
    if (!isRecord(rule)) throw new TypeError("Advisory comparison rule must be an object");
    for (const key of ["ruleId", "label", "documentField", "referenceField"] as const) {
      requiredString(rule[key], "Advisory rule " + key);
    }
    if (ids.has(rule.ruleId)) throw new TypeError("Duplicate advisory rule ID: " + rule.ruleId);
    ids.add(rule.ruleId);
    if (rule.comparison !== "exact-text" && rule.comparison !== "integer-minor-units") {
      throw new TypeError("Unsupported advisory comparison kind");
    }
  }
}

function report(
  document: ExistingDocumentExtraction,
  referenceId: string | null,
  outcome: OptionalDocumentMatchReport["outcome"],
  findings: readonly AdvisoryFinding[],
): OptionalDocumentMatchReport {
  return Object.freeze({
    scope: "detached-optional-matching-observation-only" as const,
    documentId: document.documentId,
    extractionSnapshotId: document.extractionSnapshotId,
    referenceId,
    outcome,
    findings: Object.freeze(findings),
    requiresDocumentChange: false as const,
    requiresReupload: false as const,
    blocksDocumentUse: false as const,
  });
}

/**
 * Missing reference or missing fields are NOT document failures. Rules compare
 * explicit values only; no fuzzy match, inferred currency, voucher-type rule,
 * identity claim, automatic correction or document rewrite is performed.
 *
 * The caller must select an explicit reference; no record association is
 * inferred from invoice numbers or amounts alone.
 */
export function inspectOptionalDocumentMatch(
  document: ExistingDocumentExtraction,
  reference: OptionalReference | null,
  rules: readonly AdvisoryComparisonRule[],
  requested = false,
): OptionalDocumentMatchReport {
  if (!isRecord(document)) throw new TypeError("Existing document extraction is required");
  requiredString(document.documentId, "Document ID");
  requiredString(document.extractionSnapshotId, "Extraction snapshot ID");
  const documentFields = fieldsOf(document.fields, "Document extraction");
  validateRules(rules);
  if (reference !== null) {
    if (!isRecord(reference)) throw new TypeError("Reference must be an object or null");
    requiredString(reference.referenceId, "Reference ID");
    fieldsOf(reference.fields, "Reference");
  }
  if (!requested) return report(document, reference?.referenceId ?? null, "not-requested", []);
  if (reference === null) return report(document, null, "no-reference", []);
  if (rules.length === 0) return report(document, reference.referenceId, "no-rules", []);

  const referenceFields = reference.fields;
  const findings: AdvisoryFinding[] = rules.map((rule) => {
    const documentValue = documentFields[rule.documentField];
    const referenceValue = referenceFields[rule.referenceField];
    const documentMissing = isMissing(documentValue);
    const referenceMissing = isMissing(referenceValue);
    let status: AdvisoryFindingStatus;
    let reason: AdvisoryUnavailableReason | null = null;
    if (documentMissing || referenceMissing) {
      status = "not-compared";
      reason = documentMissing && referenceMissing ? "both-fields-absent"
        : documentMissing ? "document-field-absent" : "reference-field-absent";
    } else if (!isComparable(documentValue, rule.comparison) ||
               !isComparable(referenceValue, rule.comparison)) {
      status = "not-compared";
      reason = "incompatible-field-type";
    } else {
      status = documentValue === referenceValue ? "consistent" : "difference";
    }
    return Object.freeze({
      ruleId: rule.ruleId,
      label: rule.label,
      status,
      documentField: rule.documentField,
      referenceField: rule.referenceField,
      documentValue,
      referenceValue,
      reason,
    });
  });
  const outcome = findings.some(x => x.status === "difference") ? "suggestions"
    : findings.some(x => x.status === "not-compared") ? "incomplete" : "consistent";
  return report(document, reference.referenceId, outcome, findings);
}

/**
 * Local presentation annotation only. This cannot modify the original file,
 * corrected extraction metadata, book data, permissions or audit history.
 * Durable reviewer decisions will require a separately approved data model.
 */
export function noteOptionalMatchFinding(
  observation: OptionalDocumentMatchReport,
  ruleId: string,
  disposition: "acknowledged" | "dismissed",
): LocalAdvisoryDisposition {
  if (!isRecord(observation) ||
      observation.scope !== "detached-optional-matching-observation-only" ||
      !Array.isArray(observation.findings) ||
      !observation.referenceId) {
    throw new TypeError("A compared advisory observation is required");
  }
  requiredString(ruleId, "Advisory rule ID");
  if (disposition !== "acknowledged" && disposition !== "dismissed") {
    throw new TypeError("Invalid local advisory disposition");
  }
  const finding = observation.findings.find(x => x.ruleId === ruleId);
  if (!finding || finding.status === "consistent") {
    throw new TypeError("Only a difference or unavailable comparison can be annotated");
  }
  return Object.freeze({
    scope: "local-advisory-review-only" as const,
    documentId: observation.documentId,
    extractionSnapshotId: observation.extractionSnapshotId,
    referenceId: observation.referenceId,
    ruleId,
    disposition,
  });
}
