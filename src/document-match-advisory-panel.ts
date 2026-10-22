/**
 * Detached presentation of an OPTIONAL document-to-reference comparison.
 *
 * This panel reads an already-produced advisory observation. No Tauri invoke,
 * persistence operation, source-file rewrite, extraction overwrite, implicit
 * comparison or professional approval is performed. It is NOT mounted in App.
 */
import { createElement as h } from "react";
import type { ReactElement } from "react";
import {
  noteOptionalMatchFinding,
} from "./document-match-advisory.ts";
import type {
  AdvisoryFinding,
  AdvisoryScalar,
  LocalAdvisoryDisposition,
  OptionalDocumentMatchReport,
} from "./document-match-advisory.ts";

export type DocumentMatchAdvisoryPanelProps = Readonly<{
  observation: OptionalDocumentMatchReport;
  onLocalAnnotation?: (annotation: LocalAdvisoryDisposition) => void;
}>;

const descriptions: Readonly<Record<OptionalDocumentMatchReport["outcome"], string>> = {
  "not-requested": "Not compared. Matching is optional, and the document is available without it.",
  "no-reference": "No reference selected. You can continue working with the existing document.",
  "no-rules": "No comparison fields selected. The existing document remains available.",
  consistent: "Compared fields are consistent. This does not certify the document or the accounts.",
  incomplete: "Some fields could not be compared. This is not a document validation failure.",
  suggestions: "Differences are suggestions for review, not instructions to change the document.",
};

const reasonLabels: Readonly<Record<NonNullable<AdvisoryFinding["reason"]>, string>> = {
  "document-field-absent": "Extracted field unavailable",
  "reference-field-absent": "Reference field unavailable",
  "both-fields-absent": "Neither field available",
  "incompatible-field-type": "Fields could not be compared safely",
};

function visibleValue(value: AdvisoryScalar): string {
  return value === null || value === undefined ||
    (typeof value === "string" && value.trim().length === 0)
    ? "Unavailable"
    : String(value);
}

function renderFinding(
  observation: OptionalDocumentMatchReport,
  finding: AdvisoryFinding,
  onLocalAnnotation: DocumentMatchAdvisoryPanelProps["onLocalAnnotation"],
): ReactElement {
  const label = finding.status === "difference"
    ? "Difference to review"
    : finding.status === "consistent"
      ? "Consistent"
      : "Not compared";
  const annotation = onLocalAnnotation && finding.status !== "consistent"
    ? h("div", { className: "document-match-advisory-actions" },
        h("button", {
          type: "button",
          onClick: () => onLocalAnnotation(
            noteOptionalMatchFinding(observation, finding.ruleId, "acknowledged"),
          ),
        }, "Acknowledge suggestion"),
        h("button", {
          type: "button",
          onClick: () => onLocalAnnotation(
            noteOptionalMatchFinding(observation, finding.ruleId, "dismissed"),
          ),
        }, "Dismiss suggestion"),
      )
    : null;

  return h("li", { key: finding.ruleId, "data-advisory-status": finding.status },
    h("strong", null, finding.label),
    h("span", null, " — ", label),
    h("dl", null,
      h("dt", null, "Existing extraction"),
      h("dd", null, visibleValue(finding.documentValue)),
      h("dt", null, "Selected reference"),
      h("dd", null, visibleValue(finding.referenceValue)),
    ),
    finding.reason
      ? h("p", null, reasonLabels[finding.reason])
      : null,
    annotation,
  );
}

/**
 * A passive, nonblocking panel. Any local annotation callback must be provided
 * explicitly by a future host; absence of a callback shows no inert buttons.
 * Its output contains no link to re-upload, modify source or trigger Save.
 */
export function DocumentMatchAdvisoryPanel({
  observation,
  onLocalAnnotation,
}: DocumentMatchAdvisoryPanelProps): ReactElement {
  if (!observation ||
      observation.scope !== "detached-optional-matching-observation-only" ||
      !Array.isArray(observation.findings) ||
      observation.blocksDocumentUse !== false ||
      observation.requiresDocumentChange !== false ||
      observation.requiresReupload !== false ||
      !(observation.outcome in descriptions)) {
    throw new TypeError("A valid nonblocking advisory observation is required");
  }
  return h("section", {
    className: "document-match-advisory",
    role: "region",
    "aria-label": "Optional document matching suggestions",
  },
    h("h3", null, "Optional matching"),
    h("p", null, descriptions[observation.outcome]),
    h("p", null, "Your original document and extracted record are not changed by this comparison."),
    observation.findings.length > 0
      ? h("ul", { "aria-label": "Optional comparison observations" },
          ...observation.findings.map(finding =>
            renderFinding(observation, finding, onLocalAnnotation),
          ),
        )
      : null,
    h("p", null, "Matching does not require editing or uploading the document again."),
  );
}
